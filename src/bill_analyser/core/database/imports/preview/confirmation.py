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


class ImportPreviewConfirmationMixin(object):
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
            await self.update_import_session_status(
                session_id,
                "completed",
                total_confirmed=result["confirmed_count"],
                user_id=user_id,
            )
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
