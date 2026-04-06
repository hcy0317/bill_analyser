"""Tag-domain persistence helpers for the split database facade."""

from __future__ import annotations

import sqlite3
from typing import TYPE_CHECKING, Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso

if TYPE_CHECKING:
    from collections.abc import Iterable


class DatabaseTagsMixin(DatabaseFacadeBase):
    """Tag CRUD and bill-tag relationship helpers."""

    @staticmethod
    def _rows_to_dicts(rows: Iterable[Any]) -> list[dict[str, Any]]:
        """Convert a row iterable into a list of dictionaries."""
        return [dict(row) for row in rows]

    @staticmethod
    def _row_to_dict(row: Any) -> dict[str, Any] | None:
        """Convert a single row into a dictionary when present."""
        return dict(row) if row else None

    @log_method
    async def get_all_tags(self, user_id: int = 1) -> list[dict[str, Any]]:
        """List all tags for the specified user."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM tags WHERE user_id = ? ORDER BY display_order, created_at DESC",
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return self._rows_to_dicts(rows)

    @log_method
    async def get_tag_by_id(self, tag_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """Fetch one tag row by identifier and user."""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM tags WHERE id = ? AND user_id = ?",
            (tag_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return self._row_to_dict(row)

    @log_method
    async def create_tag(self, data: dict[str, Any], user_id: int = 1) -> int:
        """Create a tag for the specified user."""
        conn = await self._get_connection()
        now = utc_now_iso()
        cursor = await conn.execute(
            """
            INSERT INTO tags (name, color, icon, hidden, created_at, updated_at, user_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("name"),
                data.get("color", "#000000"),
                data.get("icon", ""),
                data.get("hidden", False),
                now,
                now,
                user_id,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def update_tag(self, tag_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        """Update one tag row with partial field changes."""
        if not data:
            return False

        conn = await self._get_connection()
        update_data = {**data, "updated_at": utc_now_iso()}
        set_clause = ", ".join(f"{key} = ?" for key in update_data)
        values = [*update_data.values(), tag_id, user_id]
        cursor = await conn.execute(
            f"UPDATE tags SET {set_clause} WHERE id = ? AND user_id = ?",
            values,
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def delete_tag(self, tag_id: int, user_id: int = 1) -> bool:
        """Delete one tag row for the specified user."""
        conn = await self._get_connection()
        cursor = await conn.execute(
            "DELETE FROM tags WHERE id = ? AND user_id = ?",
            (tag_id, user_id),
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def update_tag_display_orders(
        self,
        orders: list[tuple],
        user_id: int = 1,
    ) -> bool:
        """Persist a batch of tag display-order updates."""
        if not orders:
            self.logger.warning("[update_tag_display_orders] 订单列表为空")
            return True

        conn = await self._get_connection()
        now = utc_now_iso()
        try:
            for tag_id, display_order in orders:
                await conn.execute(
                    (
                        "UPDATE tags SET display_order = ?, updated_at = ? "
                        "WHERE id = ? AND user_id = ?"
                    ),
                    (display_order, now, tag_id, user_id),
                )

            await conn.commit()
            self.logger.info(
                "[update_tag_display_orders] 成功更新%s个标签的显示顺序 (user_id=%s)",
                len(orders),
                user_id,
            )
            return True
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("更新标签显示顺序失败: %s", exc, exc_info=True)
            await conn.rollback()
            return False

    @log_method
    async def add_tags_to_bill(self, bill_id: int, tag_ids: list[int], user_id: int = 1) -> bool:
        """Attach tags to a bill, ignoring duplicate relationships."""
        self.logger.info(
            "[add_tags_to_bill] 开始为账单%s添加标签: %s (user_id=%s)",
            bill_id,
            tag_ids,
            user_id,
        )
        if not tag_ids:
            self.logger.info("[add_tags_to_bill] 标签列表为空，无需添加")
            return True

        conn = await self._get_connection()
        now = utc_now_iso()
        try:
            values = [(bill_id, tag_id, now) for tag_id in tag_ids]
            await conn.executemany(
                "INSERT OR IGNORE INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
                values,
            )
            await conn.commit()
            self.logger.info("[add_tags_to_bill] 成功添加%s个标签", len(tag_ids))
            return True
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("添加标签失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def get_tags_for_bill(self, bill_id: int, user_id: int = 1) -> list[dict[str, Any]]:
        """List all tags linked to a single bill."""
        _ = user_id
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT t.*
            FROM tags t
            JOIN bill_tags bt ON t.id = bt.tag_id
            WHERE bt.bill_id = ?
            ORDER BY t.name
            """,
            (bill_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_tags_for_bills(
        self,
        bill_ids: list[int],
        user_id: int = 1,
    ) -> dict[int, list[dict[str, Any]]]:
        """List tags for multiple bills and return them grouped by bill ID."""
        _ = user_id
        if not bill_ids:
            return {}

        conn = await self._get_connection()
        placeholders = ",".join("?" * len(bill_ids))
        async with conn.execute(
            f"""
            SELECT bt.bill_id, t.*
            FROM tags t
            JOIN bill_tags bt ON t.id = bt.tag_id
            WHERE bt.bill_id IN ({placeholders})
            ORDER BY t.name
            """,
            bill_ids,
        ) as cursor:
            rows = await cursor.fetchall()

        result: dict[int, list[dict[str, Any]]] = {}
        for row in rows:
            bill_id = row["bill_id"]
            tag = dict(row)
            del tag["bill_id"]
            result.setdefault(bill_id, []).append(tag)
        return result

    @log_method
    async def update_bill_tags(self, bill_id: int, tag_ids: list[int], user_id: int = 1) -> bool:
        """Replace all tags linked to one bill with the provided tag IDs."""
        self.logger.info(
            "[update_bill_tags] 开始更新账单%s的标签 (user_id=%s)，新标签: %s",
            bill_id,
            user_id,
            tag_ids,
        )
        conn = await self._get_connection()
        try:
            cursor = await conn.execute("DELETE FROM bill_tags WHERE bill_id = ?", (bill_id,))
            deleted_count = cursor.rowcount
            self.logger.info("[update_bill_tags] 删除了%s个旧标签", deleted_count)

            if tag_ids:
                now = utc_now_iso()
                values = [(bill_id, tag_id, now) for tag_id in tag_ids]
                await conn.executemany(
                    "INSERT INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
                    values,
                )
                self.logger.info("[update_bill_tags] 成功添加%s个新标签", len(tag_ids))
            else:
                self.logger.info("[update_bill_tags] 没有新标签需要添加")

            await conn.commit()
            self.logger.info("[update_bill_tags] 更新完成")
            return True
        except sqlite3.Error as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("更新账单标签失败: %s", exc, exc_info=True)
            return False
