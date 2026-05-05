"""LLM preview recommendation review helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-arguments,too-many-positional-arguments,too-many-locals

import json
from typing import Any

import aiosqlite

from bill_analyser.utils.logger import log_method


class LLMCandidatePreviewReviewMixin:
    """Review accepted or rejected LLM preview recommendations."""

    def _build_preview_llm_review_state(
        self,
        preview: dict[str, Any],
        *,
        normalized_decision: str,
        suggestion: dict[str, Any] | None,
    ) -> dict[str, Any]:
        feedback_payload = self._deserialize_preview_matching_feedback(
            preview.get("preview_matching_feedback_json")
        )
        llm_feedback = feedback_payload.get("llm") if isinstance(feedback_payload, dict) else {}
        if not isinstance(llm_feedback, dict):
            llm_feedback = {}

        previous_preview_snapshot = self._normalize_llm_previous_preview_snapshot(
            llm_feedback.get("previous_preview")
        )
        applied_preview_snapshot = self._normalize_llm_previous_preview_snapshot(
            llm_feedback.get("applied_preview")
        )
        should_restore_previous_preview = (
            normalized_decision == "reject"
            and bool(previous_preview_snapshot)
            and (
                not applied_preview_snapshot
                or self._llm_preview_matches_snapshot(preview, applied_preview_snapshot)
            )
        )
        resolved_suggestion = self._resolve_preview_llm_review_suggestion(suggestion, llm_feedback)
        update_parts: list[str] = []
        params: list[Any] = []
        if should_restore_previous_preview:
            self._append_llm_snapshot_restore_updates(previous_preview_snapshot, update_parts, params)

        current_preview_snapshot = self._build_llm_previous_preview_snapshot(preview)
        next_llm_feedback = self._build_llm_feedback_payload(
            resolved_suggestion,
            review_status="accepted" if normalized_decision == "accept" else "rejected",
            suppressed=normalized_decision == "reject",
            previous_preview=previous_preview_snapshot or current_preview_snapshot,
            applied_preview=applied_preview_snapshot or current_preview_snapshot,
        )
        feedback_payload["llm"] = next_llm_feedback
        update_parts.append("preview_matching_feedback_json = ?")
        params.append(self._serialize_preview_matching_feedback(feedback_payload))
        refreshed_snapshot = (
            previous_preview_snapshot
            if should_restore_previous_preview
            else applied_preview_snapshot or current_preview_snapshot
        )
        return {
            "applied_preview_snapshot": applied_preview_snapshot,
            "current_preview_snapshot": current_preview_snapshot,
            "feedback_payload": feedback_payload,
            "params": params,
            "previous_preview_snapshot": previous_preview_snapshot,
            "refreshed_snapshot": refreshed_snapshot,
            "resolved_suggestion": resolved_suggestion,
            "should_restore_previous_preview": should_restore_previous_preview,
            "update_parts": update_parts,
        }

    @staticmethod
    def _resolve_preview_llm_review_suggestion(
        suggestion: dict[str, Any] | None,
        llm_feedback: dict[str, Any],
    ) -> dict[str, Any]:
        resolved_suggestion = dict(suggestion or {})
        if resolved_suggestion:
            return resolved_suggestion
        return {
            "suggested_main_category": llm_feedback.get("suggested_main_category"),
            "suggested_sub_category": llm_feedback.get("suggested_sub_category"),
            "suggested_source_account": llm_feedback.get("suggested_source_account"),
            "suggested_destination_account": llm_feedback.get("suggested_destination_account"),
            "confidence": llm_feedback.get("confidence"),
            "reason": llm_feedback.get("reason"),
        }

    async def _record_preview_llm_review_event(
        self,
        conn: aiosqlite.Connection,
        *,
        user_id: int,
        session_id: str,
        preview_id: int,
        normalized_decision: str,
        user_correction: dict[str, Any] | None,
        state: dict[str, Any],
    ) -> int:
        resolved_suggestion = state["resolved_suggestion"]
        return await self.create_llm_memory_event(
            user_id=user_id,
            conn=conn,
            session_id=session_id,
            preview_id=preview_id,
            event_type="feedback",
            decision=normalized_decision,
            llm_response_raw=json.dumps(resolved_suggestion, ensure_ascii=False),
            llm_provider=None,
            llm_model=None,
            suggested_main_category=str(resolved_suggestion.get("suggested_main_category") or "") or None,
            suggested_sub_category=str(resolved_suggestion.get("suggested_sub_category") or "") or None,
            suggested_source_account=str(resolved_suggestion.get("suggested_source_account") or "") or None,
            suggested_destination_account=str(resolved_suggestion.get("suggested_destination_account") or "") or None,
            confidence=float(resolved_suggestion.get("confidence", 0.0) or 0.0),
            user_correction_category=(
                str(user_correction.get("category") or "") or None
                if isinstance(user_correction, dict)
                else None
            ),
            user_correction_account=(
                str(user_correction.get("account") or "") or None
                if isinstance(user_correction, dict)
                else None
            ),
            snapshot_before=json.dumps(state["current_preview_snapshot"], ensure_ascii=False),
            snapshot_after=json.dumps(state["refreshed_snapshot"], ensure_ascii=False),
            metadata=json.dumps(
                {
                    "reason": str(resolved_suggestion.get("reason") or ""),
                    "rollback": bool(state["should_restore_previous_preview"]),
                },
                ensure_ascii=False,
            ),
        )

    @log_method
    async def review_preview_llm_recommendation(
        self,
        *,
        user_id: int,
        session_id: str,
        preview_id: int,
        decision: str,
        suggestion: dict[str, Any] | None = None,
        user_correction: dict[str, Any] | None = None,
    ) -> dict[str, Any] | None:
        normalized_decision = str(decision or "").strip().lower()
        if normalized_decision not in {"accept", "reject"}:
            return None

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

            state = self._build_preview_llm_review_state(
                preview,
                normalized_decision=normalized_decision,
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

            event_id = await self._record_preview_llm_review_event(
                conn,
                user_id=user_id,
                session_id=session_id,
                preview_id=preview_id,
                normalized_decision=normalized_decision,
                user_correction=user_correction,
                state=state,
            )
            await conn.commit()
        except Exception:
            await conn.rollback()
            raise

        updated_preview = await self.get_preview_bill_by_id(preview_id, user_id=user_id)
        return {
            "event_id": event_id,
            "restored": state["should_restore_previous_preview"],
            "preview": updated_preview,
            "llm": state["feedback_payload"].get("llm"),
        }
