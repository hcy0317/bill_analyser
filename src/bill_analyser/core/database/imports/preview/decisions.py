"""Import-preview editing, confirmation, and temporary staging helpers."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-positional-arguments,too-many-locals,broad-exception-caught,assignment-from-no-return,too-many-nested-blocks,too-many-lines

from __future__ import annotations

import json
import sqlite3
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from datetime import date

from bill_analyser.import_contracts.parser_tags import resolve_parser_tags, serialize_parser_tags
from bill_analyser.utils.logger import log_method
from bill_analyser.core.bill_date_utils import normalize_bill_date_text
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now, utc_now_iso


class ImportPreviewDecisionsMixin(object):
        @log_method
        async def update_preview_transfer_decision(
            self,
            preview_id: int,
            decision: str,
            user_id: int = 1,
            reviewed_type: str = "转账",
            expected_state: dict[str, Any] | None = None,
        ) -> dict[str, Any] | None:
            normalized_decision = str(decision or "").strip().lower()
            if normalized_decision not in {"accept", "reject", "clear"}:
                return None
            conn = await self._get_connection()

            try:
                await conn.execute("BEGIN IMMEDIATE")

                async with conn.execute(
                    "SELECT * FROM bills_preview WHERE id = ? AND user_id = ?",
                    (preview_id, user_id),
                ) as cursor:
                    row = await cursor.fetchone()

                if not row:
                    await conn.rollback()
                    return None

                raw_preview = dict(row)
                if not self._preview_state_matches_snapshot(raw_preview, expected_state):
                    await conn.rollback()
                    return {"_state_conflict": True}

                preview = self._normalize_preview_row(raw_preview)
                feedback_payload = self._deserialize_preview_matching_feedback(preview.get("preview_matching_feedback"))
                transfer_feedback = feedback_payload.get("transfer") if isinstance(feedback_payload, dict) else {}
                if not isinstance(transfer_feedback, dict):
                    transfer_feedback = {}
                previous_preview_snapshot = self._normalize_transfer_previous_preview_snapshot(
                    transfer_feedback.get("previous_preview")
                )
                current_review_status = str(transfer_feedback.get("review_status") or "").strip().lower()
                should_restore_previous_preview = current_review_status == "accepted" and bool(previous_preview_snapshot)

                update_parts: list[str] = []
                params: list[Any] = []

                if normalized_decision == "accept":
                    if not previous_preview_snapshot:
                        previous_preview_snapshot = self._build_transfer_previous_preview_snapshot(preview)

                    feedback_payload["transfer"] = {
                        "review_status": "accepted",
                        "reviewed_type": reviewed_type,
                        "suppressed": False,
                        "previous_preview": previous_preview_snapshot,
                    }
                    update_parts.extend(
                        [
                            "preview_type = ?",
                            "preview_main_category = ?",
                            "preview_sub_category = ?",
                            "preview_recurring_id = ?",
                            "preview_recurring_name = ?",
                            "preview_recurring_candidate_count = ?",
                            "preview_recurring_match_score = ?",
                            "preview_recurring_match_reasons = ?",
                            "preview_recurring_matched_date = ?",
                        ]
                    )
                    params.extend([reviewed_type, "", "", None, "", 0, 0, "", ""])
                elif normalized_decision == "reject":
                    if should_restore_previous_preview:
                        self._append_transfer_snapshot_restore_updates(previous_preview_snapshot, update_parts, params)

                    feedback_payload["transfer"] = {
                        "review_status": "rejected",
                        "reviewed_type": "",
                        "suppressed": True,
                    }
                else:
                    if should_restore_previous_preview:
                        self._append_transfer_snapshot_restore_updates(previous_preview_snapshot, update_parts, params)
                    feedback_payload.pop("transfer", None)

                update_parts.append("preview_matching_feedback_json = ?")
                params.append(self._serialize_preview_matching_feedback(feedback_payload))
                params.extend([preview_id, user_id])

                cursor = await conn.execute(
                    f"UPDATE bills_preview SET {', '.join(update_parts)} WHERE id = ? AND user_id = ?",
                    tuple(params),
                )
                if int(cursor.rowcount or 0) < 1:
                    await conn.rollback()
                    return None

                await conn.commit()
            except Exception:
                await conn.rollback()
                raise

            return await self.get_preview_bill_by_id(preview_id, user_id=user_id)

        @log_method
        async def update_preview_investment_decision(
            self,
            preview_id: int,
            decision: str,
            user_id: int = 1,
            expected_state: dict[str, Any] | None = None,
        ) -> dict[str, Any] | None:
            normalized_decision = str(decision or "").strip().lower()
            if normalized_decision not in {"accept", "reject", "clear"}:
                return None

            conn = await self._get_connection()

            try:
                await conn.execute("BEGIN IMMEDIATE")

                async with conn.execute(
                    "SELECT * FROM bills_preview WHERE id = ? AND user_id = ?",
                    (preview_id, user_id),
                ) as cursor:
                    row = await cursor.fetchone()

                if not row:
                    await conn.rollback()
                    return None

                raw_preview = dict(row)
                if not self._preview_state_matches_snapshot(raw_preview, expected_state):
                    await conn.rollback()
                    return {"_state_conflict": True}

                preview = self._normalize_preview_row(raw_preview)
                feedback_payload = self._deserialize_preview_matching_feedback(
                    preview.get("preview_matching_feedback")
                )
                if normalized_decision == "accept":
                    feedback_payload["investment"] = {
                        "review_status": "accepted",
                        "suppressed": False,
                    }
                elif normalized_decision == "reject":
                    feedback_payload["investment"] = {
                        "review_status": "rejected",
                        "suppressed": True,
                    }
                else:
                    feedback_payload.pop("investment", None)

                cursor = await conn.execute(
                    "UPDATE bills_preview SET preview_matching_feedback_json = ? WHERE id = ? AND user_id = ?",
                    (
                        self._serialize_preview_matching_feedback(feedback_payload),
                        preview_id,
                        user_id,
                    ),
                )
                if int(cursor.rowcount or 0) < 1:
                    await conn.rollback()
                    return None

                await conn.commit()
            except Exception:
                await conn.rollback()
                raise

            return await self.get_preview_bill_by_id(preview_id, user_id=user_id)

        @log_method
        async def update_preview_learning_decision(
            self,
            preview_id: int,
            decision: str,
            user_id: int = 1,
            expected_state: dict[str, Any] | None = None,
            applied_result: dict[str, Any] | None = None,
        ) -> dict[str, Any] | None:
            normalized_decision = str(decision or "").strip().lower()
            if normalized_decision not in {"accept", "reject", "clear"}:
                return None

            conn = await self._get_connection()

            try:
                await conn.execute("BEGIN IMMEDIATE")

                async with conn.execute(
                    "SELECT * FROM bills_preview WHERE id = ? AND user_id = ?",
                    (preview_id, user_id),
                ) as cursor:
                    row = await cursor.fetchone()

                if not row:
                    await conn.rollback()
                    return None

                raw_preview = dict(row)
                if not self._preview_state_matches_snapshot(raw_preview, expected_state):
                    await conn.rollback()
                    return {"_state_conflict": True}

                preview = self._normalize_preview_row(raw_preview)
                feedback_payload = self._deserialize_preview_matching_feedback(
                    preview.get("preview_matching_feedback")
                )
                learning_feedback = feedback_payload.get("learning") if isinstance(feedback_payload, dict) else {}
                if not isinstance(learning_feedback, dict):
                    learning_feedback = {}
                previous_preview_snapshot = self._normalize_learning_previous_preview_snapshot(
                    learning_feedback.get("previous_preview")
                )
                applied_preview_snapshot = self._normalize_learning_previous_preview_snapshot(
                    learning_feedback.get("applied_preview")
                )
                current_review_status = str(learning_feedback.get("review_status") or "").strip().lower()
                should_restore_previous_preview = (
                    current_review_status == "accepted"
                    and bool(previous_preview_snapshot)
                    and (
                        not applied_preview_snapshot
                        or self._learning_preview_matches_snapshot(preview, applied_preview_snapshot)
                    )
                )

                update_parts: list[str] = []
                params: list[Any] = []

                if normalized_decision == "accept":
                    if not previous_preview_snapshot:
                        previous_preview_snapshot = self._build_learning_previous_preview_snapshot(preview)

                    applied_updates = self._build_learning_accept_preview_updates(applied_result or {}, preview)
                    for field, _default_value in self._LEARNING_PREVIEW_SNAPSHOT_FIELDS:
                        update_parts.append(f"{field} = ?")
                        params.append(applied_updates[field])

                    feedback_payload["learning"] = {
                        "review_status": "accepted",
                        "suppressed": False,
                        "previous_preview": previous_preview_snapshot,
                        "applied_preview": applied_updates,
                    }
                    normalized_rule_id = applied_result.get("rule_id") if isinstance(applied_result, dict) else None
                    if normalized_rule_id not in (None, "", 0, "0"):
                        feedback_payload["learning"]["rule_id"] = int(normalized_rule_id)
                elif normalized_decision == "reject":
                    if should_restore_previous_preview:
                        self._append_learning_snapshot_restore_updates(previous_preview_snapshot, update_parts, params)

                    feedback_payload["learning"] = {
                        "review_status": "rejected",
                        "suppressed": True,
                    }
                    normalized_rule_id = None
                    if isinstance(applied_result, dict):
                        normalized_rule_id = applied_result.get("rule_id")
                    if normalized_rule_id in (None, "", 0, "0"):
                        normalized_rule_id = learning_feedback.get("rule_id")
                    if normalized_rule_id not in (None, "", 0, "0"):
                        feedback_payload["learning"]["rule_id"] = int(normalized_rule_id)
                else:
                    if should_restore_previous_preview:
                        self._append_learning_snapshot_restore_updates(previous_preview_snapshot, update_parts, params)
                    feedback_payload.pop("learning", None)

                update_parts.append("preview_matching_feedback_json = ?")
                params.append(self._serialize_preview_matching_feedback(feedback_payload))
                params.extend([preview_id, user_id])

                cursor = await conn.execute(
                    f"UPDATE bills_preview SET {', '.join(update_parts)} WHERE id = ? AND user_id = ?",
                    tuple(params),
                )
                if int(cursor.rowcount or 0) < 1:
                    await conn.rollback()
                    return None

                event_type = f"preview_{normalized_decision}"
                if normalized_decision == "clear" and should_restore_previous_preview:
                    event_type = "preview_rollback"
                feedback_learning = (
                    feedback_payload.get("learning") if isinstance(feedback_payload, dict) else {}
                )
                feedback_rule_id = (
                    feedback_learning.get("rule_id")
                    if isinstance(feedback_learning, dict)
                    else learning_feedback.get("rule_id")
                )
                normalized_feedback_rule_id = (
                    int(feedback_rule_id) if feedback_rule_id not in (None, "", 0, "0") else None
                )
                if hasattr(self, "record_import_learning_feedback_event"):
                    await self.record_import_learning_feedback_event(
                        event_type,
                        user_id=user_id,
                        rule_id=normalized_feedback_rule_id,
                        session_id=str(preview.get("session_id") or ""),
                        preview_id=preview_id,
                        payload={
                            "decision": normalized_decision,
                            "rollback": bool(should_restore_previous_preview),
                        },
                        conn=conn,
                    )

                await conn.commit()
            except Exception:
                await conn.rollback()
                raise

            return await self.get_preview_bill_by_id(preview_id, user_id=user_id)

        @log_method
        async def update_preview_recurring_match(
            self,
            preview_id: int,
            recurring_id: int | None,
            user_id: int = 1,
            expected_state: dict[str, Any] | None = None,
        ) -> dict[str, Any] | None:
            conn = await self._get_connection()

            try:
                await conn.execute("BEGIN IMMEDIATE")

                async with conn.execute(
                    "SELECT * FROM bills_preview WHERE id = ? AND user_id = ?",
                    (preview_id, user_id),
                ) as cursor:
                    row = await cursor.fetchone()

                if not row:
                    await conn.rollback()
                    return None

                raw_preview = dict(row)
                if not self._preview_state_matches_snapshot(raw_preview, expected_state):
                    await conn.rollback()
                    return {"_state_conflict": True}

                preview = self._normalize_preview_row(raw_preview)
                recurring_rows = await self.get_enabled_recurring_templates(user_id=user_id)
                recurring_candidates = self.build_recurring_candidates_for_bill_data(
                    {
                        "date": preview.get("preview_date"),
                        "type": preview.get("preview_type"),
                        "amount": preview.get("preview_amount"),
                        "source_account_id": preview.get("preview_source_account_id"),
                        "destination_account_id": preview.get("preview_destination_account_id"),
                    },
                    recurring_rows,
                    linked_recurring_id=preview.get("preview_recurring_id"),
                    tolerance_days=3,
                )
                candidate_count = len(recurring_candidates)

                target_candidate: dict[str, Any] | None = None
                current_recurring_id = preview.get("preview_recurring_id")
                normalized_current_recurring_id = None if current_recurring_id in (None, "") else int(current_recurring_id)
                normalized_recurring_id = None if recurring_id in (None, "") else int(recurring_id)
                if normalized_recurring_id is not None:
                    target_candidate = next(
                        (
                            candidate
                            for candidate in recurring_candidates
                            if int(candidate.get("id") or 0) == normalized_recurring_id
                        ),
                        None,
                    )
                    if not target_candidate:
                        await conn.rollback()
                        return {"_invalid_recurring_id": True}

                next_feedback_json = raw_preview.get("preview_matching_feedback_json")
                if normalized_current_recurring_id != normalized_recurring_id:
                    next_feedback_json = self._clear_transfer_matching_feedback(next_feedback_json)

                update_parts = [
                    "preview_recurring_id = ?",
                    "preview_recurring_name = ?",
                    "preview_recurring_candidate_count = ?",
                    "preview_recurring_match_score = ?",
                    "preview_recurring_match_reasons = ?",
                    "preview_recurring_matched_date = ?",
                    "preview_matching_feedback_json = ?",
                ]
                params: list[Any] = [
                    normalized_recurring_id,
                    str(target_candidate.get("name") or "") if target_candidate else "",
                    candidate_count,
                    float(target_candidate.get("matchScore") or 0) if target_candidate else 0,
                    "|".join(target_candidate.get("matchReasons", [])) if target_candidate else "",
                    str(target_candidate.get("matchedOccurrenceDate") or "") if target_candidate else "",
                    next_feedback_json,
                ]
                params.extend([preview_id, user_id])

                cursor = await conn.execute(
                    f"UPDATE bills_preview SET {', '.join(update_parts)} WHERE id = ? AND user_id = ?",
                    tuple(params),
                )
                if int(cursor.rowcount or 0) < 1:
                    await conn.rollback()
                    return None

                await conn.commit()
            except Exception:
                await conn.rollback()
                raise

            return await self.get_preview_bill_by_id(preview_id, user_id=user_id)
