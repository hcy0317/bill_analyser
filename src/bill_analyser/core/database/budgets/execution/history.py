"""Budget execution history query helpers."""

from __future__ import annotations

# pylint: disable=duplicate-code,line-too-long,unused-import,too-many-locals

import json
from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import BudgetExecutionRequest

if TYPE_CHECKING:
    import aiosqlite


class BudgetExecutionHistoryQueriesMixin:
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
