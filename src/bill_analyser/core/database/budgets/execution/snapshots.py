"""Budget execution snapshot helpers."""

from __future__ import annotations

# pylint: disable=duplicate-code,line-too-long,unused-import,too-many-arguments,too-many-positional-arguments

import json
from datetime import date, timedelta
from typing import TYPE_CHECKING, Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import BudgetExecutionRequest

if TYPE_CHECKING:
    import aiosqlite


class BudgetExecutionSnapshotsMixin:
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
