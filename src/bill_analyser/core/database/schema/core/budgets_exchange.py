"""Budget and exchange-rate schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long
import sqlite3
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaCoreBudgetExchangeSchemaMixin:
    async def _init_budget_exchange_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS budgets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                category TEXT,
                sub_category TEXT,
                period_type TEXT NOT NULL,
                amount REAL NOT NULL,
                start_date TEXT NOT NULL,
                end_date TEXT,
                alert_threshold INTEGER DEFAULT 80,
                enabled BOOLEAN DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS budget_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                budget_id INTEGER NOT NULL,
                period_start TEXT NOT NULL,
                period_end TEXT NOT NULL,
                budget_amount REAL DEFAULT 0,
                spent_amount REAL DEFAULT 0,
                remaining_amount REAL,
                execution_rate REAL DEFAULT 0,
                status TEXT,
                filter_summary TEXT DEFAULT '',
                calculated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (budget_id) REFERENCES budgets(id) ON DELETE CASCADE
            )
            """
        )
        async with conn.execute("PRAGMA table_info(budget_history)") as cursor:
            budget_history_columns = [row[1] for row in await cursor.fetchall()]
        for column_name, alter_sql in [
            ("budget_amount", "ALTER TABLE budget_history ADD COLUMN budget_amount REAL DEFAULT 0"),
            ("execution_rate", "ALTER TABLE budget_history ADD COLUMN execution_rate REAL DEFAULT 0"),
            ("filter_summary", "ALTER TABLE budget_history ADD COLUMN filter_summary TEXT DEFAULT ''"),
        ]:
            if column_name in budget_history_columns:
                continue
            await conn.execute(alter_sql)
            self.logger.info("成功为 budget_history 表添加 %s 字段", column_name)

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS saved_filters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                description TEXT,
                filter_data TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, name),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS user_exchange_rates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                from_currency TEXT NOT NULL,
                to_currency TEXT NOT NULL,
                rate REAL NOT NULL,
                source TEXT DEFAULT 'manual',
                effective_date TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(user_id, from_currency, to_currency, effective_date),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS exchange_rate_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                type TEXT NOT NULL,
                base_url TEXT,
                enabled BOOLEAN DEFAULT 1,
                priority INTEGER DEFAULT 0,
                last_sync_at TEXT,
                config TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            """
        )

    async def _migrate_core_legacy_columns(self, conn: aiosqlite.Connection) -> None:
        try:
            await conn.execute("ALTER TABLE accounts ADD COLUMN currency TEXT DEFAULT 'CNY'")
            self.logger.info("成功为 accounts 表添加 currency 字段")
        except sqlite3.OperationalError:
            self.logger.debug("accounts 表已有 currency 字段")

        for description, statement in [
            ("created_from_template", "ALTER TABLE bills ADD COLUMN created_from_template INTEGER"),
            ("created_from_recurring", "ALTER TABLE bills ADD COLUMN created_from_recurring INTEGER"),
            ("import_history_id", "ALTER TABLE bills ADD COLUMN import_history_id INTEGER"),
            ("destination_amount", "ALTER TABLE bills ADD COLUMN destination_amount REAL DEFAULT 0"),
            ("destination_account_id", "ALTER TABLE bills ADD COLUMN destination_account_id INTEGER DEFAULT 0"),
            ("source_account_id", "ALTER TABLE bills ADD COLUMN source_account_id INTEGER DEFAULT 0"),
            ("payment_method", "ALTER TABLE bills ADD COLUMN payment_method TEXT DEFAULT ''"),
        ]:
            try:
                await conn.execute(statement)
                self.logger.info("成功为 bills 表添加 %s 字段", description)
            except sqlite3.OperationalError:
                self.logger.debug("bills 表已有 %s 字段", description)
