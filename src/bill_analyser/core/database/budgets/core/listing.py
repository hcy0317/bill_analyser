"""Budget listing and category lookup helpers."""

from __future__ import annotations

# pylint: disable=missing-function-docstring,line-too-long,too-many-arguments,too-many-positional-arguments
from typing import Any

from bill_analyser.utils.logger import log_method


class BudgetListingMixin:
    @log_method
    async def get_budgets(self, filters: dict[str, Any] | None = None, user_id: int = 1) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        query = "SELECT * FROM budgets WHERE user_id = ?"
        params: list[Any] = [user_id]

        if filters:
            if "period_type" in filters:
                query += " AND period_type = ?"
                params.append(filters["period_type"])
            if "enabled" in filters:
                query += " AND enabled = ?"
                params.append(1 if filters["enabled"] else 0)
            if "category" in filters:
                query += " AND category = ?"
                params.append(filters["category"])

        query += " ORDER BY created_at DESC"
        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @log_method
    async def get_budgets_for_listing(
        self,
        filters: dict[str, Any] | None = None,
        budget_type: int | None = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """Return budget list rows enriched with resolved category/type metadata."""
        budgets = await self.get_budgets(filters, user_id=user_id)
        categories = await self.get_all_categories(user_id=user_id)
        category_context = self._build_budget_category_context(categories)

        enriched_budgets: list[dict[str, Any]] = []
        for budget in budgets:
            resolved_budget_type = self._resolve_budget_category_type(
                budget.get("category"),
                budget.get("sub_category"),
                category_context,
                preferred_type=budget_type,
            )
            if budget_type is not None and resolved_budget_type != budget_type:
                continue

            item = dict(budget)
            if resolved_budget_type is None:
                item["category_info"] = None
                item["category_id"] = ""
                enriched_budgets.append(item)
                continue

            category_info = self._resolve_budget_category_info(
                budget.get("category"),
                budget.get("sub_category"),
                category_context,
                resolved_budget_type,
            )
            item["type"] = resolved_budget_type
            item["category_info"] = category_info
            item["category_id"] = (
                str(category_info.get("id") or "") if category_info else ""
            )
            enriched_budgets.append(item)

        return enriched_budgets

    @log_method
    async def get_budget_by_id(self, budget_id: int, user_id: int = 1) -> dict[str, Any] | None:
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM budgets WHERE id = ? AND user_id = ?", (budget_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_primary_category_budget(
        self,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        group_key = self._build_budget_group_key(category, period_type, start_date, user_id)
        if group_key is None:
            return None
        return await self._get_primary_category_budget_with_conn(conn, group_key)

    @log_method
    async def get_budget_by_category(
        self,
        category: str,
        sub_category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        conn = await self._get_connection()
        normalized_sub_category = self._normalize_budget_sub_category(sub_category)
        group_key = self._build_budget_group_key(category, period_type, start_date, user_id)
        if group_key is None:
            return None
        normalized_category, normalized_period_type, normalized_start_date, normalized_user_id = group_key

        if normalized_sub_category:
            async with conn.execute(
                """
                SELECT * FROM budgets
                WHERE category = ?
                  AND sub_category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
                """,
                (
                    normalized_category,
                    normalized_sub_category,
                    normalized_period_type,
                    normalized_start_date,
                    normalized_user_id,
                ),
            ) as cursor:
                row = await cursor.fetchone()
                return dict(row) if row else None

        async with conn.execute(
            """
            SELECT * FROM budgets
            WHERE category = ?
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            (normalized_category, normalized_period_type, normalized_start_date, normalized_user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_sub_category_budgets_total(
        self,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1,
    ) -> float:
        conn = await self._get_connection()
        group_key = self._build_budget_group_key(category, period_type, start_date, user_id)
        if group_key is None:
            return 0.0
        return await self._get_sub_category_budgets_total_with_conn(conn, group_key)
