"""Thin Database facade composed from domain-specific persistence mixins."""

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
from .db_matching import DatabaseMatchingMixin
from .db_recurring_suggestions import DatabaseRecurringSuggestionsMixin
from .db_runtime import DatabaseRuntimeMixin
from .db_schema import DatabaseSchemaMixin
from .db_shared import BudgetExecutionRequest, BudgetForecastRequest, BudgetGroupKey
from .db_tags import DatabaseTagsMixin
from .db_templates import DatabaseTemplatesMixin
from .db_user_data import DatabaseUserDataMixin
from .db_users_auth import DatabaseUsersAuthMixin


class DatabaseBudgetImportMixin(  # pylint: disable=too-many-ancestors
    DatabaseBudgetsReportingMixin,
    DatabaseImportPreviewMixin,
    DatabaseImportSessionsMixin,
    DatabaseImportConfigsMixin,
    DatabaseImportLearningMixin,
):
    """Aggregate the budget and import-related persistence mixins."""


class DatabaseUserSecurityMixin(
    DatabaseUserDataMixin,
    DatabaseUsersAuthMixin,
):
    """Aggregate user-profile and authentication persistence helpers."""


class DatabaseTransactionalDataMixin(  # pylint: disable=too-many-ancestors
    DatabaseRecurringSuggestionsMixin,
    DatabaseTemplatesMixin,
    DatabaseTagsMixin,
    DatabaseAccountsMixin,
    DatabaseCategoriesMixin,
    DatabaseMatchingMixin,
    DatabaseBillsMixin,
    DatabaseAuditBackupMixin,
):
    """Aggregate core transactional, taxonomy, and audit persistence helpers."""


class DatabaseDomainMixin(  # pylint: disable=too-many-ancestors
    DatabaseUserSecurityMixin,
    DatabaseTransactionalDataMixin,
):
    """Aggregate non-runtime/non-schema business persistence domains."""


class Database(  # pylint: disable=too-many-ancestors
    DatabaseBudgetImportMixin,
    DatabaseDomainMixin,
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
