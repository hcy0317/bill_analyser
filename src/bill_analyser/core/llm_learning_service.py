"""LLM Learning Service — orchestrates LLM-driven classification and rule induction."""
# pylint: disable=protected-access

from __future__ import annotations

import json
import time
from typing import Any

import aiosqlite

from ..utils.logger import get_logger
from .llm_prompts import (
    SYSTEM_PROMPT,
    build_classification_prompt,
    build_rule_induction_prompt,
    render_prompt_template,
)
from .llm_provider import LLMProvider, LLMResponse

logger = get_logger("LLMLearningService")

# Simple in-memory rate limiter
_RATE_LIMIT_WINDOW = 60  # seconds
_RATE_LIMIT_MAX_CALLS = 10
_RATE_LIMIT_BUCKETS: dict[int, list[float]] = {}

_MAX_ANALYZE_LIMIT = 20
_MAX_SESSION_SELECTION = 20
_MAX_SESSION_CATEGORY_GROUPS = 10
_MAX_PREVIEW_UPDATE_BATCH = 20
_DEFAULT_TEMPERATURE = 0.3
_DEFAULT_MAX_TOKENS = 4096


class LLMImportSessionAnalysisError(ValueError):
    """Stable import-session LLM analysis error for API/UI branching."""

    def __init__(self, code: str, message: str, status_code: int = 400) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code


class LLMLearningService:
    """Orchestrates LLM analysis of transactions and rule induction."""

    def __init__(
        self,
        db: Any,
        provider: LLMProvider,
        advanced_settings: dict[str, Any] | None = None,
    ) -> None:
        self._db = db
        self._provider = provider
        self._advanced_settings = advanced_settings or {}

    def _system_prompt(self) -> str:
        return str(self._advanced_settings.get("system_prompt") or SYSTEM_PROMPT)

    def _temperature(self) -> float:
        return float(self._advanced_settings.get("temperature", _DEFAULT_TEMPERATURE))

    def _max_tokens(self) -> int:
        return int(self._advanced_settings.get("max_tokens", _DEFAULT_MAX_TOKENS))

    def _reasoning_depth(self) -> str:
        return str(self._advanced_settings.get("reasoning_depth") or "")

    def _build_classification_prompt(self, transactions: list[dict[str, Any]]) -> str:
        default_prompt = build_classification_prompt(transactions)
        return render_prompt_template(
            str(self._advanced_settings.get("classification_prompt_template") or ""),
            default_prompt=default_prompt,
            transactions=transactions,
        )

    def _build_rule_induction_prompt(
        self,
        category_name: str,
        transactions: list[dict[str, Any]],
    ) -> str:
        default_prompt = build_rule_induction_prompt(category_name, transactions)
        return render_prompt_template(
            str(self._advanced_settings.get("rule_prompt_template") or ""),
            default_prompt=default_prompt,
            transactions=transactions,
            category_name=category_name,
        )

    async def _generate(self, prompt: str) -> LLMResponse:
        return await self._provider.generate(
            prompt=prompt,
            system_prompt=self._system_prompt(),
            temperature=self._temperature(),
            max_tokens=self._max_tokens(),
            reasoning_depth=self._reasoning_depth(),
        )

    @staticmethod
    def _normalize_limit(limit: int) -> int:
        """Validate and normalize the maximum number of items to analyze."""
        normalized_limit = int(limit)
        if normalized_limit <= 0:
            raise ValueError("limit must be a positive integer")
        if normalized_limit > _MAX_ANALYZE_LIMIT:
            raise ValueError(f"limit cannot exceed {_MAX_ANALYZE_LIMIT}")
        return normalized_limit

    def _reserve_rate_limit_slots(self, user_id: int, slots: int = 1) -> None:
        """Reserve real LLM call slots across service instances for the same user."""
        if slots <= 0:
            return

        now = time.time()
        recent_calls = [
            ts for ts in _RATE_LIMIT_BUCKETS.get(user_id, [])
            if now - ts < _RATE_LIMIT_WINDOW
        ]
        if len(recent_calls) + slots > _RATE_LIMIT_MAX_CALLS:
            raise RuntimeError(
                f"Rate limit exceeded: max {_RATE_LIMIT_MAX_CALLS} calls per {_RATE_LIMIT_WINDOW}s"
            )
        recent_calls.extend([now] * slots)
        _RATE_LIMIT_BUCKETS[user_id] = recent_calls

    @staticmethod
    def _normalize_selected_preview_ids(
        *,
        preview_ids: list[int] | None,
        preview_updates: list[dict[str, Any]] | None,
    ) -> list[int]:
        """Build a stable ordered preview-id list from request inputs."""
        ordered_ids: list[int] = []

        if preview_ids is not None:
            if not isinstance(preview_ids, list):
                raise ValueError("preview_ids must be a list")
            if len(preview_ids) > _MAX_SESSION_SELECTION:
                raise LLMImportSessionAnalysisError(
                    "PREVIEW_SELECTION_TOO_LARGE",
                    (
                        "Too many preview rows selected for session analysis "
                        f"(max {_MAX_SESSION_SELECTION})"
                    )
                )
            for preview_id in preview_ids:
                normalized_preview_id = int(preview_id)
                if normalized_preview_id > 0 and normalized_preview_id not in ordered_ids:
                    ordered_ids.append(normalized_preview_id)
        elif preview_updates is not None:
            for item in preview_updates:
                preview_id = int(item.get("id") or 0)
                if preview_id > 0 and preview_id not in ordered_ids:
                    ordered_ids.append(preview_id)

        if len(ordered_ids) > _MAX_SESSION_SELECTION:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_TOO_LARGE",
                (
                    "Too many preview rows selected for session analysis "
                    f"(max {_MAX_SESSION_SELECTION})"
                )
            )

        return ordered_ids

    @staticmethod
    def _normalize_preview_updates(
        preview_updates: list[dict[str, Any]] | None,
    ) -> list[dict[str, Any]] | None:
        """Validate, bound, and deduplicate preview updates by preview id."""
        if preview_updates is None:
            return None
        if not isinstance(preview_updates, list):
            raise ValueError("preview_updates must be a list")
        if len(preview_updates) > _MAX_PREVIEW_UPDATE_BATCH:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_TOO_LARGE",
                (
                    "Too many preview updates submitted for session analysis "
                    f"(max {_MAX_PREVIEW_UPDATE_BATCH})"
                )
            )

        deduped_updates: dict[int, dict[str, Any]] = {}
        preview_order: list[int] = []

        for update_item in preview_updates:
            if not isinstance(update_item, dict):
                raise ValueError("preview_updates entries must be objects")

            preview_id = int(update_item.get("id") or 0)
            if preview_id <= 0:
                continue

            if preview_id not in preview_order:
                preview_order.append(preview_id)

            normalized_update_item = dict(update_item)
            normalized_update_item["id"] = preview_id
            deduped_updates[preview_id] = normalized_update_item

        return [deduped_updates[preview_id] for preview_id in preview_order]

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

    # pylint: disable=too-many-locals
    async def induce_rules(
        self,
        user_id: int,
        category_id: int,
        sample_count: int = 10,
    ) -> list[dict[str, Any]]:
        """Fetch categorized samples, ask LLM to induce keyword patterns."""
        self._reserve_rate_limit_slots(user_id, 1)

        # Get category info
        category = await self._db.get_category_by_id(category_id, user_id=user_id)
        if not category:
            raise ValueError(f"Category {category_id} not found")

        category_name = f"{category.get('main_category', '')}/{category.get('sub_category', '')}"

        # Fetch sample transactions for this category
        conn = await self._db._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM bills WHERE user_id = ? "
            "AND main_category = ? AND sub_category = ? "
            "ORDER BY date DESC LIMIT ?",
            (
                user_id,
                category.get("main_category", ""),
                category.get("sub_category", ""),
                sample_count,
            ),
        ) as cursor:
            transactions = [dict(row) for row in await cursor.fetchall()]

        if not transactions:
            logger.info("No samples found for category %s (user %s)", category_name, user_id)
            return []

        prompt = self._build_rule_induction_prompt(category_name, transactions)

        try:
            response: LLMResponse = await self._generate(prompt)
        except Exception as exc:
            logger.error("LLM rule induction request failed: %s", exc)
            raise RuntimeError(f"LLM request failed: {exc}") from exc

        rules = self._parse_rule_induction_response(response.content)
        candidates: list[dict[str, Any]] = []

        source_bill_ids = [t.get("id") for t in transactions if t.get("id")]

        for rule in rules:
            rule_expression = rule.get("rule_expression", "")
            confidence = float(rule.get("confidence", 0.0))
            rule_name = rule.get("rule_name", "")

            candidate_id = await self._db.create_llm_candidate(
                user_id=user_id,
                type="rule_induction",
                source_bill_ids=source_bill_ids,
                suggested_main_category=category.get("main_category"),
                suggested_sub_category=category.get("sub_category"),
                suggested_rule_expression=rule_expression,
                confidence=confidence,
                llm_provider=response.provider,
                llm_model=response.model,
                llm_response_raw=json.dumps(response.raw_response, ensure_ascii=False),
            )
            candidates.append({
                "id": candidate_id,
                "rule_name": rule_name,
                "rule_expression": rule_expression,
                "confidence": confidence,
                "explanation": rule.get("explanation", ""),
            })

        logger.info(
            "LLM rule induction produced %d candidates for category '%s' (user %s)",
            len(candidates),
            category_name,
            user_id,
        )
        return candidates

    async def accept_candidate(self, candidate_id: int, user_id: int) -> dict[str, Any]:
        """Mark candidate as accepted.

        Optionally create a category_rule if it's a rule induction.
        """
        candidate = await self._db.get_llm_candidate_by_id(candidate_id, user_id=user_id)
        if not candidate:
            raise ValueError(f"Candidate {candidate_id} not found")

        await self._db.update_llm_candidate_status(
            candidate_id,
            "accepted",
            user_id=user_id,
        )

        result: dict[str, Any] = {"candidate_id": candidate_id, "status": "accepted"}

        # If rule induction, create category_rule automatically
        if candidate.get("type") == "rule_induction" and candidate.get("suggested_rule_expression"):
            main_cat = candidate.get("suggested_main_category", "")
            sub_cat = candidate.get("suggested_sub_category", "")

            # Find matching category
            conn = await self._db._get_connection()
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                (
                    "SELECT id FROM categories "
                    "WHERE user_id = ? AND main_category = ? AND sub_category = ?"
                ),
                (user_id, main_cat, sub_cat),
            ) as cursor:
                cat_row = await cursor.fetchone()

            if cat_row:
                rule_data = {
                    "category_id": cat_row["id"],
                    "name": f"LLM induced: {main_cat}/{sub_cat}",
                    "priority": 50,
                    "rule_expression": candidate["suggested_rule_expression"],
                    "regex_enabled": False,
                    "enabled": True,
                }
                rule_id = await self._db.create_category_rule(rule_data, user_id=user_id)
                result["created_rule_id"] = rule_id

        return result

    async def reject_candidate(self, candidate_id: int, user_id: int) -> bool:
        """Mark candidate as rejected."""
        candidate = await self._db.get_llm_candidate_by_id(candidate_id, user_id=user_id)
        if not candidate:
            raise ValueError(f"Candidate {candidate_id} not found")
        return await self._db.update_llm_candidate_status(
            candidate_id,
            "rejected",
            user_id=user_id,
        )

    @staticmethod
    def _parse_classification_response(content: str) -> list[dict[str, Any]]:
        """Parse LLM JSON response for classification suggestions."""
        try:
            # Strip possible markdown code fences
            cleaned = content.strip()
            if cleaned.startswith("```"):
                lines = cleaned.split("\n")
                lines = [l for l in lines if not l.strip().startswith("```")]
                cleaned = "\n".join(lines)
            return json.loads(cleaned)
        except (json.JSONDecodeError, TypeError) as exc:
            logger.warning("Failed to parse LLM classification response: %s", exc)
            return []

    @staticmethod
    def _parse_rule_induction_response(content: str) -> list[dict[str, Any]]:
        """Parse LLM JSON response for rule induction suggestions."""
        try:
            cleaned = content.strip()
            if cleaned.startswith("```"):
                lines = cleaned.split("\n")
                lines = [l for l in lines if not l.strip().startswith("```")]
                cleaned = "\n".join(lines)
            return json.loads(cleaned)
        except (json.JSONDecodeError, TypeError) as exc:
            logger.warning("Failed to parse LLM rule induction response: %s", exc)
            return []
