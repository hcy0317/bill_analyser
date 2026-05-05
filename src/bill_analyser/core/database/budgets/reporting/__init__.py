"""Backward-compatible budget reporting aggregator mixin."""

from __future__ import annotations

from bill_analyser.core.database.budgets.execution import DatabaseBudgetExecutionHistoryMixin
from bill_analyser.core.database.budgets.forecast import DatabaseBudgetForecastMixin


class DatabaseBudgetsReportingMixin(
    DatabaseBudgetForecastMixin,
    DatabaseBudgetExecutionHistoryMixin,
):
    """Compose budget execution/history and forecast/import-export helpers."""
