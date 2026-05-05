"""On-demand budget execution history helpers."""

from __future__ import annotations

# pylint: disable=duplicate-code,line-too-long,unused-import,too-many-branches,too-many-locals

import json
from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import BudgetExecutionRequest

if TYPE_CHECKING:
    import aiosqlite


class BudgetExecutionOnDemandMixin:
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
