"""Core budget helpers and CRUD operations for the split database facade."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .hierarchy import BudgetHierarchyMixin
from .listing import BudgetListingMixin
from .mutations import BudgetMutationMixin
from .shared import BudgetCoreSharedMixin, BudgetPeriodGroupKey


class DatabaseBudgetsCoreMixin(
    BudgetMutationMixin,
    BudgetListingMixin,
    BudgetHierarchyMixin,
    BudgetCoreSharedMixin,
    DatabaseFacadeBase,
):
    """Budget CRUD helpers plus shared category/grouping logic."""


__all__ = ["BudgetPeriodGroupKey", "DatabaseBudgetsCoreMixin"]
