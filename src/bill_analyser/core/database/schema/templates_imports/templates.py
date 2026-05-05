"""Template and recurring bill schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long,broad-exception-caught
import sqlite3
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaTemplateTablesMixin:
    async def _init_template_tables(self, conn: aiosqlite.Connection) -> None:
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
