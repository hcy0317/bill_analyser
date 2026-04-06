"""Backward-compatible budget reporting aggregator mixin."""

from __future__ import annotations

from .db_budgets_execution import DatabaseBudgetExecutionHistoryMixin
from .db_budgets_forecast import DatabaseBudgetForecastMixin


class DatabaseBudgetsReportingMixin(
    DatabaseBudgetForecastMixin,
    DatabaseBudgetExecutionHistoryMixin,
):
    """Compose budget execution/history and forecast/import-export helpers."""
