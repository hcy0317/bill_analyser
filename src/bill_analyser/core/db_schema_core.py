"""Core business-table schema helpers and legacy migrations."""

# pylint: disable=line-too-long,wrong-import-position,too-many-branches,too-many-statements

from __future__ import annotations

import sqlite3
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite

from .db_shared import DatabaseFacadeBase


class DatabaseSchemaCoreMixin(DatabaseFacadeBase):
    """Core business-table schema creation and migrations."""

    async def _init_core_business_schema(self, conn: aiosqlite.Connection) -> None:
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
            "CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session "
            "ON llm_memory_events(user_id, session_id)"
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

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_pair_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                pair_type TEXT NOT NULL,
                left_bill_id INTEGER NOT NULL,
                right_bill_id INTEGER NOT NULL,
                source TEXT NOT NULL DEFAULT 'manual',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(left_bill_id < right_bill_id),
                UNIQUE(user_id, pair_type, left_bill_id, right_bill_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (left_bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (right_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_pair_feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                candidate_id TEXT NOT NULL,
                action TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_transfer_pair_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                left_bill_id INTEGER NOT NULL,
                right_bill_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                CHECK(left_bill_id < right_bill_id),
                UNIQUE(user_id, left_bill_id, right_bill_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (left_bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (right_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_investment_pair_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                left_bill_id INTEGER NOT NULL,
                right_bill_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                CHECK(left_bill_id < right_bill_id),
                UNIQUE(user_id, left_bill_id, right_bill_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (left_bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (right_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_learning_rule_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                bill_id INTEGER NOT NULL,
                rule_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(user_id, bill_id, rule_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (rule_id) REFERENCES import_learning_rules(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_reconciliation_candidates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                family TEXT NOT NULL DEFAULT 'import_reconciliation',
                candidate_id TEXT NOT NULL,
                candidate_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                session_id TEXT,
                preview_id INTEGER,
                import_bill_key TEXT NOT NULL,
                existing_bill_id INTEGER NOT NULL,
                group_key TEXT NOT NULL,
                amount_abs REAL NOT NULL DEFAULT 0,
                time_diff_seconds INTEGER,
                score REAL NOT NULL DEFAULT 0,
                level TEXT NOT NULL DEFAULT '',
                reason TEXT NOT NULL DEFAULT '',
                import_bill_snapshot_json TEXT NOT NULL DEFAULT '{}',
                existing_bill_snapshot_json TEXT NOT NULL DEFAULT '{}',
                source_payload_json TEXT NOT NULL DEFAULT '{}',
                seen_count INTEGER NOT NULL DEFAULT 1,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                resolved_at TEXT,
                resolution_event_id INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(candidate_type IN ('transfer', 'duplicate')),
                CHECK(status IN ('pending', 'accepted', 'rejected', 'merged', 'rolled_back', 'superseded')),
                UNIQUE(user_id, candidate_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (existing_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_merge_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                family TEXT NOT NULL DEFAULT 'import_reconciliation',
                group_key TEXT NOT NULL,
                group_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                canonical_bill_id INTEGER,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(group_type IN ('transfer', 'duplicate')),
                CHECK(status IN ('pending', 'accepted', 'merged', 'rolled_back', 'superseded')),
                UNIQUE(user_id, family, group_key),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (canonical_bill_id) REFERENCES bills(id) ON DELETE SET NULL
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_merge_members (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id INTEGER NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                member_key TEXT NOT NULL,
                member_type TEXT NOT NULL,
                bill_id INTEGER,
                import_bill_key TEXT,
                candidate_id TEXT,
                role TEXT NOT NULL DEFAULT 'candidate',
                snapshot_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(member_type IN ('existing_bill', 'import_bill')),
                UNIQUE(group_id, member_key),
                FOREIGN KEY (group_id) REFERENCES bill_merge_groups(id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_merge_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                group_id INTEGER NOT NULL,
                candidate_id TEXT,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (group_id) REFERENCES bill_merge_groups(id) ON DELETE CASCADE
            )
            """
        )

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

        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_user ON bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_date ON bills(date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_type ON bills(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_category ON bills(main_category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_batch ON bills(batch_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_hash ON bills(hash)")
        await conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_user ON accounts(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_hidden ON accounts(hidden)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_types_user ON account_types(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_types_type ON account_types(type)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_transfers_from ON account_transfers(from_account_id)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_transfers_to ON account_transfers(to_account_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_transfers_date ON account_transfers(transfer_date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bill_tags_bill ON bill_tags(bill_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bill_tags_tag ON bill_tags(tag_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_left ON bill_pair_links(user_id, left_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_right ON bill_pair_links(user_id, right_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_type ON bill_pair_links(user_id, pair_type)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_feedback_user_candidate "
            "ON bill_pair_feedback(user_id, candidate_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_feedback_user_created_at "
            "ON bill_pair_feedback(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_transfer_pair_suppressions_user_left "
            "ON bill_transfer_pair_suppressions(user_id, left_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_transfer_pair_suppressions_user_right "
            "ON bill_transfer_pair_suppressions(user_id, right_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_investment_pair_suppressions_user_left "
            "ON bill_investment_pair_suppressions(user_id, left_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_investment_pair_suppressions_user_right "
            "ON bill_investment_pair_suppressions(user_id, right_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_learning_rule_suppressions_user_bill "
            "ON bill_learning_rule_suppressions(user_id, bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_learning_rule_suppressions_user_rule "
            "ON bill_learning_rule_suppressions(user_id, rule_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_status "
            "ON bill_reconciliation_candidates(user_id, family, status)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_session "
            "ON bill_reconciliation_candidates(user_id, session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_preview "
            "ON bill_reconciliation_candidates(user_id, preview_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_existing "
            "ON bill_reconciliation_candidates(user_id, existing_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_merge_groups_user_family "
            "ON bill_merge_groups(user_id, family, status)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_merge_members_group_type "
            "ON bill_merge_members(group_id, member_type)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_merge_events_user_group "
            "ON bill_merge_events(user_id, group_id, created_at DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_period ON budgets(period_type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_dates ON budgets(start_date, end_date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budget_history_budget ON budget_history(budget_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_budget_history_period ON budget_history(period_start, period_end)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies "
            "ON user_exchange_rates(from_currency, to_currency)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_date ON user_exchange_rates(effective_date DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rate_sources_enabled ON exchange_rate_sources(enabled, priority)"
        )

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
