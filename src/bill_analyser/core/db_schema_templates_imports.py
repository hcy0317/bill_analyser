"""Template/import schema helpers and related legacy migrations."""

# pylint: disable=line-too-long,wrong-import-position,broad-exception-caught

from __future__ import annotations

import sqlite3
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite

from .db_shared import DatabaseFacadeBase


class DatabaseSchemaTemplatesImportsMixin(DatabaseFacadeBase):
    """Template, import staging, and import-learning schema helpers."""

    async def _init_templates_imports_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                description TEXT,
                type TEXT NOT NULL,
                category TEXT,
                amount REAL,
                account TEXT,
                counterparty TEXT,
                tag TEXT,
                comment TEXT,
                is_favorite BOOLEAN DEFAULT 0,
                use_count INTEGER DEFAULT 0,
                last_used_at TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS recurring_bills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                template_id INTEGER,
                name TEXT NOT NULL,
                description TEXT,
                type TEXT NOT NULL,
                category TEXT,
                amount REAL NOT NULL,
                account TEXT,
                counterparty TEXT,
                tag TEXT,
                comment TEXT,
                frequency TEXT NOT NULL,
                start_date TEXT NOT NULL,
                end_date TEXT,
                next_date TEXT NOT NULL,
                enabled BOOLEAN DEFAULT 1,
                auto_create BOOLEAN DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (template_id) REFERENCES bill_templates(id) ON DELETE SET NULL
            )
            """
        )

        for statement in [
            "ALTER TABLE bill_templates ADD COLUMN destination_amount REAL DEFAULT 0",
            "ALTER TABLE bill_templates ADD COLUMN hide_amount INTEGER DEFAULT 0",
            "ALTER TABLE bill_templates ADD COLUMN display_order INTEGER DEFAULT 0",
            "ALTER TABLE bill_templates ADD COLUMN hidden INTEGER DEFAULT 0",
            "ALTER TABLE bill_templates ADD COLUMN utc_offset INTEGER DEFAULT 0",
            "ALTER TABLE recurring_bills ADD COLUMN destination_amount REAL DEFAULT 0",
            "ALTER TABLE recurring_bills ADD COLUMN hide_amount INTEGER DEFAULT 0",
            "ALTER TABLE recurring_bills ADD COLUMN display_order INTEGER DEFAULT 0",
            "ALTER TABLE recurring_bills ADD COLUMN hidden INTEGER DEFAULT 0",
            "ALTER TABLE recurring_bills ADD COLUMN utc_offset INTEGER DEFAULT 0",
            "ALTER TABLE recurring_bills ADD COLUMN scheduled_frequency_type INTEGER DEFAULT 0",
        ]:
            try:
                await conn.execute(statement)
            except sqlite3.OperationalError:
                self.logger.debug("模板表扩展字段已存在: %s", statement)

        await conn.execute("CREATE INDEX IF NOT EXISTS idx_templates_user ON bill_templates(user_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_templates_favorite ON bill_templates(is_favorite, use_count DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_templates_type ON bill_templates(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_bills_user ON recurring_bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_bills_next_date ON recurring_bills(next_date)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recurring_bills_enabled ON recurring_bills(enabled, next_date)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                file_format TEXT NOT NULL,
                description TEXT,
                field_mappings TEXT NOT NULL,
                date_format TEXT,
                encoding TEXT DEFAULT 'utf-8',
                delimiter TEXT,
                skip_rows INTEGER DEFAULT 0,
                has_header BOOLEAN DEFAULT 1,
                custom_rules TEXT,
                is_default BOOLEAN DEFAULT 0,
                use_count INTEGER DEFAULT 0,
                last_used_at TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                config_id INTEGER,
                file_name TEXT NOT NULL,
                file_format TEXT NOT NULL,
                file_size INTEGER,
                total_rows INTEGER DEFAULT 0,
                success_count INTEGER DEFAULT 0,
                error_count INTEGER DEFAULT 0,
                duplicate_count INTEGER DEFAULT 0,
                status TEXT DEFAULT 'pending',
                error_message TEXT,
                preview_data TEXT,
                imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                completed_at TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (config_id) REFERENCES import_configs(id)
            )
            """
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_configs_user ON import_configs(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_configs_format ON import_configs(file_format)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_configs_default ON import_configs(is_default)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_history_user ON import_history(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_history_status ON import_history(status)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_history_date ON import_history(imported_at)")

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL UNIQUE,
                user_id INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'parsing',
                file_count INTEGER DEFAULT 0,
                total_parsed INTEGER DEFAULT 0,
                total_preview INTEGER DEFAULT 0,
                total_confirmed INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_sessions_session ON import_sessions(session_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_sessions_user ON import_sessions(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_sessions_status ON import_sessions(status)")

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bills_parser_template (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                parser_date TEXT NOT NULL,
                parser_amount REAL NOT NULL,
                parser_type TEXT NOT NULL,
                parser_description TEXT,
                parser_id TEXT NOT NULL,
                parser_counterparty TEXT,
                parser_payment_method TEXT,
                parser_original_type TEXT,
                parser_original_category TEXT,
                parser_account_id TEXT,
                parser_is_processed TEXT DEFAULT '0',
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parser_template_session "
            "ON bills_parser_template(session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parser_template_processed ON bills_parser_template(parser_is_processed)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_parser_template_date ON bills_parser_template(parser_date)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parser_template_parser_id ON bills_parser_template(parser_id)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bills_preview (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                preview_date TEXT NOT NULL,
                preview_type TEXT NOT NULL,
                preview_amount REAL NOT NULL,
                preview_destination_amount REAL DEFAULT 0,
                preview_main_category TEXT,
                preview_sub_category TEXT,
                preview_source_account_id INTEGER,
                preview_destination_account_id INTEGER,
                preview_counterparty TEXT,
                preview_payment_method TEXT,
                preview_description TEXT,
                preview_parser_id TEXT,
                preview_recurring_id INTEGER,
                preview_recurring_name TEXT,
                preview_recurring_candidate_count INTEGER DEFAULT 0,
                preview_recurring_match_score REAL DEFAULT 0,
                preview_recurring_match_reasons TEXT,
                preview_recurring_matched_date TEXT,
                preview_selected INTEGER DEFAULT 1,
                dedup_type TEXT,
                dedup_source_ids TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_session ON bills_preview(session_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_selected ON bills_preview(preview_selected)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_type ON bills_preview(preview_type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_date ON bills_preview(preview_date)")

        for alter_statement in [
            "ALTER TABLE bills_preview ADD COLUMN preview_parser_id TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_id INTEGER",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_name TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_candidate_count INTEGER DEFAULT 0",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_score REAL DEFAULT 0",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_reasons TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_matched_date TEXT",
        ]:
            try:
                await conn.execute(alter_statement)
            except Exception:  # pragma: no cover - 兼容旧库字段已存在场景
                pass

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_annotation_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                preview_id INTEGER NOT NULL,
                annotated_type TEXT,
                annotated_category_id INTEGER,
                annotated_source_account_id INTEGER,
                annotated_destination_account_id INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(session_id, preview_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_annotation_samples_session ON import_annotation_samples(session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_annotation_samples_user ON import_annotation_samples(user_id)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_learning_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                learned_type TEXT,
                learned_category_id INTEGER,
                learned_source_account_id INTEGER,
                learned_destination_account_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                source_session_id TEXT,
                source_preview_id INTEGER,
                match_features_json TEXT,
                applied_count INTEGER NOT NULL DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rules_user_enabled "
            "ON import_learning_rules(user_id, enabled)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_learning_rule_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id INTEGER,
                user_id INTEGER NOT NULL DEFAULT 1,
                action TEXT NOT NULL,
                match_type TEXT,
                match_value TEXT,
                normalized_match_value TEXT,
                session_id TEXT,
                preview_id INTEGER,
                payload_json TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (rule_id) REFERENCES import_learning_rules(id) ON DELETE SET NULL
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rule_logs_user "
            "ON import_learning_rule_logs(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rule_logs_rule ON import_learning_rule_logs(rule_id)"
        )

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
