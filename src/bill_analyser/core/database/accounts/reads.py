"""Account read and historical suggestion helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-locals,too-many-statements,too-many-arguments,too-many-positional-arguments,too-many-branches

import json
from typing import Any

from bill_analyser.utils.logger import log_method


class AccountReadsMixin:
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
