"""Thin schema orchestrator composed from smaller schema mixins."""

from __future__ import annotations

from ..utils.logger import log_method
from .db_schema_core import DatabaseSchemaCoreMixin
from .db_schema_templates_imports import DatabaseSchemaTemplatesImportsMixin
from .db_schema_users_security import DatabaseSchemaUsersSecurityMixin


class DatabaseSchemaMixin(
    DatabaseSchemaCoreMixin,
    DatabaseSchemaTemplatesImportsMixin,
    DatabaseSchemaUsersSecurityMixin,
):
    """Thin schema orchestrator that composes smaller schema sub-modules."""

    @log_method
    async def init_db(self) -> None:
        """初始化数据库和表结构。"""
        self.logger.info("开始初始化数据库")
        conn = await self._get_connection()

        await conn.execute("PRAGMA journal_mode=WAL")
        await conn.execute("PRAGMA synchronous=NORMAL")
        await conn.execute("PRAGMA cache_size=10000")
        await conn.execute("PRAGMA temp_store=MEMORY")
        self.logger.info("已启用 WAL 模式和性能优化")

        await self._init_users_security_schema(conn)
        await self._init_core_business_schema(conn)
        await self._init_templates_imports_schema(conn)

        for table_name in [
            "bills",
            "categories",
            "account_types",
            "accounts",
            "account_transfers",
            "tags",
            "budgets",
            "budget_history",
            "saved_filters",
            "bill_templates",
            "recurring_bills",
            "import_configs",
            "import_history",
            "user_exchange_rates",
        ]:
            await self._migrate_user_id_field(conn, table_name)

        await self._migrate_categories_unique_constraint(conn)
        await self._migrate_user_exchange_rates_unique_constraint(conn)
        await self._migrate_users_cash_fields(conn)
        await self._migrate_users_import_learning_fields(conn)
        await self._migrate_users_investment_keyword_fields(conn)
        await self._migrate_learning_rules_composite_fields(conn)
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_budget_history_user_period "
            "ON budget_history(user_id, period_start, period_end)"
        )

        await conn.commit()
        self.logger.info("数据库初始化完成")
