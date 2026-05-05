"""Thin Database facade composed from domain-specific persistence mixins."""

from __future__ import annotations

from .database.accounts import DatabaseAccountsMixin
from .database.audit_backup import DatabaseAuditBackupMixin
from .database.bills import DatabaseBillsMixin
from .database.budgets.reporting import DatabaseBudgetsReportingMixin
from .database.categories import DatabaseCategoriesMixin
from .database.category_rules import DatabaseCategoryRulesMixin
from .database.llm.candidates import DatabaseLLMCandidatesMixin
from .database.llm.config import DatabaseLLMConfigMixin
from .database.imports.configs import DatabaseImportConfigsMixin
from .database.imports.learning import DatabaseImportLearningMixin
from .database.imports.preview import DatabaseImportPreviewMixin
from .database.imports.sessions import DatabaseImportSessionsMixin
from .database.matching import DatabaseMatchingMixin
from .database.reconciliation import DatabaseReconciliationMixin
from .database.recurring_suggestions import DatabaseRecurringSuggestionsMixin
from .database.runtime import DatabaseRuntimeMixin
from .database.schema import DatabaseSchemaMixin
from .database.shared import BudgetExecutionRequest, BudgetForecastRequest, BudgetGroupKey
from .database.settings_bundle import DatabaseSettingsBundleMixin
from .database.tags import DatabaseTagsMixin
from .database.templates import DatabaseTemplatesMixin
from .database.users.data import DatabaseUserDataMixin
from .database.users.auth import DatabaseUsersAuthMixin


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
    DatabaseSettingsBundleMixin,
    DatabaseRecurringSuggestionsMixin,
    DatabaseTemplatesMixin,
    DatabaseTagsMixin,
    DatabaseAccountsMixin,
    DatabaseReconciliationMixin,
    DatabaseLLMCandidatesMixin,
    DatabaseLLMConfigMixin,
    DatabaseCategoryRulesMixin,
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
