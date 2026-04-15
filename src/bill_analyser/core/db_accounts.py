"""Account-domain persistence helpers for the split database facade."""

# pylint: disable=too-many-locals,too-many-statements,too-many-arguments,too-many-positional-arguments,too-many-branches,broad-exception-caught,line-too-long

from __future__ import annotations

import json
from typing import Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseAccountsMixin(DatabaseFacadeBase):
    """Account CRUD, alias mapping, and balance synchronization helpers."""

    @log_method
    async def get_all_accounts(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取所有账户。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM accounts WHERE user_id = ? ORDER BY display_order, name",
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_account_by_id(self, account_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """根据 ID 获取账户。"""
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM accounts WHERE id = ? AND user_id = ?", (account_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_sub_accounts(self, parent_id: int, user_id: int = 1) -> list[dict[str, Any]]:
        """获取子账户列表。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM accounts WHERE parent_id = ? AND user_id = ?",
            (parent_id, user_id),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_account_alias_mapping(self, user_id: int = 1) -> dict[str, int]:
        """获取账户别名到账户 ID 的映射。"""
        conn = await self._get_connection()
        alias_map: dict[str, int] = {}

        async with conn.execute("SELECT id, name, aliases FROM accounts WHERE user_id = ?", (user_id,)) as cursor:
            rows = await cursor.fetchall()

        for row in rows:
            account_id = row["id"]
            account_name = row["name"]
            aliases_json = row["aliases"]

            if account_name:
                alias_map[account_name.lower()] = account_id
                alias_map[account_name] = account_id

            if not aliases_json:
                continue

            try:
                aliases = json.loads(aliases_json)
            except json.JSONDecodeError:
                self.logger.warning("账户 %s 的别名 JSON 解析失败: %s", account_id, aliases_json)
                continue

            if not isinstance(aliases, list):
                continue

            for alias in aliases:
                if not alias or not isinstance(alias, str):
                    continue
                alias_map[alias.lower()] = account_id
                alias_map[alias] = account_id

        self.logger.info("加载账户别名映射: %s 条", len(alias_map))
        return alias_map

    @log_method
    async def get_historical_source_account_suggestion(
        self,
        user_id: int = 1,
        payment_method: str = "",
        counterparty: str = "",
        description: str = "",
        bill_type: str = "",
    ) -> dict[str, Any] | None:
        """基于历史账单为源账户提供建议。"""
        normalized_payment_method = self._normalize_import_learning_text(payment_method)
        normalized_counterparty = self._normalize_import_learning_text(counterparty)
        normalized_description = self._normalize_import_learning_text(description)
        normalized_type = str(bill_type or "").strip().lower()

        if not any([normalized_payment_method, normalized_counterparty, normalized_description]):
            return None

        conn = await self._get_connection()
        clauses: list[str] = []
        params: list[Any] = [user_id]

        if normalized_payment_method:
            clauses.append("LOWER(TRIM(COALESCE(payment_method, ''))) = ?")
            params.append(normalized_payment_method)
        if normalized_counterparty:
            clauses.append("LOWER(TRIM(COALESCE(counterparty, ''))) = ?")
            params.append(normalized_counterparty)
        if normalized_description:
            clauses.append("LOWER(TRIM(COALESCE(description, ''))) = ?")
            params.append(normalized_description)

        query = f"""
            SELECT source_account_id, type, payment_method, counterparty, description, date
            FROM bills
            WHERE user_id = ?
              AND source_account_id IS NOT NULL
              AND source_account_id != 0
              AND ({" OR ".join(clauses)})
            ORDER BY date DESC, id DESC
            LIMIT 300
        """

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()

        score_by_account: dict[int, dict[str, Any]] = {}
        for row in rows:
            account_id = int(row["source_account_id"])
            score = 0
            reasons: list[str] = []

            row_payment_method = self._normalize_import_learning_text(row["payment_method"])
            row_counterparty = self._normalize_import_learning_text(row["counterparty"])
            row_description = self._normalize_import_learning_text(row["description"])
            row_type = str(row["type"] or "").strip().lower()

            if normalized_payment_method and row_payment_method == normalized_payment_method:
                score += 8
                reasons.append("payment_method")
            if normalized_counterparty and row_counterparty == normalized_counterparty:
                score += 5
                reasons.append("counterparty")
            if normalized_description and row_description == normalized_description:
                score += 3
                reasons.append("description")
            if normalized_type and row_type == normalized_type:
                score += 2
                reasons.append("type")

            if score <= 0:
                continue

            existing = score_by_account.get(account_id)
            if not existing:
                score_by_account[account_id] = {
                    "account_id": account_id,
                    "score": score,
                    "reasons": reasons,
                    "hits": 1,
                }
                continue

            existing["score"] += score
            existing["hits"] += 1
            existing["reasons"] = sorted(set(existing["reasons"] + reasons))

        if not score_by_account:
            return None

        best = max(score_by_account.values(), key=lambda item: (item["score"], item["hits"], -item["account_id"]))
        self.logger.debug(
            "[历史源账户建议] user_id=%d -> account_id=%s, score=%s, reasons=%s",
            user_id,
            best["account_id"],
            best["score"],
            ",".join(best["reasons"]),
        )
        return best

    @log_method
    async def get_historical_destination_account_suggestion(
        self,
        user_id: int = 1,
        payment_method: str = "",
        counterparty: str = "",
        description: str = "",
        bill_type: str = "",
        source_account_id: int | None = None,
    ) -> dict[str, Any] | None:
        """基于历史账单为目标账户提供建议。"""
        normalized_payment_method = self._normalize_import_learning_text(payment_method)
        normalized_counterparty = self._normalize_import_learning_text(counterparty)
        normalized_description = self._normalize_import_learning_text(description)
        normalized_type = str(bill_type or "").strip().lower()

        if not any([normalized_payment_method, normalized_counterparty, normalized_description]):
            return None

        conn = await self._get_connection()
        clauses: list[str] = []
        params: list[Any] = [user_id]

        if normalized_payment_method:
            clauses.append("LOWER(TRIM(COALESCE(payment_method, ''))) = ?")
            params.append(normalized_payment_method)
        if normalized_counterparty:
            clauses.append("LOWER(TRIM(COALESCE(counterparty, ''))) = ?")
            params.append(normalized_counterparty)
        if normalized_description:
            clauses.append("LOWER(TRIM(COALESCE(description, ''))) = ?")
            params.append(normalized_description)

        query = f"""
            SELECT destination_account_id, type, payment_method, counterparty, description, date
            FROM bills
            WHERE user_id = ?
              AND destination_account_id IS NOT NULL
              AND destination_account_id != 0
              AND ({" OR ".join(clauses)})
            ORDER BY date DESC, id DESC
            LIMIT 300
        """

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()

        score_by_account: dict[int, dict[str, Any]] = {}
        for row in rows:
            account_id = int(row["destination_account_id"])
            if source_account_id and int(source_account_id) == account_id:
                continue

            score = 0
            reasons: list[str] = []
            row_payment_method = self._normalize_import_learning_text(row["payment_method"])
            row_counterparty = self._normalize_import_learning_text(row["counterparty"])
            row_description = self._normalize_import_learning_text(row["description"])
            row_type = str(row["type"] or "").strip().lower()

            if normalized_payment_method and row_payment_method == normalized_payment_method:
                score += 6
                reasons.append("payment_method")
            if normalized_counterparty and row_counterparty == normalized_counterparty:
                score += 6
                reasons.append("counterparty")
            if normalized_description and row_description == normalized_description:
                score += 4
                reasons.append("description")
            if normalized_type and row_type == normalized_type:
                score += 2
                reasons.append("type")

            if score <= 0:
                continue

            existing = score_by_account.get(account_id)
            if not existing:
                score_by_account[account_id] = {
                    "account_id": account_id,
                    "score": score,
                    "reasons": reasons,
                    "hits": 1,
                }
                continue

            existing["score"] += score
            existing["hits"] += 1
            existing["reasons"] = sorted(set(existing["reasons"] + reasons))

        if not score_by_account:
            return None

        best = max(score_by_account.values(), key=lambda item: (item["score"], item["hits"], -item["account_id"]))
        self.logger.debug(
            "[历史目标账户建议] user_id=%d -> account_id=%s, score=%s, reasons=%s",
            user_id,
            best["account_id"],
            best["score"],
            ",".join(best["reasons"]),
        )
        return best

    @log_method
    async def create_account(self, data: dict[str, Any], user_id: int = 1) -> int:
        """创建账户。"""
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
        conn = await self._get_connection()
        cursor = await conn.execute("DELETE FROM accounts WHERE id = ? AND user_id = ?", (account_id, user_id))
        await conn.commit()
        return cursor.rowcount > 0

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

    @log_method
    async def update_account_balance(self, account_id: int, amount: float, operation: str = "add") -> bool:
        """更新账户余额。"""
        try:
            conn = await self._get_connection()
            async with conn.execute("SELECT balance FROM accounts WHERE id = ?", (account_id,)) as cursor:
                row = await cursor.fetchone()
                if not row:
                    self.logger.warning("账户不存在: account_id=%s", account_id)
                    return False
                current_balance = row["balance"] or 0.0

            if operation == "add":
                new_balance = current_balance + amount
                self.logger.info(
                    "增加余额: account_id=%s, 原:%s + %s = %s",
                    account_id,
                    current_balance,
                    amount,
                    new_balance,
                )
            elif operation == "subtract":
                new_balance = current_balance - amount
                self.logger.info(
                    "减少余额: account_id=%s, 原:%s - %s = %s",
                    account_id,
                    current_balance,
                    amount,
                    new_balance,
                )
            else:
                self.logger.error("未知操作类型: %s", operation)
                return False

            cursor = await conn.execute(
                "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                (new_balance, utc_now_iso(), account_id),
            )
            await conn.commit()
            return cursor.rowcount > 0
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("更新账户余额失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def calculate_account_balance(self, account_id: int, account_name: str | None = None) -> float:
        """计算账户实际余额（基于关联的所有账单）。"""
        try:
            conn = await self._get_connection()
            if not account_name:
                async with conn.execute("SELECT name FROM accounts WHERE id = ?", (account_id,)) as cursor:
                    row = await cursor.fetchone()
                    if not row:
                        self.logger.warning("账户不存在: account_id=%s", account_id)
                        return 0.0
                    account_name = row["name"]

            async with conn.execute("SELECT initial_balance FROM accounts WHERE id = ?", (account_id,)) as cursor:
                row = await cursor.fetchone()
                initial_balance = row["initial_balance"] if row else 0.0

            async with conn.execute(
                """
                SELECT
                    SUM(CASE WHEN type = '收入' THEN amount ELSE 0 END) as income,
                    SUM(CASE WHEN type = '支出' THEN amount ELSE 0 END) as expense,
                    SUM(CASE WHEN type = '转账' THEN amount ELSE 0 END) as transfer_out,
                    SUM(CASE WHEN type = '投资' THEN amount ELSE 0 END) as investment_out
                FROM bills
                WHERE source_account_id = ?
                """,
                (account_id,),
            ) as cursor:
                row = await cursor.fetchone()
                income = row["income"] or 0.0
                expense = row["expense"] or 0.0
                transfer_out = row["transfer_out"] or 0.0
                investment_out = row["investment_out"] or 0.0

            async with conn.execute(
                """
                SELECT
                    SUM(CASE WHEN type = '转账' THEN destination_amount ELSE 0 END) as transfer_in,
                    SUM(CASE WHEN type = '投资' THEN destination_amount ELSE 0 END) as investment_in
                FROM bills
                WHERE destination_account_id = ?
                """,
                (account_id,),
            ) as cursor:
                row = await cursor.fetchone()
                transfer_in = row["transfer_in"] or 0.0
                investment_in = row["investment_in"] or 0.0

            calculated_balance = (
                initial_balance + income - expense - transfer_out + transfer_in - investment_out + investment_in
            )

            self.logger.info(
                "计算账户余额: account_id=%s, name=%s, 初始=%s, 收入=%s, 支出=%s, 转账转出=%s, "
                "转账转入=%s, 投资转出=%s, 投资转入=%s, 实际=%s",
                account_id,
                account_name,
                initial_balance,
                income,
                expense,
                transfer_out,
                transfer_in,
                investment_out,
                investment_in,
                calculated_balance,
            )
            return calculated_balance
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("计算账户余额失败: %s", exc, exc_info=True)
            return 0.0

    @log_method
    async def sync_account_balance(self, account_id: int) -> bool:
        """同步单个账户余额。"""
        try:
            conn = await self._get_connection()
            async with conn.execute("SELECT name FROM accounts WHERE id = ?", (account_id,)) as cursor:
                row = await cursor.fetchone()
                if not row:
                    self.logger.warning("账户不存在: account_id=%s", account_id)
                    return False
                account_name = row["name"]

            calculated_balance = await self.calculate_account_balance(account_id, account_name)
            cursor = await conn.execute(
                "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                (calculated_balance, utc_now_iso(), account_id),
            )
            await conn.commit()
            self.logger.info("同步账户余额: account_id=%s, balance=%s", account_id, calculated_balance)
            return cursor.rowcount > 0
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("同步账户余额失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def sync_all_account_balances(self, user_id: int = 1) -> dict[str, Any]:
        """同步所有账户的余额。"""
        try:
            conn = await self._get_connection()
            async with conn.execute(
                "SELECT id, name, balance, initial_balance FROM accounts WHERE user_id = ?",
                (user_id,),
            ) as cursor:
                accounts = await cursor.fetchall()

            result = {
                "total_accounts": len(accounts),
                "synced_accounts": 0,
                "discrepancies": [],
                "errors": [],
            }
            self.logger.info("[批量同步账户余额] 开始同步 %s 个账户 (user_id=%s)", len(accounts), user_id)

            for account in accounts:
                account_id = account["id"]
                account_name = account["name"]
                old_balance = account["balance"] or 0.0
                try:
                    new_balance = await self.calculate_account_balance(account_id, account_name)
                    if abs(old_balance - new_balance) > 0.001:
                        diff = new_balance - old_balance
                        result["discrepancies"].append(
                            {
                                "account_id": account_id,
                                "name": account_name,
                                "old_balance": round(old_balance, 2),
                                "new_balance": round(new_balance, 2),
                                "diff": round(diff, 2),
                            }
                        )
                        self.logger.info(
                            "[余额差异] 账户 '%s' (ID=%s): 旧余额=%0.2f, 新余额=%0.2f, 差异=%0.2f",
                            account_name,
                            account_id,
                            old_balance,
                            new_balance,
                            diff,
                        )

                    await conn.execute(
                        "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                        (new_balance, utc_now_iso(), account_id),
                    )
                    result["synced_accounts"] += 1
                except Exception as exc:  # pragma: no cover - defensive logging branch
                    error_msg = f"账户 '{account_name}' (ID={account_id}) 同步失败: {exc!s}"
                    result["errors"].append(error_msg)
                    self.logger.error(error_msg, exc_info=True)

            await conn.commit()
            self._clear_cache("account_mappings")
            self.logger.info(
                "[批量同步账户余额完成] 成功=%s/%s, 差异=%s个, 错误=%s个",
                result["synced_accounts"],
                result["total_accounts"],
                len(result["discrepancies"]),
                len(result["errors"]),
            )
            return result
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("批量同步账户余额失败: %s", exc, exc_info=True)
            return {"total_accounts": 0, "synced_accounts": 0, "discrepancies": [], "errors": [str(exc)]}

    @log_method
    async def get_balances_before_date(self, date_str: str, user_id: int = 1) -> dict[int, float]:
        """获取指定日期前所有账户的余额（单位：元）。"""
        conn = await self._get_connection()
        balances: dict[int, float] = {}

        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills "
            "WHERE date < ? AND type = '收入' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                account_id = row[0]
                if account_id:
                    amount = row[1] or 0
                    balances[account_id] = balances.get(account_id, 0) + amount

        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills "
            "WHERE date < ? AND type = '支出' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                account_id = row[0]
                if account_id:
                    amount = row[1] or 0
                    balances[account_id] = balances.get(account_id, 0) - abs(amount)

        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills "
            "WHERE date < ? AND type = '转账' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                account_id = row[0]
                if account_id:
                    amount = row[1] or 0
                    balances[account_id] = balances.get(account_id, 0) - abs(amount)

        async with conn.execute(
            "SELECT destination_account_id, SUM(destination_amount) FROM bills "
            "WHERE date < ? AND type = '转账' AND user_id = ? GROUP BY destination_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                account_id = row[0]
                if account_id:
                    amount = row[1] or 0
                    balances[account_id] = balances.get(account_id, 0) + abs(amount)

        return balances
