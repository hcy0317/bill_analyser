"""Budget hierarchy query and synchronization helpers."""

from __future__ import annotations

# pylint: disable=missing-function-docstring,line-too-long,too-many-locals
from typing import TYPE_CHECKING, Any

from bill_analyser.core.database.shared import BudgetGroupKey
from .shared import BudgetPeriodGroupKey

if TYPE_CHECKING:
    import aiosqlite


class BudgetHierarchyMixin:
    async def _insert_budget_record(
        self,
        conn: aiosqlite.Connection,
        data: dict[str, Any],
        user_id: int,
    ) -> int:
        cursor = await conn.execute(
            """
            INSERT INTO budgets (
                name, category, sub_category, period_type, amount,
                start_date, end_date, alert_threshold, enabled,
                created_at, updated_at, user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                data.get("name", ""),
                data.get("category"),
                self._normalize_budget_sub_category(data.get("sub_category")),
                data["period_type"],
                data["amount"],
                data["start_date"],
                data.get("end_date"),
                data.get("alert_threshold", 80),
                data.get("enabled", 1),
                data["created_at"],
                data["updated_at"],
                user_id,
            ),
        )
        return int(cursor.lastrowid or 0)

    async def _get_primary_category_budget_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetGroupKey,
    ) -> dict[str, Any] | None:
        category, period_type, start_date, user_id = group_key
        async with conn.execute(
            """
            SELECT * FROM budgets
            WHERE category = ?
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            (category, period_type, start_date, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def _get_sub_category_budgets_total_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetGroupKey,
    ) -> float:
        category, period_type, start_date, user_id = group_key
        async with conn.execute(
            """
            SELECT COALESCE(SUM(amount), 0) as total
            FROM budgets
            WHERE category = ?
              AND sub_category IS NOT NULL
              AND sub_category != ''
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            (category, period_type, start_date, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return float(row["total"] if row else 0)

    async def _get_period_parent_budget_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetPeriodGroupKey,
    ) -> dict[str, Any] | None:
        category, sub_category, period_type, start_date, user_id = group_key
        sub_condition = "sub_category = ?" if sub_category else "(sub_category IS NULL OR sub_category = '')"
        params: list[Any] = [category]
        if sub_category:
            params.append(sub_category)
        params.extend([period_type, start_date, user_id])

        async with conn.execute(
            f"""
            SELECT * FROM budgets
            WHERE category = ?
              AND {sub_condition}
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
            """,
            params,
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def _get_budget_period_child_amounts_by_start_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetPeriodGroupKey,
        child_period_type: str,
        period_end: str,
    ) -> dict[str, float]:
        category, sub_category, _period_type, start_date, user_id = group_key
        sub_condition = "sub_category = ?" if sub_category else "(sub_category IS NULL OR sub_category = '')"
        params: list[Any] = [category]
        if sub_category:
            params.append(sub_category)
        params.extend([child_period_type, start_date, period_end, user_id])

        async with conn.execute(
            f"""
            SELECT start_date, amount
            FROM budgets
            WHERE category = ?
              AND {sub_condition}
              AND period_type = ?
              AND start_date >= ?
              AND start_date <= ?
              AND user_id = ?
            """,
            params,
        ) as cursor:
            rows = await cursor.fetchall()

        child_amounts_by_start: dict[str, float] = {}
        for row in rows:
            child_start = str(row["start_date"] or "").strip()
            if not child_start:
                continue
            amount = float(row["amount"] or 0)
            child_amounts_by_start[child_start] = max(
                child_amounts_by_start.get(child_start, 0.0),
                amount,
            )
        return child_amounts_by_start

    async def _sum_budget_period_children_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetPeriodGroupKey,
        child_period_type: str,
        period_end: str,
    ) -> float:
        child_amounts_by_start = await self._get_budget_period_child_amounts_by_start_with_conn(
            conn,
            group_key,
            child_period_type,
            period_end,
        )
        return sum(child_amounts_by_start.values())

    async def _sum_yearly_budget_period_children_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetPeriodGroupKey,
        period_end: str,
    ) -> float:
        quarterly_amounts = await self._get_budget_period_child_amounts_by_start_with_conn(
            conn,
            group_key,
            "quarterly",
            period_end,
        )
        monthly_amounts = await self._get_budget_period_child_amounts_by_start_with_conn(
            conn,
            group_key,
            "monthly",
            period_end,
        )
        monthly_amounts_by_quarter: dict[str, float] = {}
        for monthly_start, amount in monthly_amounts.items():
            quarter_period = self._resolve_parent_budget_period("quarterly", monthly_start)
            if quarter_period is None:
                continue
            quarter_start, _quarter_end = quarter_period
            monthly_amounts_by_quarter[quarter_start] = (
                monthly_amounts_by_quarter.get(quarter_start, 0.0) + amount
            )

        quarter_starts = set(quarterly_amounts) | set(monthly_amounts_by_quarter)
        return sum(
            max(
                quarterly_amounts.get(quarter_start, 0.0),
                monthly_amounts_by_quarter.get(quarter_start, 0.0),
            )
            for quarter_start in quarter_starts
        )

    async def _get_period_child_budgets_total_with_conn(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetPeriodGroupKey,
        period_end: str,
    ) -> float:
        _category, _sub_category, period_type, _start_date, _user_id = group_key
        if period_type == "quarterly":
            return await self._sum_budget_period_children_with_conn(
                conn,
                group_key,
                "monthly",
                period_end,
            )
        if period_type == "yearly":
            return await self._sum_yearly_budget_period_children_with_conn(
                conn,
                group_key,
                period_end,
            )
        return 0.0

    async def _synchronize_period_parent_budget_for_group(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetPeriodGroupKey | None,
        period_end: str | None,
        reference_data: dict[str, Any] | None = None,
    ) -> None:
        if group_key is None or period_end is None:
            return

        category, sub_category, period_type, start_date, user_id = group_key
        child_total = await self._get_period_child_budgets_total_with_conn(
            conn,
            group_key,
            period_end,
        )
        if child_total <= 0:
            return

        parent_budget = await self._get_period_parent_budget_with_conn(conn, group_key)
        now = self._current_budget_timestamp_text()
        if not parent_budget:
            reference = reference_data or {}
            await self._insert_budget_record(
                conn,
                {
                    "name": reference.get("name", ""),
                    "category": category,
                    "sub_category": sub_category,
                    "period_type": period_type,
                    "amount": child_total,
                    "start_date": start_date,
                    "end_date": period_end,
                    "alert_threshold": reference.get("alert_threshold", 80),
                    "enabled": reference.get("enabled", 1),
                    "created_at": reference.get("created_at", now),
                    "updated_at": now,
                },
                user_id,
            )
            return

        current_amount = float(parent_budget.get("amount") or 0)
        next_amount = max(current_amount, child_total)
        expected_end_date = str((reference_data or {}).get("end_date") or "").strip()
        assignments = ["updated_at = ?"]
        values: list[Any] = [now]
        if current_amount < child_total:
            assignments.insert(0, "amount = ?")
            values.insert(0, next_amount)
        if expected_end_date and str(parent_budget.get("end_date") or "").strip() != expected_end_date:
            assignments.insert(-1, "end_date = ?")
            values.insert(-1, expected_end_date)
        if assignments == ["updated_at = ?"]:
            return

        values.extend([parent_budget["id"], user_id])
        await conn.execute(
            f"UPDATE budgets SET {', '.join(assignments)} WHERE id = ? AND user_id = ?",
            values,
        )

    async def _synchronize_budget_period_hierarchy(
        self,
        conn: aiosqlite.Connection,
        budget: dict[str, Any] | None,
        user_id: int,
    ) -> None:
        if not budget:
            return

        period_type = str(budget.get("period_type") or "")
        if period_type not in {"monthly", "quarterly"}:
            return

        parent_period_types = ["quarterly", "yearly"] if period_type == "monthly" else ["yearly"]
        for parent_period_type in parent_period_types:
            parent_period = self._resolve_parent_budget_period(
                parent_period_type,
                budget.get("start_date"),
            )
            if parent_period is None:
                continue

            parent_start, parent_end = parent_period
            parent_reference_data = {
                **budget,
                "period_type": parent_period_type,
                "start_date": parent_start,
                "end_date": parent_end,
            }
            parent_group_key = self._build_budget_period_group_key(
                budget.get("category"),
                budget.get("sub_category"),
                parent_period_type,
                parent_start,
                user_id,
            )
            await self._synchronize_period_parent_budget_for_group(
                conn,
                parent_group_key,
                parent_end,
                reference_data=parent_reference_data,
            )
            await self._synchronize_primary_budget_for_group(
                conn,
                self._build_budget_group_key(
                    budget.get("category"),
                    parent_period_type,
                    parent_start,
                    user_id,
                ),
                reference_data=parent_reference_data,
            )

    async def _synchronize_primary_budget_for_group(
        self,
        conn: aiosqlite.Connection,
        group_key: BudgetGroupKey | None,
        reference_data: dict[str, Any] | None = None,
    ) -> None:
        if group_key is None:
            return

        category, period_type, start_date, user_id = group_key
        sub_total = await self._get_sub_category_budgets_total_with_conn(conn, group_key)
        primary_budget = await self._get_primary_category_budget_with_conn(conn, group_key)
        if sub_total <= 0:
            return

        now = self._current_budget_timestamp_text()
        if not primary_budget:
            reference = reference_data or {}
            await self._insert_budget_record(
                conn,
                {
                    "name": reference.get("name", ""),
                    "category": category,
                    "sub_category": "",
                    "period_type": period_type,
                    "amount": sub_total,
                    "start_date": start_date,
                    "end_date": reference.get("end_date"),
                    "alert_threshold": reference.get("alert_threshold", 80),
                    "enabled": reference.get("enabled", 1),
                    "created_at": reference.get("created_at", now),
                    "updated_at": now,
                },
                user_id,
            )
            return

        current_amount = float(primary_budget.get("amount") or 0)
        next_amount = max(current_amount, sub_total)
        expected_end_date = str((reference_data or {}).get("end_date") or "").strip()
        assignments = ["updated_at = ?"]
        values: list[Any] = [now]
        if current_amount < sub_total:
            assignments.insert(0, "amount = ?")
            values.insert(0, next_amount)
        if expected_end_date and str(primary_budget.get("end_date") or "").strip() != expected_end_date:
            assignments.insert(-1, "end_date = ?")
            values.insert(-1, expected_end_date)
        if assignments == ["updated_at = ?"]:
            return

        values.extend([primary_budget["id"], user_id])
        await conn.execute(
            f"UPDATE budgets SET {', '.join(assignments)} WHERE id = ? AND user_id = ?",
            values,
        )
