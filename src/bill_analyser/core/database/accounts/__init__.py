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

    def _should_use_rust_account_bridge(self) -> bool:
        """Use Rust account CRUD only for regular file DBs that share the same SQLite file."""
        db_path_text = str(self.db_path)
        if db_path_text in {":memory:", "file::memory:?cache=shared"}:
            return False
        encryption_config = getattr(self, "_encryption_config", None)
        return not bool(getattr(encryption_config, "enabled", False))


__all__ = ["DatabaseAccountsMixin"]
