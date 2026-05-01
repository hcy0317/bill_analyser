"""Yellow LLM import-preview recommendation workflow."""
# pylint: disable=duplicate-code,protected-access,too-many-arguments,too-many-branches
# pylint: disable=too-many-locals,too-many-positional-arguments,too-many-statements

from __future__ import annotations

from typing import Any

import aiosqlite

from ..llm_prompts import build_import_preview_recommendation_prompt
from ..llm_provider import LLMResponse
from .common import logger
from .errors import LLMImportSessionAnalysisError


class PreviewRecommendationsMixin:
    """Generate, accept, reject, and load context for yellow preview recommendations."""

    _MAX_PREVIEW_RECOMMEND_BATCH = 20
    _MAX_MEMORY_CONTEXT = 20

    @staticmethod
    def _normalize_preview_recommendation_preview(
        preview: dict[str, Any] | None,
    ) -> dict[str, Any] | None:
        if not isinstance(preview, dict):
            return None
        return {
            "id": int(preview.get("id") or 0),
            "category_id": preview.get("category_id"),
            "preview_type": str(preview.get("preview_type") or ""),
            "preview_date": str(preview.get("preview_date") or ""),
            "preview_amount": float(preview.get("preview_amount") or 0),
            "preview_destination_amount": float(preview.get("preview_destination_amount") or 0),
            "preview_main_category": str(preview.get("preview_main_category") or ""),
            "preview_sub_category": str(preview.get("preview_sub_category") or ""),
            "preview_source_account_id": preview.get("preview_source_account_id"),
            "preview_destination_account_id": preview.get("preview_destination_account_id"),
            "preview_description": str(preview.get("preview_description") or ""),
            "preview_counterparty": str(preview.get("preview_counterparty") or ""),
            "preview_payment_method": str(preview.get("preview_payment_method") or ""),
            "preview_parser_id": str(preview.get("preview_parser_id") or ""),
            "preview_parser_tags": list(preview.get("preview_parser_tags") or []),
        }

    @staticmethod
    def _build_preview_recommendation_result(
        *,
        preview_id: int,
        preview: dict[str, Any] | None,
        llm_payload: dict[str, Any] | None,
        event_id: int | None = None,
        applied_fields: list[str] | None = None,
        decision: str | None = None,
        restored: bool | None = None,
    ) -> dict[str, Any]:
        normalized_preview = (
            PreviewRecommendationsMixin._normalize_preview_recommendation_preview(preview)
        )
        result: dict[str, Any] = {
            "preview_id": preview_id,
            "preview": normalized_preview,
            "matching": {
                "llm": dict(llm_payload or {}),
            },
        }
        if event_id is not None:
            result["event_id"] = event_id
        if applied_fields is not None:
            result["applied_fields"] = list(applied_fields)
        if decision:
            result["decision"] = decision
        if restored is not None:
            result["restored"] = restored
        return result

    async def recommend_for_preview(
        self,
        user_id: int,
        session_id: str,
        preview_ids: list[int] | None = None,
        preview_updates: list[dict[str, Any]] | None = None,
        limit: int = 20,
    ) -> list[dict[str, Any]]:
        """Generate yellow LLM recommendations, apply them to preview draft, and persist memory."""
        session = await self._db.get_import_session(session_id, user_id=user_id)
        if not session:
            raise LLMImportSessionAnalysisError(
                "IMPORT_SESSION_NOT_FOUND",
                "Import session not found",
                status_code=404,
            )

        bounded_limit = min(int(limit), self._MAX_PREVIEW_RECOMMEND_BATCH)
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

        previews: list[dict[str, Any]]
        if normalized_preview_updates is not None or preview_ids is not None:
            if not selected_preview_ids:
                raise LLMImportSessionAnalysisError(
                    "PREVIEW_SELECTION_EMPTY",
                    "No preview rows available for LLM recommendation",
                )
            preview_loader = getattr(self._db, "get_preview_bill_by_id", None)
            if callable(preview_loader):
                previews = []
                for pid in selected_preview_ids[:bounded_limit]:
                    preview = await preview_loader(int(pid), user_id=user_id)
                    if preview and str(preview.get("session_id") or "") == session_id:
                        previews.append(preview)
            else:
                all_previews = await self._db.get_preview_by_session(session_id, user_id=user_id)
                preview_id_set = set(selected_preview_ids[:bounded_limit])
                previews = [
                    preview
                    for preview in all_previews
                    if int(preview.get("id") or 0) in preview_id_set
                ]
        else:
            previews = await self._db.get_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=True,
            )
            previews = previews[:bounded_limit]

        if not previews:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_EMPTY",
                "No preview rows available for LLM recommendation",
            )

        memory_context = await self._load_memory_context(user_id)
        existing_categories = await self._load_existing_categories(user_id)
        existing_accounts = await self._load_existing_accounts(user_id)

        transactions = [
            {
                "id": p.get("id"),
                "date": p.get("preview_date") or p.get("date") or "",
                "amount": p.get("preview_amount") or p.get("amount") or 0,
                "type": p.get("preview_type") or p.get("type") or "",
                "counterparty": p.get("preview_counterparty") or p.get("counterparty") or "",
                "description": p.get("preview_description") or p.get("description") or "",
                "payment_method": p.get("preview_payment_method") or p.get("payment_method") or "",
            }
            for p in previews
        ]

        self._reserve_rate_limit_slots(user_id, 1)

        prompt = build_import_preview_recommendation_prompt(
            transactions,
            existing_categories=existing_categories,
            existing_accounts=existing_accounts,
            memory_context=memory_context,
        )

        try:
            response: LLMResponse = await self._generate(prompt)
        except Exception as exc:
            logger.error("LLM preview recommendation failed: %s", exc)
            raise RuntimeError(f"LLM request failed: {exc}") from exc

        suggestions = self._parse_classification_response(response.content)
        suggestions_by_preview_id: dict[int, dict[str, Any]] = {}
        for suggestion in suggestions:
            preview_id = int(suggestion.get("preview_id") or 0)
            if preview_id > 0 and preview_id not in suggestions_by_preview_id:
                suggestions_by_preview_id[preview_id] = dict(suggestion)

        results: list[dict[str, Any]] = []
        for preview in previews:
            preview_id = int(preview.get("id") or 0)
            if preview_id <= 0:
                continue

            suggestion = suggestions_by_preview_id.get(preview_id)
            if not suggestion:
                continue

            apply_result = await self._db.apply_preview_llm_recommendation(
                user_id=user_id,
                session_id=session_id,
                preview_id=preview_id,
                suggestion=suggestion,
                prompt_text=prompt,
                llm_provider=response.provider,
                llm_model=response.model,
            )
            if not apply_result:
                continue

            results.append(
                self._build_preview_recommendation_result(
                    preview_id=preview_id,
                    preview=apply_result.get("preview"),
                    llm_payload=apply_result.get("llm"),
                    event_id=(
                        int(apply_result.get("event_id"))
                        if apply_result.get("event_id") not in (None, "")
                        else None
                    ),
                    applied_fields=list(apply_result.get("applied_fields") or []),
                )
            )

        logger.info(
            "LLM preview recommendation produced %d suggestions for session %s (user %s)",
            len(results), session_id, user_id,
        )
        return results

    async def accept_preview_recommendation(
        self,
        user_id: int,
        session_id: str,
        preview_id: int,
        suggestion: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Record an accept decision for a yellow LLM recommendation and keep the applied draft."""
        result = await self._db.review_preview_llm_recommendation(
            user_id=user_id,
            session_id=session_id,
            preview_id=preview_id,
            decision="accept",
            suggestion=suggestion,
        )
        if not result:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_EMPTY",
                "Preview recommendation is no longer available",
                status_code=404,
            )
        return self._build_preview_recommendation_result(
            preview_id=preview_id,
            preview=result.get("preview"),
            llm_payload=result.get("llm"),
            event_id=(
                int(result.get("event_id"))
                if result.get("event_id") not in (None, "")
                else None
            ),
            decision="accept",
            restored=False,
        )

    async def reject_preview_recommendation(
        self,
        user_id: int,
        session_id: str,
        preview_id: int,
        suggestion: dict[str, Any] | None = None,
        user_correction: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Reject a yellow LLM recommendation, restore snapshot when safe, and append memory."""
        result = await self._db.review_preview_llm_recommendation(
            user_id=user_id,
            session_id=session_id,
            preview_id=preview_id,
            decision="reject",
            suggestion=suggestion,
            user_correction=user_correction,
        )
        if not result:
            raise LLMImportSessionAnalysisError(
                "PREVIEW_SELECTION_EMPTY",
                "Preview recommendation is no longer available",
                status_code=404,
            )
        return self._build_preview_recommendation_result(
            preview_id=preview_id,
            preview=result.get("preview"),
            llm_payload=result.get("llm"),
            event_id=(
                int(result.get("event_id"))
                if result.get("event_id") not in (None, "")
                else None
            ),
            decision="reject",
            restored=bool(result.get("restored")),
        )

    async def _load_memory_context(self, user_id: int) -> list[dict[str, Any]]:
        """Load recent feedback events for LLM prompt context enrichment."""
        try:
            events = await self._db.get_llm_memory_events(
                user_id,
                event_type="feedback",
                limit=self._MAX_MEMORY_CONTEXT,
            )
            context: list[dict[str, Any]] = []
            for event in events:
                context.append({
                    "decision": event.get("decision"),
                    "suggested_main_category": event.get("suggested_main_category") or "",
                    "suggested_sub_category": event.get("suggested_sub_category") or "",
                    "description_hint": (event.get("metadata") or "")[:80],
                })
            return context
        except Exception:  # pylint: disable=broad-exception-caught
            return []

    async def _load_existing_categories(self, user_id: int) -> list[str]:
        """Load user's existing category paths for prompt context."""
        try:
            conn = await self._db._get_connection()
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT main_category, sub_category FROM categories "
                "WHERE user_id = ? ORDER BY main_category, sub_category",
                (user_id,),
            ) as cursor:
                rows = await cursor.fetchall()
                return [
                    "/".join(filter(None, [row["main_category"], row["sub_category"]]))
                    for row in rows
                    if row["main_category"]
                ]
        except Exception:  # pylint: disable=broad-exception-caught
            return []

    async def _load_existing_accounts(self, user_id: int) -> list[str]:
        """Load user's visible account names for prompt context."""
        try:
            accounts = await self._db.get_all_accounts(user_id=user_id)
        except Exception:  # pylint: disable=broad-exception-caught
            return []

        ordered_names: list[str] = []
        for account in accounts:
            account_name = str(account.get("name") or "").strip()
            if account_name and account_name not in ordered_names:
                ordered_names.append(account_name)
        return ordered_names
