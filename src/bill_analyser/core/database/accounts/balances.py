"""Account balance synchronization helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-locals,too-many-statements,too-many-arguments,too-many-positional-arguments,broad-exception-caught

from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.investment.matching import classify_investment_pnl_change


class AccountBalancesMixin:
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

            pnl_correction = await self._calculate_same_account_investment_pnl_correction(account_id)

            calculated_balance = (
                initial_balance + income - expense - transfer_out + transfer_in - investment_out + investment_in
                + pnl_correction
            )

            self.logger.info(
                "计算账户余额: account_id=%s, name=%s, 初始=%s, 收入=%s, 支出=%s, 转账转出=%s, "
                "转账转入=%s, 投资转出=%s, 投资转入=%s, 投资盈亏修正=%s, 实际=%s",
                account_id,
                account_name,
                initial_balance,
                income,
                expense,
                transfer_out,
                transfer_in,
                investment_out,
                investment_in,
                pnl_correction,
                calculated_balance,
            )
            return calculated_balance
        except Exception as exc:  # pragma: no cover - defensive logging branch
            self.logger.error("计算账户余额失败: %s", exc, exc_info=True)
            return 0.0

    async def _calculate_same_account_investment_pnl_correction(self, account_id: int) -> float:
        """修正同账户投资盈亏变化流水，让收益/亏损真实影响账户余额。"""
        conn = await self._get_connection()
        correction = 0.0
        async with conn.execute(
            """
            SELECT
                amount, destination_amount, type, counterparty, description,
                payment_method, main_category, sub_category
            FROM bills
            WHERE type IN ('投资', 'investment', '5')
              AND source_account_id = ?
              AND destination_account_id = ?
            """,
            (account_id, account_id),
        ) as cursor:
            async for row in cursor:
                bill = dict(row)
                pnl_signal = classify_investment_pnl_change(bill)
                if not pnl_signal:
                    continue
                amount = float(bill.get("amount") or 0.0)
                destination_amount = float(bill.get("destination_amount") or 0.0)
                pnl_amount = abs(amount) or abs(destination_amount)
                if pnl_amount <= 0:
                    continue
                desired_effect = pnl_amount if pnl_signal.get("direction") == "gain" else -pnl_amount
                current_effect = -amount + destination_amount
                correction += desired_effect - current_effect
        return correction

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
