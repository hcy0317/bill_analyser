"""Import-learning helpers for composite matches, annotations, and rules."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-locals,broad-exception-caught,too-few-public-methods

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite

from bill_analyser.core.database.time import utc_now_iso

_UNSET: Any = object()


class ImportLearningEventsMixin:
    """Record import-learning feedback events and concept statistics."""

    async def _record_import_learning_rule_log(
        self,
        conn: aiosqlite.Connection,
        *,
        rule_id: int | None,
        user_id: int,
        action: str,
        match_type: str,
        match_value: str,
        normalized_match_value: str,
        session_id: str | None = None,
        preview_id: int | None = None,
        payload: dict[str, Any] | None = None,
    ) -> None:
        await conn.execute(
            """
                INSERT INTO import_learning_rule_logs (
                    rule_id, user_id, action, match_type, match_value,
                    normalized_match_value, session_id, preview_id,
                    payload_json, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
            (
                rule_id,
                user_id,
                action,
                match_type,
                match_value,
                normalized_match_value,
                session_id,
                preview_id,
                json.dumps(payload or {}, ensure_ascii=False, sort_keys=True),
                utc_now_iso(),
            ),
        )

    @staticmethod
    def _build_import_learning_concept_key(
        *,
        rule_id: int | None = None,
        suggestion_id: int | None = None,
        candidate_id: str | None = None,
        fallback: str = "",
    ) -> tuple[str, str]:
        if rule_id:
            return (f"rule:{int(rule_id)}", "rule")
        if suggestion_id:
            return (f"suggestion:{int(suggestion_id)}", "suggestion")
        if candidate_id:
            return (f"candidate:{candidate_id}", "candidate")
        return (fallback or "global", "global")

    async def record_import_learning_feedback_event(
        self,
        event_type: str,
        *,
        user_id: int = 1,
        rule_id: int | None = None,
        suggestion_id: int | None = None,
        session_id: str | None = None,
        preview_id: int | None = None,
        bill_id: int | None = None,
        candidate_id: str | None = None,
        payload: dict[str, Any] | None = None,
        conn: aiosqlite.Connection | None = None,
    ) -> None:
        normalized_event_type = str(event_type or "").strip()
        if not normalized_event_type:
            return

        active_conn = conn or await self._get_connection()
        now = utc_now_iso()
        payload = dict(payload or {})
        await active_conn.execute(
            """
                INSERT INTO import_learning_feedback_events (
                    user_id, event_type, rule_id, suggestion_id, session_id,
                    preview_id, bill_id, candidate_id, payload_json, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
            (
                user_id,
                normalized_event_type,
                rule_id,
                suggestion_id,
                session_id,
                preview_id,
                bill_id,
                candidate_id,
                json.dumps(payload, ensure_ascii=False, sort_keys=True),
                now,
            ),
        )

        concept_key, concept_type = self._build_import_learning_concept_key(
            rule_id=rule_id,
            suggestion_id=suggestion_id,
            candidate_id=candidate_id,
            fallback=normalized_event_type,
        )
        accepted_delta = 1 if "accept" in normalized_event_type else 0
        rejected_delta = 1 if "reject" in normalized_event_type else 0
        auto_applied_delta = int(payload.get("applied_count") or 1) if "auto_apply" in normalized_event_type else 0
        rollback_delta = 1 if "rollback" in normalized_event_type or bool(payload.get("rollback")) else 0
        await active_conn.execute(
            """
                INSERT INTO import_learning_concept_stats (
                    user_id, concept_key, concept_type,
                    accepted_count, rejected_count, auto_applied_count, rollback_count, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id, concept_key, concept_type) DO UPDATE SET
                    accepted_count = accepted_count + excluded.accepted_count,
                    rejected_count = rejected_count + excluded.rejected_count,
                    auto_applied_count = auto_applied_count + excluded.auto_applied_count,
                    rollback_count = rollback_count + excluded.rollback_count,
                    updated_at = excluded.updated_at
                """,
            (
                user_id,
                concept_key,
                concept_type,
                accepted_delta,
                rejected_delta,
                auto_applied_delta,
                rollback_delta,
                now,
            ),
        )

        if conn is None:
            await active_conn.commit()
