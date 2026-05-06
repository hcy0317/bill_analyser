"""Import-preview editing, confirmation, and temporary staging helpers."""

# pylint: disable=missing-function-docstring,line-too-long,wrong-import-position,too-many-arguments,too-many-positional-arguments,too-many-locals,broad-exception-caught,assignment-from-no-return,too-many-nested-blocks,too-many-lines

from __future__ import annotations

import json
import sqlite3
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from datetime import date

from bill_analyser.import_contracts.parser_tags import resolve_parser_tags, serialize_parser_tags
from bill_analyser.import_contracts.preview_selection import coerce_preview_selected
from bill_analyser.utils.logger import log_method
from bill_analyser.core.bill_date_utils import normalize_bill_date_text
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now, utc_now_iso


class ImportPreviewUpdatesMixin(object):
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
                            params.append(1 if coerce_preview_selected(value) else 0)
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
