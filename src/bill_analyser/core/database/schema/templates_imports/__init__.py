"""Template/import schema helpers and related legacy migrations."""

from __future__ import annotations

from typing import TYPE_CHECKING

from bill_analyser.core.database.shared import DatabaseFacadeBase
from .learning import SchemaImportLearningTablesMixin
from .migrations import SchemaTemplateImportMigrationsMixin
from .staging import SchemaImportStagingMixin
from .suggestions import SchemaRecurringSuggestionsMixin
from .templates import SchemaTemplateTablesMixin

if TYPE_CHECKING:
    import aiosqlite


class DatabaseSchemaTemplatesImportsMixin(
    SchemaTemplateTablesMixin,
    SchemaImportStagingMixin,
    SchemaImportLearningTablesMixin,
    SchemaRecurringSuggestionsMixin,
    SchemaTemplateImportMigrationsMixin,
    DatabaseFacadeBase,
):
    """Template, import staging, and import-learning schema helpers."""

    async def _init_templates_imports_schema(self, conn: aiosqlite.Connection) -> None:
        await self._init_template_tables(conn)
        await self._init_import_config_history_tables(conn)
        await self._init_import_session_preview_tables(conn)
        await self._init_import_learning_sample_tables(conn)
        await self._init_import_learning_rule_tables(conn)
        await self._init_import_learning_model_tables(conn)
        await self._init_import_learning_suggestion_tables(conn)
        await self._init_recurring_suggestion_tables(conn)


__all__ = ["DatabaseSchemaTemplatesImportsMixin"]
