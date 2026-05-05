"""Account mutation and transaction move helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-locals,too-many-statements,too-many-arguments,too-many-positional-arguments,broad-exception-caught

import json
from typing import Any

from bill_analyser.core import account_rust_bridge
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.utils.logger import log_method


class AccountMutationsMixin:
    """Account master-data mutation and account operation helpers."""

    @log_method
    async def create_account(self, data: dict[str, Any], user_id: int = 1) -> int:
        """创建账户。"""
        if self._should_use_rust_account_bridge():
            return account_rust_bridge.create_account(self.db_path, data, user_id=user_id)
        return await self._create_account_python(data, user_id=user_id)

    async def _create_account_python(self, data: dict[str, Any], user_id: int = 1) -> int:
        """通过 aiosqlite fallback 创建账户。"""
        conn = await self._get_connection()
        now = utc_now_iso()
        sub_accounts = data.get("subAccounts", [])

        aliases_value = data.get("aliases")
        if isinstance(aliases_value, list):
            aliases_value = json.dumps(
                [str(alias).strip() for alias in aliases_value if str(alias).strip()],
                ensure_ascii=False,
            )

        parent_id = data.get("parentId") or data.get("parent_id", 0)
        cursor = await conn.execute(
            """
            INSERT INTO accounts (
                name, type, category, currency, icon, color,
                balance, initial_balance, hidden, display_order,
                comment, aliases, parent_id, created_at, updated_at, user_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("name"),
                data.get("type", 1),
                data.get("category"),
                data.get("currency", "CNY"),
                data.get("icon"),
                data.get("color"),
                data.get("balance", 0.0),
                data.get("initial_balance", 0.0),
                1 if data.get("hidden", False) else 0,
                data.get("display_order", 0),
                data.get("comment"),
                aliases_value,
                parent_id,
                now,
                now,
                user_id,
            ),
        )

        account_id = int(cursor.lastrowid or 0)
        await conn.commit()

        if isinstance(sub_accounts, list):
            for sub_account in sub_accounts:
                sub_account["parentId"] = account_id
                await self.create_account(sub_account, user_id)

        return account_id

    @log_method
    async def update_account(self, account_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        """更新账户。"""
        if self._should_use_rust_account_bridge():
            return account_rust_bridge.update_account(self.db_path, account_id, data, user_id=user_id)
        return await self._update_account_python(account_id, data, user_id=user_id)

    async def _update_account_python(self, account_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        """通过 aiosqlite fallback 更新账户。"""
        if not data:
            return False

        conn = await self._get_connection()
        update_data = {**data, "updated_at": utc_now_iso()}

        if "parentId" in update_data:
            update_data["parent_id"] = update_data.pop("parentId")
        if "subAccounts" in update_data:
            self.logger.warning("账户更新数据包含 subAccounts 字段，已移除: account_id=%s", account_id)
            del update_data["subAccounts"]

        valid_columns = {
            "name",
            "type",
            "category",
            "currency",
            "icon",
            "color",
            "balance",
            "initial_balance",
            "hidden",
            "display_order",
            "comment",
            "aliases",
            "parent_id",
            "updated_at",
        }
        filtered_data = {key: value for key, value in update_data.items() if key in valid_columns}
        if not filtered_data:
            self.logger.warning(
                "账户更新数据过滤后为空: account_id=%s, 原始字段=%s",
                account_id,
                list(update_data.keys()),
            )
            return False

        filtered_data.pop("id", None)
        set_clause = ", ".join(f"{key} = ?" for key in filtered_data)
        values = [*filtered_data.values(), account_id, user_id]

        self.logger.info(
            "更新账户: id=%s, user_id=%s, 字段=%s",
            account_id,
            user_id,
            list(filtered_data.keys()),
        )
        cursor = await conn.execute(f"UPDATE accounts SET {set_clause} WHERE id = ? AND user_id = ?", values)
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def delete_account(self, account_id: int, user_id: int = 1) -> bool:
        """删除账户。"""
        if self._should_use_rust_account_bridge():
            return account_rust_bridge.delete_account(self.db_path, account_id, user_id=user_id)
        return await self._delete_account_python(account_id, user_id=user_id)

    async def _delete_account_python(self, account_id: int, user_id: int = 1) -> bool:
        """通过 aiosqlite fallback 删除账户。"""
        conn = await self._get_connection()
        cursor = await conn.execute("DELETE FROM accounts WHERE id = ? AND user_id = ?", (account_id, user_id))
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def update_account_display_orders(
        self,
        orders: list[tuple[int, int]],
        user_id: int = 1,
    ) -> bool:
        """批量更新账户显示顺序。"""
        if self._should_use_rust_account_bridge():
            return account_rust_bridge.update_display_orders(self.db_path, orders, user_id=user_id)
        return await self._update_account_display_orders_python(orders, user_id=user_id)

    async def _update_account_display_orders_python(
        self,
        orders: list[tuple[int, int]],
        user_id: int = 1,
    ) -> bool:
        """通过 aiosqlite fallback 批量更新账户显示顺序。"""
        if not orders:
            return True

        conn = await self._get_connection()
        now = utc_now_iso()
        for account_id, display_order in orders:
            await conn.execute(
                "UPDATE accounts SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                (display_order, now, account_id, user_id),
            )
        await conn.commit()
        return True

    @log_method
    async def move_all_transactions(
        self,
        from_account_id: int,
        to_account_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """将指定账户下的全部交易迁移到另一个账户。"""
        if from_account_id == to_account_id:
            return {"success": False, "message": "Source and target accounts must be different"}

        source_account = await self.get_account_by_id(from_account_id, user_id=user_id)
        if not source_account:
            return {"success": False, "message": "Source account not found"}

        target_account = await self.get_account_by_id(to_account_id, user_id=user_id)
        if not target_account:
            return {"success": False, "message": "Target account not found"}

        conn = await self._get_connection()
        now = utc_now_iso()
        try:
            source_cursor = await conn.execute(
                "UPDATE bills SET source_account_id = ?, updated_at = ? WHERE user_id = ? AND source_account_id = ?",
                (to_account_id, now, user_id, from_account_id),
            )
            destination_cursor = await conn.execute(
                "UPDATE bills SET destination_account_id = ?, updated_at = ? WHERE user_id = ? AND destination_account_id = ?",
                (to_account_id, now, user_id, from_account_id),
            )
            transfer_from_cursor = await conn.execute(
                "UPDATE account_transfers SET from_account_id = ? WHERE user_id = ? AND from_account_id = ?",
                (to_account_id, user_id, from_account_id),
            )
            transfer_to_cursor = await conn.execute(
                "UPDATE account_transfers SET to_account_id = ? WHERE user_id = ? AND to_account_id = ?",
                (to_account_id, user_id, from_account_id),
            )
            await conn.commit()

            await self.sync_account_balance(from_account_id)
            await self.sync_account_balance(to_account_id)
            self._clear_cache()

            moved_count = (
                source_cursor.rowcount
                + destination_cursor.rowcount
                + transfer_from_cursor.rowcount
                + transfer_to_cursor.rowcount
            )
            return {"success": True, "message": "Transactions moved successfully", "moved_count": moved_count}
        except Exception as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error("移动账户交易失败: %s", exc, exc_info=True)
            return {"success": False, "message": str(exc)}

    @log_method
    async def delete_all_transactions_by_account(
        self,
        account_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """删除指定账户关联的全部交易。"""
        account = await self.get_account_by_id(account_id, user_id=user_id)
        if not account:
            return {"success": False, "message": "Account not found"}

        conn = await self._get_connection()
        try:
            async with conn.execute(
                "SELECT id FROM bills WHERE user_id = ? AND (source_account_id = ? OR destination_account_id = ?)",
                (user_id, account_id, account_id),
            ) as cursor:
                bill_rows = await cursor.fetchall()

            bill_ids = [int(row["id"]) for row in bill_rows]
            deleted_bill_count = 0
            if bill_ids:
                await self._delete_bill_pair_links_for_bill_ids(
                    conn,
                    bill_ids,
                    user_id=user_id,
                )
                await self._delete_bill_transfer_pair_suppressions_for_bill_ids(
                    conn,
                    bill_ids,
                    user_id=user_id,
                )
                await self._delete_bill_investment_pair_suppressions_for_bill_ids(
                    conn,
                    bill_ids,
                    user_id=user_id,
                )
                await self._delete_bill_learning_rule_suppressions_for_bill_ids(
                    conn,
                    bill_ids,
                    user_id=user_id,
                )
                placeholders = ",".join(["?" for _ in bill_ids])
                await conn.execute(f"DELETE FROM bill_tags WHERE bill_id IN ({placeholders})", bill_ids)
                bill_cursor = await conn.execute(
                    f"DELETE FROM bills WHERE id IN ({placeholders}) AND user_id = ?",
                    [*bill_ids, user_id],
                )
                deleted_bill_count = bill_cursor.rowcount

            transfer_cursor = await conn.execute(
                "DELETE FROM account_transfers WHERE user_id = ? AND (from_account_id = ? OR to_account_id = ?)",
                (user_id, account_id, account_id),
            )
            await conn.commit()

            await self.sync_account_balance(account_id)
            self._clear_cache()

            deleted_count = deleted_bill_count + transfer_cursor.rowcount
            return {"success": True, "message": "Transactions deleted successfully", "deleted_count": deleted_count}
        except Exception as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error("删除账户交易失败: %s", exc, exc_info=True)
            return {"success": False, "message": str(exc)}
