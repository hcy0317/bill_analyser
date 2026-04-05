"""Import-session lifecycle and parser-template staging helpers."""

# pylint: disable=missing-function-docstring,line-too-long,too-many-arguments,too-many-positional-arguments,broad-exception-caught

from __future__ import annotations

from typing import Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseImportSessionsMixin(DatabaseFacadeBase):
    """Import session lifecycle and parser-template staging helpers."""

    @log_method
    async def create_import_session(self, session_id: str, user_id: int = 1, file_count: int = 0) -> int:
        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            INSERT INTO import_sessions (
                session_id, user_id, status, file_count,
                total_parsed, total_preview, total_confirmed,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (session_id, user_id, "parsing", file_count, 0, 0, 0, now, now),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def update_import_session_status(
        self,
        session_id: str,
        status: str,
        total_parsed: int | None = None,
        total_preview: int | None = None,
        total_confirmed: int | None = None,
    ) -> bool:
        conn = await self._get_connection()
        now = utc_now_iso()
        update_parts = ["status = ?", "updated_at = ?"]
        params: list[Any] = [status, now]
        if total_parsed is not None:
            update_parts.append("total_parsed = ?")
            params.append(total_parsed)
        if total_preview is not None:
            update_parts.append("total_preview = ?")
            params.append(total_preview)
        if total_confirmed is not None:
            update_parts.append("total_confirmed = ?")
            params.append(total_confirmed)
        params.append(session_id)
        await conn.execute(
            f"UPDATE import_sessions SET {', '.join(update_parts)} WHERE session_id = ?",
            tuple(params),
        )
        await conn.commit()
        return True

    @log_method
    async def get_import_session(
        self,
        session_id: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM import_sessions WHERE session_id = ? AND user_id = ?",
            (session_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    @log_method
    async def insert_parser_templates(
        self,
        session_id: str,
        bills: list[dict[str, Any]],
        parser_id: str,
        user_id: int = 1,
    ) -> int:
        if not bills:
            return 0
        conn = await self._get_connection()
        now = utc_now_iso()
        inserted_count = 0

        for index in range(0, len(bills), self.batch_size):
            batch = bills[index : index + self.batch_size]
            for bill in batch:
                try:
                    amount = float(bill.get("amount", 0))
                    if bill.get("type"):
                        bill_type = bill.get("type")
                    elif amount > 0:
                        bill_type = "收入"
                    elif amount < 0:
                        bill_type = "支出"
                    else:
                        bill_type = "其他"

                    await conn.execute(
                        """
                        INSERT INTO bills_parser_template (
                            session_id, user_id, parser_date, parser_amount,
                            parser_type, parser_description, parser_id,
                            parser_counterparty, parser_payment_method,
                            parser_original_type, parser_original_category,
                            parser_account_id, parser_is_processed, created_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            session_id,
                            user_id,
                            bill.get("date", ""),
                            amount,
                            bill_type,
                            bill.get("description", ""),
                            parser_id,
                            bill.get("counterparty", ""),
                            bill.get("payment_method", ""),
                            bill.get("original_type", ""),
                            bill.get("original_category", ""),
                            bill.get("account_id", ""),
                            "0",
                            now,
                        ),
                    )
                    inserted_count += 1
                except Exception as exc:  # pragma: no cover - defensive logging branch
                    self.logger.error("[插入解析模板失败] %s - %s", bill.get("date"), exc)
            await conn.commit()
        return inserted_count

    @log_method
    async def get_parser_templates_by_session(
        self,
        session_id: str,
        processed_only: bool | None = None,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM bills_parser_template WHERE session_id = ?"
        params: list[Any] = [session_id]
        if processed_only is True:
            query += " AND parser_is_processed = '1'"
        elif processed_only is False:
            query += " AND parser_is_processed = '0'"
        query += " ORDER BY parser_date ASC"

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def update_parser_template_status(
        self,
        template_ids: list[int],
        processed: bool = True,
        account_id: str | None = None,
    ) -> int:
        if not template_ids:
            return 0
        conn = await self._get_connection()
        processed_value = "1" if processed else "0"
        update_parts = ["parser_is_processed = ?"]
        params: list[Any] = [processed_value]
        if account_id is not None:
            update_parts.append("parser_account_id = ?")
            params.append(account_id)
        placeholders = ",".join(["?" for _ in template_ids])
        params.extend(template_ids)
        await conn.execute(
            f"UPDATE bills_parser_template SET {', '.join(update_parts)} WHERE id IN ({placeholders})",
            tuple(params),
        )
        await conn.commit()
        return len(template_ids)
