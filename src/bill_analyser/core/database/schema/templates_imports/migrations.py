"""Template/import schema migration helpers."""

from __future__ import annotations

# pylint: disable=line-too-long
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaTemplateImportMigrationsMixin:
    async def _migrate_learning_rules_composite_fields(self, conn: aiosqlite.Connection) -> None:
        """为 import_learning_rules 表补齐解析器复合匹配字段。"""
        async with conn.execute("PRAGMA table_info(import_learning_rules)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
        for column_name, alter_statement in {
            "parser_id": "ALTER TABLE import_learning_rules ADD COLUMN parser_id TEXT",
            "composite_match_hash": "ALTER TABLE import_learning_rules ADD COLUMN composite_match_hash TEXT",
            "match_features_json": "ALTER TABLE import_learning_rules ADD COLUMN match_features_json TEXT",
        }.items():
            if column_name in columns:
                continue
            self.logger.info("为 import_learning_rules 表添加 %s 字段", column_name)
            await conn.execute(alter_statement)
