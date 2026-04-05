"""Thin Database facade composed from domain-specific persistence mixins."""

# pylint: disable=too-many-ancestors,duplicate-code

from __future__ import annotations

from .db_accounts import DatabaseAccountsMixin
from .db_audit_backup import DatabaseAuditBackupMixin
from .db_bills import DatabaseBillsMixin
from .db_budgets_reporting import DatabaseBudgetsReportingMixin
from .db_categories import DatabaseCategoriesMixin
from .db_import_configs import DatabaseImportConfigsMixin
from .db_import_learning import DatabaseImportLearningMixin
from .db_import_preview import DatabaseImportPreviewMixin
from .db_import_sessions import DatabaseImportSessionsMixin
from .db_runtime import DatabaseRuntimeMixin
from .db_schema import DatabaseSchemaMixin
from .db_shared import BudgetExecutionRequest, BudgetForecastRequest, BudgetGroupKey
from .db_tags import DatabaseTagsMixin
from .db_templates import DatabaseTemplatesMixin
from .db_user_data import DatabaseUserDataMixin
from .db_users_auth import DatabaseUsersAuthMixin


class Database(
    DatabaseBudgetsReportingMixin,
    DatabaseImportPreviewMixin,
    DatabaseImportSessionsMixin,
    DatabaseImportConfigsMixin,
    DatabaseImportLearningMixin,
    DatabaseUserDataMixin,
    DatabaseUsersAuthMixin,
    DatabaseTemplatesMixin,
    DatabaseTagsMixin,
    DatabaseAccountsMixin,
    DatabaseCategoriesMixin,
    DatabaseBillsMixin,
    DatabaseAuditBackupMixin,
    DatabaseSchemaMixin,
    DatabaseRuntimeMixin,
):
    """Thin async database facade composed from domain-specific mixins."""


__all__ = [
    "BudgetExecutionRequest",
    "BudgetForecastRequest",
    "BudgetGroupKey",
    "Database",
]
