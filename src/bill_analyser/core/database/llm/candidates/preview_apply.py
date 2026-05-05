"""LLM preview recommendation apply helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-arguments,too-many-positional-arguments,too-many-locals

import json
from typing import Any

import aiosqlite

from bill_analyser.utils.logger import log_method


class LLMCandidatePreviewApplyMixin:
    """Apply LLM recommendations to import-preview rows."""

    async def _fetch_preview_llm_row(
        self,
        conn: aiosqlite.Connection,
        *,
        preview_id: int,
        user_id: int,
        session_id: str,
    ) -> dict[str, Any] | None:
        async with conn.execute(
            "SELECT * FROM bills_preview WHERE id = ? AND user_id = ? AND session_id = ?",
            (preview_id, user_id, session_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _build_preview_llm_apply_state(
        self,
        conn: aiosqlite.Connection,
        *,
        user_id: int,
        preview: dict[str, Any],
        suggestion: dict[str, Any],
    ) -> dict[str, Any]:
        previous_preview_snapshot = self._build_llm_previous_preview_snapshot(preview)
        current_main_category = str(preview.get("preview_main_category") or "").strip()
        current_sub_category = str(preview.get("preview_sub_category") or "").strip()
        current_source_account_id = self._normalize_optional_account_id(preview.get("preview_source_account_id"))
        current_destination_account_id = self._normalize_optional_account_id(
            preview.get("preview_destination_account_id")
        )

        suggested_main_category = str(suggestion.get("suggested_main_category") or "").strip()
        suggested_sub_category = str(suggestion.get("suggested_sub_category") or "").strip()
        suggested_source_account = str(suggestion.get("suggested_source_account") or "").strip()
        suggested_destination_account = str(suggestion.get("suggested_destination_account") or "").strip()

        resolved_source_account_id = await self._resolve_account_id_by_name(
            conn,
            user_id=user_id,
            account_name=suggested_source_account,
        )
        resolved_destination_account_id = await self._resolve_account_id_by_name(
            conn,
            user_id=user_id,
            account_name=suggested_destination_account,
        )

        next_main_category = current_main_category
        next_sub_category = current_sub_category
        if not current_main_category and not current_sub_category and suggested_main_category:
            next_main_category = suggested_main_category
            next_sub_category = suggested_sub_category

        next_source_account_id = current_source_account_id
        if current_source_account_id is None and resolved_source_account_id is not None:
            next_source_account_id = resolved_source_account_id

        next_destination_account_id = current_destination_account_id
        if current_destination_account_id is None and resolved_destination_account_id is not None:
            next_destination_account_id = resolved_destination_account_id

        applied_preview_snapshot = {
            "preview_main_category": next_main_category,
            "preview_sub_category": next_sub_category,
            "preview_source_account_id": next_source_account_id,
            "preview_destination_account_id": next_destination_account_id,
        }
        update_parts, params, applied_fields = self._build_preview_llm_apply_updates(
            current_main_category=current_main_category,
            current_sub_category=current_sub_category,
            current_source_account_id=current_source_account_id,
            current_destination_account_id=current_destination_account_id,
            applied_preview_snapshot=applied_preview_snapshot,
        )
        feedback_payload = self._build_preview_llm_apply_feedback(
            preview,
            suggestion,
            update_parts=update_parts,
            params=params,
            applied_fields=applied_fields,
            previous_preview_snapshot=previous_preview_snapshot,
            applied_preview_snapshot=applied_preview_snapshot,
        )
        return {
            "applied_fields": applied_fields,
            "feedback_payload": feedback_payload,
            "previous_preview_snapshot": previous_preview_snapshot,
            "applied_preview_snapshot": applied_preview_snapshot,
            "params": params,
            "resolved_source_account_id": resolved_source_account_id,
            "resolved_destination_account_id": resolved_destination_account_id,
            "suggested_main_category": suggested_main_category,
            "suggested_sub_category": suggested_sub_category,
            "suggested_source_account": suggested_source_account,
            "suggested_destination_account": suggested_destination_account,
            "update_parts": update_parts,
        }

    @staticmethod
    def _build_preview_llm_apply_updates(
        *,
        current_main_category: str,
        current_sub_category: str,
        current_source_account_id: int | None,
        current_destination_account_id: int | None,
        applied_preview_snapshot: dict[str, Any],
    ) -> tuple[list[str], list[Any], list[str]]:
        update_parts: list[str] = []
        params: list[Any] = []
        applied_fields: list[str] = []
        for field, current_value in [
            ("preview_main_category", current_main_category),
            ("preview_sub_category", current_sub_category),
            ("preview_source_account_id", current_source_account_id),
            ("preview_destination_account_id", current_destination_account_id),
        ]:
            next_value = applied_preview_snapshot[field]
            if next_value != current_value:
                update_parts.append(f"{field} = ?")
                params.append(next_value)
                applied_fields.append(field)
        return update_parts, params, applied_fields

    def _build_preview_llm_apply_feedback(
        self,
        preview: dict[str, Any],
        suggestion: dict[str, Any],
        *,
        update_parts: list[str],
        params: list[Any],
        applied_fields: list[str],
        previous_preview_snapshot: dict[str, Any],
        applied_preview_snapshot: dict[str, Any],
    ) -> dict[str, Any]:
        next_feedback_json = preview.get("preview_matching_feedback_json")
        if applied_fields:
            next_feedback_json = self._clear_transfer_matching_feedback(next_feedback_json)
        feedback_payload = self._deserialize_preview_matching_feedback(next_feedback_json)
        feedback_payload["llm"] = self._build_llm_feedback_payload(
            suggestion,
            review_status="pending",
            suppressed=False,
            previous_preview=previous_preview_snapshot,
            applied_preview=applied_preview_snapshot,
        )
        update_parts.append("preview_matching_feedback_json = ?")
        params.append(self._serialize_preview_matching_feedback(feedback_payload))
        return feedback_payload

    async def _record_preview_llm_recommendation_event(
        self,
        conn: aiosqlite.Connection,
        *,
        user_id: int,
        session_id: str,
        preview_id: int,
        preview: dict[str, Any],
        suggestion: dict[str, Any],
        prompt_text: str | None,
        llm_provider: str | None,
        llm_model: str | None,
        state: dict[str, Any],
    ) -> int:
        return await self.create_llm_memory_event(
            user_id=user_id,
            conn=conn,
            session_id=session_id,
            preview_id=preview_id,
            event_type="recommendation",
            decision=None,
            prompt_text=prompt_text,
            llm_response_raw=json.dumps(suggestion, ensure_ascii=False),
            llm_provider=llm_provider,
            llm_model=llm_model,
            suggested_main_category=state["suggested_main_category"] or None,
            suggested_sub_category=state["suggested_sub_category"] or None,
            suggested_source_account=state["suggested_source_account"] or None,
            suggested_destination_account=state["suggested_destination_account"] or None,
            confidence=float(suggestion.get("confidence", 0.0) or 0.0),
            snapshot_before=json.dumps(state["previous_preview_snapshot"], ensure_ascii=False),
            snapshot_after=json.dumps(state["applied_preview_snapshot"], ensure_ascii=False),
            metadata=json.dumps(
                {
                    "reason": str(suggestion.get("reason") or ""),
                    "counterparty": str(preview.get("preview_counterparty") or ""),
                    "payment_method": str(preview.get("preview_payment_method") or ""),
                    "description": str(preview.get("preview_description") or ""),
                    "applied_fields": state["applied_fields"],
                    "resolved_source_account_id": state["resolved_source_account_id"],
                    "resolved_destination_account_id": state["resolved_destination_account_id"],
                },
                ensure_ascii=False,
            ),
        )

    @log_method
    async def apply_preview_llm_recommendation(
        self,
        *,
        user_id: int,
        session_id: str,
        preview_id: int,
        suggestion: dict[str, Any],
        prompt_text: str | None,
        llm_provider: str | None,
        llm_model: str | None,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        try:
            await conn.execute("BEGIN IMMEDIATE")
            conn.row_factory = aiosqlite.Row
            preview = await self._fetch_preview_llm_row(
                conn,
                preview_id=preview_id,
                user_id=user_id,
                session_id=session_id,
            )
            if not preview:
                await conn.rollback()
                return None

            state = await self._build_preview_llm_apply_state(
                conn,
                user_id=user_id,
                preview=preview,
                suggestion=suggestion,
            )
            state["params"].extend([preview_id, user_id, session_id])
            cursor = await conn.execute(
                f"UPDATE bills_preview SET {', '.join(state['update_parts'])} "
                "WHERE id = ? AND user_id = ? AND session_id = ?",
                tuple(state["params"]),
            )
            if int(cursor.rowcount or 0) < 1:
                await conn.rollback()
                return None

            event_id = await self._record_preview_llm_recommendation_event(
                conn,
                user_id=user_id,
                session_id=session_id,
                preview_id=preview_id,
                preview=preview,
                suggestion=suggestion,
                prompt_text=prompt_text,
                llm_provider=llm_provider,
                llm_model=llm_model,
                state=state,
            )
            await conn.commit()
        except Exception:
            await conn.rollback()
            raise

        updated_preview = await self.get_preview_bill_by_id(preview_id, user_id=user_id)
        return {
            "event_id": event_id,
            "applied_fields": state["applied_fields"],
            "preview": updated_preview,
            "llm": state["feedback_payload"].get("llm"),
        }
