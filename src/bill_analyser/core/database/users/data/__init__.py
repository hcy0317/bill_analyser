"""User data-management and user-specific exchange-rate helpers."""

# pyright: reportAttributeAccessIssue=false

# pylint: disable=missing-function-docstring,line-too-long,broad-exception-caught

from __future__ import annotations

from typing import Any

from bill_analyser.utils.config import load_default_user_settings

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now, utc_now_iso


class DatabaseUserDataMixin(DatabaseFacadeBase):
    """User data management and user-specific exchange rate helpers."""

    def _get_protected_usernames(self) -> set[str]:
        """Return usernames that must never be purged automatically."""
        protected = {"admin"}
        try:
            default_user = load_default_user_settings()
        except Exception:  # pragma: no cover - defensive config fallback
            default_user = None

        if isinstance(default_user, dict):
            username = str(default_user.get("username") or "").strip().lower()
            if username:
                protected.add(username)

        return protected

    async def _list_user_scoped_tables(self, conn: Any) -> list[str]:
        """Return all tables that expose a user_id column."""
        async with conn.execute(
            "SELECT name FROM sqlite_master "
            "WHERE type = 'table' AND name NOT LIKE 'sqlite_%' "
            "ORDER BY name ASC"
        ) as cursor:
            table_rows = await cursor.fetchall()

        user_scoped_tables: list[str] = []
        for row in table_rows:
            table_name = str(row[0] if not isinstance(row, dict) else row.get("name") or "").strip()
            if not table_name:
                continue

            async with conn.execute(f'PRAGMA table_info("{table_name}")') as table_info_cursor:
                columns = await table_info_cursor.fetchall()

            column_names = {
                str(column[1] if not isinstance(column, dict) else column.get("name") or "")
                for column in columns
            }
            if "user_id" in column_names:
                user_scoped_tables.append(table_name)

        return user_scoped_tables

    @log_method
    async def clear_user_transactions(self, user_id: int = 1) -> dict[str, Any]:
        conn = await self._get_connection()
        try:
            cursor = await conn.execute("SELECT COUNT(*) FROM bills WHERE user_id = ?", (user_id,))
            row = await cursor.fetchone()
            deleted_count = int(row[0] if row else 0)
            if deleted_count == 0:
                return {"success": True, "deleted_count": 0}

            async with conn.execute("SELECT id FROM bills WHERE user_id = ?", (user_id,)) as cursor:
                bill_rows = await cursor.fetchall()
            await self._delete_bill_pair_links_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await self._delete_bill_transfer_pair_suppressions_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await self._delete_bill_investment_pair_suppressions_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await self._delete_bill_learning_rule_suppressions_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await conn.execute(
                "DELETE FROM bill_tags WHERE bill_id IN (SELECT id FROM bills WHERE user_id = ?)",
                (user_id,),
            )
            await conn.execute("DELETE FROM bills WHERE user_id = ?", (user_id,))
            await conn.commit()
            await self.sync_all_account_balances(user_id)
            self._clear_cache("account_mappings")
            return {"success": True, "deleted_count": deleted_count}
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("清空用户交易失败: user_id=%s, error=%s", user_id, exc)
            await conn.rollback()
            return {"success": False, "message": str(exc), "deleted_count": 0}

    @log_method
    async def clear_user_data(self, user_id: int = 1) -> dict[str, Any]:
        conn = await self._get_connection()
        try:
            counts: dict[str, int] = {}
            count_queries = {
                "bills": "SELECT COUNT(*) FROM bills WHERE user_id = ?",
                "accounts": "SELECT COUNT(*) FROM accounts WHERE user_id = ?",
                "categories": "SELECT COUNT(*) FROM categories WHERE user_id = ?",
                "tags": "SELECT COUNT(*) FROM tags WHERE user_id = ?",
                "templates": "SELECT COUNT(*) FROM bill_templates WHERE user_id = ?",
                "recurring_bills": "SELECT COUNT(*) FROM recurring_bills WHERE user_id = ?",
                "budgets": "SELECT COUNT(*) FROM budgets WHERE user_id = ?",
            }
            for key, query in count_queries.items():
                cursor = await conn.execute(query, (user_id,))
                row = await cursor.fetchone()
                counts[key] = int(row[0] if row else 0)

            async with conn.execute("SELECT id FROM bills WHERE user_id = ?", (user_id,)) as cursor:
                bill_rows = await cursor.fetchall()
            await self._delete_bill_pair_links_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await self._delete_bill_transfer_pair_suppressions_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await self._delete_bill_investment_pair_suppressions_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )
            await self._delete_bill_learning_rule_suppressions_for_bill_ids(
                conn,
                [int(row[0]) for row in bill_rows if row and row[0] is not None],
                user_id=user_id,
            )

            await conn.execute("DELETE FROM bills_preview WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM bills_parser_template WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM import_sessions WHERE user_id = ?", (user_id,))
            await conn.execute(
                "DELETE FROM bill_tags WHERE bill_id IN (SELECT id FROM bills WHERE user_id = ?)",
                (user_id,),
            )
            await conn.execute("DELETE FROM bills WHERE user_id = ?", (user_id,))
            await conn.execute(
                "DELETE FROM budget_history WHERE budget_id IN (SELECT id FROM budgets WHERE user_id = ?)",
                (user_id,),
            )
            await conn.execute("DELETE FROM budgets WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM recurring_bills WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM bill_templates WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM saved_filters WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM account_transfers WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM tags WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM categories WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM account_types WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM accounts WHERE user_id = ?", (user_id,))
            await conn.commit()

            self._clear_cache("account_mappings")
            self._clear_cache("category_mappings")
            return {"success": True, "counts": counts}
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("清空用户业务数据失败: user_id=%s, error=%s", user_id, exc)
            await conn.rollback()
            return {"success": False, "message": str(exc)}

    @log_method
    async def purge_user_account(self, user_id: int, allow_protected: bool = False) -> dict[str, Any]:
        """Purge one user row plus all user-scoped residue except protected users."""
        conn = await self._get_connection()
        try:
            user = await self.get_user_by_id(int(user_id))
            if not user:
                return {"success": True, "deleted": False, "reason": "user_not_found", "user_id": int(user_id)}

            username = str(user.get("username") or "").strip()
            normalized_username = username.lower()
            if not allow_protected and normalized_username in self._get_protected_usernames():
                raise ValueError(f"Refusing to purge protected user: {username}")

            business_cleanup = await self.clear_user_data(user_id=int(user_id))
            if not business_cleanup.get("success"):
                return {
                    "success": False,
                    "deleted": False,
                    "reason": "clear_user_data_failed",
                    "user_id": int(user_id),
                    "message": business_cleanup.get("message"),
                }

            deleted_counts: dict[str, int] = {}
            for table_name in await self._list_user_scoped_tables(conn):
                if table_name == "users":
                    continue
                cursor = await conn.execute(
                    f'DELETE FROM "{table_name}" WHERE user_id = ?',
                    (int(user_id),),
                )
                deleted_counts[table_name] = int(cursor.rowcount or 0)

            cursor = await conn.execute("DELETE FROM users WHERE id = ?", (int(user_id),))
            deleted_counts["users"] = int(cursor.rowcount or 0)
            await conn.commit()

            self._clear_cache("account_mappings")
            self._clear_cache("category_mappings")

            return {
                "success": True,
                "deleted": bool(deleted_counts["users"] > 0),
                "user_id": int(user_id),
                "username": username,
                "business_counts": business_cleanup.get("counts") or {},
                "deleted_counts": deleted_counts,
            }
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("彻底清理用户失败: user_id=%s, error=%s", user_id, exc)
            await conn.rollback()
            return {
                "success": False,
                "deleted": False,
                "user_id": int(user_id),
                "message": str(exc),
            }

    @log_method
    async def get_user_data_statistics(self, user_id: int = 1) -> dict[str, int]:
        conn = await self._get_connection()
        count_queries = {
            "billCount": "SELECT COUNT(*) FROM bills WHERE user_id = ?",
            "accountCount": "SELECT COUNT(*) FROM accounts WHERE user_id = ?",
            "categoryCount": "SELECT COUNT(*) FROM categories WHERE user_id = ?",
            "tagCount": "SELECT COUNT(*) FROM tags WHERE user_id = ?",
            "templateCount": "SELECT COUNT(*) FROM bill_templates WHERE user_id = ?",
        }
        statistics: dict[str, int] = {}
        for key, query in count_queries.items():
            async with conn.execute(query, (user_id,)) as cursor:
                row = await cursor.fetchone()
                statistics[key] = int(row[0] if row else 0)
        return statistics

    @log_method
    async def get_user_custom_exchange_rates(self, base_currency: str, user_id: int = 1) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        normalized_base = (base_currency or "CNY").upper()
        query = """
            SELECT r1.*
            FROM user_exchange_rates r1
            INNER JOIN (
                SELECT to_currency, MAX(effective_date) AS max_effective_date
                FROM user_exchange_rates
                WHERE user_id = ? AND from_currency = ?
                GROUP BY to_currency
            ) r2 ON r1.to_currency = r2.to_currency
                AND r1.effective_date = r2.max_effective_date
            WHERE r1.user_id = ? AND r1.from_currency = ?
            ORDER BY r1.to_currency ASC
        """
        async with conn.execute(query, (user_id, normalized_base, user_id, normalized_base)) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def upsert_user_custom_exchange_rate(
        self,
        base_currency: str,
        target_currency: str,
        rate: float,
        user_id: int = 1,
    ) -> dict[str, Any]:
        conn = await self._get_connection()
        normalized_base = (base_currency or "CNY").upper()
        normalized_target = (target_currency or "").upper()
        now = utc_now_iso()
        try:
            await conn.execute(
                "DELETE FROM user_exchange_rates WHERE user_id = ? AND from_currency = ? AND to_currency = ?",
                (user_id, normalized_base, normalized_target),
            )
            cursor = await conn.execute(
                """
                INSERT INTO user_exchange_rates (
                    from_currency, to_currency, rate, source,
                    effective_date, created_at, updated_at, user_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (normalized_base, normalized_target, rate, "manual", now, now, now, user_id),
            )
            await conn.commit()
            return {
                "success": True,
                "id": int(cursor.lastrowid or 0),
                "from_currency": normalized_base,
                "to_currency": normalized_target,
                "rate": rate,
                "update_time": int(utc_now().timestamp()),
            }
        except Exception as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error(
                "更新用户自定义汇率失败: user_id=%s, %s->%s, error=%s",
                user_id,
                normalized_base,
                normalized_target,
                exc,
            )
            return {"success": False, "message": str(exc)}

    @log_method
    async def delete_user_custom_exchange_rate(
        self,
        base_currency: str,
        target_currency: str,
        user_id: int = 1,
    ) -> bool:
        conn = await self._get_connection()
        normalized_base = (base_currency or "CNY").upper()
        normalized_target = (target_currency or "").upper()
        try:
            cursor = await conn.execute(
                "DELETE FROM user_exchange_rates WHERE user_id = ? AND from_currency = ? AND to_currency = ?",
                (user_id, normalized_base, normalized_target),
            )
            await conn.commit()
            return bool(cursor.rowcount > 0)
        except Exception as exc:  # pragma: no cover - defensive logging branch
            await conn.rollback()
            self.logger.error(
                "删除用户自定义汇率失败: user_id=%s, %s->%s, error=%s",
                user_id,
                normalized_base,
                normalized_target,
                exc,
            )
            return False
