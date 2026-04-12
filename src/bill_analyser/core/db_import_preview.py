"""Import-preview editing, confirmation, and temporary staging helpers."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-positional-arguments,too-many-locals,broad-exception-caught,assignment-from-no-return,too-many-nested-blocks

from __future__ import annotations

import json
import sqlite3
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from datetime import date

from ..parsers.parser_tags import resolve_parser_tags, serialize_parser_tags
from ..utils.logger import log_method
from .bill_date_utils import normalize_bill_date_text
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now, utc_now_iso


class DatabaseImportPreviewMixin(DatabaseFacadeBase):
    """Preview editing, recurring matching, confirmation, and temp-data cleanup helpers."""

    _TRANSFER_PREVIEW_SNAPSHOT_FIELDS: tuple[tuple[str, Any], ...] = (
        ("preview_type", ""),
        ("preview_main_category", ""),
        ("preview_sub_category", ""),
        ("preview_recurring_id", None),
        ("preview_recurring_name", ""),
        ("preview_recurring_candidate_count", 0),
        ("preview_recurring_match_score", 0),
        ("preview_recurring_match_reasons", ""),
        ("preview_recurring_matched_date", ""),
    )

    @staticmethod
    def _deserialize_preview_matching_feedback(raw_payload: Any) -> dict[str, Any]:
        if isinstance(raw_payload, dict):
            return dict(raw_payload)

        if raw_payload in (None, ""):
            return {}

        try:
            payload = json.loads(str(raw_payload))
        except (TypeError, ValueError, json.JSONDecodeError):
            return {}

        return dict(payload) if isinstance(payload, dict) else {}

    @staticmethod
    def _serialize_preview_matching_feedback(payload: dict[str, Any]) -> str:
        if not payload:
            return ""
        return json.dumps(payload, ensure_ascii=False, sort_keys=True)

    @classmethod
    def _clear_matching_feedback_key(cls, raw_payload: Any, key: str) -> str:
        feedback_payload = cls._deserialize_preview_matching_feedback(raw_payload)
        feedback_payload.pop(str(key), None)
        return cls._serialize_preview_matching_feedback(feedback_payload)

    @classmethod
    def _clear_transfer_matching_feedback(cls, raw_payload: Any) -> str:
        return cls._clear_matching_feedback_key(raw_payload, "transfer")

    @staticmethod
    def _preview_state_matches_snapshot(preview: dict[str, Any], expected_state: dict[str, Any] | None) -> bool:
        if not expected_state:
            return True

        current_recurring_id = preview.get("preview_recurring_id")
        normalized_current_recurring_id = None if current_recurring_id in (None, "") else int(current_recurring_id)
        expected_recurring_id = expected_state.get("preview_recurring_id")
        normalized_expected_recurring_id = None if expected_recurring_id in (None, "") else int(expected_recurring_id)

        return (
            str(preview.get("session_id") or "") == str(expected_state.get("session_id") or "")
            and str(preview.get("preview_type") or "") == str(expected_state.get("preview_type") or "")
            and str(preview.get("preview_main_category") or "") == str(expected_state.get("preview_main_category") or "")
            and str(preview.get("preview_sub_category") or "") == str(expected_state.get("preview_sub_category") or "")
            and normalized_current_recurring_id == normalized_expected_recurring_id
            and str(preview.get("preview_matching_feedback_json") or "")
            == str(expected_state.get("preview_matching_feedback_json") or "")
        )

    def _normalize_preview_row(self, preview: dict[str, Any]) -> dict[str, Any]:
        normalized_preview = dict(preview)
        normalized_preview["preview_parser_tags"] = resolve_parser_tags(
            normalized_preview.get("preview_parser_tags_json"),
            parser_id=normalized_preview.get("preview_parser_id", ""),
            payment_method=normalized_preview.get("preview_payment_method", ""),
        )
        normalized_preview["preview_matching_feedback"] = self._deserialize_preview_matching_feedback(
            normalized_preview.get("preview_matching_feedback_json")
        )
        return normalized_preview

    @classmethod
    def _build_transfer_previous_preview_snapshot(cls, preview: dict[str, Any]) -> dict[str, Any]:
        snapshot: dict[str, Any] = {}
        for field, default_value in cls._TRANSFER_PREVIEW_SNAPSHOT_FIELDS:
            value = preview.get(field, default_value)
            if field == "preview_recurring_candidate_count":
                snapshot[field] = int(value or 0)
            elif field == "preview_recurring_match_score":
                snapshot[field] = float(value or 0)
            elif field == "preview_recurring_id":
                snapshot[field] = None if value in (None, "") else value
            else:
                snapshot[field] = default_value if value is None else value
        return snapshot

    @classmethod
    def _normalize_transfer_previous_preview_snapshot(cls, raw_payload: Any) -> dict[str, Any]:
        if not isinstance(raw_payload, dict):
            return {}

        snapshot: dict[str, Any] = {}
        for field, default_value in cls._TRANSFER_PREVIEW_SNAPSHOT_FIELDS:
            value = raw_payload.get(field, default_value)
            if field == "preview_recurring_candidate_count":
                snapshot[field] = int(value or 0)
            elif field == "preview_recurring_match_score":
                snapshot[field] = float(value or 0)
            elif field == "preview_recurring_id":
                snapshot[field] = None if value in (None, "") else value
            else:
                snapshot[field] = default_value if value is None else value
        return snapshot

    @classmethod
    def _append_transfer_snapshot_restore_updates(
        cls,
        snapshot: dict[str, Any],
        update_parts: list[str],
        params: list[Any],
    ) -> None:
        normalized_snapshot = cls._normalize_transfer_previous_preview_snapshot(snapshot)
        if not normalized_snapshot:
            return

        for field, default_value in cls._TRANSFER_PREVIEW_SNAPSHOT_FIELDS:
            update_parts.append(f"{field} = ?")
            params.append(normalized_snapshot.get(field, default_value))

    @log_method
    async def insert_preview_bill(
        self,
        session_id: str,
        preview_data: dict[str, Any],
        user_id: int = 1,
        dedup_type: str | None = None,
        dedup_source_ids: list[int] | None = None,
    ) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        source_ids_str = ",".join(map(str, dedup_source_ids)) if dedup_source_ids else ""
        normalized_preview_date = normalize_bill_date_text(
            preview_data.get("preview_date", "")
        )
        cursor = await conn.execute(
            """
            INSERT INTO bills_preview (
                session_id, user_id, preview_date, preview_type,
                preview_amount, preview_destination_amount,
                preview_main_category, preview_sub_category,
                preview_source_account_id, preview_destination_account_id,
                preview_counterparty, preview_payment_method, preview_description,
                preview_parser_id, preview_parser_tags_json,
                preview_recurring_id, preview_recurring_name,
                preview_recurring_candidate_count, preview_recurring_match_score,
                preview_recurring_match_reasons, preview_recurring_matched_date,
                preview_selected, dedup_type, dedup_source_ids, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                session_id,
                user_id,
                normalized_preview_date,
                preview_data.get("preview_type", ""),
                preview_data.get("preview_amount", 0),
                preview_data.get("preview_destination_amount", 0),
                preview_data.get("preview_main_category", ""),
                preview_data.get("preview_sub_category", ""),
                preview_data.get("preview_source_account_id"),
                preview_data.get("preview_destination_account_id"),
                preview_data.get("preview_counterparty", ""),
                preview_data.get("preview_payment_method", ""),
                preview_data.get("preview_description", ""),
                preview_data.get("preview_parser_id", ""),
                serialize_parser_tags(
                    preview_data.get("preview_parser_tags") or preview_data.get("preview_parser_tags_json"),
                    parser_id=preview_data.get("preview_parser_id", ""),
                    payment_method=preview_data.get("preview_payment_method", ""),
                ),
                preview_data.get("preview_recurring_id"),
                preview_data.get("preview_recurring_name", ""),
                preview_data.get("preview_recurring_candidate_count", 0),
                preview_data.get("preview_recurring_match_score", 0),
                preview_data.get("preview_recurring_match_reasons", ""),
                preview_data.get("preview_recurring_matched_date", ""),
                1,
                dedup_type,
                source_ids_str,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def insert_preview_bills_batch(
        self,
        session_id: str,
        preview_list: list[dict[str, Any]],
        user_id: int = 1,
    ) -> int:
        if not preview_list:
            return 0
        conn = await self._get_connection()
        now = utc_now_iso()
        inserted_count = 0

        for index in range(0, len(preview_list), self.batch_size):
            batch = preview_list[index : index + self.batch_size]
            for item in batch:
                try:
                    preview_data = item.get("preview_data", item)
                    normalized_preview_date = normalize_bill_date_text(
                        preview_data.get("preview_date", "")
                    )
                    dedup_type = item.get("dedup_type", "remaining")
                    dedup_source_ids = item.get("dedup_source_ids", [])
                    source_ids_str = ",".join(map(str, dedup_source_ids)) if dedup_source_ids else ""
                    await conn.execute(
                        """
                        INSERT INTO bills_preview (
                            session_id, user_id, preview_date, preview_type,
                            preview_amount, preview_destination_amount,
                            preview_main_category, preview_sub_category,
                            preview_source_account_id, preview_destination_account_id,
                            preview_counterparty, preview_payment_method, preview_description,
                            preview_parser_id, preview_parser_tags_json,
                            preview_recurring_id, preview_recurring_name,
                            preview_recurring_candidate_count, preview_recurring_match_score,
                            preview_recurring_match_reasons, preview_recurring_matched_date,
                            preview_selected, dedup_type, dedup_source_ids, created_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            session_id,
                            user_id,
                            normalized_preview_date,
                            preview_data.get("preview_type", ""),
                            preview_data.get("preview_amount", 0),
                            preview_data.get("preview_destination_amount", 0),
                            preview_data.get("preview_main_category", ""),
                            preview_data.get("preview_sub_category", ""),
                            preview_data.get("preview_source_account_id"),
                            preview_data.get("preview_destination_account_id"),
                            preview_data.get("preview_counterparty", ""),
                            preview_data.get("preview_payment_method", ""),
                            preview_data.get("preview_description", ""),
                            preview_data.get("preview_parser_id", ""),
                            serialize_parser_tags(
                                preview_data.get("preview_parser_tags") or preview_data.get("preview_parser_tags_json"),
                                parser_id=preview_data.get("preview_parser_id", ""),
                                payment_method=preview_data.get("preview_payment_method", ""),
                            ),
                            preview_data.get("preview_recurring_id"),
                            preview_data.get("preview_recurring_name", ""),
                            preview_data.get("preview_recurring_candidate_count", 0),
                            preview_data.get("preview_recurring_match_score", 0),
                            preview_data.get("preview_recurring_match_reasons", ""),
                            preview_data.get("preview_recurring_matched_date", ""),
                            1,
                            dedup_type,
                            source_ids_str,
                            now,
                        ),
                    )
                    inserted_count += 1
                except Exception as exc:  # pragma: no cover - defensive logging branch
                    self.logger.error("[批量插入预览失败] %s", exc)
            await conn.commit()
        return inserted_count

    @log_method
    async def get_preview_by_session(
        self,
        session_id: str,
        user_id: int = 1,
        selected_only: bool = False,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM bills_preview WHERE session_id = ? AND user_id = ?"
        params: list[Any] = [session_id, user_id]
        if selected_only:
            query += " AND preview_selected = 1"
        query += " ORDER BY preview_date ASC"

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        categories = await self.get_all_categories(user_id=user_id)
        category_id_map = {
            (category.get("main_category", ""), category.get("sub_category", "")): category.get("id")
            for category in categories
        }
        previews: list[dict[str, Any]] = []
        for row in rows:
            preview = self._normalize_preview_row(dict(row))
            preview["is_selected"] = preview.get("preview_selected", 1)
            preview["category_id"] = category_id_map.get(
                (preview.get("preview_main_category", ""), preview.get("preview_sub_category", ""))
            )
            previews.append(preview)
        return previews

    @log_method
    async def get_preview_bill_by_id(self, preview_id: int, user_id: int = 1) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM bills_preview WHERE id = ? AND user_id = ?",
            (preview_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return self._normalize_preview_row(dict(row)) if row else None

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
        if normalized_decision not in {"reject", "clear"}:
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
            if normalized_decision == "reject":
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

    @log_method
    async def get_recurring_candidates_for_preview(
        self,
        preview_id: int,
        user_id: int = 1,
        tolerance_days: int = 3,
    ) -> dict[str, Any]:
        preview = await self.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return {"preview": None, "linked_recurring_id": None, "candidates": []}

        recurring_rows = await self.get_enabled_recurring_templates(user_id=user_id)
        bill_data = {
            "date": preview.get("preview_date"),
            "type": preview.get("preview_type"),
            "amount": preview.get("preview_amount"),
            "source_account_id": preview.get("preview_source_account_id"),
            "destination_account_id": preview.get("preview_destination_account_id"),
        }
        candidates = self.build_recurring_candidates_for_bill_data(
            bill_data,
            recurring_rows,
            linked_recurring_id=preview.get("preview_recurring_id"),
            tolerance_days=tolerance_days,
        )
        return {
            "preview": preview,
            "linked_recurring_id": preview.get("preview_recurring_id"),
            "candidates": candidates,
        }

    @log_method
    async def update_preview_selection(
        self,
        preview_ids: list[int],
        selected: bool,
        user_id: int = 1,
    ) -> int:
        if not preview_ids:
            return 0
        conn = await self._get_connection()
        selected_value = 1 if selected else 0
        placeholders = ",".join(["?" for _ in preview_ids])
        await conn.execute(
            f"UPDATE bills_preview SET preview_selected = ? WHERE user_id = ? AND id IN ({placeholders})",
            (selected_value, user_id, *preview_ids),
        )
        await conn.commit()
        return len(preview_ids)

    @log_method
    async def reset_session_preview_selection(self, session_id: str, user_id: int = 1) -> int:
        conn = await self._get_connection()
        cursor = await conn.execute(
            "UPDATE bills_preview SET preview_selected = 0 WHERE session_id = ? AND user_id = ?",
            (session_id, user_id),
        )
        await conn.commit()
        return int(cursor.rowcount or 0)

    @log_method
    async def update_preview_bill(
        self,
        preview_id: int,
        update_data: dict[str, Any],
        user_id: int = 1,
    ) -> bool:
        if not update_data:
            return False
        conn = await self._get_connection()
        update_parts: list[str] = []
        params: list[Any] = []
        field_mapping = {
            "preview_date": "preview_date",
            "preview_type": "preview_type",
            "preview_amount": "preview_amount",
            "preview_destination_amount": "preview_destination_amount",
            "preview_main_category": "preview_main_category",
            "preview_sub_category": "preview_sub_category",
            "preview_source_account_id": "preview_source_account_id",
            "preview_destination_account_id": "preview_destination_account_id",
            "preview_counterparty": "preview_counterparty",
            "preview_payment_method": "preview_payment_method",
            "preview_description": "preview_description",
            "preview_recurring_id": "preview_recurring_id",
            "preview_recurring_name": "preview_recurring_name",
            "preview_recurring_candidate_count": "preview_recurring_candidate_count",
            "preview_recurring_match_score": "preview_recurring_match_score",
            "preview_recurring_match_reasons": "preview_recurring_match_reasons",
            "preview_recurring_matched_date": "preview_recurring_matched_date",
            "preview_selected": "preview_selected",
            "is_selected": "preview_selected",
        }
        if "category_id" in update_data:
            category = await self.get_category_by_id(update_data["category_id"], user_id=user_id)
            if category:
                update_parts.extend(["preview_main_category = ?", "preview_sub_category = ?"])
                params.extend([category.get("main_category", ""), category.get("sub_category", "")])
        for key, column in field_mapping.items():
            if key in update_data:
                update_parts.append(f"{column} = ?")
                params.append(update_data[key])
        if update_data.get("clear_transfer_decision"):
            async with conn.execute(
                "SELECT preview_matching_feedback_json FROM bills_preview WHERE id = ? AND user_id = ?",
                (preview_id, user_id),
            ) as cursor:
                feedback_row = await cursor.fetchone()
            if feedback_row:
                update_parts.append("preview_matching_feedback_json = ?")
                params.append(self._clear_transfer_matching_feedback(feedback_row[0]))
        if not update_parts:
            return False
        params.extend([preview_id, user_id])
        cursor = await conn.execute(
            f"UPDATE bills_preview SET {', '.join(update_parts)} WHERE id = ? AND user_id = ?",
            tuple(params),
        )
        await conn.commit()
        return int(cursor.rowcount or 0) > 0

    @log_method
    async def update_preview_bills_batch(
        self,
        session_id: str,
        updates: list[dict[str, Any]],
        user_id: int = 1,
    ) -> int:
        if not updates:
            return 0
        conn = await self._get_connection()
        updated_count = 0

        for update_item in updates:
            try:
                preview_id = update_item.get("id")
                if not preview_id:
                    continue
                update_parts: list[str] = []
                params: list[Any] = []
                field_mapping = {
                    "date": "preview_date",
                    "type": "preview_type",
                    "amount": "preview_amount",
                    "destinationAmount": "preview_destination_amount",
                    "mainCategory": "preview_main_category",
                    "subCategory": "preview_sub_category",
                    "sourceAccountId": "preview_source_account_id",
                    "destinationAccountId": "preview_destination_account_id",
                    "counterparty": "preview_counterparty",
                    "paymentMethod": "preview_payment_method",
                    "description": "preview_description",
                    "isSelected": "preview_selected",
                    "preview_date": "preview_date",
                    "preview_type": "preview_type",
                    "preview_amount": "preview_amount",
                    "preview_destination_amount": "preview_destination_amount",
                    "preview_main_category": "preview_main_category",
                    "preview_sub_category": "preview_sub_category",
                    "preview_source_account_id": "preview_source_account_id",
                    "preview_destination_account_id": "preview_destination_account_id",
                    "preview_counterparty": "preview_counterparty",
                    "preview_payment_method": "preview_payment_method",
                    "preview_description": "preview_description",
                    "preview_recurring_id": "preview_recurring_id",
                    "preview_recurring_name": "preview_recurring_name",
                    "preview_recurring_candidate_count": "preview_recurring_candidate_count",
                    "preview_recurring_match_score": "preview_recurring_match_score",
                    "preview_recurring_match_reasons": "preview_recurring_match_reasons",
                    "preview_recurring_matched_date": "preview_recurring_matched_date",
                    "preview_selected": "preview_selected",
                    "is_selected": "preview_selected",
                    "selected": "preview_selected",
                }
                for key, column in field_mapping.items():
                    if key not in update_item:
                        continue
                    value = update_item[key]
                    if column == "preview_selected":
                        update_parts.append("preview_selected = ?")
                        params.append(1 if value else 0)
                    else:
                        update_parts.append(f"{column} = ?")
                        params.append(value)
                if "category_id" in update_item and update_item.get("category_id"):
                    category = await self.get_category_by_id(update_item["category_id"], user_id=user_id)
                    if category:
                        update_parts.extend(["preview_main_category = ?", "preview_sub_category = ?"])
                        params.extend([category.get("main_category", ""), category.get("sub_category", "")])
                if update_item.get("clear_transfer_decision"):
                    async with conn.execute(
                        (
                            "SELECT preview_matching_feedback_json FROM bills_preview "
                            "WHERE id = ? AND session_id = ? AND user_id = ?"
                        ),
                        (preview_id, session_id, user_id),
                    ) as cursor:
                        feedback_row = await cursor.fetchone()
                    if feedback_row:
                        update_parts.append("preview_matching_feedback_json = ?")
                        params.append(self._clear_transfer_matching_feedback(feedback_row[0]))
                if update_parts:
                    params.extend([preview_id, session_id, user_id])
                    cursor = await conn.execute(
                        (
                            f"UPDATE bills_preview SET {', '.join(update_parts)} "
                            "WHERE id = ? AND session_id = ? AND user_id = ?"
                        ),
                        tuple(params),
                    )
                    if cursor.rowcount > 0:
                        updated_count += 1
            except Exception as exc:  # pragma: no cover - defensive logging branch
                self.logger.error("[批量更新预览失败] id=%s, error=%s", update_item.get("id"), exc)

        await conn.commit()
        return updated_count

    @log_method
    async def confirm_preview_to_bills(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        conn = await self._get_connection()
        now = utc_now_iso()
        batch_id = utc_now().strftime("%Y%m%d%H%M%S")
        result = {"confirmed_count": 0, "skipped_count": 0, "duplicate_count": 0, "errors": []}
        recurring_advances: dict[int, date] = {}
        previews = await self.get_preview_by_session(session_id, user_id=user_id, selected_only=True)

        for preview in previews:
            try:
                bill_type = preview.get("preview_type", "")
                preview_date_text = normalize_bill_date_text(
                    preview.get("preview_date", "")
                )
                amount = abs(float(preview.get("preview_amount", 0)))
                if bill_type in ["支出", "expense"]:
                    amount = -abs(amount)
                elif bill_type in ["收入", "income", "转账", "transfer", "投资", "investment"]:
                    amount = abs(amount)

                bill_data = {
                    "date": preview_date_text,
                    "type": bill_type,
                    "amount": amount,
                    "counterparty": preview.get("preview_counterparty", ""),
                    "description": preview.get("preview_description", ""),
                }
                bill_hash = self._calculate_hash(bill_data)
                await conn.execute(
                    """
                    INSERT INTO bills (
                        user_id, date, type, amount, counterparty, description,
                        payment_method, main_category, sub_category,
                        source_account_id, destination_account_id, destination_amount,
                        batch_id, hash, created_from_recurring, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        user_id,
                        preview_date_text,
                        bill_type,
                        amount,
                        preview.get("preview_counterparty", ""),
                        preview.get("preview_description", ""),
                        preview.get("preview_payment_method", ""),
                        preview.get("preview_main_category", ""),
                        preview.get("preview_sub_category", ""),
                        preview.get("preview_source_account_id"),
                        preview.get("preview_destination_account_id"),
                        preview.get("preview_destination_amount", 0),
                        batch_id,
                        bill_hash,
                        preview.get("preview_recurring_id"),
                        now,
                        now,
                    ),
                )
                result["confirmed_count"] += 1

                recurring_id = preview.get("preview_recurring_id")
                preview_date = self._parse_date_value(preview.get("preview_date"))
                if recurring_id and preview_date:
                    recurring_id_int = int(recurring_id)
                    recorded_date = recurring_advances.get(recurring_id_int)
                    if recorded_date is None or preview_date > recorded_date:
                        recurring_advances[recurring_id_int] = preview_date
            except sqlite3.IntegrityError:
                result["duplicate_count"] += 1
            except Exception as exc:  # pragma: no cover - defensive logging branch
                result["errors"].append(str(exc))
                self.logger.error("[确认失败] %s", exc)

        for recurring_id, matched_date in recurring_advances.items():
            async with conn.execute(
                "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?",
                (recurring_id, user_id),
            ) as cursor:
                recurring_row = await cursor.fetchone()
            recurring = dict(recurring_row) if recurring_row else None
            if not recurring:
                continue
            next_date = self._get_next_recurring_occurrence_after(recurring, matched_date)
            next_occurrence = next_date.isoformat() if next_date else recurring.get("next_date")
            await conn.execute(
                "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                (next_occurrence, now, recurring_id, user_id),
            )

        await conn.commit()
        await self.update_import_session_status(session_id, "completed", total_confirmed=result["confirmed_count"])
        return result

    @log_method
    async def clear_session_data(self, session_id: str, user_id: int = 1) -> dict[str, int]:
        conn = await self._get_connection()
        result = {"parser_count": 0, "preview_count": 0, "annotation_count": 0}
        cursor = await conn.execute(
            "DELETE FROM bills_parser_template WHERE session_id = ? AND user_id = ?",
            (session_id, user_id),
        )
        result["parser_count"] = int(cursor.rowcount or 0)
        cursor = await conn.execute(
            "DELETE FROM bills_preview WHERE session_id = ? AND user_id = ?",
            (session_id, user_id),
        )
        result["preview_count"] = int(cursor.rowcount or 0)
        cursor = await conn.execute(
            "DELETE FROM import_annotation_samples WHERE session_id = ? AND user_id = ?",
            (session_id, user_id),
        )
        result["annotation_count"] = int(cursor.rowcount or 0)
        await conn.commit()
        return result

    @log_method
    async def get_unprocessed_templates_for_dedup(self, session_id: str) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM bills_parser_template
            WHERE session_id = ? AND parser_is_processed = '0'
            ORDER BY parser_date ASC, id ASC
            """,
            (session_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        templates: list[dict[str, Any]] = []
        for row in rows:
            template = dict(row)
            template["parser_tags"] = resolve_parser_tags(
                template.get("parser_tags_json"),
                parser_id=template.get("parser_id", ""),
                payment_method=template.get("parser_payment_method", ""),
            )
            templates.append(template)
        return templates

    @log_method
    async def get_existing_bills_for_dedup(
        self,
        user_id: int,
        start_date: str,
        end_date: str,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT id, date, type, amount, counterparty, description,
                   payment_method, main_category, sub_category,
                   source_account_id, destination_account_id
            FROM bills
            WHERE user_id = ?
              AND date >= ?
              AND date <= ?
            ORDER BY date ASC
            """,
            (user_id, start_date, end_date + " 23:59:59"),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]
