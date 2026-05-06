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


class ImportPreviewReadsMixin(object):
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
        async def count_preview_by_session(
            self,
            session_id: str,
            user_id: int = 1,
            selected_only: bool = False,
        ) -> int:
            conn = await self._get_connection()
            query = "SELECT COUNT(*) AS total FROM bills_preview WHERE session_id = ? AND user_id = ?"
            params: list[Any] = [session_id, user_id]
            if selected_only:
                query += " AND preview_selected = 1"

            async with conn.execute(query, tuple(params)) as cursor:
                row = await cursor.fetchone()

            return int((dict(row).get("total") if row else 0) or 0)

        @log_method
        async def get_preview_page_by_session(
            self,
            session_id: str,
            *,
            user_id: int = 1,
            page: int = 1,
            page_size: int = 50,
            sort_by: str | None = None,
            sort_direction: str | None = None,
            selected_only: bool = False,
        ) -> tuple[list[dict[str, Any]], int]:
            normalized_page = max(int(page or 1), 1)
            normalized_page_size = max(int(page_size or 50), 1)
            offset = (normalized_page - 1) * normalized_page_size

            total = await self.count_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=selected_only,
            )

            conn = await self._get_connection()
            query = "SELECT * FROM bills_preview WHERE session_id = ? AND user_id = ?"
            params: list[Any] = [session_id, user_id]
            if selected_only:
                query += " AND preview_selected = 1"

            normalized_sort_by = str(sort_by or "").strip()
            normalized_sort_direction = "DESC" if str(sort_direction or "").strip().lower() == "desc" else "ASC"
            sort_field = self._SERVER_PAGED_PREVIEW_SORT_FIELDS.get(normalized_sort_by)

            if sort_field == "preview_amount":
                query += f" ORDER BY COALESCE({sort_field}, 0) {normalized_sort_direction}, id ASC"
            elif sort_field:
                sort_expression = f"COALESCE({sort_field}, '')"
                if normalized_sort_by in {"type", "counterparty", "paymentMethod", "comment"}:
                    sort_expression += " COLLATE NOCASE"
                query += f" ORDER BY {sort_expression} {normalized_sort_direction}, id ASC"
            else:
                query += " ORDER BY preview_date ASC, id ASC"

            query += " LIMIT ? OFFSET ?"
            params.extend([normalized_page_size, offset])

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

            return previews, total

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
        async def get_preview_by_ids(
            self,
            session_id: str,
            preview_ids: list[int],
            *,
            user_id: int = 1,
        ) -> list[dict[str, Any]]:
            normalized_preview_ids = [int(preview_id) for preview_id in preview_ids if int(preview_id) > 0]
            if not normalized_preview_ids:
                return []

            conn = await self._get_connection()
            placeholders = ",".join(["?"] * len(normalized_preview_ids))
            query = (
                "SELECT * FROM bills_preview "
                "WHERE session_id = ? AND user_id = ? AND id IN (" + placeholders + ")"
            )
            params: list[Any] = [session_id, user_id, *normalized_preview_ids]

            async with conn.execute(query, tuple(params)) as cursor:
                rows = await cursor.fetchall()

            preview_lookup = {
                int(row["id"]): self._normalize_preview_row(dict(row))
                for row in rows
            }

            return [
                preview_lookup[preview_id]
                for preview_id in normalized_preview_ids
                if preview_id in preview_lookup
            ]

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
        async def get_unprocessed_templates_for_dedup(self, session_id: str, user_id: int = 1) -> list[dict[str, Any]]:
            conn = await self._get_connection()
            async with conn.execute(
                """
                SELECT * FROM bills_parser_template
                WHERE session_id = ? AND user_id = ? AND parser_is_processed = '0'
                ORDER BY parser_date ASC, id ASC
                """,
                (session_id, user_id),
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
