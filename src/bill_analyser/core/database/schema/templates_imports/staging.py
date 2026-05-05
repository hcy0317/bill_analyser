"""Import config, history, session, parser, and preview schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long,broad-exception-caught
import sqlite3
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaImportStagingMixin:
    async def _init_import_config_history_tables(self, conn: aiosqlite.Connection) -> None:
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

    async def _init_import_session_preview_tables(self, conn: aiosqlite.Connection) -> None:
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
                parser_tags_json TEXT,
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
                preview_parser_tags_json TEXT,
                preview_recurring_id INTEGER,
                preview_recurring_name TEXT,
                preview_recurring_candidate_count INTEGER DEFAULT 0,
                preview_recurring_match_score REAL DEFAULT 0,
                preview_recurring_match_reasons TEXT,
                preview_recurring_matched_date TEXT,
                preview_selected INTEGER DEFAULT 1,
                dedup_type TEXT,
                dedup_source_ids TEXT,
                preview_matching_feedback_json TEXT,
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
            "ALTER TABLE bills_parser_template ADD COLUMN parser_tags_json TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_parser_id TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_parser_tags_json TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_id INTEGER",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_name TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_candidate_count INTEGER DEFAULT 0",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_score REAL DEFAULT 0",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_reasons TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_matched_date TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_matching_feedback_json TEXT",
        ]:
            try:
                await conn.execute(alter_statement)
            except Exception:  # pragma: no cover - 兼容旧库字段已存在场景
                pass
