"""Core bills, category, LLM, account, and tag schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long,too-many-statements
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaCoreBusinessTablesMixin:
    async def _init_bills_categories_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bills (
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

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS categories (
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
        async with conn.execute("PRAGMA table_info(categories)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
        if "type" not in columns:
            self.logger.info("添加 type 字段到 categories 表")
            await conn.execute("ALTER TABLE categories ADD COLUMN type INTEGER DEFAULT 1")
            await conn.execute("UPDATE categories SET type = 2 WHERE main_category = '收入'")
            await conn.execute("UPDATE categories SET type = 3 WHERE main_category = '转账'")
        if "priority" not in columns:
            self.logger.info("添加 priority 字段到 categories 表")
            await conn.execute("ALTER TABLE categories ADD COLUMN priority INTEGER DEFAULT 0")
        if "keywords" not in columns:
            self.logger.info("添加 keywords 字段到 categories 表")
            await conn.execute("ALTER TABLE categories ADD COLUMN keywords TEXT")
        if "hidden" not in columns:
            self.logger.info("添加 hidden 字段到 categories 表")
            await conn.execute("ALTER TABLE categories ADD COLUMN hidden BOOLEAN DEFAULT 0")
        if "icon" not in columns:
            self.logger.info("添加 icon 字段到 categories 表")
            await conn.execute("ALTER TABLE categories ADD COLUMN icon TEXT")
        if "color" not in columns:
            self.logger.info("添加 color 字段到 categories 表")
            await conn.execute("ALTER TABLE categories ADD COLUMN color TEXT")

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS category_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                category_id INTEGER NOT NULL,
                name TEXT NOT NULL DEFAULT '',
                priority INTEGER NOT NULL DEFAULT 100,
                rule_expression TEXT NOT NULL,
                regex_enabled BOOLEAN DEFAULT 0,
                enabled BOOLEAN DEFAULT 1,
                applied_count INTEGER DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_category_rules_user_priority "
            "ON category_rules(user_id, enabled, priority)"
        )

    async def _init_llm_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS llm_candidates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                type TEXT NOT NULL DEFAULT 'classification',
                source_bill_ids TEXT,
                suggested_main_category TEXT,
                suggested_sub_category TEXT,
                suggested_rule_expression TEXT,
                confidence REAL DEFAULT 0.0,
                llm_provider TEXT,
                llm_model TEXT,
                llm_response_raw TEXT,
                status TEXT DEFAULT 'pending',
                created_at TEXT DEFAULT (datetime('now', 'localtime')),
                reviewed_at TEXT
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_llm_candidates_user_status "
            "ON llm_candidates(user_id, status)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS llm_memory_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                session_id TEXT,
                preview_id INTEGER,
                event_type TEXT NOT NULL DEFAULT 'recommendation',
                decision TEXT,
                prompt_text TEXT,
                llm_response_raw TEXT,
                llm_provider TEXT,
                llm_model TEXT,
                suggested_main_category TEXT,
                suggested_sub_category TEXT,
                suggested_source_account TEXT,
                suggested_destination_account TEXT,
                confidence REAL DEFAULT 0.0,
                user_correction_category TEXT,
                user_correction_account TEXT,
                snapshot_before TEXT,
                snapshot_after TEXT,
                metadata TEXT,
                created_at TEXT DEFAULT (datetime('now', 'localtime'))
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_llm_memory_events_user "
            "ON llm_memory_events(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_llm_memory_events_user_order "
            "ON llm_memory_events(user_id, created_at DESC, id DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session "
            "ON llm_memory_events(user_id, session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session_order "
            "ON llm_memory_events(user_id, session_id, created_at DESC, id DESC)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS llm_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                provider TEXT NOT NULL DEFAULT 'openai',
                model TEXT NOT NULL DEFAULT '',
                api_key TEXT DEFAULT '',
                base_url TEXT DEFAULT '',
                advanced_settings TEXT NOT NULL DEFAULT '{}',
                is_active INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                UNIQUE(user_id, name)
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_llm_configs_user_active "
            "ON llm_configs(user_id, is_active)"
        )
        async with conn.execute("PRAGMA table_info(llm_configs)") as cursor:
            llm_config_columns = [row[1] for row in await cursor.fetchall()]
        if "advanced_settings" not in llm_config_columns:
            self.logger.info("添加 advanced_settings 字段到 llm_configs 表")
            await conn.execute("ALTER TABLE llm_configs ADD COLUMN advanced_settings TEXT NOT NULL DEFAULT '{}'")

    async def _init_accounts_tags_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS account_types (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                type INTEGER NOT NULL,
                icon TEXT,
                display_order INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                type INTEGER NOT NULL,
                category INTEGER,
                currency TEXT DEFAULT 'CNY',
                icon TEXT,
                color TEXT,
                balance REAL DEFAULT 0,
                initial_balance REAL DEFAULT 0,
                hidden BOOLEAN DEFAULT 0,
                display_order INTEGER DEFAULT 0,
                comment TEXT,
                aliases TEXT,
                parent_id INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        async with conn.execute("PRAGMA table_info(accounts)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
        if "parent_id" not in columns:
            self.logger.info("添加 parent_id 列到 accounts 表")
            await conn.execute("ALTER TABLE accounts ADD COLUMN parent_id INTEGER DEFAULT 0")
        if "aliases" not in columns:
            self.logger.info("添加 aliases 列到 accounts 表 (用于账户别名匹配)")
            await conn.execute("ALTER TABLE accounts ADD COLUMN aliases TEXT")

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS account_transfers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                from_account_id INTEGER NOT NULL,
                to_account_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                transfer_date TEXT NOT NULL,
                note TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (from_account_id) REFERENCES accounts(id),
                FOREIGN KEY (to_account_id) REFERENCES accounts(id)
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                color TEXT,
                icon TEXT,
                display_order INTEGER DEFAULT 0,
                hidden BOOLEAN DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, name),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        async with conn.execute("PRAGMA table_info(tags)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
        if "hidden" not in columns:
            self.logger.info("添加 hidden 字段到 tags 表")
            await conn.execute("ALTER TABLE tags ADD COLUMN hidden BOOLEAN DEFAULT 0")

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_tags (
                bill_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (bill_id, tag_id),
                FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            )
            """
        )
