"""Budget execution detail helpers."""

from __future__ import annotations

# pylint: disable=duplicate-code,line-too-long,unused-import,too-many-locals,too-many-arguments,too-many-positional-arguments

import json
from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import BudgetExecutionRequest

if TYPE_CHECKING:
    import aiosqlite


class BudgetExecutionDetailsMixin:
    @staticmethod
    def _coerce_budget_execution_request(
        request: BudgetExecutionRequest | None = None,
        **kwargs: Any,
    ) -> BudgetExecutionRequest:
        """Build a request bundle from explicit kwargs while preserving legacy call sites."""
        if request is not None:
            return request
        return BudgetExecutionRequest(
            budget_type=int(kwargs.get("budget_type", 3) or 3),
            period_type=kwargs.get("period_type"),
            start_date=kwargs.get("start_date"),
            end_date=kwargs.get("end_date"),
            budget_id=kwargs.get("budget_id"),
            category_id=kwargs.get("category_id"),
            account_ids=tuple(kwargs.get("account_ids") or ()),
            tag_ids=tuple(kwargs.get("tag_ids") or ()),
            user_id=int(kwargs.get("user_id", 1) or 1),
        )

    async def _fetch_budget_execution_candidates(
        self,
        conn: aiosqlite.Connection,
        request: BudgetExecutionRequest,
        category_context: dict[str, Any],
    ) -> list[dict[str, Any]]:
        """Fetch candidate budgets and filter them against the resolved request scope."""
        budget_query = "SELECT * FROM budgets WHERE enabled = 1 AND user_id = ?"
        budget_params: list[Any] = [request.user_id]

        if request.budget_type in {3, 5}:
            budget_query += " AND (period_type IS NOT NULL)"
        if request.period_type:
            budget_query += " AND period_type = ?"
            budget_params.append(request.period_type)
        if request.budget_id:
            budget_query += " AND id = ?"
            budget_params.append(request.budget_id)
        if request.category_id:
            category_info = await self.get_category_by_id(
                request.category_id,
                user_id=request.user_id,
            )
            if not category_info:
                return []
            budget_query += " AND category = ?"
            budget_params.append(category_info["main_category"])
            normalized_sub_category = self._normalize_budget_sub_category(
                category_info.get("sub_category"),
            )
            if normalized_sub_category:
                budget_query += " AND sub_category = ?"
                budget_params.append(normalized_sub_category)

        async with conn.execute(budget_query, budget_params) as cursor:
            raw_budgets = [dict(row) for row in await cursor.fetchall()]
        filtered_budgets = self._filter_budget_execution_candidates(
            raw_budgets,
            request,
            category_context,
        )
        return self._dedupe_budget_execution_candidates(filtered_budgets)

    def _filter_budget_execution_candidates(
        self,
        raw_budgets: list[dict[str, Any]],
        request: BudgetExecutionRequest,
        category_context: dict[str, Any],
    ) -> list[dict[str, Any]]:
        """Filter raw budgets by resolved category type and period overlap."""
        budgets: list[dict[str, Any]] = []
        for budget in raw_budgets:
            resolved_budget_type = self._resolve_budget_category_type(
                budget.get("category"),
                budget.get("sub_category"),
                category_context,
                preferred_type=request.budget_type,
            )
            if resolved_budget_type != request.budget_type:
                continue
            if request.start_date and request.end_date and not self._budget_overlaps_period(
                budget.get("start_date"),
                budget.get("end_date"),
                request.start_date,
                request.end_date,
            ):
                continue
            budgets.append({**budget, "_resolved_budget_type": resolved_budget_type})
        return budgets

    @staticmethod
    def _dedupe_budget_execution_candidates(
        budgets: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        """Collapse synchronized parent duplicates for one exact period/category key."""
        selected: dict[tuple[str, str, str, str, int], dict[str, Any]] = {}
        order: list[tuple[str, str, str, str, int]] = []
        for budget in budgets:
            category = str(budget.get("category") or "").strip()
            sub_category = str(budget.get("sub_category") or "").strip()
            period_type = str(budget.get("period_type") or "").strip()
            start_date = str(budget.get("start_date") or "").strip()
            if category and period_type and start_date:
                key = (category, sub_category, period_type, start_date, 0)
            else:
                key = (category, sub_category, period_type, start_date, int(budget.get("id") or 0))
            current = selected.get(key)
            if current is None:
                selected[key] = budget
                order.append(key)
                continue
            budget_rank = (float(budget.get("amount") or 0), int(budget.get("id") or 0))
            current_rank = (
                float(current.get("amount") or 0),
                int(current.get("id") or 0),
            )
            if budget_rank >= current_rank:
                selected[key] = budget
        return [selected[key] for key in order]

    def _resolve_budget_execution_window(
        self,
        budget: dict[str, Any],
        request: BudgetExecutionRequest,
    ) -> tuple[str | None, str | None]:
        """Resolve the effective bill-query date window for a single budget."""
        budget_defined_start = str(budget.get("start_date") or "").strip() or None
        budget_defined_end = str(budget.get("end_date") or "").strip() or None
        budget_start = request.start_date or budget_defined_start
        budget_end = request.end_date or budget_defined_end
        if request.start_date and budget_defined_start:
            budget_start = max(request.start_date, budget_defined_start)
        if request.end_date and budget_defined_end:
            budget_end = min(request.end_date, budget_defined_end)
        return budget_start, self._normalize_budget_query_end_date(budget_end)

    def _build_budget_spent_query(
        self,
        budget: dict[str, Any],
        type_name: str,
        request: BudgetExecutionRequest,
    ) -> tuple[str, list[Any]]:
        """Build the aggregate spent-amount query for one budget execution row."""
        bill_query = """
            SELECT COALESCE(SUM(amount), 0) as spent
            FROM bills
            WHERE type = ? AND user_id = ?
        """
        bill_params: list[Any] = [type_name, request.user_id]

        if budget.get("category"):
            bill_query += " AND main_category = ?"
            bill_params.append(budget["category"])
        if budget.get("sub_category"):
            bill_query += " AND sub_category = ?"
            bill_params.append(budget["sub_category"])

        budget_start, budget_end = self._resolve_budget_execution_window(budget, request)
        if budget_start:
            bill_query += " AND date >= ?"
            bill_params.append(budget_start)
        if budget_end:
            bill_query += " AND date <= ?"
            bill_params.append(budget_end)

        if request.account_ids:
            placeholders = ",".join("?" * len(request.account_ids))
            bill_query += (
                " AND ("
                f"source_account_id IN ({placeholders}) "
                f"OR destination_account_id IN ({placeholders})"
                ")"
            )
            bill_params.extend(request.account_ids * 2)

        if request.tag_ids:
            placeholders = ",".join("?" * len(request.tag_ids))
            bill_query += (
                " AND id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN "
                f"({placeholders}))"
            )
            bill_params.extend(request.tag_ids)

        return bill_query, bill_params

    async def _get_budget_spent_amount(
        self,
        conn: aiosqlite.Connection,
        budget: dict[str, Any],
        type_name: str,
        request: BudgetExecutionRequest,
    ) -> float:
        """Execute the spent-amount query for one budget item."""
        bill_query, bill_params = self._build_budget_spent_query(
            budget,
            type_name,
            request,
        )
        async with conn.execute(bill_query, bill_params) as cursor:
            row = await cursor.fetchone()
        return abs(row["spent"]) if row else 0

    def _build_budget_execution_item(
        self,
        budget: dict[str, Any],
        spent: float,
        category_context: dict[str, Any],
        fallback_budget_type: int,
    ) -> dict[str, Any]:
        """Build the API/CLI execution row for one resolved budget."""
        budget_amount = budget.get("amount", 0)
        execution_rate = (spent / budget_amount * 100) if budget_amount > 0 else 0
        resolved_budget_type = int(
            budget.get("_resolved_budget_type") or fallback_budget_type
        )
        category_info = self._resolve_budget_category_info(
            budget.get("category"),
            budget.get("sub_category"),
            category_context,
            resolved_budget_type,
        )
        return {
            "id": budget["id"],
            "name": budget["name"],
            "category": budget.get("category", ""),
            "sub_category": budget.get("sub_category", ""),
            "category_info": category_info,
            "category_id": (
                str(category_info.get("id") or "") if category_info else ""
            ),
            "period_type": budget.get("period_type", "monthly"),
            "budget_amount": budget_amount,
            "spent_amount": spent,
            "remaining_amount": budget_amount - spent,
            "execution_rate": round(execution_rate, 2),
            "type": resolved_budget_type,
            "alert_threshold": budget.get("alert_threshold", 80),
            "start_date": budget.get("start_date"),
            "end_date": budget.get("end_date"),
            "enabled": budget.get("enabled", 1),
        }

    async def _get_budget_execution_details_for_request(
        self,
        request: BudgetExecutionRequest,
    ) -> list[dict[str, Any]]:
        """Resolve execution rows for the provided request bundle."""
        conn = await self._get_connection()
        categories = await self.get_all_categories(user_id=request.user_id)
        category_context = self._build_budget_category_context(categories)
        budgets = await self._fetch_budget_execution_candidates(conn, request, category_context)
        type_name = self._get_budget_type_name(request.budget_type)

        results: list[dict[str, Any]] = []
        for budget in budgets:
            spent = await self._get_budget_spent_amount(conn, budget, type_name, request)
            results.append(
                self._build_budget_execution_item(
                    budget,
                    spent,
                    category_context,
                    request.budget_type,
                )
            )
        return results

    @log_method
    async def get_budget_execution_details(  # pylint: disable=too-many-arguments,too-many-positional-arguments
        self,
        budget_type: int = 3,
        period_type: str | None = None,
        start_date: str | None = None,
        end_date: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """Public execution-details API using the immutable request bundle."""
        resolved_request = self._coerce_budget_execution_request(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=account_ids,
            tag_ids=tag_ids,
            user_id=user_id,
        )
        return await self._get_budget_execution_details_for_request(resolved_request)
