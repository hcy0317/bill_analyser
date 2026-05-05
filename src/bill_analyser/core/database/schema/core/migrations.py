"""Core schema legacy migration helpers."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-locals
import sqlite3
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaCoreMigrationsMixin:
    async def _migrate_user_id_field(self, conn: aiosqlite.Connection, table_name: str) -> None:
        """为表添加 user_id 字段（如果不存在）。"""
        try:
            async with conn.execute(f"PRAGMA table_info({table_name})") as cursor:
                columns = [row[1] for row in await cursor.fetchall()]
            if "user_id" in columns:
                return

            self.logger.info("为 %s 表添加 user_id 字段", table_name)
            await conn.execute(f"ALTER TABLE {table_name} ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1")
            await conn.execute(f"CREATE INDEX IF NOT EXISTS idx_{table_name}_user_id ON {table_name}(user_id)")
        except sqlite3.OperationalError as exc:
            self.logger.warning("迁移 %s.user_id 字段失败: %s", table_name, exc)

    async def _migrate_categories_unique_constraint(self, conn: aiosqlite.Connection) -> None:
        """修复 categories 表 UNIQUE 约束以包含 user_id。"""
        async with conn.execute("PRAGMA index_list(categories)") as cursor:
            indexes = await cursor.fetchall()
        unique_index_names = [row[1] for row in indexes if row[2]]

        async with conn.execute("PRAGMA table_info(categories)") as cursor:
            columns_info = await cursor.fetchall()
        column_names = [column[1] for column in columns_info]

        if not unique_index_names:
            return

        constraint_needs_migration = False
        for index_name in unique_index_names:
            async with conn.execute(f"PRAGMA index_info({index_name})") as cursor:
                indexed_columns = [row[2] for row in await cursor.fetchall()]
            if indexed_columns == ["main_category", "sub_category"]:
                constraint_needs_migration = True
                break

        if not constraint_needs_migration:
            return

        self.logger.info("开始迁移 categories 表 UNIQUE 约束: 添加 user_id")
        select_columns = [column for column in column_names if column != "id"]
        if "user_id" not in select_columns:
            select_columns.append("user_id")
        column_sql = ", ".join(select_columns)

        await conn.execute(
            """
            CREATE TABLE categories_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                type INTEGER DEFAULT 1,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL,
                description TEXT,
                priority INTEGER DEFAULT 0,
                keywords TEXT,
                hidden BOOLEAN DEFAULT 0,
                icon TEXT,
                color TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(user_id, main_category, sub_category),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(f"INSERT OR IGNORE INTO categories_new ({column_sql}) SELECT {column_sql} FROM categories")
        await conn.execute("DROP TABLE categories")
        await conn.execute("ALTER TABLE categories_new RENAME TO categories")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id)")
        self.logger.info("categories 表 UNIQUE 约束迁移完成")

    async def _migrate_bills_hash_unique_constraint(self, conn: aiosqlite.Connection) -> None:
        """修复 bills.hash 的 UNIQUE 约束以包含 user_id。"""
        async with conn.execute("PRAGMA index_list(bills)") as cursor:
            indexes = await cursor.fetchall()
        unique_index_names = [row[1] for row in indexes if row[2]]

        constraint_needs_migration = False
        for index_name in unique_index_names:
            async with conn.execute(f"PRAGMA index_info({index_name})") as cursor:
                indexed_columns = [row[2] for row in await cursor.fetchall()]
            if indexed_columns == ["hash"]:
                constraint_needs_migration = True
                break

        if not constraint_needs_migration:
            return

        self.logger.info("开始迁移 bills.hash UNIQUE 约束: 添加 user_id")
        async with conn.execute("PRAGMA table_info(bills)") as cursor:
            columns_info = await cursor.fetchall()
        column_names = [column[1] for column in columns_info]
        column_sql = ", ".join(column_names)

        await conn.execute(
            """
            CREATE TABLE bills_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                date TEXT NOT NULL,
                type TEXT NOT NULL,
                amount REAL NOT NULL,
                counterparty TEXT NOT NULL,
                description TEXT NOT NULL,
                payment_method TEXT DEFAULT '',
                main_category TEXT,
                sub_category TEXT,
                batch_id TEXT,
                hash TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                source_account_id INTEGER DEFAULT 0,
                destination_account_id INTEGER DEFAULT 0,
                destination_amount REAL DEFAULT 0,
                created_from_template INTEGER,
                created_from_recurring INTEGER,
                import_history_id INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(f"INSERT INTO bills_new ({column_sql}) SELECT {column_sql} FROM bills")
        await conn.execute("DROP TABLE bills")
        await conn.execute("ALTER TABLE bills_new RENAME TO bills")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_user ON bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_date ON bills(date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_type ON bills(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_category ON bills(main_category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_batch ON bills(batch_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_hash ON bills(hash)")
        await conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash)")
        self.logger.info("bills.hash UNIQUE 约束迁移完成")

    async def _migrate_user_exchange_rates_unique_constraint(self, conn: aiosqlite.Connection) -> None:
        """修复 user_exchange_rates 的 UNIQUE 约束以包含 user_id。"""
        async with conn.execute("PRAGMA index_list(user_exchange_rates)") as cursor:
            indexes = await cursor.fetchall()
        unique_index_names = [row[1] for row in indexes if row[2]]

        constraint_needs_migration = False
        for index_name in unique_index_names:
            async with conn.execute(f"PRAGMA index_info({index_name})") as cursor:
                indexed_columns = [row[2] for row in await cursor.fetchall()]
            if indexed_columns == ["from_currency", "to_currency", "effective_date"]:
                constraint_needs_migration = True
                break

        if not constraint_needs_migration:
            return

        self.logger.info("开始迁移 user_exchange_rates 表 UNIQUE 约束: 添加 user_id")
        async with conn.execute("PRAGMA table_info(user_exchange_rates)") as cursor:
            columns_info = await cursor.fetchall()
        column_names = [column[1] for column in columns_info]
        select_columns = [column for column in column_names if column != "id"]
        if "user_id" not in select_columns:
            select_columns.insert(0, "user_id")
        column_sql = ", ".join(select_columns)

        await conn.execute(
            """
            CREATE TABLE user_exchange_rates_new (
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
            f"INSERT OR IGNORE INTO user_exchange_rates_new ({column_sql}) SELECT {column_sql} FROM user_exchange_rates"
        )
        await conn.execute("DROP TABLE user_exchange_rates")
        await conn.execute("ALTER TABLE user_exchange_rates_new RENAME TO user_exchange_rates")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies "
            "ON user_exchange_rates(from_currency, to_currency)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_date ON user_exchange_rates(effective_date DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_user_exchange_rates_user_id ON user_exchange_rates(user_id)")
        self.logger.info("user_exchange_rates 表 UNIQUE 约束迁移完成")
