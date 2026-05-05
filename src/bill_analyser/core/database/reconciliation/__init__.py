"""Import reconciliation candidate and merge-ledger persistence helpers."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .base import ReconciliationBaseMixin, ReconciliationProjectionConflictError
from .persistence import ReconciliationPersistenceMixin
from .projection import ReconciliationProjectionMixin
from .actions import ReconciliationActionsMixin


class DatabaseReconciliationMixin(  # pylint: disable=too-many-ancestors
    ReconciliationBaseMixin,
    ReconciliationPersistenceMixin,
    ReconciliationProjectionMixin,
    ReconciliationActionsMixin,
    DatabaseFacadeBase,
):
    """Compatibility facade composed from db_reconciliation persistence parts."""


__all__ = [
    "DatabaseReconciliationMixin",
    "ReconciliationProjectionConflictError",
]
