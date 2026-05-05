"""Account-domain persistence helpers for the split database facade."""

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .balances import AccountBalancesMixin
from .mutations import AccountMutationsMixin
from .reads import AccountReadsMixin


class DatabaseAccountsMixin(
    AccountReadsMixin,
    AccountMutationsMixin,
    AccountBalancesMixin,
    DatabaseFacadeBase,
):
    """Account CRUD, alias mapping, and balance synchronization helpers."""


__all__ = ["DatabaseAccountsMixin"]
