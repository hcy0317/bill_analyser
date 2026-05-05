"""Core business-table schema helpers and legacy migrations."""

from __future__ import annotations

from typing import TYPE_CHECKING

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .budgets_exchange import SchemaCoreBudgetExchangeSchemaMixin
from .business import SchemaCoreBusinessTablesMixin
from .indexes import SchemaCoreIndexesMixin
from .matching import SchemaCoreMatchingSchemaMixin
from .migrations import SchemaCoreMigrationsMixin

if TYPE_CHECKING:
    import aiosqlite


class DatabaseSchemaCoreMixin(
    SchemaCoreBusinessTablesMixin,
    SchemaCoreMatchingSchemaMixin,
    SchemaCoreBudgetExchangeSchemaMixin,
    SchemaCoreIndexesMixin,
    SchemaCoreMigrationsMixin,
    DatabaseFacadeBase,
):
    """Core business-table schema creation and migrations."""

    async def _init_core_business_schema(self, conn: aiosqlite.Connection) -> None:
        await self._init_bills_categories_schema(conn)
        await self._init_llm_schema(conn)
        await self._init_accounts_tags_schema(conn)
        await self._init_pairing_suppression_schema(conn)
        await self._init_reconciliation_merge_schema(conn)
        await self._init_budget_exchange_schema(conn)
        await self._init_core_business_indexes(conn)
        await self._migrate_core_legacy_columns(conn)


__all__ = ["DatabaseSchemaCoreMixin"]
