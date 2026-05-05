"""Budget execution, snapshot, and history helpers for the split database facade."""

from __future__ import annotations

from bill_analyser.core.database.budgets.core import DatabaseBudgetsCoreMixin
from .details import BudgetExecutionDetailsMixin
from .history import BudgetExecutionHistoryQueriesMixin
from .on_demand import BudgetExecutionOnDemandMixin
from .snapshots import BudgetExecutionSnapshotsMixin


class DatabaseBudgetExecutionHistoryMixin(
    BudgetExecutionDetailsMixin,
    BudgetExecutionSnapshotsMixin,
    BudgetExecutionHistoryQueriesMixin,
    BudgetExecutionOnDemandMixin,
    DatabaseBudgetsCoreMixin,
):
    """Budget execution, snapshot history, and on-demand history helpers."""


__all__ = ["DatabaseBudgetExecutionHistoryMixin"]
