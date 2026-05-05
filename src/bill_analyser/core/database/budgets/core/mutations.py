"""Budget create, update, and delete helpers."""

from __future__ import annotations

# pylint: disable=missing-function-docstring,line-too-long,too-many-locals
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.time import utc_now


class BudgetMutationMixin:
    @log_method
    async def create_budget(self, data: dict[str, Any], user_id: int = 1) -> int:
        conn = await self._get_connection()
        normalized_data = {
            **data,
            "name": data.get("name", ""),
            "sub_category": self._normalize_budget_sub_category(data.get("sub_category")),
        }
        budget_id = await self._insert_budget_record(conn, normalized_data, user_id)
        effective_budget = {**normalized_data, "id": budget_id}
        await self._synchronize_primary_budget_for_group(
            conn,
            self._build_budget_group_key(
                effective_budget.get("category"),
                effective_budget.get("period_type"),
                effective_budget.get("start_date"),
                user_id,
            ),
            reference_data=effective_budget,
        )
        await self._synchronize_budget_period_hierarchy(conn, effective_budget, user_id)
        await conn.commit()
        return budget_id

    @log_method
    async def update_budget(self, budget_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        conn = await self._get_connection()
        existing_budget = await self.get_budget_by_id(budget_id, user_id=user_id)
        if not existing_budget:
            return False

        normalized_data = self._normalize_budget_update_data(existing_budget, data)
        assignments, values = self._build_budget_update_assignments(normalized_data)
        if not assignments:
            return False

        values.extend([budget_id, user_id])
        cursor = await conn.execute(
            f"UPDATE budgets SET {', '.join(assignments)} WHERE id = ? AND user_id = ?",
            values,
        )
        if cursor.rowcount > 0:
            updated_budget = {**existing_budget, **normalized_data}
            group_keys = self._collect_budget_sync_group_keys(user_id, existing_budget, updated_budget)
            for group_key in group_keys:
                await self._synchronize_primary_budget_for_group(
                    conn,
                    group_key,
                    reference_data=updated_budget or normalized_data,
                )
            await self._synchronize_budget_period_hierarchy(conn, existing_budget, user_id)
            await self._synchronize_budget_period_hierarchy(conn, updated_budget, user_id)
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def delete_budget(self, budget_id: int, user_id: int = 1) -> bool:
        conn = await self._get_connection()
        budget = await self.get_budget_by_id(budget_id, user_id=user_id)
        if not budget:
            return False

        category = budget.get("category")
        period_type = budget.get("period_type")
        start_date = budget.get("start_date")
        is_primary_budget = not self._normalize_budget_sub_category(budget.get("sub_category"))

        if is_primary_budget:
            cursor = await conn.execute(
                """
                DELETE FROM budgets
                WHERE category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
                """,
                (category, period_type, start_date, user_id),
            )
        else:
            cursor = await conn.execute("DELETE FROM budgets WHERE id = ? AND user_id = ?", (budget_id, user_id))
            await self._synchronize_primary_budget_for_group(
                conn,
                self._build_budget_group_key(category, period_type, start_date, user_id),
                reference_data=budget,
            )
        await self._synchronize_budget_period_hierarchy(conn, budget, user_id)
        await conn.commit()
        return cursor.rowcount > 0
