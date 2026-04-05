"""Budget execution, history, forecast, and import/export helpers."""

# pylint: disable=missing-function-docstring,line-too-long,too-many-lines,wrong-import-position,too-many-arguments,too-many-positional-arguments,too-many-locals,broad-exception-caught,chained-comparison

from __future__ import annotations

import json
from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import aiosqlite

from ..utils.logger import log_method
from .db_budgets_core import DatabaseBudgetsCoreMixin
from .db_shared import BudgetExecutionRequest, BudgetForecastRequest


class DatabaseBudgetsReportingMixin(DatabaseBudgetsCoreMixin):
    """Budget execution, snapshot history, forecast, import, and export helpers."""

    async def _fetch_budget_execution_candidates(
        self,
        conn: aiosqlite.Connection,
        request: BudgetExecutionRequest,
        category_context: dict[str, Any],
    ) -> list[dict[str, Any]]:
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
            cat_info = await self.get_category_by_id(request.category_id, user_id=request.user_id)
            if not cat_info:
                return []
            budget_query += " AND category = ?"
            budget_params.append(cat_info["main_category"])
            normalized_sub_category = self._normalize_budget_sub_category(cat_info.get("sub_category"))
            if normalized_sub_category:
                budget_query += " AND sub_category = ?"
                budget_params.append(normalized_sub_category)

        async with conn.execute(budget_query, budget_params) as cursor:
            raw_budgets = [dict(row) for row in await cursor.fetchall()]
        return self._filter_budget_execution_candidates(raw_budgets, request, category_context)

    def _filter_budget_execution_candidates(
        self,
        raw_budgets: list[dict[str, Any]],
        request: BudgetExecutionRequest,
        category_context: dict[str, Any],
    ) -> list[dict[str, Any]]:
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

    def _resolve_budget_execution_window(
        self,
        budget: dict[str, Any],
        request: BudgetExecutionRequest,
    ) -> tuple[str | None, str | None]:
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
                f"source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders})"
                ")"
            )
            bill_params.extend(request.account_ids * 2)

        if request.tag_ids:
            placeholders = ",".join("?" * len(request.tag_ids))
            bill_query += f" AND id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))"
            bill_params.extend(request.tag_ids)

        return bill_query, bill_params

    async def _get_budget_spent_amount(
        self,
        conn: aiosqlite.Connection,
        budget: dict[str, Any],
        type_name: str,
        request: BudgetExecutionRequest,
    ) -> float:
        bill_query, bill_params = self._build_budget_spent_query(budget, type_name, request)
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
        budget_amount = budget.get("amount", 0)
        execution_rate = (spent / budget_amount * 100) if budget_amount > 0 else 0
        resolved_budget_type = int(budget.get("_resolved_budget_type") or fallback_budget_type)
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
            "category_id": str(category_info.get("id") or "") if category_info else "",
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
        conn = await self._get_connection()
        categories = await self.get_all_categories(user_id=request.user_id)
        category_context = self._build_budget_category_context(categories)
        budgets = await self._fetch_budget_execution_candidates(conn, request, category_context)
        type_name = self._get_budget_type_name(request.budget_type)

        results: list[dict[str, Any]] = []
        for budget in budgets:
            spent = await self._get_budget_spent_amount(conn, budget, type_name, request)
            results.append(self._build_budget_execution_item(budget, spent, category_context, request.budget_type))
        return results

    @log_method
    async def get_budget_execution_details(
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
        request = BudgetExecutionRequest(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=tuple(account_ids or ()),
            tag_ids=tuple(tag_ids or ()),
            user_id=user_id,
        )
        return await self._get_budget_execution_details_for_request(request)

    @staticmethod
    def _build_budget_history_filter_summary(
        budget_type: int | None = None,
        period_type: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
    ) -> str:
        summary = {
            "budget_type": int(budget_type) if budget_type else None,
            "period_type": period_type or "",
            "budget_id": int(budget_id) if budget_id else None,
            "category_id": int(category_id) if category_id else None,
            "account_ids": sorted(int(item) for item in (account_ids or [])),
            "tag_ids": sorted(int(item) for item in (tag_ids or [])),
        }
        return json.dumps(summary, ensure_ascii=False, sort_keys=True)

    @classmethod
    def _build_budget_history_filter_summary_for_request(
        cls,
        request: BudgetExecutionRequest,
    ) -> str:
        return cls._build_budget_history_filter_summary(
            budget_type=request.budget_type,
            period_type=request.period_type,
            budget_id=request.budget_id,
            category_id=request.category_id,
            account_ids=list(request.account_ids),
            tag_ids=list(request.tag_ids),
        )

    async def _create_budget_execution_snapshots_for_request(
        self,
        request: BudgetExecutionRequest,
    ) -> dict[str, Any]:
        snapshots = await self._get_budget_execution_details_for_request(request)
        conn = await self._get_connection()
        calculated_at = self._current_budget_timestamp_text()
        filter_summary = self._build_budget_history_filter_summary_for_request(request)

        created_count = 0
        for snapshot in snapshots:
            await conn.execute(
                """
                DELETE FROM budget_history
                WHERE user_id = ? AND budget_id = ? AND period_start = ? AND period_end = ?
                  AND filter_summary = ?
                """,
                (request.user_id, snapshot["id"], request.start_date, request.end_date, filter_summary),
            )
            status = "over_budget" if snapshot["spent_amount"] > snapshot["budget_amount"] else "within_budget"
            await conn.execute(
                """
                INSERT INTO budget_history (
                    user_id, budget_id, period_start, period_end,
                    budget_amount, spent_amount, remaining_amount,
                    execution_rate, status, filter_summary, calculated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    request.user_id,
                    snapshot["id"],
                    request.start_date,
                    request.end_date,
                    snapshot["budget_amount"],
                    snapshot["spent_amount"],
                    snapshot["remaining_amount"],
                    snapshot["execution_rate"],
                    status,
                    filter_summary,
                    calculated_at,
                ),
            )
            created_count += 1

        await conn.commit()
        return {
            "created_count": created_count,
            "period_start": request.start_date,
            "period_end": request.end_date,
            "filter_summary": filter_summary,
            "calculated_at": calculated_at,
        }

    @log_method
    async def create_budget_execution_snapshots(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str | None = None,
        end_date: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        request = BudgetExecutionRequest(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=tuple(account_ids or ()),
            tag_ids=tuple(tag_ids or ()),
            user_id=user_id,
        )
        return await self._create_budget_execution_snapshots_for_request(request)

    async def _fetch_budget_execution_history_items(
        self,
        conn: aiosqlite.Connection,
        request: BudgetExecutionRequest,
        filter_summary: str,
    ) -> list[dict[str, Any]]:
        query = """
            SELECT
                bh.id,
                bh.budget_id,
                bh.period_start,
                bh.period_end,
                bh.budget_amount,
                bh.spent_amount,
                bh.remaining_amount,
                bh.execution_rate,
                bh.status,
                bh.filter_summary,
                bh.calculated_at,
                b.name,
                b.category,
                b.sub_category,
                b.period_type,
                b.alert_threshold,
                b.enabled
            FROM budget_history bh
            INNER JOIN budgets b ON b.id = bh.budget_id
            WHERE bh.user_id = ? AND b.user_id = ?
        """
        params: list[Any] = [request.user_id, request.user_id]
        if request.budget_id:
            query += " AND bh.budget_id = ?"
            params.append(request.budget_id)
        if request.start_date:
            query += " AND bh.period_end >= ?"
            params.append(request.start_date)
        if request.end_date:
            query += " AND bh.period_start <= ?"
            params.append(request.end_date)
        query += " AND bh.filter_summary = ?"
        params.append(filter_summary)
        query += " ORDER BY bh.period_start DESC, bh.calculated_at DESC, bh.budget_id ASC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @staticmethod
    def _extract_exact_budget_history_items(
        history_items: list[dict[str, Any]],
        request: BudgetExecutionRequest,
    ) -> list[dict[str, Any]]:
        if not request.start_date or not request.end_date:
            return history_items
        return [
            item
            for item in history_items
            if item.get("period_start") == request.start_date and item.get("period_end") == request.end_date
        ]

    @staticmethod
    def _merge_budget_execution_history_items(
        history_items: list[dict[str, Any]],
        on_demand_items: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        if not history_items:
            return list(on_demand_items)
        history_keys = {
            (item.get("budget_id"), item.get("period_start"), item.get("period_end"))
            for item in history_items
        }
        merged_items = list(history_items)
        for item in on_demand_items:
            item_key = (item.get("budget_id"), item.get("period_start"), item.get("period_end"))
            if item_key not in history_keys:
                merged_items.append(item)
        return merged_items

    @staticmethod
    def _sort_budget_execution_history_items(history_items: list[dict[str, Any]]) -> list[dict[str, Any]]:
        return sorted(
            history_items,
            key=lambda item: (
                item.get("period_start", ""),
                item.get("period_end", ""),
                str(item.get("category", "")),
                str(item.get("sub_category", "")),
                int(item.get("budget_id", 0) or 0),
            ),
            reverse=True,
        )

    async def _get_budget_execution_history_for_request(
        self,
        request: BudgetExecutionRequest,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        filter_summary = self._build_budget_history_filter_summary_for_request(request)
        history_items = await self._fetch_budget_execution_history_items(conn, request, filter_summary)
        if not request.start_date or not request.end_date:
            return history_items

        exact_history_items = self._extract_exact_budget_history_items(history_items, request)
        if exact_history_items:
            return exact_history_items

        on_demand_items = await self._build_budget_execution_history_on_demand_for_request(request)
        if not history_items:
            return on_demand_items

        merged_items = self._merge_budget_execution_history_items(history_items, on_demand_items)
        return self._sort_budget_execution_history_items(merged_items)

    @log_method
    async def get_budget_execution_history(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str | None = None,
        end_date: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        request = BudgetExecutionRequest(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=tuple(account_ids or ()),
            tag_ids=tuple(tag_ids or ()),
            user_id=user_id,
        )
        return await self._get_budget_execution_history_for_request(request)

    @classmethod
    def _iter_budget_history_period_ranges(
        cls,
        period_type: str,
        start_date: str | None,
        end_date: str | None,
    ) -> list[dict[str, str]]:
        start = cls._parse_budget_history_date(start_date)
        end = cls._parse_budget_history_date(end_date)
        if not start or not end or start > end:
            return []

        period_ranges: list[dict[str, str]] = []
        if period_type == "daily":
            current_start = start
            while current_start <= end:
                period_ranges.append(
                    {
                        "start_date": current_start.strftime("%Y-%m-%d"),
                        "end_date": current_start.strftime("%Y-%m-%d"),
                    }
                )
                current_start += timedelta(days=1)
            return period_ranges

        if period_type == "weekly":
            current_start = start - timedelta(days=start.weekday())
            while current_start <= end:
                current_end = current_start + timedelta(days=6)
                period_ranges.append(
                    {
                        "start_date": current_start.strftime("%Y-%m-%d"),
                        "end_date": current_end.strftime("%Y-%m-%d"),
                    }
                )
                current_start += timedelta(days=7)
            return period_ranges

        if period_type == "yearly":
            current_start = date(start.year, 1, 1)
            while current_start <= end:
                next_start = date(current_start.year + 1, 1, 1)
                current_end = next_start - timedelta(days=1)
                period_ranges.append(
                    {
                        "start_date": current_start.strftime("%Y-%m-%d"),
                        "end_date": current_end.strftime("%Y-%m-%d"),
                    }
                )
                current_start = next_start
            return period_ranges

        if period_type == "quarterly":
            quarter_start_month = ((start.month - 1) // 3) * 3 + 1
            current_start = date(start.year, quarter_start_month, 1)
            while current_start <= end:
                next_start = cls._add_months(current_start, 3)
                current_end = next_start - timedelta(days=1)
                period_ranges.append(
                    {
                        "start_date": current_start.strftime("%Y-%m-%d"),
                        "end_date": current_end.strftime("%Y-%m-%d"),
                    }
                )
                current_start = next_start
            return period_ranges

        current_start = date(start.year, start.month, 1)
        while current_start <= end:
            next_start = cls._add_months(current_start, 1)
            current_end = next_start - timedelta(days=1)
            period_ranges.append(
                {
                    "start_date": current_start.strftime("%Y-%m-%d"),
                    "end_date": current_end.strftime("%Y-%m-%d"),
                }
            )
            current_start = next_start
        return period_ranges

    @classmethod
    def _budget_overlaps_period(
        cls,
        budget_start: str | None,
        budget_end: str | None,
        period_start: str,
        period_end: str,
    ) -> bool:
        parsed_budget_start = cls._parse_budget_history_date(budget_start)
        parsed_budget_end = cls._parse_budget_history_date(budget_end)
        parsed_period_start = cls._parse_budget_history_date(period_start)
        parsed_period_end = cls._parse_budget_history_date(period_end)
        if not parsed_period_start or not parsed_period_end:
            return False
        if parsed_budget_start and parsed_budget_start > parsed_period_end:
            return False
        if parsed_budget_end and parsed_budget_end < parsed_period_start:
            return False
        return True

    @staticmethod
    def _build_budget_history_item_from_detail(
        detail: dict[str, Any],
        period_range: dict[str, str],
        request: BudgetExecutionRequest,
        filter_summary: str,
    ) -> dict[str, Any]:
        period_start = period_range["start_date"]
        period_end = period_range["end_date"]
        spent_amount = detail.get("spent_amount", 0)
        budget_amount = detail.get("budget_amount", 0)
        return {
            "id": f"{detail.get('id', '')}_{period_start}_{period_end}",
            "budget_id": detail.get("id"),
            "period_start": period_start,
            "period_end": period_end,
            "budget_amount": budget_amount,
            "spent_amount": spent_amount,
            "remaining_amount": detail.get("remaining_amount", 0),
            "execution_rate": detail.get("execution_rate", 0),
            "status": "over_budget" if spent_amount > budget_amount else "within_budget",
            "filter_summary": filter_summary,
            "calculated_at": "",
            "name": detail.get("name", ""),
            "category": detail.get("category", ""),
            "sub_category": detail.get("sub_category", ""),
            "category_id": detail.get("category_id", ""),
            "category_info": detail.get("category_info"),
            "type": detail.get("type", request.budget_type),
            "period_type": detail.get("period_type", request.period_type or "monthly"),
            "alert_threshold": detail.get("alert_threshold", 80),
            "enabled": detail.get("enabled", 1),
        }

    async def _build_budget_execution_history_on_demand_for_request(
        self,
        request: BudgetExecutionRequest,
    ) -> list[dict[str, Any]]:
        period_ranges = self._iter_budget_history_period_ranges(
            request.period_type or "monthly",
            request.start_date,
            request.end_date,
        )
        if not period_ranges:
            return []

        filter_summary = self._build_budget_history_filter_summary_for_request(request)
        history_items: list[dict[str, Any]] = []
        for period_range in period_ranges:
            period_request = BudgetExecutionRequest(
                budget_type=request.budget_type,
                period_type=request.period_type,
                start_date=period_range["start_date"],
                end_date=period_range["end_date"],
                budget_id=request.budget_id,
                category_id=request.category_id,
                account_ids=request.account_ids,
                tag_ids=request.tag_ids,
                user_id=request.user_id,
            )
            execution_details = await self._get_budget_execution_details_for_request(period_request)
            for detail in execution_details:
                if not self._budget_overlaps_period(
                    detail.get("start_date"),
                    detail.get("end_date"),
                    period_range["start_date"],
                    period_range["end_date"],
                ):
                    continue
                history_items.append(
                    self._build_budget_history_item_from_detail(detail, period_range, request, filter_summary)
                )

        return self._sort_budget_execution_history_items(history_items)

    @log_method
    async def _build_budget_execution_history_on_demand(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str | None = None,
        end_date: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        request = BudgetExecutionRequest(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=tuple(account_ids or ()),
            tag_ids=tuple(tag_ids or ()),
            user_id=user_id,
        )
        return await self._build_budget_execution_history_on_demand_for_request(request)

    @staticmethod
    def _build_budget_forecast_group_by(period_type: str) -> str:
        if period_type == "daily":
            return "date(date)"
        if period_type == "weekly":
            return "strftime('%Y-%W', date)"
        if period_type == "quarterly":
            return (
                "strftime('%Y', date) || '-Q' || "
                "(CAST(((CAST(strftime('%m', date) AS INTEGER) - 1) / 3) AS INTEGER) + 1)"
            )
        if period_type == "monthly":
            return "strftime('%Y-%m', date)"
        return "strftime('%Y', date)"

    async def _fetch_budget_forecast_rows(
        self,
        conn: aiosqlite.Connection,
        request: BudgetForecastRequest,
    ) -> list[dict[str, Any]]:
        history_start_date, history_end_date = self._expand_forecast_history_window(
            request.period_type,
            request.start_date,
            request.end_date,
            request.history_periods,
        )
        group_by = self._build_budget_forecast_group_by(request.period_type)
        query = f"""
            SELECT
                {group_by} as period,
                main_category,
                COALESCE(SUM(amount), 0) as total_amount,
                COUNT(*) as transaction_count
            FROM bills
            WHERE type = ? AND user_id = ?
        """
        params: list[Any] = [self._get_budget_type_name(request.budget_type), request.user_id]
        if history_start_date:
            query += " AND date >= ?"
            params.append(history_start_date)
        if history_end_date:
            query += " AND date <= ?"
            params.append(self._normalize_budget_query_end_date(history_end_date))
        query += f" GROUP BY {group_by}, main_category ORDER BY period DESC, total_amount DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    @staticmethod
    def _aggregate_budget_forecast_rows(
        rows: list[dict[str, Any]],
    ) -> tuple[dict[str, dict[str, Any]], int]:
        category_totals: dict[str, dict[str, Any]] = {}
        period_keys: set[str] = set()
        for row in rows:
            period = row["period"]
            category = row["main_category"] or "未分类"
            amount = abs(row["total_amount"])
            period_keys.add(period)
            bucket = category_totals.setdefault(category, {"total": 0.0, "periods": []})
            bucket["total"] += amount
            bucket["periods"].append({"period": period, "amount": amount})
        return category_totals, len(period_keys) if period_keys else 1

    def _build_budget_forecast_budget_map(
        self,
        budget_rows: list[dict[str, Any]],
        category_context: dict[str, Any],
        budget_type: int,
    ) -> dict[str, dict[str, float]]:
        budget_map: dict[str, dict[str, float]] = {}
        for row in budget_rows:
            category_name = row["category"] or "未分类"
            resolved_budget_type = self._resolve_budget_category_type(
                category_name,
                row["sub_category"],
                category_context,
                preferred_type=budget_type,
            )
            if int(resolved_budget_type or 0) != int(budget_type):
                continue
            bucket = budget_map.setdefault(category_name, {"primary": 0.0, "sub_total": 0.0})
            amount_value = float(row["amount"] or 0)
            if not row["sub_category"]:
                bucket["primary"] += amount_value
            else:
                bucket["sub_total"] += amount_value
        return budget_map

    async def _fetch_budget_forecast_budget_map(
        self,
        conn: aiosqlite.Connection,
        request: BudgetForecastRequest,
        category_context: dict[str, Any],
    ) -> dict[str, dict[str, float]]:
        budget_query = """
            SELECT category, sub_category, amount
            FROM budgets
            WHERE period_type = ? AND enabled = 1 AND user_id = ?
        """
        budget_params: list[Any] = [request.period_type, request.user_id]
        if request.start_date:
            budget_query += " AND (end_date IS NULL OR end_date = '' OR end_date >= ?)"
            budget_params.append(request.start_date)
        if request.end_date:
            budget_query += " AND (start_date IS NULL OR start_date = '' OR start_date <= ?)"
            budget_params.append(request.end_date)

        async with conn.execute(budget_query, budget_params) as cursor:
            budget_rows = [dict(row) for row in await cursor.fetchall()]
        return self._build_budget_forecast_budget_map(budget_rows, category_context, request.budget_type)

    @staticmethod
    def _normalize_forecast_strategy(strategy: str) -> str:
        return strategy or "historical_average"

    @staticmethod
    def _normalize_forecast_history_periods(history_periods: int) -> int:
        return max(int(history_periods or 0), 1)

    @staticmethod
    def _calculate_forecast_amount(
        recent_amounts: list[float],
        normalized_strategy: str,
    ) -> tuple[float, float, str]:
        average_amount = sum(recent_amounts) / len(recent_amounts)
        moving_window_size = min(3, len(recent_amounts))
        moving_average_amount = (
            sum(recent_amounts[-moving_window_size:]) / moving_window_size
            if moving_window_size > 0
            else 0
        )
        if normalized_strategy == "moving_average":
            return average_amount, moving_average_amount, f"基于最近{moving_window_size}个周期的移动平均"
        return average_amount, average_amount, f"基于最近{len(recent_amounts)}个周期的历史均值"

    @classmethod
    def _calculate_forecast_backtest_mape(
        cls,
        recent_amounts: list[float],
        normalized_strategy: str,
    ) -> float | None:
        backtest_errors: list[float] = []
        for index in range(1, len(recent_amounts)):
            actual_amount = recent_amounts[index]
            if actual_amount <= 0:
                continue
            history_slice = recent_amounts[:index]
            if normalized_strategy == "moving_average":
                window_size = min(3, len(history_slice))
                predicted_amount = sum(history_slice[-window_size:]) / window_size if window_size > 0 else 0
            else:
                predicted_amount = sum(history_slice) / len(history_slice)
            backtest_errors.append(abs(actual_amount - predicted_amount) / actual_amount)
        if not backtest_errors:
            return None
        return round((sum(backtest_errors) / len(backtest_errors)) * 100, 2)

    @staticmethod
    def _resolve_forecast_confidence(backtest_mape: float | None) -> str:
        if backtest_mape is None:
            return "low"
        if backtest_mape <= 10:
            return "high"
        if backtest_mape <= 20:
            return "medium"
        return "low"

    @staticmethod
    def _resolve_forecast_trend(recent_amounts: list[float]) -> str:
        if len(recent_amounts) < 2:
            return "stable"
        latest_amount = recent_amounts[-1]
        baseline_avg = sum(recent_amounts[:-1]) / len(recent_amounts[:-1])
        if baseline_avg > 0 and latest_amount > baseline_avg * 1.05:
            return "up"
        if baseline_avg > 0 and latest_amount < baseline_avg * 0.95:
            return "down"
        return "stable"

    @staticmethod
    def _resolve_forecast_budget_amount(
        budget_map: dict[str, dict[str, float]],
        category: str,
    ) -> float:
        if category not in budget_map:
            return 0.0
        budget_info = budget_map[category]
        return budget_info["primary"] if budget_info["primary"] > 0 else budget_info["sub_total"]

    def _build_budget_forecast_item(
        self,
        category: str,
        data: dict[str, Any],
        budget_map: dict[str, dict[str, float]],
        category_context: dict[str, Any],
        request: BudgetForecastRequest,
        normalized_strategy: str,
        normalized_history_periods: int,
        target_period_key: str | None,
        num_periods: int,
    ) -> dict[str, Any] | None:
        sorted_periods = sorted(data["periods"], key=lambda item: item["period"])
        recent_periods = sorted_periods[-normalized_history_periods:]
        if not recent_periods:
            return None

        recent_amounts = [float(item["amount"]) for item in recent_periods]
        average_amount, forecast_amount, strategy_explanation = self._calculate_forecast_amount(
            recent_amounts,
            normalized_strategy,
        )
        backtest_mape = self._calculate_forecast_backtest_mape(recent_amounts, normalized_strategy)
        confidence = self._resolve_forecast_confidence(backtest_mape)
        trend = self._resolve_forecast_trend(recent_amounts)
        current_period_amounts = {item["period"]: item["amount"] for item in sorted_periods}
        current_spent = current_period_amounts.get(target_period_key, 0.0)
        budget_amount = self._resolve_forecast_budget_amount(budget_map, category)
        category_info = self._resolve_budget_category_info(category, "", category_context, request.budget_type)
        return {
            "category": category,
            "category_info": category_info,
            "total_amount": round(sum(recent_amounts), 2),
            "average_amount": round(average_amount, 2),
            "period_count": num_periods,
            "sample_periods": len(recent_amounts),
            "current_spent": round(current_spent, 2),
            "budget_amount": round(budget_amount, 2),
            "forecast_amount": round(forecast_amount, 2),
            "projected_over_budget": budget_amount > 0 and forecast_amount > budget_amount,
            "forecast_strategy": normalized_strategy,
            "strategy_explanation": strategy_explanation,
            "backtest_mape": backtest_mape,
            "confidence": confidence,
            "trend": trend,
            "periods": recent_periods,
        }

    async def _get_period_forecast_for_request(
        self,
        request: BudgetForecastRequest,
    ) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        rows = await self._fetch_budget_forecast_rows(conn, request)
        normalized_strategy = self._normalize_forecast_strategy(request.forecast_strategy)
        normalized_history_periods = self._normalize_forecast_history_periods(request.history_periods)
        target_period_key = self._build_forecast_period_key(request.period_type, request.start_date)
        category_totals, num_periods = self._aggregate_budget_forecast_rows(rows)

        categories = await self.get_all_categories(user_id=request.user_id)
        category_context = self._build_budget_category_context(categories)
        budget_map = await self._fetch_budget_forecast_budget_map(conn, request, category_context)

        results: list[dict[str, Any]] = []
        for category, data in category_totals.items():
            forecast_item = self._build_budget_forecast_item(
                category,
                data,
                budget_map,
                category_context,
                request,
                normalized_strategy,
                normalized_history_periods,
                target_period_key,
                num_periods,
            )
            if forecast_item is not None:
                results.append(forecast_item)
        results.sort(key=lambda item: item["forecast_amount"], reverse=True)
        return results

    @log_method
    async def get_period_forecast(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str | None = None,
        end_date: str | None = None,
        forecast_strategy: str = "historical_average",
        history_periods: int = 6,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        request = BudgetForecastRequest(
            budget_type=budget_type,
            period_type=period_type,
            start_date=start_date,
            end_date=end_date,
            forecast_strategy=forecast_strategy,
            history_periods=history_periods,
            user_id=user_id,
        )
        return await self._get_period_forecast_for_request(request)

    @log_method
    async def import_budgets(self, budgets_data: list[dict[str, Any]], user_id: int = 1) -> dict[str, Any]:
        conn = await self._get_connection()
        now = self._current_budget_timestamp_text()
        created_count = 0
        updated_count = 0
        error_count = 0
        errors: list[str] = []

        for index, data in enumerate(budgets_data, start=1):
            try:
                if not data.get("name") or not data.get("amount"):
                    errors.append(f"第{index}条: 缺少必填字段(name或amount)")
                    error_count += 1
                    continue

                async with conn.execute(
                    "SELECT id FROM budgets WHERE name = ? AND user_id = ?",
                    (data["name"], user_id),
                ) as cursor:
                    existing = await cursor.fetchone()

                if existing:
                    await conn.execute(
                        """
                        UPDATE budgets SET
                            category = ?,
                            sub_category = ?,
                            period_type = ?,
                            amount = ?,
                            start_date = ?,
                            end_date = ?,
                            alert_threshold = ?,
                            enabled = ?,
                            updated_at = ?
                        WHERE id = ? AND user_id = ?
                        """,
                        (
                            data.get("category"),
                            data.get("sub_category"),
                            data.get("period_type", "monthly"),
                            data["amount"],
                            data.get("start_date"),
                            data.get("end_date"),
                            data.get("alert_threshold", 80),
                            data.get("enabled", 1),
                            now,
                            existing["id"],
                            user_id,
                        ),
                    )
                    updated_count += 1
                else:
                    await conn.execute(
                        """
                        INSERT INTO budgets (
                            name, category, sub_category, period_type, amount,
                            start_date, end_date, alert_threshold, enabled,
                            created_at, updated_at, user_id
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            data["name"],
                            data.get("category"),
                            data.get("sub_category"),
                            data.get("period_type", "monthly"),
                            data["amount"],
                            data.get("start_date"),
                            data.get("end_date"),
                            data.get("alert_threshold", 80),
                            data.get("enabled", 1),
                            now,
                            now,
                            user_id,
                        ),
                    )
                    created_count += 1
            except Exception as exc:  # pragma: no cover - defensive logging branch
                errors.append(f"第{index}条: {exc!s}")
                error_count += 1
                self.logger.error("导入预算失败: %s", exc)

        await conn.commit()
        return {
            "created": created_count,
            "updated": updated_count,
            "errors": error_count,
            "error_details": errors,
        }

    @log_method
    async def export_budgets(self, user_id: int = 1) -> list[dict[str, Any]]:
        conn = await self._get_connection()
        export_fields = [
            "name",
            "category",
            "sub_category",
            "period_type",
            "amount",
            "start_date",
            "end_date",
            "alert_threshold",
            "enabled",
        ]
        async with conn.execute("SELECT * FROM budgets WHERE user_id = ? ORDER BY created_at", (user_id,)) as cursor:
            rows = await cursor.fetchall()
        budgets: list[dict[str, Any]] = []
        for row in rows:
            budgets.append({field: row[field] for field in export_fields if field in row.keys()})
        return budgets
