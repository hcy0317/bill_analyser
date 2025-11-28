"""
Database Module - 异步数据库管理模块

使用 aiosqlite 实现异步数据库操作,支持 WAL 模式、批量写入和去重。
"""

import hashlib
import json
import sqlite3
# import threading  # 已移除
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, Dict, List, Optional

import aiosqlite

from ..utils.logger import get_logger, log_method, log_step


class Database:
    """异步数据库管理器"""

    def __init__(self, db_path: Optional[str] = None):
        """
        初始化数据库管理器

        Args:
            db_path: 数据库文件路径，如果为 None 则使用默认路径
        """
        self.logger = get_logger('Database')

        if db_path:
            self.db_path = Path(db_path)
        else:
            # 数据库路径指向项目根目录的 data 文件夹
            self.db_path = Path(__file__).parent.parent.parent / "data" / "bills.db"

        # 确保数据目录存在
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

        self._connection: Optional[aiosqlite.Connection] = None
        # 使用线程锁而非asyncio.Lock,避免事件循环绑定问题
        # Lock已移除 - SQLite自带线程安全

        # 批量写入配置
        self.batch_size = 1000

        # 缓存配置
        self._cache = {}
        self._cache_expiry = {}
        self._cache_ttl = 60  # 缓存60秒

        self.logger.info(f"数据库管理器已初始化: {self.db_path}")

    @log_method
    async def init_db(self):
        """初始化数据库和表结构"""
        self.logger.info("开始初始化数据库")
        conn = await self._get_connection()

        # 启用 WAL 模式
        await conn.execute("PRAGMA journal_mode=WAL")
        await conn.execute("PRAGMA synchronous=NORMAL")
        await conn.execute("PRAGMA cache_size=10000")
        await conn.execute("PRAGMA temp_store=MEMORY")

        self.logger.info("已启用 WAL 模式和性能优化")

        # 创建账单表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS bills (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL DEFAULT 1,
        date TEXT NOT NULL,
        type TEXT NOT NULL,
        amount REAL NOT NULL,
        counterparty TEXT NOT NULL,
        description TEXT NOT NULL,
        main_category TEXT,
        sub_category TEXT,
        batch_id TEXT,
        hash TEXT UNIQUE,
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
        """)

        # 创建分类表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 检查并添加新字段(迁移)
        async with conn.execute("PRAGMA table_info(categories)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]

            if 'type' not in columns:
                self.logger.info("添加 type 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN type INTEGER DEFAULT 1")

                # 迁移旧数据类型
                await conn.execute("UPDATE categories SET type = 2 WHERE main_category = '收入'")
                await conn.execute("UPDATE categories SET type = 3 WHERE main_category = '转账'")

            if 'priority' not in columns:
                self.logger.info("添加 priority 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN priority INTEGER DEFAULT 0")

            if 'keywords' not in columns:
                self.logger.info("添加 keywords 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN keywords TEXT")

            if 'hidden' not in columns:
                self.logger.info("添加 hidden 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN hidden BOOLEAN DEFAULT 0")

            if 'icon' not in columns:
                self.logger.info("添加 icon 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN icon TEXT")

            if 'color' not in columns:
                self.logger.info("添加 color 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN color TEXT")

        # 创建账户类型表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建账户表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        parent_id INTEGER DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        """)

        # 检查 accounts 表是否有 parent_id 列 (用于迁移旧数据库)
        cursor = await conn.execute("PRAGMA table_info(accounts)")
        columns = [row[1] for row in await cursor.fetchall()]
        if 'parent_id' not in columns:
            self.logger.info("添加 parent_id 列到 accounts 表")
            await conn.execute("ALTER TABLE accounts ADD COLUMN parent_id INTEGER DEFAULT 0")

        # === 多用户数据隔离迁移 ===
        # 为现有表添加 user_id 字段（如果不存在）
        await self._migrate_user_id_field(conn, 'bills')
        await self._migrate_user_id_field(conn, 'categories')
        await self._migrate_user_id_field(conn, 'account_types')
        await self._migrate_user_id_field(conn, 'accounts')
        await self._migrate_user_id_field(conn, 'account_transfers')
        await self._migrate_user_id_field(conn, 'tags')
        await self._migrate_user_id_field(conn, 'budgets')
        await self._migrate_user_id_field(conn, 'saved_filters')
        await self._migrate_user_id_field(conn, 'bill_templates')
        await self._migrate_user_id_field(conn, 'recurring_bills')
        await self._migrate_user_id_field(conn, 'import_configs')
        await self._migrate_user_id_field(conn, 'import_history')

        # 创建账户转账记录表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建标签表
        # 创建标签表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 检查 tags 表是否有 hidden 列
        async with conn.execute("PRAGMA table_info(tags)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
            if 'hidden' not in columns:
                self.logger.info("添加 hidden 字段到 tags 表")
                await conn.execute("ALTER TABLE tags ADD COLUMN hidden BOOLEAN DEFAULT 0")

        # 创建账单标签关联表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS bill_tags (
        bill_id INTEGER NOT NULL,
        tag_id INTEGER NOT NULL,
        created_at TEXT NOT NULL,
        PRIMARY KEY (bill_id, tag_id),
        FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE,
        FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        )
        """)

        # 创建预算表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建预算历史记录表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS budget_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        budget_id INTEGER NOT NULL,
        period_start TEXT NOT NULL,
        period_end TEXT NOT NULL,
        spent_amount REAL DEFAULT 0,
        remaining_amount REAL,
        status TEXT,
        calculated_at TEXT NOT NULL,
        FOREIGN KEY (budget_id) REFERENCES budgets(id) ON DELETE CASCADE
        )
        """)

        # 创建保存的筛选器表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建索引 (为user_id添加索引以优化多用户查询)
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_user ON bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_date ON bills(date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_type ON bills(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_category ON bills(main_category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_batch ON bills(batch_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_hash ON bills(hash)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_user ON accounts(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(type)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_hidden ON accounts(hidden)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_types_user ON account_types(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_types_type ON account_types(type)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_transfers_from "
            "ON account_transfers(from_account_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_transfers_to "
            "ON account_transfers(to_account_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_transfers_date "
            "ON account_transfers(transfer_date)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bill_tags_bill ON bill_tags(bill_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bill_tags_tag ON bill_tags(tag_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_period ON budgets(period_type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_dates ON budgets(start_date, end_date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budget_history_budget ON budget_history(budget_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_budget_history_period "
            "ON budget_history(period_start, period_end)"
        )

        # 汇率管理表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS user_exchange_rates (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        from_currency TEXT NOT NULL,
        to_currency TEXT NOT NULL,
        rate REAL NOT NULL,
        source TEXT DEFAULT 'manual',
        effective_date TEXT NOT NULL,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(from_currency, to_currency, effective_date)
        )
        """)

        await conn.execute("""
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
        """)

        # 汇率索引
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies "
            "ON user_exchange_rates(from_currency, to_currency)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_date "
            "ON user_exchange_rates(effective_date DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rate_sources_enabled "
            "ON exchange_rate_sources(enabled, priority)"
        )

        # 为accounts表添加currency字段（如果不存在）
        try:
            await conn.execute("ALTER TABLE accounts ADD COLUMN currency TEXT DEFAULT 'CNY'")
            self.logger.info("成功为accounts表添加currency字段")
        except sqlite3.OperationalError:
            # 字段已存在，忽略
            self.logger.debug("accounts表已有currency字段")

        # 创建账单模板表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建定期账单表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建模板索引
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_templates_user ON bill_templates(user_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_templates_favorite "
            "ON bill_templates(is_favorite, use_count DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_templates_type ON bill_templates(type)")

        # 创建定期账单索引
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_bills_user ON recurring_bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_bills_next_date ON recurring_bills(next_date)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recurring_bills_enabled "
            "ON recurring_bills(enabled, next_date)"
        )

        # 创建导入配置表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建导入历史表 (添加 user_id 字段用于多用户数据隔离)
        await conn.execute("""
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
        """)

        # 创建索引
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_configs_user ON import_configs(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_configs_format ON import_configs(file_format)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_configs_default ON import_configs(is_default)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_history_user ON import_history(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_history_status ON import_history(status)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_history_date ON import_history(imported_at)")

        # === 用户认证系统表 ===
        # 创建用户表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            nickname TEXT,
            avatar TEXT,
            default_account_id INTEGER,
            transaction_edit_scope INTEGER DEFAULT 0,
            language TEXT DEFAULT 'zh_Hans',
            default_currency TEXT DEFAULT 'CNY',
            first_day_of_week INTEGER DEFAULT 1,
            fiscal_year_start INTEGER DEFAULT 1,
            calendar_display_type INTEGER DEFAULT 0,
            date_display_type INTEGER DEFAULT 0,
            long_date_format INTEGER DEFAULT 0,
            short_date_format INTEGER DEFAULT 0,
            long_time_format INTEGER DEFAULT 0,
            short_time_format INTEGER DEFAULT 0,
            fiscal_year_format INTEGER DEFAULT 0,
            currency_display_type INTEGER DEFAULT 0,
            numeral_system INTEGER DEFAULT 0,
            decimal_separator INTEGER DEFAULT 0,
            digit_grouping_symbol INTEGER DEFAULT 0,
            digit_grouping INTEGER DEFAULT 0,
            coordinate_display_type INTEGER DEFAULT 0,
            expense_amount_color INTEGER DEFAULT 0,
            income_amount_color INTEGER DEFAULT 0,
            is_active BOOLEAN DEFAULT 1,
            email_verified BOOLEAN DEFAULT 0,
            two_factor_enabled BOOLEAN DEFAULT 0,
            two_factor_secret TEXT,
            failed_login_attempts INTEGER DEFAULT 0,
            locked_until TEXT,
            last_login_at TEXT,
            last_login_ip TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        """)

        # 创建会话表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token_hash TEXT NOT NULL UNIQUE,
            refresh_token_hash TEXT UNIQUE,
            expires_at TEXT NOT NULL,
            refresh_expires_at TEXT,
            user_agent TEXT,
            ip_address TEXT,
            is_active BOOLEAN DEFAULT 1,
            last_activity_at TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        """)

        # 创建认证日志表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS auth_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER,
            username TEXT,
            event_type TEXT NOT NULL,
            ip_address TEXT,
            user_agent TEXT,
            success BOOLEAN NOT NULL,
            error_message TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
        )
        """)

        # 创建用户相关索引
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(is_active, expires_at)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_auth_logs_user ON auth_logs(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_auth_logs_event ON auth_logs(event_type, created_at)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_auth_logs_created ON auth_logs(created_at)")

        # 创建操作审计日志表
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_type TEXT NOT NULL,
            operation_target TEXT NOT NULL,
            target_id INTEGER,
            details TEXT,
            affected_count INTEGER DEFAULT 0,
            ip_address TEXT,
            user_agent TEXT,
            session_id TEXT,
            status TEXT NOT NULL,
            error_message TEXT,
            created_at TEXT NOT NULL
        )
        """)

        # 创建审计日志索引
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_logs_type "
            "ON audit_logs(operation_type, created_at)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_logs_target "
            "ON audit_logs(operation_target, target_id)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at DESC)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_logs_status ON audit_logs(status)")

        # 创建应用配置表（用于存储密码等安全配置）
        await conn.execute("""
        CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL UNIQUE,
            value TEXT,
            value_type TEXT DEFAULT 'string',
            description TEXT,
            is_encrypted BOOLEAN DEFAULT 0,
            updated_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
        """)

        await conn.execute("CREATE INDEX IF NOT EXISTS idx_app_settings_key ON app_settings(key)")

        # 为bills表添加模板和定期账单关联字段（如果不存在）
        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN created_from_template INTEGER")
            self.logger.info("成功为bills表添加created_from_template字段")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有created_from_template字段")

        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN created_from_recurring INTEGER")
            self.logger.info("成功为bills表添加created_from_recurring字段")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有created_from_recurring字段")

        # 为bills表添加导入来源字段（如果不存在）
        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN import_history_id INTEGER")
            self.logger.info("成功为bills表添加import_history_id字段")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有import_history_id字段")

        # 为bills表添加转账/投资相关字段（如果不存在）
        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN destination_amount REAL DEFAULT 0")
            self.logger.info("成功为bills表添加destination_amount字段（用于转账/投资）")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有destination_amount字段")

        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN destination_account_id INTEGER DEFAULT 0")
            self.logger.info("成功为bills表添加destination_account_id字段（用于转账/投资）")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有destination_account_id字段")

        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN source_account_id INTEGER DEFAULT 0")
            self.logger.info("成功为bills表添加source_account_id字段（用于账单-账户关联）")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有source_account_id字段")

        await conn.commit()

        self.logger.info("数据库初始化完成")

    async def _migrate_user_id_field(self, conn: aiosqlite.Connection, table_name: str) -> None:
        """
        为表添加 user_id 字段（如果不存在）

        Args:
            conn: 数据库连接
            table_name: 表名
        """
        try:
            cursor = await conn.execute(f"PRAGMA table_info({table_name})")
            columns = [row[1] for row in await cursor.fetchall()]

            if 'user_id' not in columns:
                self.logger.info(f"为 {table_name} 表添加 user_id 字段")
                await conn.execute(
                    f"ALTER TABLE {table_name} ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1"
                )
                self.logger.info(f"成功为 {table_name} 表添加 user_id 字段")
            else:
                self.logger.debug(f"{table_name} 表已有 user_id 字段")
        except Exception as e:
            self.logger.error(f"为 {table_name} 表添加 user_id 字段失败: {e}")

    @log_method
    async def _get_connection(self) -> aiosqlite.Connection:
        """获取数据库连接"""
        if self._connection is None:
            self._connection = await aiosqlite.connect(str(self.db_path))
            self._connection.row_factory = aiosqlite.Row
        return self._connection

    @log_method
    def _calculate_hash(self, bill: Dict[str, Any]) -> str:
        """
        计算账单哈希值用于去重

        Args:
        bill: 账单数据

        Returns:
        str: 哈希值
        """
        # 使用关键字段生成哈希
        key_fields = [
        str(bill.get('date', '')),
        str(bill.get('type', '')),
        str(bill.get('amount', '')),
        str(bill.get('counterparty', '')),
        str(bill.get('description', ''))
        ]
        key_string = '|'.join(key_fields)
        return hashlib.md5(key_string.encode('utf-8')).hexdigest()

    @log_method
    async def insert_bills(self, bills: List[Dict[str, Any]], batch_id: Optional[str] = None,
                          user_id: int = 1) -> int:
        """
        批量插入账单

        Args:
        bills: 账单列表
        batch_id: 批次ID
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            int: 成功插入的数量
        """
        if not bills:
            self.logger.warning("账单列表为空")
            return 0

        self.logger.info(f"准备插入 {len(bills)} 条账单 (user_id={user_id})")

        if batch_id is None:
            batch_id = datetime.now().strftime('%Y%m%d%H%M%S')
        conn = await self._get_connection()
        inserted_count = 0

        # 分批插入
        for i in range(0, len(bills), self.batch_size):
            batch = bills[i:i + self.batch_size]
            self.logger.debug(f"插入批次 {i//self.batch_size + 1}: {len(batch)} 条")

            for bill in batch:
                try:
                    # 计算哈希
                    bill_hash = self._calculate_hash(bill)

                    # 准备数据
                    now = datetime.now().isoformat()

                    await conn.execute("""
                        INSERT INTO bills (
                            user_id, date, type, amount, counterparty, description,
                            main_category, sub_category, batch_id, hash,
                            created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """, (
                        user_id,
                        bill.get('date'),
                        bill.get('type'),
                        bill.get('amount'),
                        bill.get('counterparty'),
                        bill.get('description'),
                        bill.get('main_category'),
                        bill.get('sub_category'),
                        batch_id,
                        bill_hash,
                        now,
                        now
                    ))

                    inserted_count += 1

                except sqlite3.IntegrityError:
                    self.logger.debug(f"跳过重复账单: {bill.get('date')} {bill.get('description')}")
                except Exception as e:
                    self.logger.error(f"插入账单失败: {e}")

            # 提交批次
            await conn.commit()

        self.logger.info(f"成功插入 {inserted_count} 条账单")
        return inserted_count

    @log_method
    async def insert_bill(self, bill: Dict[str, Any], user_id: int = 1) -> int:
        """
        插入单条账单

        Args:
        bill: 账单数据
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
        int: 账单ID
        """
        conn = await self._get_connection()

        # 计算哈希
        bill_hash = self._calculate_hash(bill)
        now = datetime.now().isoformat()

        cursor = await conn.execute("""
        INSERT INTO bills (
        user_id, date, type, amount, counterparty, description, channel,
        main_category, sub_category, comment, account, tag,
        created_from_template, created_from_recurring, import_history_id,
        hash, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
        user_id,
        bill.get('date'),
        bill.get('type'),
        bill.get('amount'),
        bill.get('counterparty'),
        bill.get('description'),
        bill.get('channel'),
        bill.get('category'),  # main_category
        bill.get('sub_category'),
        bill.get('comment'),
        bill.get('account'),
        bill.get('tag'),
        bill.get('created_from_template'),
        bill.get('created_from_recurring'),
        bill.get('import_history_id'),
        bill_hash,
        now,
        now
        ))

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def check_duplicate(self, bill: Dict[str, Any], user_id: int = 1) -> bool:
        """
        检查账单是否重复

        Args:
        bill: 账单数据
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
        bool: 是否重复
        """
        bill_hash = self._calculate_hash(bill)
        conn = await self._get_connection()

        cursor = await conn.execute(
        "SELECT COUNT(*) as count FROM bills WHERE user_id = ? AND hash = ?",
        (user_id, bill_hash,)
        )

        row = await cursor.fetchone()
        return row['count'] > 0

    @log_method
    async def get_bills(self, filters: Optional[Dict[str, Any]] = None,
           limit: Optional[int] = None,
           offset: int = 0,
           user_id: int = 1) -> List[Dict[str, Any]]:
        # pylint: disable=too-many-branches
        """
        查询账单

        支持高级筛选:
        - 金额范围筛选: min_amount, max_amount
        - 金额过滤器: amount_filter (eq:100, ne:100, gt:100, lt:100, gte:100, lte:100, between:100:200)
        - 多条件组合: type, main_category, sub_category, date_from, date_to, keyword等

        Args:
        filters: 过滤条件字典
        - id: 账单ID
        - date_from/date_to: 日期范围
        - start_date/end_date: 日期范围(别名)
        - type: 账单类型('支出'/'收入')
        - main_category: 主分类
        - sub_category: 子分类
        - batch_id: 批次ID
        - counterparty: 交易对方(支持模糊匹配)
        - description: 描述(支持模糊匹配)
        - keyword: 关键词(在描述和交易对方中搜索)
        - min_amount: 最小金额
        - max_amount: 最大金额
        - amount_filter: 金额过滤器(格式: "类型:值1[:值2]")
        limit: 限制返回数量
        offset: 偏移量
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
        List[Dict]: 账单列表
        """
        conn = await self._get_connection()

        # 构建查询 - 添加 user_id 过滤
        query = "SELECT * FROM bills WHERE user_id = ?"
        params = [user_id]

        if filters:
            # ID筛选
            if 'id' in filters:
                query += " AND id = ?"
                params.append(filters['id'])

            # 日期范围筛选(支持多种参数名)
            date_from = filters.get('date_from') or filters.get('start_date')
            if date_from:
                query += " AND date >= ?"
                params.append(date_from)
                self.logger.debug(f"日期筛选: >= {date_from}")

            date_to = filters.get('date_to') or filters.get('end_date')
            if date_to:
                query += " AND date <= ?"
                params.append(date_to)
                self.logger.debug(f"日期筛选: <= {date_to}")

            # 类型筛选
            if 'type' in filters:
                query += " AND type = ?"
                params.append(filters['type'])
                self.logger.debug(f"类型筛选: {filters['type']}")

            # 分类筛选
            if 'main_category' in filters:
                query += " AND main_category = ?"
                params.append(filters['main_category'])
                self.logger.debug(f"主分类筛选: {filters['main_category']}")

            if 'sub_category' in filters:
                query += " AND sub_category = ?"
                params.append(filters['sub_category'])
                self.logger.debug(f"子分类筛选: {filters['sub_category']}")

            # 批次ID筛选
            if 'batch_id' in filters:
                query += " AND batch_id = ?"
                params.append(filters['batch_id'])

            # 交易对方模糊匹配
            if 'counterparty' in filters:
                query += " AND counterparty LIKE ?"
                params.append(f"%{filters['counterparty']}%")
                self.logger.debug(f"交易对方筛选: %{filters['counterparty']}%")

            # 描述模糊匹配
            if 'description' in filters:
                query += " AND description LIKE ?"
                params.append(f"%{filters['description']}%")
                self.logger.debug(f"描述筛选: %{filters['description']}%")

            # 关键词搜索(在描述和交易对方中)
            if 'keyword' in filters and filters['keyword']:
                query += " AND (description LIKE ? OR counterparty LIKE ?)"
                keyword_pattern = f"%{filters['keyword']}%"
                params.extend([keyword_pattern, keyword_pattern])
                self.logger.debug(f"关键词搜索: {keyword_pattern}")

            # 账户筛选 (列表)
            if 'account_ids' in filters and filters['account_ids']:
                account_ids = filters['account_ids']
                if isinstance(account_ids, list) and account_ids:
                    placeholders = ','.join(['?'] * len(account_ids))
                    # 只要源账户或目标账户在列表中即可
                    query += f" AND (source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
                    params.extend(account_ids)
                    params.extend(account_ids)
                    self.logger.debug(f"账户筛选: {account_ids}")

            # 分类筛选 (列表 - 包含主分类和子分类的元组或字典)
            # 格式: [{'main': '餐饮', 'sub': '早餐'}, {'main': '交通', 'sub': ''}]
            if 'categories' in filters and filters['categories']:
                categories = filters['categories']
                if isinstance(categories, list) and categories:
                    cat_conditions = []
                    for cat in categories:
                        main = cat.get('main')
                        sub = cat.get('sub')
                        if main and sub:
                            cat_conditions.append("(main_category = ? AND sub_category = ?)")
                            params.extend([main, sub])
                        elif main:
                            cat_conditions.append("(main_category = ?)")
                            params.append(main)

                    if cat_conditions:
                        query += " AND (" + " OR ".join(cat_conditions) + ")"
                        self.logger.debug(f"分类列表筛选: {len(categories)}个分类")

            # 标签筛选
            if 'tag_ids' in filters and filters['tag_ids']:
                tag_ids = filters['tag_ids']
                if isinstance(tag_ids, list) and tag_ids:
                    placeholders = ','.join(['?'] * len(tag_ids))
                    query += f" AND id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))"
                    params.extend(tag_ids)
                    self.logger.debug(f"标签筛选: {tag_ids}")

            # 金额范围筛选
            if 'min_amount' in filters and filters['min_amount'] is not None:
                query += " AND amount >= ?"
                params.append(float(filters['min_amount']))
                self.logger.debug(f"最小金额筛选: >= {filters['min_amount']}")

            if 'max_amount' in filters and filters['max_amount'] is not None:
                query += " AND amount <= ?"
                params.append(float(filters['max_amount']))
                self.logger.debug(f"最大金额筛选: <= {filters['max_amount']}")

            # 金额过滤器(高级筛选)
            # 格式: "类型:值1[:值2]"
            # 支持类型: eq(等于), ne(不等于), gt(大于), lt(小于), gte(大于等于), lte(小于等于), between(范围)
            if 'amount_filter' in filters and filters['amount_filter']:
                amount_filter = filters['amount_filter']
                self.logger.debug(f"金额过滤器: {amount_filter}")

                parts = amount_filter.split(':')
                if len(parts) >= 2:
                    filter_type = parts[0].lower()

                    try:
                        if filter_type == 'eq':  # 等于
                            value = float(parts[1])
                            query += " AND amount = ?"
                            params.append(value)
                            self.logger.debug(f"金额等于: {value}")

                        elif filter_type == 'ne':  # 不等于
                            value = float(parts[1])
                            query += " AND amount != ?"
                            params.append(value)
                            self.logger.debug(f"金额不等于: {value}")

                        elif filter_type == 'gt':  # 大于
                            value = float(parts[1])
                            query += " AND amount > ?"
                            params.append(value)
                            self.logger.debug(f"金额大于: {value}")

                        elif filter_type == 'lt':  # 小于
                            value = float(parts[1])
                            query += " AND amount < ?"
                            params.append(value)
                            self.logger.debug(f"金额小于: {value}")

                        elif filter_type == 'gte':  # 大于等于
                            value = float(parts[1])
                            query += " AND amount >= ?"
                            params.append(value)
                            self.logger.debug(f"金额大于等于: {value}")

                        elif filter_type == 'lte':  # 小于等于
                            value = float(parts[1])
                            query += " AND amount <= ?"
                            params.append(value)
                            self.logger.debug(f"金额小于等于: {value}")

                        elif filter_type == 'between' and len(parts) >= 3:  # 范围
                            value1 = float(parts[1])
                            value2 = float(parts[2])
                            query += " AND amount BETWEEN ? AND ?"
                            params.extend([value1, value2])
                            self.logger.debug(f"金额范围: {value1} - {value2}")

                        else:
                            self.logger.warning(f"未知的金额过滤器类型: {filter_type}")

                    except (ValueError, IndexError) as e:
                        self.logger.error(f"金额过滤器格式错误: {amount_filter}, 错误: {e}")

        query += " ORDER BY date DESC"

        if limit:
            query += f" LIMIT {limit} OFFSET {offset}"

        self.logger.debug(f"执行查询: {query}")
        self.logger.debug(f"查询参数: {params}")

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            bills = [dict(row) for row in rows]

        self.logger.info(f"查询到 {len(bills)} 条账单")
        return bills

    @log_method
    async def create_bill(self, bill_data: Dict[str, Any], user_id: int = 1) -> Optional[int]:
        """
        创建单条账单

        Args:
            bill_data: 账单数据字典
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            Optional[int]: 创建的账单ID,失败返回None
        """
        conn = await self._get_connection()

        try:
            # 必填字段
            required_fields = ['date', 'type', 'amount', 'description']
            for field in required_fields:
                if field not in bill_data:
                    self.logger.error(f"缺少必填字段: {field}")
                    return None

            # 添加 user_id 到账单数据
            bill_data['user_id'] = user_id

            # 构建INSERT语句
            columns = list(bill_data.keys())
            placeholders = ', '.join(['?' for _ in columns])
            columns_str = ', '.join(columns)

            query = f"INSERT INTO bills ({columns_str}) VALUES ({placeholders})"
            values = [bill_data[col] for col in columns]

            cursor = await conn.execute(query, values)
            await conn.commit()

            bill_id = cursor.lastrowid
            self.logger.info(f"已创建账单 ID: {bill_id}")
            return bill_id

        except Exception as e:
            self.logger.error(f"创建账单失败: {e}")
            return None

    @log_method
    async def delete_bill(self, bill_id: int, user_id: int = 1) -> bool:
        """
        删除账单

        Args:
        bill_id: 账单ID
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
        bool: 是否删除成功
        """
        conn = await self._get_connection()

        try:
            await conn.execute("DELETE FROM bills WHERE id = ? AND user_id = ?", (bill_id, user_id))
            await conn.commit()

            self.logger.info(f"已删除账单 ID: {bill_id} (user_id={user_id})")
            return True

        except Exception as e:
            self.logger.error(f"删除账单失败 ID={bill_id}: {e}")
            return False

    @log_method
    async def update_bill(self, bill_id: int, updates: Dict[str, Any], user_id: int = 1) -> bool:
        """
        更新账单

        Args:
        bill_id: 账单ID
        updates: 更新字段字典
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
        bool: 是否更新成功
        """
        if not updates:
            return False
        conn = await self._get_connection()

        try:
            # 构建更新语句
            set_clause = ", ".join(f"{key} = ?" for key in updates.keys())
            set_clause += ", updated_at = ?"

            values = list(updates.values())
            values.append(datetime.now().isoformat())
            values.append(bill_id)
            values.append(user_id)

            query = f"UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?"

            await conn.execute(query, values)
            await conn.commit()

            self.logger.info(f"已更新账单 ID: {bill_id} (user_id={user_id})")
            return True

        except Exception as e:
            self.logger.error(f"更新账单失败 ID={bill_id}: {e}")
            return False

    @log_method
    async def move_all_transactions(self, from_account_id: int, to_account_id: int) -> Dict[str, Any]:
        """
        将一个账户的所有交易移动到另一个账户

        注意：bills表使用source_account_id和destination_account_id双边账户系统
        需要同时更新两个字段

        Args:
            from_account_id: 源账户ID
            to_account_id: 目标账户ID

        Returns:
            Dict: 包含成功数和失败数的统计信息
        """
        conn = await self._get_connection()

        try:
            # 先检查两个账户是否存在
            cursor = await conn.execute(
                "SELECT id, name FROM accounts WHERE id IN (?, ?)",
                (from_account_id, to_account_id)
            )
            accounts = await cursor.fetchall()

            if len(accounts) != 2:
                self.logger.error(f"账户不存在: from={from_account_id}, to={to_account_id}")
                return {'success': False, 'message': '账户不存在', 'moved_count': 0}

            # 查询需要移动的交易数量（source_account_id或destination_account_id匹配）
            cursor = await conn.execute(
                "SELECT COUNT(*) FROM bills WHERE source_account_id = ? OR destination_account_id = ?",
                (from_account_id, from_account_id)
            )
            count = (await cursor.fetchone())[0]

            if count == 0:
                self.logger.info(f"源账户 {from_account_id} 没有交易记录")
                return {'success': True, 'moved_count': 0}

            # 执行批量更新 - 更新source_account_id
            now = datetime.now().isoformat()
            cursor = await conn.execute(
                "UPDATE bills SET source_account_id = ?, updated_at = ? WHERE source_account_id = ?",
                (to_account_id, now, from_account_id)
            )
            source_updated = cursor.rowcount

            # 执行批量更新 - 更新destination_account_id
            cursor = await conn.execute(
                "UPDATE bills SET destination_account_id = ?, updated_at = ? WHERE destination_account_id = ?",
                (to_account_id, now, from_account_id)
            )
            dest_updated = cursor.rowcount

            await conn.commit()

            total_updated = source_updated + dest_updated
            self.logger.info(
                f"成功将 {total_updated} 条交易从账户 {from_account_id} 移动到 {to_account_id} "
                f"(源账户更新:{source_updated}, 目标账户更新:{dest_updated})"
            )

            # 同步源账户和目标账户的余额
            self.logger.info(
                f"开始同步账户余额: 源账户={from_account_id}, 目标账户={to_account_id}"
            )

            # 获取同步前的余额
            cursor = await conn.execute(
                "SELECT id, name, balance FROM accounts WHERE id IN (?, ?)",
                (from_account_id, to_account_id)
            )
            accounts_before = {
                row['id']: {'name': row['name'], 'balance': row['balance'] or 0.0}
                for row in await cursor.fetchall()
            }

            # 同步源账户余额
            sync_result_from = await self.sync_account_balance(from_account_id)
            if sync_result_from:
                # 获取同步后的源账户余额
                cursor = await conn.execute(
                    "SELECT balance FROM accounts WHERE id = ?",
                    (from_account_id,)
                )
                row = await cursor.fetchone()
                new_balance_from = row['balance'] if row else 0.0
                old_balance_from = accounts_before.get(from_account_id, {}).get('balance', 0.0)
                self.logger.info(
                    f"源账户 {from_account_id} 余额已同步: {old_balance_from} → {new_balance_from}"
                )
            else:
                self.logger.warning(f"源账户 {from_account_id} 余额同步失败")

            # 同步目标账户余额
            sync_result_to = await self.sync_account_balance(to_account_id)
            if sync_result_to:
                # 获取同步后的目标账户余额
                cursor = await conn.execute(
                    "SELECT balance FROM accounts WHERE id = ?",
                    (to_account_id,)
                )
                row = await cursor.fetchone()
                new_balance_to = row['balance'] if row else 0.0
                old_balance_to = accounts_before.get(to_account_id, {}).get('balance', 0.0)
                self.logger.info(
                    f"目标账户 {to_account_id} 余额已同步: {old_balance_to} → {new_balance_to}"
                )
            else:
                self.logger.warning(f"目标账户 {to_account_id} 余额同步失败")

            # 清除账户映射缓存，确保前端获取最新余额
            self._clear_cache('account_mappings')

            return {'success': True, 'moved_count': total_updated}

        except Exception as e:
            self.logger.error(f"移动交易失败: from={from_account_id}, to={to_account_id}, error={e}")
            await conn.rollback()
            return {'success': False, 'message': str(e), 'moved_count': 0}

    @log_method
    async def delete_all_transactions_by_account(self, account_id: int) -> Dict[str, Any]:
        """
        删除指定账户的所有交易

        注意：bills表使用source_account_id和destination_account_id双边账户系统
        需要删除source_account_id或destination_account_id匹配的所有记录

        Args:
            account_id: 账户ID

        Returns:
            Dict: 包含成功状态和删除数量的统计信息
        """
        conn = await self._get_connection()

        try:
            # 先检查账户是否存在
            cursor = await conn.execute(
                "SELECT id, name FROM accounts WHERE id = ?",
                (account_id,)
            )
            account = await cursor.fetchone()

            if not account:
                self.logger.error(f"账户不存在: {account_id}")
                return {'success': False, 'message': '账户不存在', 'deleted_count': 0}

            # 查询需要删除的交易数量（source_account_id或destination_account_id匹配）
            cursor = await conn.execute(
                "SELECT COUNT(*) FROM bills WHERE source_account_id = ? OR destination_account_id = ?",
                (account_id, account_id)
            )
            count = (await cursor.fetchone())[0]

            if count == 0:
                self.logger.info(f"账户 {account_id} 没有交易记录")
                return {'success': True, 'deleted_count': 0}

            # 执行批量删除
            await conn.execute(
                "DELETE FROM bills WHERE source_account_id = ? OR destination_account_id = ?",
                (account_id, account_id)
            )

            await conn.commit()

            self.logger.info(f"成功删除账户 {account_id} 的 {count} 条交易")

            # 同步账户余额
            self.logger.info(f"开始同步账户余额: account_id={account_id}")

            # 获取同步前的余额
            cursor = await conn.execute(
                "SELECT name, balance FROM accounts WHERE id = ?",
                (account_id,)
            )
            row = await cursor.fetchone()
            if row:
                account_name = row['name']
                old_balance = row['balance'] or 0.0

                # 同步账户余额
                sync_result = await self.sync_account_balance(account_id)
                if sync_result:
                    # 获取同步后的余额
                    cursor = await conn.execute(
                        "SELECT balance FROM accounts WHERE id = ?",
                        (account_id,)
                    )
                    row = await cursor.fetchone()
                    new_balance = row['balance'] if row else 0.0
                    self.logger.info(
                        f"账户 {account_id} ({account_name}) "
                        f"余额已同步: {old_balance} → {new_balance}"
                    )
                else:
                    self.logger.warning(f"账户 {account_id} 余额同步失败")

                # 清除账户映射缓存，确保前端获取最新余额
                self._clear_cache('account_mappings')
            else:
                self.logger.warning(f"未找到账户 {account_id}，无法同步余额")

            return {'success': True, 'deleted_count': count}

        except Exception as e:
            self.logger.error(f"删除账户交易失败: account_id={account_id}, error={e}")
            await conn.rollback()
            return {'success': False, 'message': str(e), 'deleted_count': 0}

    @log_method
    async def batch_update_bills(self, bill_ids: List[int], updates: Dict[str, Any]) -> Dict[str, Any]:
        """
        批量更新账单

        Args:
        bill_ids: 账单ID列表
        updates: 更新字段字典

        Returns:
        Dict: 包含成功数、失败数和失败ID列表的统计信息
        """
        if not bill_ids:
            self.logger.warning("账单ID列表为空")
            return {'success_count': 0, 'failed_count': 0, 'failed_ids': []}

        if not updates:
            self.logger.warning("更新字段为空")
            return {'success_count': 0, 'failed_count': 0, 'failed_ids': []}

        self.logger.info(f"准备批量更新 {len(bill_ids)} 条账单")
        self.logger.debug(f"更新字段: {list(updates.keys())}")
        conn = await self._get_connection()
        success_count = 0
        failed_ids = []

        # 构建更新语句
        set_clause = ", ".join(f"{key} = ?" for key in updates.keys())
        set_clause += ", updated_at = ?"
        now = datetime.now().isoformat()

        for i, bill_id in enumerate(bill_ids, 1):
            try:
                self.logger.debug(f"更新进度: {i}/{len(bill_ids)}, bill_id={bill_id}")

                values = list(updates.values())
                values.append(now)
                values.append(bill_id)

                query = f"UPDATE bills SET {set_clause} WHERE id = ?"

                cursor = await conn.execute(query, values)

                if cursor.rowcount > 0:
                    success_count += 1
                    self.logger.debug(f"账单更新成功: ID={bill_id}")
                else:
                    failed_ids.append(bill_id)
                    self.logger.warning(f"账单不存在或未更新: ID={bill_id}")

            except Exception as e:
                failed_ids.append(bill_id)
                self.logger.error(f"更新账单失败 ID={bill_id}: {e}")

        await conn.commit()

        result = {
        'success_count': success_count,
        'failed_count': len(failed_ids),
        'failed_ids': failed_ids
        }

        self.logger.info(f"批量更新完成: 成功={success_count}, 失败={len(failed_ids)}")
        return result

    @log_method
    async def batch_update_categories(self, bill_ids: List[int],
                         main_category: str,
                         sub_category: str) -> Dict[str, Any]:
        """
        批量修改账单分类

        Args:
        bill_ids: 账单ID列表
        main_category: 主分类
        sub_category: 子分类

        Returns:
        Dict: 包含成功数、失败数和失败ID列表的统计信息
        """
        if not bill_ids:
            self.logger.warning("账单ID列表为空")
            return {'success_count': 0, 'failed_count': 0, 'failed_ids': []}

        self.logger.info(f"准备批量修改 {len(bill_ids)} 条账单的分类")
        self.logger.debug(f"目标分类: {main_category}/{sub_category}")

        updates = {
            'main_category': main_category,
            'sub_category': sub_category
        }

        return await self.batch_update_bills(bill_ids, updates)

    @log_method
    @log_step("去除重复账单")
    async def deduplicate(self) -> int:
        """
        去除重复账单

        Returns:
        int: 删除的重复账单数量
        """
        conn = await self._get_connection()

        # 找出重复的账单（保留最早的）
        query = """
            DELETE FROM bills
            WHERE id NOT IN (
                SELECT MIN(id)
                FROM bills
                GROUP BY hash
            )
        """

        cursor = await conn.execute(query)
        deleted_count = cursor.rowcount
        await conn.commit()

        self.logger.info(f"已删除 {deleted_count} 条重复账单")
        return deleted_count

    @log_method
    async def get_statistics(self) -> Dict[str, Any]:
        """
        获取数据库统计信息

        Returns:
        Dict: 统计信息
        """
        conn = await self._get_connection()

        stats = {}

        # 总账单数
        async with conn.execute("SELECT COUNT(*) FROM bills") as cursor:
            row = await cursor.fetchone()
            stats['total_bills'] = row[0]

        # 按类型统计
        async with conn.execute("""
            SELECT type, COUNT(*) as count, SUM(amount) as total
            FROM bills
            GROUP BY type
        """) as cursor:
            rows = await cursor.fetchall()
            stats['by_type'] = {row[0]: {'count': row[1], 'total': row[2]} for row in rows}

        # 按分类统计
        async with conn.execute("""
            SELECT main_category, COUNT(*) as count, SUM(amount) as total
            FROM bills
            WHERE main_category IS NOT NULL
            GROUP BY main_category
        """) as cursor:
            rows = await cursor.fetchall()
            stats['by_category'] = {row[0]: {'count': row[1], 'total': row[2]} for row in rows}

        self.logger.info("获取统计信息成功")
        return stats

    @log_method
    async def save_filter(self, name: str, filter_data: Dict[str, Any],
             description: Optional[str] = None) -> int:
        """
        保存筛选条件

        Args:
        name: 筛选条件名称(唯一)
        filter_data: 筛选条件数据(字典格式)
        description: 描述

        Returns:
        int: 保存的筛选条件ID
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        # 将筛选条件转为JSON字符串
        filter_json = json.dumps(filter_data, ensure_ascii=False)

        self.logger.info(f"保存筛选条件: {name}")
        self.logger.debug(f"筛选条件数据: {filter_json}")

        try:
            # 尝试插入
            cursor = await conn.execute("""
                INSERT INTO saved_filters (name, filter_data, description, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?)
            """, (name, filter_json, description, now, now))

            await conn.commit()
            filter_id = cursor.lastrowid
            self.logger.info(f"筛选条件保存成功: ID={filter_id}, 名称={name}")
            return filter_id

        except sqlite3.IntegrityError:
            # 如果名称已存在,则更新
            self.logger.info(f"筛选条件名称已存在,执行更新: {name}")
            await conn.execute("""
                UPDATE saved_filters
                SET filter_data = ?, description = ?, updated_at = ?
                WHERE name = ?
            """, (filter_json, description, now, name))

            await conn.commit()

            # 获取ID
            async with conn.execute(
                "SELECT id FROM saved_filters WHERE name = ?", (name,)
            ) as cursor:
                row = await cursor.fetchone()
                filter_id = row[0] if row else 0

            self.logger.info(f"筛选条件更新成功: ID={filter_id}, 名称={name}")
            return filter_id

    @log_method
    async def get_saved_filters(self) -> List[Dict[str, Any]]:
        """
        获取所有保存的筛选条件

        Returns:
        List[Dict]: 筛选条件列表
        """
        conn = await self._get_connection()

        self.logger.info("查询所有保存的筛选条件")

        async with conn.execute("""
            SELECT id, name, filter_data, description, created_at, updated_at
            FROM saved_filters
            ORDER BY updated_at DESC
        """) as cursor:
            rows = await cursor.fetchall()

        filters = []
        for row in rows:
            filter_dict = dict(row)
            # 将JSON字符串解析为字典
            filter_dict['filter_data'] = json.loads(filter_dict['filter_data'])
            filters.append(filter_dict)

        self.logger.info(f"查询到 {len(filters)} 个保存的筛选条件")
        return filters

    @log_method
    async def get_saved_filter(self, filter_id: Optional[int] = None,
                  name: Optional[str] = None) -> Optional[Dict[str, Any]]:
        """
        获取单个保存的筛选条件

        Args:
        filter_id: 筛选条件ID
        name: 筛选条件名称

        Returns:
        Optional[Dict]: 筛选条件,不存在时返回None
        """
        if not filter_id and not name:
            self.logger.warning("必须提供filter_id或name参数")
            return None
        conn = await self._get_connection()

        if filter_id:
            self.logger.info(f"查询筛选条件: ID={filter_id}")
            query = """
                SELECT id, name, filter_data, description, created_at, updated_at
                FROM saved_filters
                WHERE id = ?
            """
            params = (filter_id,)
        else:
            self.logger.info(f"查询筛选条件: 名称={name}")
            query = """
                SELECT id, name, filter_data, description, created_at, updated_at
                FROM saved_filters
                WHERE name = ?
            """
            params = (name,)

        async with conn.execute(query, params) as cursor:
            row = await cursor.fetchone()

        if row:
            filter_dict = dict(row)
            filter_dict['filter_data'] = json.loads(filter_dict['filter_data'])
            self.logger.info(f"查询到筛选条件: {filter_dict['name']}")
            return filter_dict

        self.logger.info("筛选条件不存在")
        return None

    @log_method
    async def delete_saved_filter(self, filter_id: Optional[int] = None,
                      name: Optional[str] = None) -> bool:
        """
        删除保存的筛选条件

        Args:
        filter_id: 筛选条件ID
        name: 筛选条件名称

        Returns:
        bool: 是否删除成功
        """
        if not filter_id and not name:
            self.logger.warning("必须提供filter_id或name参数")
            return False
        conn = await self._get_connection()

        if filter_id:
            self.logger.info(f"删除筛选条件: ID={filter_id}")
            query = "DELETE FROM saved_filters WHERE id = ?"
            params = (filter_id,)
        else:
            self.logger.info(f"删除筛选条件: 名称={name}")
            query = "DELETE FROM saved_filters WHERE name = ?"
            params = (name,)

        cursor = await conn.execute(query, params)
        await conn.commit()

        if cursor.rowcount > 0:
            self.logger.info("筛选条件删除成功")
            return True

        self.logger.warning("筛选条件不存在或已被删除")
        return False

    @log_method
    async def close(self):
        """关闭数据库连接"""
        if self._connection:
            await self._connection.close()
            self._connection = None
            self.logger.info("数据库连接已关闭")

    @log_method
    async def __aenter__(self):
        """异步上下文管理器入口"""
        await self.init_db()
        return self

    @log_method
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """异步上下文管理器退出"""
        await self.close()

    # ==================== UI Backend API所需的额外方法 ====================

    @log_method
    async def query_bills(self, page: int = 1, page_size: int = 20,
                         filters: Optional[Dict[str, Any]] = None,
                         user_id: int = 1) -> tuple:
        """
        分页查询账单

        Args:
            page: 页码（从1开始）
            page_size: 每页数量
            filters: 过滤条件

        Returns:
            tuple: (账单列表, 总数)
        """
        offset = (page - 1) * page_size

        # 查询账单列表 - 传递 user_id 参数
        bills = await self.get_bills(filters=filters, limit=page_size, offset=offset, user_id=user_id)

        # 查询总数 - 添加 user_id 过滤
        conn = await self._get_connection()
        count_query = "SELECT COUNT(*) as total FROM bills WHERE user_id = ?"
        params = [user_id]

        if filters:
            if 'type' in filters:
                count_query += " AND type = ?"
                params.append(filters['type'])
            if 'main_category' in filters:
                count_query += " AND main_category = ?"
                params.append(filters['main_category'])
            if 'sub_category' in filters:
                count_query += " AND sub_category = ?"
                params.append(filters['sub_category'])
            if 'start_date' in filters:
                count_query += " AND date >= ?"
                params.append(filters['start_date'])
            if 'end_date' in filters:
                count_query += " AND date <= ?"
                params.append(filters['end_date'])
            if 'keyword' in filters:
                count_query += " AND (description LIKE ? OR counterparty LIKE ?)"
                keyword = f"%{filters['keyword']}%"
                params.extend([keyword, keyword])

            if 'account_ids' in filters and filters['account_ids']:
                account_ids = filters['account_ids']
                if isinstance(account_ids, list) and account_ids:
                    placeholders = ','.join(['?'] * len(account_ids))
                    count_query += f" AND (source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
                    params.extend(account_ids)
                    params.extend(account_ids)

            if 'categories' in filters and filters['categories']:
                categories = filters['categories']
                if isinstance(categories, list) and categories:
                    cat_conditions = []
                    for cat in categories:
                        main = cat.get('main')
                        sub = cat.get('sub')
                        if main and sub:
                            cat_conditions.append("(main_category = ? AND sub_category = ?)")
                            params.extend([main, sub])
                        elif main:
                            cat_conditions.append("(main_category = ?)")
                            params.append(main)

                    if cat_conditions:
                        count_query += " AND (" + " OR ".join(cat_conditions) + ")"

            # 标签筛选
            if 'tag_ids' in filters and filters['tag_ids']:
                tag_ids = filters['tag_ids']
                if isinstance(tag_ids, list) and tag_ids:
                    placeholders = ','.join(['?'] * len(tag_ids))
                    count_query += f" AND id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))"
                    params.extend(tag_ids)

            # 金额范围筛选
            if 'min_amount' in filters and filters['min_amount'] is not None:
                count_query += " AND amount >= ?"
                params.append(float(filters['min_amount']))

            if 'max_amount' in filters and filters['max_amount'] is not None:
                count_query += " AND amount <= ?"
                params.append(float(filters['max_amount']))

            # 金额过滤器(高级筛选)
            if 'amount_filter' in filters and filters['amount_filter']:
                amount_filter = filters['amount_filter']
                parts = amount_filter.split(':')
                if len(parts) >= 2:
                    filter_type = parts[0].lower()
                    try:
                        if filter_type == 'eq':
                            value = float(parts[1])
                            count_query += " AND amount = ?"
                            params.append(value)
                        elif filter_type == 'ne':
                            value = float(parts[1])
                            count_query += " AND amount != ?"
                            params.append(value)
                        elif filter_type == 'gt':
                            value = float(parts[1])
                            count_query += " AND amount > ?"
                            params.append(value)
                        elif filter_type == 'lt':
                            value = float(parts[1])
                            count_query += " AND amount < ?"
                            params.append(value)
                        elif filter_type == 'gte':
                            value = float(parts[1])
                            count_query += " AND amount >= ?"
                            params.append(value)
                        elif filter_type == 'lte':
                            value = float(parts[1])
                            count_query += " AND amount <= ?"
                            params.append(value)
                        elif filter_type == 'between' and len(parts) >= 3:
                            value1 = float(parts[1])
                            value2 = float(parts[2])
                            count_query += " AND amount BETWEEN ? AND ?"
                            params.extend([value1, value2])
                    except (ValueError, IndexError):
                        pass

        async with conn.execute(count_query, params) as cursor:
            row = await cursor.fetchone()
            total = row['total'] if row else 0

        return bills, total

    @log_method
    async def get_bill_by_id(self, bill_id: int, user_id: int = 1) -> Optional[Dict[str, Any]]:
        """
        根据ID获取账单

        Args:
            bill_id: 账单ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            Optional[Dict]: 账单数据，不存在返回None
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM bills WHERE id = ? AND user_id = ?",
            (bill_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def batch_delete_bills(self, bill_ids: List[int]) -> int:
        """
        批量删除账单

        Args:
            bill_ids: 账单ID列表

        Returns:
            int: 删除的数量
        """
        if not bill_ids:
            return 0

        conn = await self._get_connection()
        placeholders = ','.join(['?' for _ in bill_ids])

        cursor = await conn.execute(
            f"DELETE FROM bills WHERE id IN ({placeholders})",
            bill_ids
        )
        await conn.commit()

        deleted = cursor.rowcount
        self.logger.info(f"批量删除了 {deleted} 条账单")
        return deleted

    @log_method
    async def get_all_categories(self, user_id: int = 1) -> List[Dict[str, Any]]:
        """
        获取所有分类 (从 categories 表)

        Args:
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            List[Dict]: 分类列表
        """
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row

        # 优先从 categories 表获取 (priority ASC: 优先级越小越靠前)
        self.logger.info(f"[get_all_categories] 查询分类 (user_id={user_id})，排序：priority ASC")
        async with conn.execute(
            "SELECT * FROM categories WHERE user_id = ? ORDER BY priority ASC, main_category, sub_category",
            (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            categories = [dict(row) for row in rows]

        self.logger.info(f"[get_all_categories] 返回 {len(categories)} 个分类")
        if categories:
            # 记录前5个分类的排序信息
            for i, cat in enumerate(categories[:5]):
                self.logger.debug(
                    f"[get_all_categories] #{i+1} 分类: "
                    f"{cat.get('main_category')}/{cat.get('sub_category')}, "
                    f"priority={cat.get('priority', 0)}"
                )

        # 如果 categories 表为空，尝试从 bills 表提取(兼容旧数据)
        if not categories:
            self.logger.info("categories 表为空，从 bills 表提取分类")
            async with conn.execute(
                "SELECT DISTINCT main_category, sub_category FROM bills "
                "WHERE main_category IS NOT NULL "
                "ORDER BY main_category, sub_category"
            ) as cursor:
                rows = await cursor.fetchall()
                # 构造临时分类对象
                categories = []
                for row in rows:
                    categories.append({
                        'id': 0, # 虚拟ID
                        'main_category': row[0],
                        'sub_category': row[1],
                        'description': '',
                        'priority': 0,
                        'keywords': ''
                    })

        return categories

    @log_method
    async def get_category_by_name(self, main_category: str, sub_category: str, 
                                   user_id: int = 1) -> Optional[Dict[str, Any]]:
        """根据名称获取分类
        
        Args:
            main_category: 主分类名称
            sub_category: 子分类名称
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM categories WHERE main_category = ? AND sub_category = ? AND user_id = ?",
            (main_category, sub_category, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_category(self, category_data: Dict[str, Any], user_id: int = 1) -> Optional[int]:
        """
        创建分类

        Args:
            category_data: 分类数据字典，包含main_category, sub_category等字段
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            Optional[int]: 创建成功返回分类ID，失败返回None
        """
        conn = await self._get_connection()

        main_cat = category_data.get('main_category')
        sub_cat = category_data.get('sub_category', '')

        self.logger.info(f"开始创建分类: {main_cat}/{sub_cat} (user_id={user_id})")

        try:
            columns = ['type', 'main_category', 'sub_category', 'description', 'priority', 'keywords', 'hidden', 'icon', 'color', 'created_at', 'user_id']
            placeholders = ', '.join(['?' for _ in columns])

            values = [
                category_data.get('type', 1),
                main_cat,
                sub_cat,
                category_data.get('description', ''),
                category_data.get('priority', 0),
                category_data.get('keywords', ''),
                category_data.get('hidden', False),
                category_data.get('icon', ''),
                category_data.get('color', ''),
                datetime.now().isoformat(),
                user_id
            ]

            self.logger.debug(f"执行插入: columns={columns}, values={values}")

            cursor = await conn.execute(
                f"INSERT INTO categories ({', '.join(columns)}) VALUES ({placeholders})",
                values
            )
            await conn.commit()

            cat_id = cursor.lastrowid
            self.logger.info(f"创建分类成功: ID={cat_id}, {main_cat}/{sub_cat}")
            return cat_id
        except sqlite3.IntegrityError as e:
            self.logger.warning(f"分类已存在（UNIQUE约束）: {main_cat}/{sub_cat} - {e}")
            return None
        except Exception as e:
            self.logger.error(f"创建分类失败: {type(e).__name__}: {e}", exc_info=True)
            return None    @log_method
    async def update_category(self, category_id: int, updates: Dict[str, Any], 
                              user_id: int = 1) -> bool:
        """更新分类
        
        Args:
            category_id: 分类ID
            updates: 更新数据字典
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        try:
            # 过滤掉无效字段
            valid_fields = ['type', 'main_category', 'sub_category', 'description', 'priority', 'keywords', 'hidden', 'icon', 'color']
            safe_updates = {k: v for k, v in updates.items() if k in valid_fields}

            if not safe_updates:
                return False

            set_clause = ", ".join(f"{k} = ?" for k in safe_updates.keys())
            values = list(safe_updates.values())
            values.extend([category_id, user_id])

            await conn.execute(f"UPDATE categories SET {set_clause} WHERE id = ? AND user_id = ?", values)
            await conn.commit()

            self.logger.info(f"已更新分类 ID: {category_id} (user_id={user_id})")
            return True
        except Exception as e:
            self.logger.error(f"更新分类失败: {e}")
            return False

    @log_method
    async def delete_category(self, category_id: int, user_id: int = 1) -> bool:
        """删除分类（级联删除子分类）

        如果删除的是父级分类（sub_category为空），则会自动删除该主分类下的所有子分类

        Args:
            category_id: 分类ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            bool: 删除成功返回True，失败返回False
        """
        conn = await self._get_connection()
        try:
            # 先查询要删除的分类信息 (限制user_id)
            conn.row_factory = aiosqlite.Row
            async with conn.execute(
                "SELECT id, main_category, sub_category FROM categories WHERE id = ? AND user_id = ?",
                (category_id, user_id)
            ) as cursor:
                category = await cursor.fetchone()

            if not category:
                self.logger.warning(f"分类ID {category_id} 不存在或不属于用户 {user_id}")
                return False

            category = dict(category)
            main_category = category['main_category']
            sub_category = category['sub_category']

            # 判断是否为父级分类（sub_category为空字符串）
            if sub_category == '' or sub_category is None:
                # 父级分类：级联删除所有子分类 (限制user_id)
                self.logger.info(f"删除父级分类 '{main_category}' 及其所有子分类 (user_id={user_id})")

                # 先查询有多少子分类
                async with conn.execute(
                    "SELECT COUNT(*) as count FROM categories WHERE main_category = ? AND sub_category != '' AND user_id = ?",
                    (main_category, user_id)
                ) as cursor:
                    result = await cursor.fetchone()
                    child_count = result[0] if result else 0

                # 删除该主分类下的所有记录（包括父级和子级）
                cursor = await conn.execute(
                    "DELETE FROM categories WHERE main_category = ? AND user_id = ?",
                    (main_category, user_id)
                )
                deleted_count = cursor.rowcount
                await conn.commit()

                self.logger.info(
                    f"已删除父级分类 '{main_category}' (ID: {category_id}) "
                    f"及其 {child_count} 个子分类，共删除 {deleted_count} 条记录 (user_id={user_id})"
                )

                # 清除缓存
                self._clear_cache('category_mappings')

                return True
            else:
                # 子分类：仅删除该子分类 (限制user_id)
                self.logger.info(f"删除子分类 '{main_category}/{sub_category}' (ID: {category_id}, user_id={user_id})")

                await conn.execute("DELETE FROM categories WHERE id = ? AND user_id = ?", (category_id, user_id))
                await conn.commit()

                self.logger.info(f"已删除子分类 '{main_category}/{sub_category}' (ID: {category_id}, user_id={user_id})")

                # 清除缓存
                self._clear_cache('category_mappings')

                return True

        except Exception as e:
            self.logger.error(f"删除分类失败: {e}", exc_info=True)
            return False

    @log_method
    async def get_category_by_id(self, category_id: int, user_id: int = 1) -> Optional[Dict[str, Any]]:
        """获取单个分类
        
        Args:
            category_id: 分类ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM categories WHERE id = ? AND user_id = ?", 
            (category_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_category_statistics(self, period: str = 'month',
                                      start_date: Optional[str] = None,
                                      end_date: Optional[str] = None) -> List[Dict[str, Any]]:
        """
        获取分类统计

        Args:
            period: 统计周期(目前未使用,保留以便未来扩展)
            start_date: 开始日期
            end_date: 结束日期

        Returns:
            List[Dict]: 分类统计结果
        """
        # period 参数保留以便未来扩展
        _ = period
        conn = await self._get_connection()

        query = """
        SELECT
            main_category,
            sub_category,
            type,
            COUNT(*) as count,
            SUM(amount) as total_amount,
            AVG(amount) as avg_amount,
            MIN(amount) as min_amount,
            MAX(amount) as max_amount
        FROM bills
        WHERE main_category IS NOT NULL
        """
        params = []

        if start_date:
            query += " AND date >= ?"
            params.append(start_date)
        if end_date:
            query += " AND date <= ?"
            params.append(end_date)

        query += " GROUP BY main_category, sub_category, type ORDER BY total_amount DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_all_accounts(self, user_id: int = 1) -> List[Dict[str, Any]]:
        """获取所有账户
        
        Args:
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM accounts WHERE user_id = ? ORDER BY display_order, name",
            (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_account_by_id(self, account_id: int, user_id: int = 1) -> Optional[Dict[str, Any]]:
        """根据ID获取账户
        
        Args:
            account_id: 账户ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM accounts WHERE id = ? AND user_id = ?",
            (account_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_sub_accounts(self, parent_id: int, user_id: int = 1) -> List[Dict[str, Any]]:
        """获取子账户列表
        
        Args:
            parent_id: 父账户ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM accounts WHERE parent_id = ? AND user_id = ?",
            (parent_id, user_id)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def create_account(self, data: Dict[str, Any], user_id: int = 1) -> int:
        """创建账户
        
        Args:
            data: 账户数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        # 提取子账户
        sub_accounts = data.get('subAccounts', [])

        # 获取 parentId (前端字段) 或 parent_id (后端字段)
        parent_id = data.get('parentId') or data.get('parent_id', 0)

        cursor = await conn.execute("""
            INSERT INTO accounts (
                name, type, category, currency, icon, color,
                balance, initial_balance, hidden, display_order,
                comment, parent_id, created_at, updated_at, user_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            data.get('name'),
            data.get('type', 1),
            data.get('category'),
            data.get('currency', 'CNY'),
            data.get('icon'),
            data.get('color'),
            data.get('balance', 0.0),
            data.get('initial_balance', 0.0),
            1 if data.get('hidden', False) else 0,
            data.get('display_order', 0),
            data.get('comment'),
            parent_id,
            now,
            now,
            user_id
        ))

        account_id = cursor.lastrowid
        await conn.commit()

        # 递归创建子账户
        if sub_accounts and isinstance(sub_accounts, list):
            for sub_account in sub_accounts:
                sub_account['parentId'] = account_id
                # 递归调用（传递user_id）
                await self.create_account(sub_account, user_id)

        return account_id

    @log_method
    async def update_account(self, account_id: int, data: Dict[str, Any], 
                             user_id: int = 1) -> bool:
        """更新账户
        
        Args:
            account_id: 账户ID
            data: 更新数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        if not data:
            return False

        conn = await self._get_connection()

        # 准备更新数据
        update_data = data.copy()
        update_data['updated_at'] = datetime.now().isoformat()

        # 映射 parentId -> parent_id
        if 'parentId' in update_data:
            update_data['parent_id'] = update_data.pop('parentId')

        # 移除subAccounts字段（这是嵌套数据，不应该更新到父账户表）
        if 'subAccounts' in update_data:
            self.logger.warning(f"账户更新数据包含subAccounts字段，已移除: account_id={account_id}")
            del update_data['subAccounts']

        # 定义允许更新的字段白名单
        valid_columns = {
            'name', 'type', 'category', 'currency', 'icon', 'color',
            'balance', 'initial_balance', 'hidden', 'display_order',
            'comment', 'parent_id', 'updated_at'
        }

        # 过滤掉不在白名单中的字段
        # 这可以防止前端传递多余字段导致SQL错误（如 clientSessionId, balanceTime 等）
        filtered_data = {k: v for k, v in update_data.items() if k in valid_columns}

        if not filtered_data:
            self.logger.warning(f"账户更新数据过滤后为空: account_id={account_id}, 原始字段={list(update_data.keys())}")
            return False

        if 'id' in filtered_data:
            del filtered_data['id']

        set_clause = ", ".join(f"{key} = ?" for key in filtered_data.keys())
        values = list(filtered_data.values())
        values.extend([account_id, user_id])

        self.logger.info(f"更新账户: id={account_id}, user_id={user_id}, 字段={list(filtered_data.keys())}")

        cursor = await conn.execute(
            f"UPDATE accounts SET {set_clause} WHERE id = ? AND user_id = ?",
            values
        )
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def delete_account(self, account_id: int, user_id: int = 1) -> bool:
        """删除账户
        
        Args:
            account_id: 账户ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        cursor = await conn.execute(
            "DELETE FROM accounts WHERE id = ? AND user_id = ?", 
            (account_id, user_id)
        )
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def update_account_balance(self, account_id: int, amount: float, operation: str = 'add') -> bool:
        """
        更新账户余额

        Args:
            account_id: 账户ID
            amount: 金额（正数）
            operation: 操作类型，'add' 增加余额（收入），'subtract' 减少余额（支出）

        Returns:
            bool: 更新是否成功
        """
        try:
            conn = await self._get_connection()

            # 获取当前余额
            async with conn.execute(
                "SELECT balance FROM accounts WHERE id = ?",
                (account_id,)
            ) as cursor:
                row = await cursor.fetchone()
                if not row:
                    self.logger.warning(f"账户不存在: account_id={account_id}")
                    return False

                current_balance = row['balance'] or 0.0

            # 计算新余额
            if operation == 'add':
                new_balance = current_balance + amount
                self.logger.info(f"增加余额: account_id={account_id}, 原:{current_balance} + {amount} = {new_balance}")
            elif operation == 'subtract':
                new_balance = current_balance - amount
                self.logger.info(f"减少余额: account_id={account_id}, 原:{current_balance} - {amount} = {new_balance}")
            else:
                self.logger.error(f"未知操作类型: {operation}")
                return False

            # 更新余额
            cursor = await conn.execute(
                "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                (new_balance, datetime.now().isoformat(), account_id)
            )
            await conn.commit()

            return cursor.rowcount > 0

        except Exception as e:
            self.logger.error(f"更新账户余额失败: {e}", exc_info=True)
            return False

    @log_method
    async def calculate_account_balance(self, account_id: int, account_name: str = None) -> float:
        """
        计算账户实际余额（基于关联的所有账单）

        Args:
            account_id: 账户ID
            account_name: 账户名称（用于匹配bills表的channel字段）

        Returns:
            float: 计算出的实际余额
        """
        try:
            conn = await self._get_connection()

            # 如果没有提供账户名称，查询获取
            if not account_name:
                async with conn.execute(
                    "SELECT name FROM accounts WHERE id = ?",
                    (account_id,)
                ) as cursor:
                    row = await cursor.fetchone()
                    if not row:
                        self.logger.warning(f"账户不存在: account_id={account_id}")
                        return 0.0
                    account_name = row['name']

            # 获取初始余额
            async with conn.execute(
                "SELECT initial_balance FROM accounts WHERE id = ?",
                (account_id,)
            ) as cursor:
                row = await cursor.fetchone()
                initial_balance = row['initial_balance'] if row else 0.0

            # 计算所有账单的余额变动（基于source_account_id和destination_account_id字段）
            # 收入：增加余额
            # 支出：减少余额
            # 转账：源账户减少，目标账户增加
            # 投资：源账户减少，目标账户增加

            # 作为源账户的交易（收入增加，支出/转账/投资减少）
            async with conn.execute("""
                SELECT
                    SUM(CASE WHEN type = '收入' THEN amount ELSE 0 END) as income,
                    SUM(CASE WHEN type = '支出' THEN amount ELSE 0 END) as expense,
                    SUM(CASE WHEN type = '转账' THEN amount ELSE 0 END) as transfer_out,
                    SUM(CASE WHEN type = '投资' THEN amount ELSE 0 END) as investment_out
                FROM bills
                WHERE source_account_id = ?
            """, (account_id,)) as cursor:
                row = await cursor.fetchone()
                income = row['income'] or 0.0
                expense = row['expense'] or 0.0
                transfer_out = row['transfer_out'] or 0.0
                investment_out = row['investment_out'] or 0.0

            # 作为目标账户的交易（转账/投资增加）
            async with conn.execute("""
                SELECT
                    SUM(CASE WHEN type = '转账' THEN destination_amount ELSE 0 END) as transfer_in,
                    SUM(CASE WHEN type = '投资' THEN destination_amount ELSE 0 END) as investment_in
                FROM bills
                WHERE destination_account_id = ?
            """, (account_id,)) as cursor:
                row = await cursor.fetchone()
                transfer_in = row['transfer_in'] or 0.0
                investment_in = row['investment_in'] or 0.0

            # 余额计算: 初始余额 + 收入 - 支出 - 转账转出 + 转账转入 - 投资转出 + 投资转入
            calculated_balance = (
                initial_balance +
                income -
                expense -
                transfer_out +
                transfer_in -
                investment_out +
                investment_in
            )

            self.logger.info(
                f"计算账户余额: account_id={account_id}, name={account_name}, "
                f"初始={initial_balance}, 收入={income}, 支出={expense}, "
                f"转账转出={transfer_out}, 转账转入={transfer_in}, "
                f"投资转出={investment_out}, 投资转入={investment_in}, "
                f"实际={calculated_balance}"
            )

            return calculated_balance

        except Exception as e:
            self.logger.error(f"计算账户余额失败: {e}", exc_info=True)
            return 0.0

    @log_method
    async def sync_account_balance(self, account_id: int) -> bool:
        """
        同步账户余额（将计算出的实际余额写入balance字段）

        Args:
            account_id: 账户ID

        Returns:
            bool: 同步是否成功
        """
        try:
            conn = await self._get_connection()

            # 获取账户名称
            async with conn.execute(
                "SELECT name FROM accounts WHERE id = ?",
                (account_id,)
            ) as cursor:
                row = await cursor.fetchone()
                if not row:
                    self.logger.warning(f"账户不存在: account_id={account_id}")
                    return False
                account_name = row['name']

            # 计算实际余额
            calculated_balance = await self.calculate_account_balance(account_id, account_name)

            # 更新余额
            cursor = await conn.execute(
                "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                (calculated_balance, datetime.now().isoformat(), account_id)
            )
            await conn.commit()

            self.logger.info(f"同步账户余额: account_id={account_id}, balance={calculated_balance}")

            return cursor.rowcount > 0

        except Exception as e:
            self.logger.error(f"同步账户余额失败: {e}", exc_info=True)
            return False

    @log_method
    async def get_all_tags(self, user_id: int = 1) -> List[Dict[str, Any]]:
        """获取所有标签
        
        Args:
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM tags WHERE user_id = ? ORDER BY display_order, created_at DESC",
            (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_tag_by_id(self, tag_id: int, user_id: int = 1) -> Optional[Dict[str, Any]]:
        """根据ID获取标签
        
        Args:
            tag_id: 标签ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM tags WHERE id = ? AND user_id = ?",
            (tag_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_tag(self, data: Dict[str, Any], user_id: int = 1) -> int:
        """创建标签
        
        Args:
            data: 标签数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute("""
            INSERT INTO tags (name, color, icon, hidden, created_at, updated_at, user_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        """, (
            data.get('name'),
            data.get('color', '#000000'),
            data.get('icon', ''),
            data.get('hidden', False),
            now, now,
            user_id
        ))

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def update_tag(self, tag_id: int, data: Dict[str, Any], 
                         user_id: int = 1) -> bool:
        """更新标签
        
        Args:
            tag_id: 标签ID
            data: 更新数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        if not data:
            return False

        conn = await self._get_connection()
        data['updated_at'] = datetime.now().isoformat()

        set_clause = ", ".join(f"{key} = ?" for key in data.keys())
        values = list(data.values())
        values.extend([tag_id, user_id])

        cursor = await conn.execute(
            f"UPDATE tags SET {set_clause} WHERE id = ? AND user_id = ?",
            values
        )
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def delete_tag(self, tag_id: int, user_id: int = 1) -> bool:
        """删除标签
        
        Args:
            tag_id: 标签ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        cursor = await conn.execute(
            "DELETE FROM tags WHERE id = ? AND user_id = ?", 
            (tag_id, user_id)
        )
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def update_tag_display_orders(self, orders: List[tuple], 
                                        user_id: int = 1) -> bool:
        """批量更新标签显示顺序
        
        Args:
            orders: [(tag_id, display_order), ...] 标签ID和显示顺序的元组列表
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        
        Returns:
            bool: 更新是否成功
        """
        if not orders:
            self.logger.warning("[update_tag_display_orders] 订单列表为空")
            return True

        conn = await self._get_connection()
        now = datetime.now().isoformat()

        try:
            # 批量更新display_order和updated_at（限制user_id）
            for tag_id, display_order in orders:
                await conn.execute(
                    "UPDATE tags SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                    (display_order, now, tag_id, user_id)
                )
            
            await conn.commit()
            self.logger.info(f"[update_tag_display_orders] 成功更新{len(orders)}个标签的显示顺序 (user_id={user_id})")
            return True
        except Exception as e:
            self.logger.error(f"更新标签显示顺序失败: {e}", exc_info=True)
            await conn.rollback()
            return False

    @log_method
    async def get_all_templates(self, user_id: int = 1) -> List[Dict[str, Any]]:
        """获取所有模板"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM bill_templates WHERE user_id = ? ORDER BY is_favorite DESC, use_count DESC, name",
            (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_template_by_id(self, template_id: int) -> Optional[Dict[str, Any]]:
        """根据ID获取模板"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM bill_templates WHERE id = ?",
            (template_id,)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_template(self, data: Dict[str, Any]) -> int:
        """创建模板"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute("""
            INSERT INTO bill_templates
            (name, description, type, category, amount, account, counterparty, tag, comment, is_favorite, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            data.get('name'),
            data.get('description'),
            data.get('type'),
            data.get('category'),
            data.get('amount'),
            data.get('account'),
            data.get('counterparty'),
            data.get('tag'),
            data.get('comment'),
            data.get('is_favorite', 0),
            now, now
        ))

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def update_template(self, template_id: int, data: Dict[str, Any]) -> bool:
        """更新模板"""
        if not data:
            return False

        conn = await self._get_connection()
        data['updated_at'] = datetime.now().isoformat()

        set_clause = ", ".join(f"{key} = ?" for key in data.keys())
        values = list(data.values())
        values.append(template_id)

        cursor = await conn.execute(
            f"UPDATE bill_templates SET {set_clause} WHERE id = ?",
            values
        )
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def delete_template(self, template_id: int) -> bool:
        """删除模板"""
        conn = await self._get_connection()

        cursor = await conn.execute("DELETE FROM bill_templates WHERE id = ?", (template_id,))
        await conn.commit()

        return cursor.rowcount > 0

    # ==================== 用户认证管理方法 ====================

    @log_method
    async def create_user(self, data: Dict[str, Any]) -> int:
        """
        创建用户

        Args:
            data: 用户数据，必须包含 username, email, password_hash

        Returns:
            int: 新创建的用户ID
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute("""
            INSERT INTO users (
                username, email, password_hash, nickname, avatar,
                language, default_currency, first_day_of_week,
                is_active, email_verified, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            data.get('username'),
            data.get('email'),
            data.get('password_hash'),
            data.get('nickname', data.get('username')),
            data.get('avatar', ''),
            data.get('language', 'zh_Hans'),
            data.get('default_currency', 'CNY'),
            data.get('first_day_of_week', 1),
            data.get('is_active', 1),
            data.get('email_verified', 0),
            now, now
        ))

        await conn.commit()
        user_id = cursor.lastrowid

        self.logger.info(f"创建用户成功: ID={user_id}, username={data.get('username')}")
        return user_id

    @log_method
    async def get_user_by_username(self, username: str) -> Optional[Dict[str, Any]]:
        """根据用户名获取用户"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM users WHERE username = ?",
            (username,)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_by_email(self, email: str) -> Optional[Dict[str, Any]]:
        """根据邮箱获取用户"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM users WHERE email = ?",
            (email,)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_by_id(self, user_id: int) -> Optional[Dict[str, Any]]:
        """根据ID获取用户"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM users WHERE id = ?",
            (user_id,)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def update_user(self, user_id: int, data: Dict[str, Any]) -> bool:
        """更新用户信息"""
        if not data:
            return False

        conn = await self._get_connection()
        data['updated_at'] = datetime.now().isoformat()

        set_clause = ", ".join(f"{key} = ?" for key in data.keys())
        values = list(data.values())
        values.append(user_id)

        cursor = await conn.execute(
            f"UPDATE users SET {set_clause} WHERE id = ?",
            values
        )
        await conn.commit()

        self.logger.info(f"更新用户成功: ID={user_id}")
        return cursor.rowcount > 0

    @log_method
    async def update_user_last_login(self, user_id: int, ip_address: str = None):
        """更新用户最后登录时间"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        await conn.execute("""
            UPDATE users
            SET last_login_at = ?, last_login_ip = ?, failed_login_attempts = 0, locked_until = NULL
            WHERE id = ?
        """, (now, ip_address, user_id))

        await conn.commit()
        self.logger.info(f"更新用户最后登录时间: ID={user_id}, IP={ip_address}")

    @log_method
    async def increment_failed_login(self, user_id: int, lockout_minutes: int = 15):
        """增加失败登录次数，超过阈值则锁定账户"""
        conn = await self._get_connection()

        # 获取当前失败次数
        async with conn.execute(
            "SELECT failed_login_attempts FROM users WHERE id = ?",
            (user_id,)
        ) as cursor:
            row = await cursor.fetchone()
            if not row:
                return False

            failed_attempts = row[0] + 1

            # 如果失败次数达到5次，锁定账户
            if failed_attempts >= 5:
                locked_until = (datetime.now() + timedelta(minutes=lockout_minutes)).isoformat()
                await conn.execute("""
                    UPDATE users
                    SET failed_login_attempts = ?, locked_until = ?
                    WHERE id = ?
                """, (failed_attempts, locked_until, user_id))
                self.logger.warning(f"用户账户已锁定: ID={user_id}, 锁定至={locked_until}")
            else:
                await conn.execute("""
                    UPDATE users
                    SET failed_login_attempts = ?
                    WHERE id = ?
                """, (failed_attempts, user_id))

            await conn.commit()
            return True

    @log_method
    async def is_user_locked(self, user_id: int) -> bool:
        """检查用户是否被锁定"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT locked_until FROM users WHERE id = ?",
            (user_id,)
        ) as cursor:
            row = await cursor.fetchone()
            if not row or not row[0]:
                return False

            locked_until = datetime.fromisoformat(row[0])
            if datetime.now() < locked_until:
                return True

            # 锁定时间已过，清除锁定状态
            await conn.execute("""
                UPDATE users
                SET locked_until = NULL, failed_login_attempts = 0
                WHERE id = ?
            """, (user_id,))
            await conn.commit()

            return False

    @log_method
    async def create_session(self, data: Dict[str, Any]) -> int:
        """创建会话"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute("""
            INSERT INTO sessions (
                user_id, token_hash, refresh_token_hash,
                expires_at, refresh_expires_at,
                user_agent, ip_address, is_active,
                last_activity_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            data.get('user_id'),
            data.get('token_hash'),
            data.get('refresh_token_hash'),
            data.get('expires_at'),
            data.get('refresh_expires_at'),
            data.get('user_agent'),
            data.get('ip_address'),
            1,  # is_active
            now,  # last_activity_at
            now   # created_at
        ))

        await conn.commit()
        session_id = cursor.lastrowid

        self.logger.info(f"创建会话成功: ID={session_id}, user_id={data.get('user_id')}")
        return session_id

    @log_method
    async def get_session_by_token_hash(self, token_hash: str) -> Optional[Dict[str, Any]]:
        """根据token哈希获取会话"""
        conn = await self._get_connection()

        async with conn.execute("""
            SELECT s.*, u.username, u.email, u.is_active as user_is_active
            FROM sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.token_hash = ? AND s.is_active = 1
        """, (token_hash,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_sessions(self, user_id: int) -> List[Dict[str, Any]]:
        """获取用户所有活跃会话"""
        conn = await self._get_connection()

        async with conn.execute("""
            SELECT * FROM sessions
            WHERE user_id = ? AND is_active = 1
            ORDER BY created_at DESC
        """, (user_id,)) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def update_session_activity(self, session_id: int):
        """更新会话活动时间"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        await conn.execute("""
            UPDATE sessions
            SET last_activity_at = ?
            WHERE id = ?
        """, (now, session_id))

        await conn.commit()

    @log_method
    async def invalidate_session(self, token_hash: str) -> bool:
        """使会话失效"""
        conn = await self._get_connection()

        cursor = await conn.execute("""
            UPDATE sessions
            SET is_active = 0
            WHERE token_hash = ?
        """, (token_hash,))

        await conn.commit()
        self.logger.info(f"会话已失效: token_hash={token_hash[:16]}...")
        return cursor.rowcount > 0

    @log_method
    async def invalidate_user_sessions(self, user_id: int) -> int:
        """使用户的所有会话失效"""
        conn = await self._get_connection()

        cursor = await conn.execute("""
            UPDATE sessions
            SET is_active = 0
            WHERE user_id = ?
        """, (user_id,))

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(f"用户所有会话已失效: user_id={user_id}, 数量={count}")
        return count

    @log_method
    async def cleanup_expired_sessions(self) -> int:
        """清理过期会话"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute("""
            DELETE FROM sessions
            WHERE expires_at < ? OR
                  (refresh_expires_at IS NOT NULL AND refresh_expires_at < ?)
        """, (now, now))

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(f"清理过期会话: 数量={count}")
        return count

    @log_method
    async def create_auth_log(self, data: Dict[str, Any]):
        """创建认证日志"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        await conn.execute("""
            INSERT INTO auth_logs (
                user_id, username, event_type, ip_address, user_agent,
                success, error_message, metadata, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            data.get('user_id'),
            data.get('username'),
            data.get('event_type'),
            data.get('ip_address'),
            data.get('user_agent'),
            data.get('success', False),
            data.get('error_message'),
            data.get('metadata'),
            now
        ))

        await conn.commit()

    @log_method
    async def get_auth_logs(
        self,
        user_id: int = None,
        event_type: str = None,
        limit: int = 100
    ) -> List[Dict[str, Any]]:
        """获取认证日志"""
        conn = await self._get_connection()

        query = "SELECT * FROM auth_logs WHERE 1=1"
        params = []

        if user_id:
            query += " AND user_id = ?"
            params.append(user_id)

        if event_type:
            query += " AND event_type = ?"
            params.append(event_type)

        query += " ORDER BY created_at DESC LIMIT ?"
        params.append(limit)

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def cleanup_old_auth_logs(self, days: int = 90) -> int:
        """清理旧的认证日志"""
        conn = await self._get_connection()
        cutoff_date = (datetime.now() - timedelta(days=days)).isoformat()

        cursor = await conn.execute("""
            DELETE FROM auth_logs
            WHERE created_at < ?
        """, (cutoff_date,))

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(f"清理旧认证日志: 数量={count}, 保留天数={days}")
        return count

    # === 预算管理方法 ===
    @log_method
    async def get_budgets(self, filters: Dict[str, Any] = None, 
                          user_id: int = 1) -> List[Dict[str, Any]]:
        """获取预算列表
        
        Args:
            filters: 筛选条件
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        query = "SELECT * FROM budgets WHERE user_id = ?"
        params = [user_id]

        if filters:
            if 'period_type' in filters:
                query += " AND period_type = ?"
                params.append(filters['period_type'])

            if 'enabled' in filters:
                query += " AND enabled = ?"
                params.append(1 if filters['enabled'] else 0)

            if 'category' in filters:
                query += " AND category = ?"
                params.append(filters['category'])

        query += " ORDER BY created_at DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_budget_by_id(self, budget_id: int, user_id: int = 1) -> Optional[Dict[str, Any]]:
        """根据ID获取预算
        
        Args:
            budget_id: 预算ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM budgets WHERE id = ? AND user_id = ?",
            (budget_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_budget(self, data: Dict[str, Any], user_id: int = 1) -> int:
        """创建预算
        
        Args:
            data: 预算数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        cursor = await conn.execute("""
            INSERT INTO budgets (
                name, category, sub_category, period_type, amount,
                start_date, end_date, alert_threshold, enabled,
                created_at, updated_at, user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            data['name'],
            data.get('category'),
            data.get('sub_category'),
            data['period_type'],
            data['amount'],
            data['start_date'],
            data.get('end_date'),
            data.get('alert_threshold', 80),
            data.get('enabled', 1),
            data['created_at'],
            data['updated_at'],
            user_id
        ))

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def get_primary_category_budget(
        self,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1
    ) -> Optional[Dict[str, Any]]:
        """
        获取一级分类预算

        Args:
            category: 主分类名称
            period_type: 周期类型
            start_date: 开始日期
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            预算记录，如果不存在则返回None
        """
        conn = await self._get_connection()

        async with conn.execute("""
            SELECT * FROM budgets
            WHERE category = ?
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
        """, (category, period_type, start_date, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_budget_by_category(
        self,
        category: str,
        sub_category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1
    ) -> Optional[Dict[str, Any]]:
        """
        根据分类信息查找预算（用于唯一性检测）

        Args:
            category: 主分类名称
            sub_category: 子分类名称（空字符串表示一级分类预算）
            period_type: 周期类型
            start_date: 开始日期
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            预算记录，如果不存在则返回None
        """
        conn = await self._get_connection()

        if sub_category:
            # 查找二级分类预算
            async with conn.execute("""
                SELECT * FROM budgets
                WHERE category = ?
                  AND sub_category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
            """, (category, sub_category, period_type, start_date, user_id)) as cursor:
                row = await cursor.fetchone()
                return dict(row) if row else None
        else:
            # 查找一级分类预算
            async with conn.execute("""
                SELECT * FROM budgets
                WHERE category = ?
                  AND (sub_category IS NULL OR sub_category = '')
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
            """, (category, period_type, start_date, user_id)) as cursor:
                row = await cursor.fetchone()
                return dict(row) if row else None

    @log_method
    async def get_sub_category_budgets_total(
        self,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int = 1
    ) -> float:
        """
        获取某一级分类下所有二级分类预算的总金额

        Args:
            category: 主分类名称
            period_type: 周期类型
            start_date: 开始日期
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            总金额
        """
        conn = await self._get_connection()

        async with conn.execute("""
            SELECT COALESCE(SUM(amount), 0) as total
            FROM budgets
            WHERE category = ?
              AND sub_category IS NOT NULL
              AND sub_category != ''
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
        """, (category, period_type, start_date, user_id)) as cursor:
            row = await cursor.fetchone()
            return row['total'] if row else 0

    @log_method
    async def update_budget(self, budget_id: int, data: Dict[str, Any], 
                            user_id: int = 1) -> bool:
        """更新预算
        
        Args:
            budget_id: 预算ID
            data: 更新数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        # 构建更新字段
        fields = []
        values = []

        for key in ['name', 'category', 'sub_category', 'period_type', 'amount',
                    'start_date', 'end_date', 'alert_threshold', 'enabled']:
            if key in data:
                fields.append(f"{key} = ?")
                values.append(data[key])

        if 'updated_at' in data:
            fields.append("updated_at = ?")
            values.append(data['updated_at'])

        if not fields:
            return False

        values.extend([budget_id, user_id])

        query = f"UPDATE budgets SET {', '.join(fields)} WHERE id = ? AND user_id = ?"
        cursor = await conn.execute(query, values)
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def delete_budget(self, budget_id: int, user_id: int = 1) -> bool:
        """删除预算
        
        Args:
            budget_id: 预算ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        cursor = await conn.execute(
            "DELETE FROM budgets WHERE id = ? AND user_id = ?", 
            (budget_id, user_id)
        )
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def get_budget_execution_details(
        self,
        budget_type: int = 3,
        start_date: str = None,
        end_date: str = None,
        category_id: int = None,
        account_ids: List[int] = None,
        tag_ids: List[int] = None,
        user_id: int = 1
    ) -> List[Dict[str, Any]]:
        """
        获取预算执行详情

        Args:
            budget_type: 预算类型 (3=支出, 5=投资)
            start_date: 开始日期 YYYY-MM-DD
            end_date: 结束日期 YYYY-MM-DD
            category_id: 分类ID筛选
            account_ids: 账户ID列表筛选
            tag_ids: 标签ID列表筛选

        Returns:
            预算执行详情列表
        """
        self.logger.info(
            f"[get_budget_execution_details] 参数: type={budget_type}, "
            f"dates={start_date}~{end_date}, category={category_id}, "
            f"accounts={account_ids}, tags={tag_ids}, user_id={user_id}"
        )

        conn = await self._get_connection()

        # 1. 获取符合条件的预算 - 添加 user_id 过滤
        budget_query = "SELECT * FROM budgets WHERE enabled = 1 AND user_id = ?"
        budget_params = [user_id]

        if budget_type == 3:
            budget_query += " AND (period_type IS NOT NULL)"  # 支出预算
        elif budget_type == 5:
            budget_query += " AND (period_type IS NOT NULL)"  # 投资预算

        if category_id:
            # 根据category_id获取分类名称
            cat_info = await self.get_category_by_id(category_id)
            if cat_info:
                budget_query += " AND category = ?"
                budget_params.append(cat_info['main_category'])

        async with conn.execute(budget_query, budget_params) as cursor:
            budgets = [dict(row) for row in await cursor.fetchall()]

        # 2. 对每个预算计算实际支出
        type_name = '支出' if budget_type == 3 else '投资'

        results = []
        for budget in budgets:
            self.logger.debug(
                f"[get_budget_execution_details] 处理预算: id={budget['id']}, "
                f"category={budget.get('category')}, sub_category={budget.get('sub_category')}, "
                f"start_date={budget.get('start_date')}, end_date={budget.get('end_date')}"
            )
            
            # 构建账单查询 - 添加 user_id 过滤
            bill_query = """
                SELECT COALESCE(SUM(amount), 0) as spent
                FROM bills
                WHERE type = ? AND user_id = ?
            """
            bill_params = [type_name, user_id]

            # 分类筛选
            if budget.get('category'):
                bill_query += " AND main_category = ?"
                bill_params.append(budget['category'])

            if budget.get('sub_category'):
                bill_query += " AND sub_category = ?"
                bill_params.append(budget['sub_category'])

            # 日期筛选 - 使用预算自身的日期范围
            budget_start = start_date or budget.get('start_date')
            budget_end = end_date or budget.get('end_date')
            
            if budget_start:
                bill_query += " AND date >= ?"
                bill_params.append(budget_start)
            
            if budget_end:
                bill_query += " AND date <= ?"
                bill_params.append(budget_end)

            # 账户筛选
            if account_ids:
                placeholders = ','.join('?' * len(account_ids))
                bill_query += f" AND (source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
                bill_params.extend(account_ids * 2)

            self.logger.debug(
                f"[get_budget_execution_details] 查询SQL: {bill_query}, 参数: {bill_params}"
            )
            
            # 执行查询
            async with conn.execute(bill_query, bill_params) as cursor:
                row = await cursor.fetchone()
                spent = abs(row['spent']) if row else 0
            
            self.logger.debug(
                f"[get_budget_execution_details] 预算 {budget.get('category')}/{budget.get('sub_category')} "
                f"查询结果: spent={spent}"
            )

            # 计算执行度
            budget_amount = budget.get('amount', 0)
            execution_rate = (spent / budget_amount * 100) if budget_amount > 0 else 0

            # 获取分类信息（正确匹配一级或二级分类）
            category_info = None
            fallback_sub_category = None  # 用于一级分类没有图标时的fallback
            if budget.get('category'):
                categories = await self.get_all_categories(user_id=user_id)
                for cat in categories:
                    if budget.get('sub_category'):
                        # 二级分类预算：匹配完整的主分类+子分类
                        if (cat['main_category'] == budget['category'] and
                                cat['sub_category'] == budget['sub_category']):
                            category_info = cat
                            break
                    else:
                        # 一级分类预算：匹配主分类（子分类为空）
                        if (cat['main_category'] == budget['category'] and
                                not cat['sub_category']):
                            category_info = cat
                            # 不要break，继续找一个有icon的子分类作为fallback
                        elif (cat['main_category'] == budget['category'] and
                              cat['sub_category'] and cat.get('icon')):
                            # 记录第一个有图标的子分类
                            if not fallback_sub_category:
                                fallback_sub_category = cat

                # 如果一级分类没有图标，使用子分类的图标
                if category_info and not category_info.get('icon') and fallback_sub_category:
                    category_info = dict(category_info)  # 复制一份以防止修改原数据
                    category_info['icon'] = fallback_sub_category.get('icon', '')
                    if not category_info.get('color') and fallback_sub_category.get('color'):
                        category_info['color'] = fallback_sub_category['color']

            results.append({
                'id': budget['id'],
                'name': budget['name'],
                'category': budget.get('category', ''),
                'sub_category': budget.get('sub_category', ''),
                'category_info': category_info,
                'period_type': budget.get('period_type', 'monthly'),
                'budget_amount': budget_amount,
                'spent_amount': spent,
                'remaining_amount': budget_amount - spent,
                'execution_rate': round(execution_rate, 2),
                'alert_threshold': budget.get('alert_threshold', 80),
                'start_date': budget.get('start_date'),
                'end_date': budget.get('end_date'),
                'enabled': budget.get('enabled', 1)
            })

        self.logger.info(f"[get_budget_execution_details] 返回{len(results)}条预算执行详情")
        return results

    @log_method
    async def get_period_forecast(
        self,
        budget_type: int = 3,
        period_type: str = 'monthly',
        start_date: str = None,
        end_date: str = None
    ) -> List[Dict[str, Any]]:
        """
        获取周期预计（基于历史数据预测）

        Args:
            budget_type: 预算类型 (3=支出, 5=投资)
            period_type: 周期类型 (daily/weekly/monthly/yearly)
            start_date: 开始日期
            end_date: 结束日期

        Returns:
            周期预测列表
        """
        self.logger.info(
            f"[get_period_forecast] 参数: type={budget_type}, "
            f"period={period_type}, dates={start_date}~{end_date}"
        )

        conn = await self._get_connection()
        type_name = '支出' if budget_type == 3 else '投资'

        # 根据周期类型确定分组方式
        if period_type == 'daily':
            date_format = '%Y-%m-%d'
            group_by = "date"
        elif period_type == 'weekly':
            date_format = '%Y-%W'
            group_by = "strftime('%Y-%W', date)"
        elif period_type == 'monthly':
            date_format = '%Y-%m'
            group_by = "strftime('%Y-%m', date)"
        else:  # yearly
            date_format = '%Y'
            group_by = "strftime('%Y', date)"

        # 查询历史数据
        query = f"""
            SELECT
                {group_by} as period,
                main_category,
                COALESCE(SUM(amount), 0) as total_amount,
                COUNT(*) as transaction_count
            FROM bills
            WHERE type = ?
        """
        params = [type_name]

        if start_date:
            query += " AND date >= ?"
            params.append(start_date)

        if end_date:
            query += " AND date <= ?"
            params.append(end_date)

        query += f" GROUP BY {group_by}, main_category ORDER BY period DESC, total_amount DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        # 按分类汇总
        category_totals = {}
        period_count = set()

        for row in rows:
            period = row['period']
            category = row['main_category'] or '未分类'
            amount = abs(row['total_amount'])

            period_count.add(period)

            if category not in category_totals:
                category_totals[category] = {
                    'total': 0,
                    'periods': []
                }
            category_totals[category]['total'] += amount
            category_totals[category]['periods'].append({
                'period': period,
                'amount': amount
            })

        # 计算平均值和预测
        num_periods = len(period_count) if period_count else 1
        results = []

        for category, data in category_totals.items():
            avg_amount = data['total'] / num_periods

            # 获取分类信息
            category_info = None
            categories = await self.get_all_categories()
            for cat in categories:
                if cat['main_category'] == category and not cat['sub_category']:
                    category_info = cat
                    break

            results.append({
                'category': category,
                'category_info': category_info,
                'total_amount': data['total'],
                'average_amount': round(avg_amount, 2),
                'period_count': num_periods,
                'forecast_amount': round(avg_amount, 2),  # 简单预测：使用平均值
                'periods': data['periods'][-6:]  # 最近6个周期的数据
            })

        # 按预测金额排序
        results.sort(key=lambda x: x['forecast_amount'], reverse=True)

        self.logger.info(f"[get_period_forecast] 返回{len(results)}条预测数据")
        return results

    @log_method
    async def import_budgets(self, budgets_data: List[Dict[str, Any]]) -> Dict[str, Any]:
        """
        批量导入预算

        Args:
            budgets_data: 预算数据列表

        Returns:
            导入结果统计
        """
        self.logger.info(f"[import_budgets] 开始导入{len(budgets_data)}条预算")

        conn = await self._get_connection()
        now = datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        created_count = 0
        updated_count = 0
        error_count = 0
        errors = []

        for idx, data in enumerate(budgets_data):
            try:
                # 检查必填字段
                if not data.get('name') or not data.get('amount'):
                    errors.append(f"第{idx+1}条: 缺少必填字段(name或amount)")
                    error_count += 1
                    continue

                # 检查是否存在同名预算
                existing = None
                async with conn.execute(
                    "SELECT id FROM budgets WHERE name = ?",
                    (data['name'],)
                ) as cursor:
                    existing = await cursor.fetchone()

                if existing:
                    # 更新现有预算
                    await conn.execute("""
                        UPDATE budgets SET
                            category = ?,
                            sub_category = ?,
                            period_type = ?,
                            amount = ?,
                            start_date = ?,
                            end_date = ?,
                            alert_threshold = ?,
                            enabled = ?,
                            updated_at = ?
                        WHERE id = ?
                    """, (
                        data.get('category'),
                        data.get('sub_category'),
                        data.get('period_type', 'monthly'),
                        data['amount'],
                        data.get('start_date'),
                        data.get('end_date'),
                        data.get('alert_threshold', 80),
                        data.get('enabled', 1),
                        now,
                        existing['id']
                    ))
                    updated_count += 1
                else:
                    # 创建新预算
                    await conn.execute("""
                        INSERT INTO budgets (
                            name, category, sub_category, period_type, amount,
                            start_date, end_date, alert_threshold, enabled,
                            created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """, (
                        data['name'],
                        data.get('category'),
                        data.get('sub_category'),
                        data.get('period_type', 'monthly'),
                        data['amount'],
                        data.get('start_date'),
                        data.get('end_date'),
                        data.get('alert_threshold', 80),
                        data.get('enabled', 1),
                        now,
                        now
                    ))
                    created_count += 1

            except Exception as e:
                errors.append(f"第{idx+1}条: {str(e)}")
                error_count += 1
                self.logger.error(f"导入预算失败: {e}")

        await conn.commit()

        result = {
            'created': created_count,
            'updated': updated_count,
            'errors': error_count,
            'error_details': errors
        }

        self.logger.info(
            f"[import_budgets] 导入完成: 创建={created_count}, 更新={updated_count}, 错误={error_count}"
        )
        return result

    @log_method
    async def export_budgets(self) -> List[Dict[str, Any]]:
        """
        导出所有预算

        Returns:
            预算数据列表
        """
        self.logger.info("[export_budgets] 开始导出预算")

        conn = await self._get_connection()
        export_fields = [
            'name', 'category', 'sub_category', 'period_type', 'amount',
            'start_date', 'end_date', 'alert_threshold', 'enabled'
        ]

        async with conn.execute("SELECT * FROM budgets ORDER BY created_at") as cursor:
            rows = await cursor.fetchall()
            budgets = []
            for row in rows:
                budget = {field: row[field] for field in export_fields if field in row.keys()}
                budgets.append(budget)

        self.logger.info(f"[export_budgets] 导出{len(budgets)}条预算")
        return budgets

    @log_method
    async def delete_categories_by_main_category(self, main_category: str) -> bool:
        """根据主分类名称删除所有相关分类"""
        conn = await self._get_connection()
        try:
            await conn.execute("DELETE FROM categories WHERE main_category = ?", (main_category,))
            await conn.commit()
            self.logger.info(f"已删除主分类及其子分类: {main_category}")
            return True
        except Exception as e:
            self.logger.error(f"删除主分类失败: {e}")
            return False

    @log_method
    async def update_main_category_name(self, old_name: str, new_name: str) -> bool:
        """更新主分类名称（级联更新所有子分类）"""
        conn = await self._get_connection()
        try:
            await conn.execute(
                "UPDATE categories SET main_category = ? WHERE main_category = ?",
                (new_name, old_name)
            )
            await conn.commit()
            self.logger.info(f"已更新主分类名称: {old_name} -> {new_name}")

            # 清除缓存
            self._clear_cache('account_mappings')
            self._clear_cache('category_mappings')

            return True
        except Exception as e:
            self.logger.error(f"更新主分类名称失败: {e}")
            return False

    # ==================== 缓存方法 ====================

    def _clear_cache(self, key: str = None):
        """清除缓存

        Args:
            key: 缓存键，None表示清除所有
        """
        if key:
            self._cache.pop(key, None)
            self._cache_expiry.pop(key, None)
            self.logger.debug(f"已清除缓存: {key}")
        else:
            self._cache.clear()
            self._cache_expiry.clear()
            self.logger.debug("已清除所有缓存")

    def _is_cache_valid(self, key: str) -> bool:
        """检查缓存是否有效

        Args:
            key: 缓存键

        Returns:
            bool: 是否有效
        """
        if key not in self._cache:
            return False

        expiry = self._cache_expiry.get(key)
        if not expiry:
            return False

        return datetime.now() < expiry

    @log_method
    async def get_account_mappings(self) -> Dict[str, Any]:
        """获取账户映射(带缓存)

        Returns:
            Dict: {
                'id_to_account': {id: account_dict},
                'name_to_id': {name: id},
                'id_to_name': {id: name}
            }
        """
        cache_key = 'account_mappings'

        # 检查缓存
        if self._is_cache_valid(cache_key):
            self.logger.debug("使用缓存的账户映射")
            return self._cache[cache_key]

        # 查询数据库
        accounts = await self.get_all_accounts()

        # 构建映射
        mappings = {
            'id_to_account': {acc['id']: acc for acc in accounts},
            'name_to_id': {acc['name']: acc['id'] for acc in accounts},
            'id_to_name': {acc['id']: acc['name'] for acc in accounts}
        }

        # 更新缓存
        self._cache[cache_key] = mappings
        self._cache_expiry[cache_key] = datetime.now() + timedelta(seconds=self._cache_ttl)

        self.logger.info(f"已构建账户映射: {len(accounts)} 个账户")
        return mappings

    @log_method
    async def get_category_mappings(self) -> Dict[str, Any]:
        """获取分类映射(带缓存)

        Returns:
            Dict: {
                'id_to_category': {id: category_dict},
                'name_to_id': {(main, sub): id},
                'id_to_name': {id: (main, sub)}
            }
        """
        cache_key = 'category_mappings'

        # 检查缓存
        if self._is_cache_valid(cache_key):
            self.logger.debug("使用缓存的分类映射")
            return self._cache[cache_key]

        # 查询数据库
        categories = await self.get_all_categories()

        # 构建映射
        mappings = {
            'id_to_category': {cat['id']: cat for cat in categories},
            'name_to_id': {
                (cat['main_category'], cat['sub_category']): cat['id']
                for cat in categories
            },
            'id_to_name': {
                cat['id']: (cat['main_category'], cat['sub_category'])
                for cat in categories
            }
        }

        # 更新缓存
        self._cache[cache_key] = mappings
        self._cache_expiry[cache_key] = datetime.now() + timedelta(seconds=self._cache_ttl)

        self.logger.info(f"已构建分类映射: {len(categories)} 个分类")
        return mappings

    @log_method
    async def get_balances_before_date(self, date_str: str, user_id: int = 1) -> Dict[int, float]:
        """
        获取指定日期前所有账户的余额（单位：元）

        Args:
            date_str: 日期字符串 (YYYY-MM-DD)
            user_id: 用户ID

        Returns:
            Dict[int, float]: {account_id: balance}
        """
        conn = await self._get_connection()
        balances = {}

        # 1. 收入 (source_account_id)
        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills WHERE date < ? AND type = '收入' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id)
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:  # 忽略ID为0或None
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) + amount

        # 2. 支出 (source_account_id)
        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills WHERE date < ? AND type = '支出' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id)
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) - abs(amount)

        # 3. 转出 (source_account_id)
        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills WHERE date < ? AND type = '转账' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id)
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) - abs(amount)

        # 4. 转入 (destination_account_id)
        async with conn.execute(
            "SELECT destination_account_id, SUM(destination_amount) FROM bills WHERE date < ? AND type = '转账' AND user_id = ? GROUP BY destination_account_id",
            (date_str, user_id)
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) + abs(amount)

        return balances

    @log_method
    async def add_tags_to_bill(self, bill_id: int, tag_ids: List[int], user_id: int = 1) -> bool:
        """添加标签到账单
        
        Args:
            bill_id: 账单ID
            tag_ids: 标签ID列表
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        self.logger.info(f"[add_tags_to_bill] 开始为账单{bill_id}添加标签: {tag_ids} (user_id={user_id})")

        if not tag_ids:
            self.logger.info("[add_tags_to_bill] 标签列表为空，无需添加")
            return True

        conn = await self._get_connection()
        now = datetime.now().isoformat()

        try:
            # 批量插入
            values = [(bill_id, tag_id, now) for tag_id in tag_ids]
            await conn.executemany(
                "INSERT OR IGNORE INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
                values
            )
            await conn.commit()
            self.logger.info(f"[add_tags_to_bill] 成功添加{len(tag_ids)}个标签")
            return True
        except Exception as e:
            self.logger.error(f"添加标签失败: {e}", exc_info=True)
            return False

    @log_method
    async def get_tags_for_bill(self, bill_id: int, user_id: int = 1) -> List[Dict[str, Any]]:
        """获取账单的所有标签
        
        Args:
            bill_id: 账单ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute("""
            SELECT t.*
            FROM tags t
            JOIN bill_tags bt ON t.id = bt.tag_id
            WHERE bt.bill_id = ?
            ORDER BY t.name
        """, (bill_id,)) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_tags_for_bills(self, bill_ids: List[int], user_id: int = 1) -> Dict[int, List[Dict[str, Any]]]:
        """批量获取账单标签
        
        Args:
            bill_ids: 账单ID列表
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        if not bill_ids:
            return {}

        conn = await self._get_connection()
        placeholders = ','.join('?' * len(bill_ids))

        async with conn.execute(f"""
            SELECT bt.bill_id, t.*
            FROM tags t
            JOIN bill_tags bt ON t.id = bt.tag_id
            WHERE bt.bill_id IN ({placeholders})
            ORDER BY t.name
        """, bill_ids) as cursor:
            rows = await cursor.fetchall()

            result = {}
            for row in rows:
                bill_id = row['bill_id']
                tag = dict(row)
                del tag['bill_id'] # remove bill_id from tag object

                if bill_id not in result:
                    result[bill_id] = []
                result[bill_id].append(tag)

            return result
    @log_method
    async def update_bill_tags(self, bill_id: int, tag_ids: List[int], user_id: int = 1) -> bool:
        """更新账单标签（覆盖）
        
        Args:
            bill_id: 账单ID
            tag_ids: 标签ID列表
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        self.logger.info(f"[update_bill_tags] 开始更新账单{bill_id}的标签 (user_id={user_id})，新标签: {tag_ids}")

        conn = await self._get_connection()

        try:
            # 1. 删除旧标签
            cursor = await conn.execute("DELETE FROM bill_tags WHERE bill_id = ?", (bill_id,))
            deleted_count = cursor.rowcount
            self.logger.info(f"[update_bill_tags] 删除了{deleted_count}个旧标签")

            # 2. 添加新标签
            if tag_ids:
                now = datetime.now().isoformat()
                values = [(bill_id, tag_id, now) for tag_id in tag_ids]
                await conn.executemany(
                    "INSERT INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
                    values
                )
                self.logger.info(f"[update_bill_tags] 成功添加{len(tag_ids)}个新标签")
            else:
                self.logger.info("[update_bill_tags] 没有新标签需要添加")

            await conn.commit()
            self.logger.info("[update_bill_tags] 更新完成")
            return True
        except Exception as e:
            self.logger.error(f"更新账单标签失败: {e}", exc_info=True)
            return False

    # ==================== 审计日志相关方法 ====================

    @log_method
    async def create_audit_log(
        self,
        operation_type: str,
        operation_target: str,
        target_id: Optional[int] = None,
        details: Optional[Dict[str, Any]] = None,
        affected_count: int = 0,
        ip_address: Optional[str] = None,
        user_agent: Optional[str] = None,
        session_id: Optional[str] = None,
        status: str = 'success',
        error_message: Optional[str] = None
    ) -> int:
        """
        创建操作审计日志

        Args:
            operation_type: 操作类型 (move_transactions, delete_transactions, etc.)
            operation_target: 操作目标 (account, bill, category, etc.)
            target_id: 目标对象ID
            details: 详细信息字典
            affected_count: 影响的记录数
            ip_address: IP地址
            user_agent: 用户代理
            session_id: 会话ID
            status: 状态 (success, failed, partial)
            error_message: 错误消息

        Returns:
            int: 审计日志ID
        """
        conn = await self._get_connection()

        now = datetime.now().isoformat()
        details_json = json.dumps(details, ensure_ascii=False) if details else None

        cursor = await conn.execute(
            """
            INSERT INTO audit_logs (
                operation_type, operation_target, target_id, details,
                affected_count, ip_address, user_agent, session_id,
                status, error_message, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                operation_type, operation_target, target_id, details_json,
                affected_count, ip_address, user_agent, session_id,
                status, error_message, now
            )
        )

        await conn.commit()

        log_id = cursor.lastrowid
        self.logger.info(
            f"创建审计日志: type={operation_type}, target={operation_target}, "
            f"target_id={target_id}, status={status}, affected={affected_count}"
        )

        return log_id

    @log_method
    async def get_audit_logs(
        self,
        operation_type: Optional[str] = None,
        operation_target: Optional[str] = None,
        target_id: Optional[int] = None,
        status: Optional[str] = None,
        limit: int = 100,
        offset: int = 0
    ) -> List[Dict[str, Any]]:
        """
        查询审计日志

        Args:
            operation_type: 操作类型过滤
            operation_target: 操作目标过滤
            target_id: 目标ID过滤
            status: 状态过滤
            limit: 返回数量限制
            offset: 偏移量

        Returns:
            List[Dict]: 审计日志列表
        """
        conn = await self._get_connection()

        # 构建查询条件
        conditions = []
        params = []

        if operation_type:
            conditions.append("operation_type = ?")
            params.append(operation_type)

        if operation_target:
            conditions.append("operation_target = ?")
            params.append(operation_target)

        if target_id is not None:
            conditions.append("target_id = ?")
            params.append(target_id)

        if status:
            conditions.append("status = ?")
            params.append(status)

        where_clause = f"WHERE {' AND '.join(conditions)}" if conditions else ""

        query = f"""
        SELECT * FROM audit_logs
        {where_clause}
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
        """

        params.extend([limit, offset])

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
            logs = []
            for row in rows:
                log_dict = dict(row)
                # 解析JSON details
                if log_dict.get('details'):
                    try:
                        log_dict['details'] = json.loads(log_dict['details'])
                    except json.JSONDecodeError:
                        pass
                logs.append(log_dict)
            return logs

    # ==================== 密码验证相关方法 ====================

    @log_method
    async def get_app_setting(self, key: str) -> Optional[str]:
        """
        获取应用配置

        Args:
            key: 配置键

        Returns:
            Optional[str]: 配置值，不存在返回None
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT value, is_encrypted FROM app_settings WHERE key = ?",
            (key,)
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                value = row['value']
                # TODO: 如果is_encrypted为True，解密value
                return value
            return None

    @log_method
    async def set_app_setting(
        self,
        key: str,
        value: str,
        value_type: str = 'string',
        description: Optional[str] = None,
        is_encrypted: bool = False
    ) -> bool:
        """
        设置应用配置

        Args:
            key: 配置键
            value: 配置值
            value_type: 值类型 (string, int, bool, json)
            description: 描述
            is_encrypted: 是否加密存储

        Returns:
            bool: 是否成功
        """
        conn = await self._get_connection()

        now = datetime.now().isoformat()

        # TODO: 如果is_encrypted为True，加密value
        encrypted_value = value

        try:
            await conn.execute(
                """
                INSERT INTO app_settings (key, value, value_type, description, is_encrypted, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    value_type = excluded.value_type,
                    description = excluded.description,
                    is_encrypted = excluded.is_encrypted,
                    updated_at = excluded.updated_at
                """,
                (key, encrypted_value, value_type, description, is_encrypted, now, now)
            )

            await conn.commit()
            self.logger.info(f"设置应用配置: key={key}, encrypted={is_encrypted}")
            return True

        except Exception as e:
            self.logger.error(f"设置应用配置失败: key={key}, error={e}")
            return False

    @log_method
    async def verify_operation_password(self, password: str) -> bool:
        """
        验证操作密码

        Args:
            password: 待验证的密码

        Returns:
            bool: 密码是否正确
        """
        # 从配置中获取密码（支持环境变量优先）
        import os

        # 1. 首先检查环境变量
        env_password = os.getenv('BILL_ANALYSER_OPERATION_PASSWORD')
        if env_password:
            result = password == env_password
            self.logger.info(f"使用环境变量密码验证: {'成功' if result else '失败'}")
            return result

        # 2. 从数据库读取配置
        stored_password = await self.get_app_setting('operation_password')

        # 3. 如果没有配置密码，默认接受任何密码（开发模式）
        if not stored_password:
            self.logger.warning("未配置操作密码，默认允许操作（不安全！）")
            return True

        # 4. 验证密码
        result = password == stored_password
        self.logger.info(f"数据库密码验证: {'成功' if result else '失败'}")
        return result

