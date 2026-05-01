"""Classification and import-session rule-induction flows."""
# pylint: disable=protected-access,too-few-public-methods,duplicate-code

from __future__ import annotations

import json
from typing import Any

import aiosqlite

from ..llm_provider import LLMResponse
from .common import logger
from .errors import LLMImportSessionAnalysisError
from .limits import _MAX_SESSION_CATEGORY_GROUPS


class SessionAnalysisMixin:
    """Analyze persisted transactions and selected import-preview session rows."""

    # pylint: disable=too-many-arguments,too-many-positional-arguments,too-many-locals
    async def analyze_transactions(
        self,
        user_id: int,
        bill_ids: list[int] | None = None,
        limit: int = 20,
        session_id: str | None = None,
        preview_ids: list[int] | None = None,
        preview_updates: list[dict[str, Any]] | None = None,
    ) -> list[dict[str, Any]]:
        """Analyze persisted bills or import-preview session rows and create LLM candidates."""
        normalized_limit = self._normalize_limit(limit)

        if session_id:
            return await self._analyze_import_session(
                user_id=user_id,
                session_id=session_id,
                preview_ids=preview_ids,
                preview_updates=preview_updates,
                limit=normalized_limit,
            )

        # Fetch uncategorized transactions
        if bill_ids:
            transactions = []
            for bid in bill_ids[:normalized_limit]:
                bill = await self._db.get_bill_by_id(bid, user_id=user_id)
                if bill:
                    transactions.append(bill)
        else:
            conn = await self._db._get_connection()
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? "
                "AND (main_category IS NULL OR main_category = '' OR main_category = '未分类') "
                "ORDER BY date DESC LIMIT ?",
                (user_id, normalized_limit),
            ) as cursor:
                transactions = [dict(row) for row in await cursor.fetchall()]

        if not transactions:
            logger.info("No uncategorized transactions found for user %s", user_id)
            return []

        self._reserve_rate_limit_slots(user_id, 1)
        prompt = self._build_classification_prompt(transactions)

        try:
            response: LLMResponse = await self._generate(prompt)
        except Exception as exc:
            logger.error("LLM classification request failed: %s", exc)
            raise RuntimeError(f"LLM request failed: {exc}") from exc

        suggestions = self._parse_classification_response(response.content)
        candidates: list[dict[str, Any]] = []

        for suggestion in suggestions:
            bill_id = suggestion.get("bill_id")
            main_cat = suggestion.get("suggested_main_category", "")
            sub_cat = suggestion.get("suggested_sub_category", "")
            confidence = float(suggestion.get("confidence", 0.0))

            source_ids = [bill_id] if bill_id else []

            candidate_id = await self._db.create_llm_candidate(
                user_id=user_id,
                type="classification",
                source_bill_ids=source_ids,
                suggested_main_category=main_cat,
                suggested_sub_category=sub_cat,
                suggested_rule_expression=None,
                confidence=confidence,
                llm_provider=response.provider,
                llm_model=response.model,
                llm_response_raw=json.dumps(response.raw_response, ensure_ascii=False),
            )
            candidates.append({
                "id": candidate_id,
                "bill_id": bill_id,
                "suggested_main_category": main_cat,
                "suggested_sub_category": sub_cat,
                "confidence": confidence,
            })

        logger.info(
            "LLM classification produced %d candidates for user %s",
            len(candidates),
            user_id,
        )
        return candidates

    # pylint: disable=too-many-arguments,too-many-locals,too-many-branches,too-many-statements
    async def _analyze_import_session(
        self,
        *,
        user_id: int,
        session_id: str,
        preview_ids: list[int] | None,
        preview_updates: list[dict[str, Any]] | None,
        limit: int,
    ) -> list[dict[str, Any]]:
        """Generate rule-induction candidates from import preview rows."""
        session = await self._db.get_import_session(session_id, user_id=user_id)
        if not session:
            raise LLMImportSessionAnalysisError(
                "IMPORT_SESSION_NOT_FOUND",
                "Import session not found",
                status_code=404,
            )

        normalized_preview_updates = self._normalize_preview_updates(preview_updates)
        selected_preview_ids = self._normalize_selected_preview_ids(
            preview_ids=preview_ids,
            preview_updates=normalized_preview_updates,
        )

        if normalized_preview_updates:
            update_preview_batch = getattr(self._db, "update_preview_bills_batch", None)
            if callable(update_preview_batch):
                await update_preview_batch(
                    session_id,
                    normalized_preview_updates,
                    user_id=user_id,
                )

        if normalized_preview_updates is not None or preview_ids is not None:
            if not selected_preview_ids:
                logger.info(
                    "No valid preview ids selected for import session %s (user %s)",
                    session_id,
                    user_id,
                )
                raise LLMImportSessionAnalysisError(
                    "PREVIEW_SELECTION_EMPTY",
                    "No preview rows selected for import-session LLM analysis",
                )
            preview_loader = getattr(self._db, "get_preview_bill_by_id", None)
            if callable(preview_loader):
                previews = []
                for preview_id in selected_preview_ids[:limit]:
                    preview = await preview_loader(preview_id, user_id=user_id)
                    if preview and str(preview.get("session_id") or "") == session_id:
                        previews.append(preview)
            else:
                previews = await self._db.get_preview_by_session(session_id, user_id=user_id)
                selected_preview_id_set = set(selected_preview_ids)
                previews = [
                    preview
                    for preview in previews
                    if int(preview.get("id") or 0) in selected_preview_id_set
                ][:limit]
        else:
            previews = await self._db.get_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=True,
            )
            previews = previews[:limit]

        if not previews:
            logger.info(
                "No preview rows found for import session %s (user %s)",
                session_id,
                user_id,
            )
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_EMPTY",
                "No preview rows available for import-session LLM analysis",
            )

        preview_groups: dict[tuple[str, str], list[dict[str, Any]]] = {}
        for preview in previews:
            preview_id = int(preview.get("id") or 0)
            if preview_id <= 0:
                continue

            main_category = str(preview.get("preview_main_category") or "").strip()
            sub_category = str(preview.get("preview_sub_category") or "").strip()
            if not main_category and not sub_category:
                continue

            preview_groups.setdefault((main_category, sub_category), []).append(preview)

        if not preview_groups:
            logger.info(
                "No categorized preview rows found for import session %s (user %s)",
                session_id,
                user_id,
            )
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_INSUFFICIENT",
                "Selected preview rows need confirmed category data before LLM rule induction",
                status_code=422,
            )

        usable_preview_groups = {
            key: [
                preview
                for preview in group
                if any(
                    str(preview.get(field) or "").strip()
                    for field in (
                        "preview_counterparty",
                        "preview_description",
                        "preview_payment_method",
                    )
                )
            ]
            for key, group in preview_groups.items()
        }
        preview_groups = {
            key: group
            for key, group in usable_preview_groups.items()
            if group
        }
        if not preview_groups:
            logger.info(
                "No preview rows with usable text fields found for import session %s (user %s)",
                session_id,
                user_id,
            )
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_INSUFFICIENT",
                "Selected preview rows need counterparty, description, or payment method data",
                status_code=422,
            )

        if len(preview_groups) > _MAX_SESSION_CATEGORY_GROUPS:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_TOO_LARGE",
                (
                    "Too many category groups selected for session analysis "
                    f"(max {_MAX_SESSION_CATEGORY_GROUPS})"
                )
            )

        self._reserve_rate_limit_slots(user_id, len(preview_groups))
        candidates: list[dict[str, Any]] = []
        for (main_category, sub_category), category_previews in preview_groups.items():
            category_name = "/".join(
                [part for part in (main_category, sub_category) if part]
            )
            prompt = self._build_rule_induction_prompt(
                category_name,
                [
                    {
                        "id": preview.get("id"),
                        "counterparty": preview.get("preview_counterparty", ""),
                        "description": preview.get("preview_description", ""),
                        "payment_method": preview.get("preview_payment_method", ""),
                    }
                    for preview in category_previews
                ],
            )

            try:
                response: LLMResponse = await self._generate(prompt)
            except Exception as exc:
                logger.error("LLM import-session rule induction failed: %s", exc)
                raise RuntimeError(f"LLM request failed: {exc}") from exc

            rules = self._parse_rule_induction_response(response.content)
            source_preview_ids = [
                int(preview.get("id") or 0)
                for preview in category_previews
                if int(preview.get("id") or 0) > 0
            ]

            for rule in rules:
                rule_expression = str(rule.get("rule_expression") or "").strip()
                if not rule_expression:
                    continue

                confidence = float(rule.get("confidence", 0.0))
                rule_name = str(rule.get("rule_name") or "").strip()
                candidate_id = await self._db.create_llm_candidate(
                    user_id=user_id,
                    type="rule_induction",
                    source_bill_ids=source_preview_ids,
                    suggested_main_category=main_category or None,
                    suggested_sub_category=sub_category or None,
                    suggested_rule_expression=rule_expression,
                    confidence=confidence,
                    llm_provider=response.provider,
                    llm_model=response.model,
                    llm_response_raw=json.dumps(response.raw_response, ensure_ascii=False),
                )
                candidates.append({
                    "id": candidate_id,
                    "session_id": session_id,
                    "source_preview_ids": source_preview_ids,
                    "rule_name": rule_name,
                    "rule_expression": rule_expression,
                    "confidence": confidence,
                    "category_name": category_name,
                    "explanation": rule.get("explanation", ""),
                })

        logger.info(
            "LLM import-session rule induction produced %d candidates for session %s (user %s)",
            len(candidates),
            session_id,
            user_id,
        )
        return candidates
