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


class ImportPreviewInsertsMixin(object):
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
