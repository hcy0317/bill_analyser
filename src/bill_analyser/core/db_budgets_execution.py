"""Budget execution, snapshot, and history helpers for the split database facade."""

# pylint: disable=duplicate-code

from __future__ import annotations

import json
from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

from ..utils.logger import log_method
from .db_budgets_core import DatabaseBudgetsCoreMixin
from .db_shared import BudgetExecutionRequest

if TYPE_CHECKING:
    import aiosqlite


class DatabaseBudgetExecutionHistoryMixin(DatabaseBudgetsCoreMixin):
    """Budget execution, snapshot history, and on-demand history helpers."""

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

    @classmethod
    def _build_budget_history_filter_summary(
        cls,
        request: BudgetExecutionRequest | None = None,
        **kwargs: Any,
    ) -> str:
        """Build a stable JSON signature for budget history filters."""
        resolved_request = cls._coerce_budget_execution_request(request, **kwargs)
        summary = {
            "budget_type": resolved_request.budget_type,
            "period_type": resolved_request.period_type or "",
            "budget_id": resolved_request.budget_id,
            "category_id": resolved_request.category_id,
            "account_ids": sorted(int(item) for item in resolved_request.account_ids),
            "tag_ids": sorted(int(item) for item in resolved_request.tag_ids),
        }
        return json.dumps(summary, ensure_ascii=False, sort_keys=True)

    @classmethod
    def _build_budget_history_filter_summary_for_request(
        cls,
        request: BudgetExecutionRequest,
    ) -> str:
        """Build the canonical history filter summary for one request bundle."""
        return cls._build_budget_history_filter_summary(request=request)

    async def _create_budget_execution_snapshots_for_request(
        self,
        request: BudgetExecutionRequest,
    ) -> dict[str, Any]:
        """Persist snapshot rows for the resolved execution result set."""
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
                (
                    request.user_id,
                    snapshot["id"],
                    request.start_date,
                    request.end_date,
                    filter_summary,
                ),
            )
            status = (
                "over_budget"
                if snapshot["spent_amount"] > snapshot["budget_amount"]
                else "within_budget"
            )
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
    async def create_budget_execution_snapshots(  # pylint: disable=too-many-arguments,too-many-positional-arguments
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
        """Public snapshot-creation API using the immutable request bundle."""
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
        return await self._create_budget_execution_snapshots_for_request(resolved_request)

    async def _fetch_budget_execution_history_items(
        self,
        conn: aiosqlite.Connection,
        request: BudgetExecutionRequest,
        filter_summary: str,
    ) -> list[dict[str, Any]]:
        """Fetch persisted history rows for the given filter summary."""
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
        query += (
            " ORDER BY bh.period_start DESC, bh.calculated_at DESC, bh.budget_id ASC"
        )

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    def _enrich_budget_execution_history_items(
        self,
        history_items: list[dict[str, Any]],
        request: BudgetExecutionRequest,
        category_context: dict[str, Any],
    ) -> list[dict[str, Any]]:
        """Attach the same category/type contract used by live execution rows."""
        enriched_items: list[dict[str, Any]] = []
        for item in history_items:
            resolved_budget_type = self._resolve_budget_category_type(
                item.get("category"),
                item.get("sub_category"),
                category_context,
                preferred_type=request.budget_type,
            )
            if resolved_budget_type != request.budget_type:
                continue

            category_info = self._resolve_budget_category_info(
                item.get("category"),
                item.get("sub_category"),
                category_context,
                resolved_budget_type,
            )
            enriched_items.append(
                {
                    **item,
                    "type": resolved_budget_type,
                    "category_info": category_info,
                    "category_id": (
                        str(category_info.get("id") or "") if category_info else ""
                    ),
                }
            )
        return enriched_items

    @staticmethod
    def _extract_exact_budget_history_items(
        history_items: list[dict[str, Any]],
        request: BudgetExecutionRequest,
    ) -> list[dict[str, Any]]:
        """Return only exact period matches when an explicit date window is supplied."""
        if not request.start_date or not request.end_date:
            return history_items
        return [
            item
            for item in history_items
            if item.get("period_start") == request.start_date
            and item.get("period_end") == request.end_date
        ]

    @staticmethod
    def _merge_budget_execution_history_items(
        history_items: list[dict[str, Any]],
        on_demand_items: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        """Merge persisted and on-demand history rows without duplicating the same budget window."""
        if not history_items:
            return list(on_demand_items)
        history_keys = {
            (item.get("budget_id"), item.get("period_start"), item.get("period_end"))
            for item in history_items
        }
        merged_items = list(history_items)
        for item in on_demand_items:
            item_key = (
                item.get("budget_id"),
                item.get("period_start"),
                item.get("period_end"),
            )
            if item_key not in history_keys:
                merged_items.append(item)
        return merged_items

    @staticmethod
    def _sort_budget_execution_history_items(
        history_items: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        """Sort merged history rows in descending period/category order."""
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
        """Resolve execution history from snapshots with on-demand fallback."""
        conn = await self._get_connection()
        categories = await self.get_all_categories(user_id=request.user_id)
        category_context = self._build_budget_category_context(categories)
        filter_summary = self._build_budget_history_filter_summary_for_request(request)
        history_items = self._enrich_budget_execution_history_items(
            await self._fetch_budget_execution_history_items(
                conn,
                request,
                filter_summary,
            ),
            request,
            category_context,
        )
        if not request.start_date or not request.end_date:
            return history_items

        exact_history_items = self._extract_exact_budget_history_items(
            history_items,
            request,
        )
        if exact_history_items:
            return exact_history_items

        on_demand_items = await self._build_budget_execution_history_on_demand_for_request(
            request,
        )
        if not history_items:
            return on_demand_items

        merged_items = self._merge_budget_execution_history_items(history_items, on_demand_items)
        return self._sort_budget_execution_history_items(merged_items)

    # pylint: disable=too-many-arguments,too-many-positional-arguments
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
        """Public execution-history API using the immutable request bundle."""
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
        return await self._get_budget_execution_history_for_request(resolved_request)
    # pylint: enable=too-many-arguments,too-many-positional-arguments

    @classmethod
    def _iter_budget_history_period_ranges(
        cls,
        period_type: str,
        start_date: str | None,
        end_date: str | None,
    ) -> list[dict[str, str]]:
        """Expand an explicit window into daily/weekly/monthly/etc period slices."""
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
        """Return whether one budget row overlaps the requested history window."""
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
        """Convert an execution row into an on-demand history row."""
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
        """Build history rows on the fly when no matching snapshots are present."""
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
                    self._build_budget_history_item_from_detail(
                        detail,
                        period_range,
                        request,
                        filter_summary,
                    )
                )

        return self._sort_budget_execution_history_items(history_items)

    @log_method
    async def _build_budget_execution_history_on_demand(
        self,
        request: BudgetExecutionRequest | None = None,
        **kwargs: Any,
    ) -> list[dict[str, Any]]:
        """Compatibility wrapper for the request-bundle-based on-demand history builder."""
        resolved_request = self._coerce_budget_execution_request(request, **kwargs)
        return await self._build_budget_execution_history_on_demand_for_request(resolved_request)
