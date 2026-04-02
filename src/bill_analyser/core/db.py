"""
数据库模块

使用 aiosqlite 实现异步数据库操作，支持 WAL 模式、批量写入和去重。
"""

import hashlib
import json
import os
import sqlite3

# import threading  # 已移除
from datetime import date, datetime, timedelta
from pathlib import Path
from typing import Any

import aiosqlite

from bill_analyser.constants import DATA_DIR, TEST_DB_DIR_ENV

from ..utils.logger import get_logger, log_method, log_step


def _is_relative_to(path: Path, parent: Path) -> bool:
    """兼容 Python 版本差异的 Path 前缀判断。"""
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def _resolve_database_path(db_path: str | None) -> Path:
    """解析数据库路径，并在测试场景下将测试库重定向到 tests 运行目录。"""
    test_db_dir = os.environ.get(TEST_DB_DIR_ENV, "").strip()

    if db_path is None:
        if test_db_dir:
            return Path(test_db_dir) / "pytest_default.db"
        return DATA_DIR / "bills.db"

    if db_path in {":memory:", "file::memory:?cache=shared"}:
        return Path(db_path)

    candidate = Path(db_path)
    if not test_db_dir:
        return candidate

    filename = candidate.name.lower()
    is_test_db_name = filename.startswith("test") and filename.endswith(".db")
    if not is_test_db_name:
        return candidate

    redirected_root = Path(test_db_dir)

    if not candidate.is_absolute():
        parts = tuple(part.lower() for part in candidate.parts)
        if parts and parts[0] == "data":
            return redirected_root / Path(*candidate.parts[1:])

        try:
            resolved_candidate = (Path.cwd() / candidate).resolve()
            resolved_data_dir = DATA_DIR.resolve()
        except OSError:
            return candidate

        if _is_relative_to(resolved_candidate, resolved_data_dir):
            return redirected_root / resolved_candidate.relative_to(resolved_data_dir)

        return candidate

    try:
        resolved_candidate = candidate.resolve()
        resolved_data_dir = DATA_DIR.resolve()
    except OSError:
        return candidate

    if _is_relative_to(resolved_candidate, resolved_data_dir):
        return redirected_root / resolved_candidate.relative_to(resolved_data_dir)

    return candidate


class Database:
    """异步数据库管理器"""

    def __init__(self, db_path: str | None = None):
        """
        初始化数据库管理器

        参数：
            db_path: 数据库文件路径，如果为 None 则使用默认路径
        """
        self.logger = get_logger("Database")

        self.db_path = _resolve_database_path(db_path)

        # 确保数据目录存在
        if str(self.db_path) not in {":memory:", "file::memory:?cache=shared"}:
            self.db_path.parent.mkdir(parents=True, exist_ok=True)

        self._connection: aiosqlite.Connection | None = None
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
        payment_method TEXT DEFAULT '',
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

            if "type" not in columns:
                self.logger.info("添加 type 字段到 categories 表")
                await conn.execute("ALTER TABLE categories ADD COLUMN type INTEGER DEFAULT 1")

                # 迁移旧数据类型
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
        aliases TEXT,
        parent_id INTEGER DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        """)

        # 检查 accounts 表是否有 parent_id 列 (用于迁移旧数据库)
        cursor = await conn.execute("PRAGMA table_info(accounts)")
        columns = [row[1] for row in await cursor.fetchall()]
        if "parent_id" not in columns:
            self.logger.info("添加 parent_id 列到 accounts 表")
            await conn.execute("ALTER TABLE accounts ADD COLUMN parent_id INTEGER DEFAULT 0")

        # 检查 accounts 表是否有 aliases 列 (用于账户别名匹配)
        if "aliases" not in columns:
            self.logger.info("添加 aliases 列到 accounts 表 (用于账户别名匹配)")
            await conn.execute("ALTER TABLE accounts ADD COLUMN aliases TEXT")

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
            if "hidden" not in columns:
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
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_period ON budgets(period_type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_dates ON budgets(start_date, end_date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budget_history_budget ON budget_history(budget_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_budget_history_period ON budget_history(period_start, period_end)"
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
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_date ON user_exchange_rates(effective_date DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rate_sources_enabled ON exchange_rate_sources(enabled, priority)"
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
            "CREATE INDEX IF NOT EXISTS idx_templates_favorite ON bill_templates(is_favorite, use_count DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_templates_type ON bill_templates(type)")

        # 创建定期账单索引
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_bills_user ON recurring_bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_bills_next_date ON recurring_bills(next_date)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recurring_bills_enabled ON recurring_bills(enabled, next_date)"
        )

        # v6.88: 为模板域补齐前端 REST 契约所需字段
        template_alter_statements = [
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
        ]

        for statement in template_alter_statements:
            try:
                await conn.execute(statement)
            except sqlite3.OperationalError:
                self.logger.debug("模板表扩展字段已存在: %s", statement)

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
            cash_account_id INTEGER,
            cash_transfer_category_id INTEGER,
            import_learning_enabled BOOLEAN DEFAULT 1,
            investment_platform_keywords TEXT,
            investment_product_keywords TEXT,
            investment_exclude_keywords TEXT,
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

        await conn.execute("""
        CREATE TABLE IF NOT EXISTS user_two_factor_recovery_codes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            code_hash TEXT NOT NULL,
            used_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, code_hash),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        """)

        await conn.execute("""
        CREATE TABLE IF NOT EXISTS user_external_auths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            external_auth_category TEXT NOT NULL,
            external_auth_type TEXT NOT NULL,
            external_user_id TEXT,
            external_username TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, external_auth_type),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        """)

        await conn.execute("""
        CREATE TABLE IF NOT EXISTS user_application_cloud_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, setting_key),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
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
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_two_factor_recovery_codes_user "
            "ON user_two_factor_recovery_codes(user_id, used_at)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_two_factor_recovery_codes_hash "
            "ON user_two_factor_recovery_codes(user_id, code_hash)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_user_external_auths_user ON user_external_auths(user_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_external_auths_type ON user_external_auths(external_auth_type)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_application_cloud_settings_user "
            "ON user_application_cloud_settings(user_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_application_cloud_settings_key "
            "ON user_application_cloud_settings(setting_key)"
        )

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
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_logs_type ON audit_logs(operation_type, created_at)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_logs_target ON audit_logs(operation_target, target_id)"
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

        await conn.execute("""
        CREATE TABLE IF NOT EXISTS backup_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            backup_name TEXT NOT NULL UNIQUE,
            storage_type TEXT NOT NULL DEFAULT 'local',
            file_path TEXT NOT NULL,
            checksum TEXT,
            encrypted BOOLEAN DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'created',
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        """)

        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_backup_records_status_created ON backup_records(status, created_at DESC)"
        )

        await conn.execute("""
        CREATE TABLE IF NOT EXISTS backup_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_type TEXT NOT NULL,
            schedule_expr TEXT,
            retention_days INTEGER DEFAULT 30,
            retention_count INTEGER DEFAULT 10,
            enabled BOOLEAN DEFAULT 1,
            last_run_at TEXT,
            last_status TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        """)

        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_backup_jobs_type_enabled ON backup_jobs(job_type, enabled)"
        )

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

        # v6.32: 为bills表添加payment_method字段（用于存储支付方式/渠道）
        try:
            await conn.execute("ALTER TABLE bills ADD COLUMN payment_method TEXT DEFAULT ''")
            self.logger.info("成功为bills表添加payment_method字段（用于存储支付方式/渠道）")
        except sqlite3.OperationalError:
            self.logger.debug("bills表已有payment_method字段")

        # ==================== v6.47: 账单导入系统三阶段表 ====================

        # 创建导入会话表 (管理导入生命周期)
        await conn.execute("""
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
        """)
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_sessions_session ON import_sessions(session_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_sessions_user ON import_sessions(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_import_sessions_status ON import_sessions(status)")

        # 创建解析器模板表 (阶段1: 解析后的原始数据)
        await conn.execute("""
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
        """)
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parser_template_session ON bills_parser_template(session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parser_template_processed ON bills_parser_template(parser_is_processed)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_parser_template_date ON bills_parser_template(parser_date)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_parser_template_parser_id ON bills_parser_template(parser_id)"
        )

        # 创建预览账单表 (阶段2: 去重后待确认的数据)
        await conn.execute("""
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
        """)
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_session ON bills_preview(session_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_selected ON bills_preview(preview_selected)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_type ON bills_preview(preview_type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_preview_date ON bills_preview(preview_date)")
        preview_alter_statements = [
            "ALTER TABLE bills_preview ADD COLUMN preview_parser_id TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_id INTEGER",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_name TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_candidate_count INTEGER DEFAULT 0",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_score REAL DEFAULT 0",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_reasons TEXT",
            "ALTER TABLE bills_preview ADD COLUMN preview_recurring_matched_date TEXT",
        ]
        for alter_stmt in preview_alter_statements:
            try:
                await conn.execute(alter_stmt)
            except Exception:  # pragma: no cover - 兼容旧库字段已存在场景
                pass
        await conn.execute("""
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
        """)
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_annotation_samples_session ON import_annotation_samples(session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_annotation_samples_user ON import_annotation_samples(user_id)"
        )
        await conn.execute("""
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
        """)
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rules_user_enabled "
            "ON import_learning_rules(user_id, enabled)"
        )
        await conn.execute("""
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
        """)
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rule_logs_user "
            "ON import_learning_rule_logs(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rule_logs_rule ON import_learning_rule_logs(rule_id)"
        )

        self.logger.info("v6.47: 账单导入三阶段表创建完成")

        # === v6.69: 多用户数据隔离迁移 ===
        # 所有 CREATE TABLE 语句执行完毕后，再为现有表添加 user_id 字段
        # 这样可以避免在表不存在时调用 _migrate_user_id_field 导致的错误
        await self._migrate_user_id_field(conn, "bills")
        await self._migrate_user_id_field(conn, "categories")
        await self._migrate_user_id_field(conn, "account_types")
        await self._migrate_user_id_field(conn, "accounts")
        await self._migrate_user_id_field(conn, "account_transfers")
        await self._migrate_user_id_field(conn, "tags")
        await self._migrate_user_id_field(conn, "budgets")
        await self._migrate_user_id_field(conn, "budget_history")
        await self._migrate_user_id_field(conn, "saved_filters")
        await self._migrate_user_id_field(conn, "bill_templates")
        await self._migrate_user_id_field(conn, "recurring_bills")
        await self._migrate_user_id_field(conn, "import_configs")
        await self._migrate_user_id_field(conn, "import_history")
        await self._migrate_user_id_field(conn, "user_exchange_rates")

        cursor = await conn.execute("PRAGMA table_info(budget_history)")
        budget_history_columns = [row[1] for row in await cursor.fetchall()]
        budget_history_alters = [
            ("budget_amount", "ALTER TABLE budget_history ADD COLUMN budget_amount REAL DEFAULT 0"),
            ("execution_rate", "ALTER TABLE budget_history ADD COLUMN execution_rate REAL DEFAULT 0"),
            ("filter_summary", "ALTER TABLE budget_history ADD COLUMN filter_summary TEXT DEFAULT ''"),
        ]
        for column_name, alter_sql in budget_history_alters:
            if column_name not in budget_history_columns:
                await conn.execute(alter_sql)
                self.logger.info("成功为 budget_history 表添加 %s 字段", column_name)
            else:
                self.logger.debug("budget_history 表已有 %s 字段", column_name)

        # 旧数据库的 budget_history 可能在此之前没有 user_id 字段。
        # 必须在迁移完成后再创建依赖该字段的索引，避免启动时报
        # sqlite3.OperationalError: no such column: user_id
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_budget_history_user_period "
            "ON budget_history(user_id, period_start, period_end)"
        )

        # v6.66: 修复 categories 表 UNIQUE 约束，添加 user_id
        # 旧约束: UNIQUE(main_category, sub_category)
        # 新约束: UNIQUE(user_id, main_category, sub_category)
        await self._migrate_categories_unique_constraint(conn)

        # v6.78: 为 users 表添加现金存取相关字段
        # cash_account_id: 现金账户ID
        # cash_transfer_category_id: 存取分类ID
        await self._migrate_users_cash_fields(conn)

        # v2026-03-07: 为 users 表添加导入学习开关字段
        await self._migrate_users_import_learning_fields(conn)

        # v2026-03-07: 为 users 表添加投资识别关键词配置字段
        await self._migrate_users_investment_keyword_fields(conn)

        # v7: 为 import_learning_rules 表添加复合匹配字段
        await self._migrate_learning_rules_composite_fields(conn)

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

            if "user_id" not in columns:
                self.logger.info(f"为 {table_name} 表添加 user_id 字段")
                await conn.execute(f"ALTER TABLE {table_name} ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1")
                self.logger.info(f"成功为 {table_name} 表添加 user_id 字段")
            else:
                self.logger.debug(f"{table_name} 表已有 user_id 字段")
        except Exception as e:
            self.logger.error(f"为 {table_name} 表添加 user_id 字段失败: {e}")

    async def _migrate_categories_unique_constraint(self, conn: aiosqlite.Connection) -> None:
        """
        v6.66: 迁移 categories 表的 UNIQUE 约束，添加 user_id

        旧约束: UNIQUE(main_category, sub_category) - 导致不同用户无法有相同分类名
        新约束: UNIQUE(user_id, main_category, sub_category) - 允许不同用户有相同分类名

        SQLite 不支持直接修改约束，需要重建表
        """
        try:
            # 检查当前约束是否已包含 user_id
            cursor = await conn.execute("SELECT sql FROM sqlite_master WHERE type='table' AND name='categories'")
            row = await cursor.fetchone()
            if not row:
                self.logger.debug("categories 表不存在，跳过约束迁移")
                return

            table_sql = row[0]

            # 如果已经包含正确的约束，跳过迁移
            if "UNIQUE(user_id, main_category, sub_category)" in table_sql:
                self.logger.debug("categories 表已有正确的 UNIQUE 约束，跳过迁移")
                return

            # 如果没有旧约束（新建的表），跳过迁移
            if "UNIQUE(main_category, sub_category)" not in table_sql:
                self.logger.debug("categories 表无需迁移（可能是新建的表）")
                return

            self.logger.info("开始迁移 categories 表 UNIQUE 约束...")
            self.logger.info("旧约束: UNIQUE(main_category, sub_category)")
            self.logger.info("新约束: UNIQUE(user_id, main_category, sub_category)")

            # 1. 创建临时表（使用新约束）
            await conn.execute("""
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
            """)

            # 2. 复制数据到新表
            await conn.execute("""
                INSERT INTO categories_new 
                    (id, user_id, type, main_category, sub_category, description, 
                     priority, keywords, hidden, icon, color, created_at)
                SELECT 
                    id, user_id, type, main_category, sub_category, description,
                    priority, keywords, hidden, icon, color, created_at
                FROM categories
            """)

            # 3. 删除旧表
            await conn.execute("DROP TABLE categories")

            # 4. 重命名新表
            await conn.execute("ALTER TABLE categories_new RENAME TO categories")

            # 5. 重建索引
            await conn.execute("CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id)")

            await conn.commit()
            self.logger.info("categories 表 UNIQUE 约束迁移完成")

        except Exception as e:
            self.logger.error(f"categories 表约束迁移失败: {e}", exc_info=True)
            # 尝试回滚
            try:
                await conn.execute("DROP TABLE IF EXISTS categories_new")
                await conn.rollback()
            except Exception:
                pass

    async def _migrate_users_cash_fields(self, conn: aiosqlite.Connection) -> None:
        """
        v6.78: 为 users 表添加现金存取相关字段

        新增字段:
        - cash_account_id: 现金账户ID (用于现金存取自动识别)
        - cash_transfer_category_id: 存取分类ID (匹配此分类时自动转换为转账)

        这两个字段用于现金存取转账特异检测功能
        """
        try:
            cursor = await conn.execute("PRAGMA table_info(users)")
            columns = [row[1] for row in await cursor.fetchall()]

            # 添加 cash_account_id 字段
            if "cash_account_id" not in columns:
                self.logger.info("为 users 表添加 cash_account_id 字段")
                await conn.execute("ALTER TABLE users ADD COLUMN cash_account_id INTEGER DEFAULT NULL")
                self.logger.info("成功为 users 表添加 cash_account_id 字段")
            else:
                self.logger.debug("users 表已有 cash_account_id 字段")

            # 添加 cash_transfer_category_id 字段
            if "cash_transfer_category_id" not in columns:
                self.logger.info("为 users 表添加 cash_transfer_category_id 字段")
                await conn.execute("ALTER TABLE users ADD COLUMN cash_transfer_category_id INTEGER DEFAULT NULL")
                self.logger.info("成功为 users 表添加 cash_transfer_category_id 字段")
            else:
                self.logger.debug("users 表已有 cash_transfer_category_id 字段")

        except Exception as e:
            self.logger.error(f"为 users 表添加现金存取字段失败: {e}", exc_info=True)

    async def _migrate_users_import_learning_fields(self, conn: aiosqlite.Connection) -> None:
        """为 users 表添加导入学习开关字段。"""
        try:
            cursor = await conn.execute("PRAGMA table_info(users)")
            columns = [row[1] for row in await cursor.fetchall()]

            if "import_learning_enabled" not in columns:
                self.logger.info("为 users 表添加 import_learning_enabled 字段")
                await conn.execute("ALTER TABLE users ADD COLUMN import_learning_enabled INTEGER DEFAULT 1")
                self.logger.info("成功为 users 表添加 import_learning_enabled 字段")
            else:
                self.logger.debug("users 表已有 import_learning_enabled 字段")

        except Exception as e:
            self.logger.error(f"为 users 表添加导入学习字段失败: {e}", exc_info=True)

    async def _migrate_users_investment_keyword_fields(self, conn: aiosqlite.Connection) -> None:
        """为 users 表添加投资识别关键词配置字段。"""
        try:
            cursor = await conn.execute("PRAGMA table_info(users)")
            columns = [row[1] for row in await cursor.fetchall()]

            for column_name in [
                "investment_platform_keywords",
                "investment_product_keywords",
                "investment_exclude_keywords",
            ]:
                if column_name not in columns:
                    self.logger.info("为 users 表添加 %s 字段", column_name)
                    await conn.execute(f"ALTER TABLE users ADD COLUMN {column_name} TEXT DEFAULT NULL")
                    self.logger.info("成功为 users 表添加 %s 字段", column_name)
                else:
                    self.logger.debug("users 表已有 %s 字段", column_name)

        except Exception as e:
            self.logger.error(f"为 users 表添加投资识别关键词字段失败: {e}", exc_info=True)

    async def _migrate_learning_rules_composite_fields(self, conn: aiosqlite.Connection) -> None:
        """为 import_learning_rules 表添加复合匹配字段。"""
        try:
            cursor = await conn.execute("PRAGMA table_info(import_learning_rules)")
            columns = [row[1] for row in await cursor.fetchall()]

            for col_name, col_def in [
                ("parser_id", "TEXT DEFAULT NULL"),
                ("composite_match_hash", "TEXT DEFAULT NULL"),
                ("match_features_json", "TEXT DEFAULT NULL"),
            ]:
                if col_name not in columns:
                    self.logger.info("为 import_learning_rules 表添加 %s 字段", col_name)
                    await conn.execute(f"ALTER TABLE import_learning_rules ADD COLUMN {col_name} {col_def}")

            # 为复合匹配哈希创建索引
            await conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_learning_rules_composite_hash "
                "ON import_learning_rules(user_id, composite_match_hash) "
                "WHERE composite_match_hash IS NOT NULL"
            )
        except Exception as e:
            self.logger.error("为 import_learning_rules 添加复合匹配字段失败: %s", e, exc_info=True)

    @staticmethod
    def _normalize_import_learning_text(raw_value: Any) -> str:
        """标准化导入学习文本，用于稳定匹配。"""
        if raw_value is None:
            return ""

        text = str(raw_value).strip().lower()
        if not text:
            return ""

        parts = [part.strip() for part in text.split("|") if part.strip()]
        if parts:
            text = " | ".join(parts)

        return " ".join(text.split())

    @staticmethod
    def build_composite_match_hash(
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> str | None:
        """构建复合匹配哈希键。

        至少需要两个非空字段才生成复合键，否则返回 None。
        """
        features = Database.build_composite_match_features(
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        if not features:
            return None

        key_aliases = {
            "parser_id": "p",
            "counterparty": "c",
            "description": "d",
            "payment_method": "m",
        }
        # 按 key 排序确保稳定
        return "|".join(f"{key_aliases[k]}={v}" for k, v in sorted(features.items()))

    @staticmethod
    def build_composite_match_features(
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> dict[str, str] | None:
        """构建用于复合学习和后续 ML 演进的结构化特征。

        仅保留稳定字段，明确排除时间等高唯一性字段。
        至少需要两个非空特征才返回。
        """
        norm = Database._normalize_import_learning_text
        features = {
            "parser_id": norm(parser_id),
            "counterparty": norm(counterparty),
            "description": norm(description),
            "payment_method": norm(payment_method),
        }
        non_empty = {key: value for key, value in features.items() if value}
        if len(non_empty) < 2:
            return None
        return non_empty

    async def _record_import_learning_rule_log(
        self,
        conn: aiosqlite.Connection,
        *,
        rule_id: int | None,
        user_id: int,
        action: str,
        match_type: str,
        match_value: str,
        normalized_match_value: str,
        session_id: str | None = None,
        preview_id: int | None = None,
        payload: dict[str, Any] | None = None,
    ) -> None:
        """记录导入学习规则审计日志。"""
        await conn.execute(
            """
            INSERT INTO import_learning_rule_logs (
                rule_id, user_id, action, match_type, match_value,
                normalized_match_value, session_id, preview_id,
                payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                rule_id,
                user_id,
                action,
                match_type,
                match_value,
                normalized_match_value,
                session_id,
                preview_id,
                json.dumps(payload or {}, ensure_ascii=False),
                datetime.now().isoformat(),
            ),
        )

    @log_method
    async def _get_connection(self) -> aiosqlite.Connection:
        """获取数据库连接"""
        if self._connection is None:
            self._connection = await aiosqlite.connect(str(self.db_path))
            self._connection.row_factory = aiosqlite.Row
        return self._connection

    @log_method
    def _calculate_hash(self, bill: dict[str, Any]) -> str:
        """
        计算账单哈希值用于去重

        Args:
        bill: 账单数据

        Returns:
        str: 哈希值
        """
        # 使用关键字段生成哈希
        key_fields = [
            str(bill.get("date", "")),
            str(bill.get("type", "")),
            str(bill.get("amount", "")),
            str(bill.get("counterparty", "")),
            str(bill.get("description", "")),
        ]
        key_string = "|".join(key_fields)
        return hashlib.md5(key_string.encode("utf-8")).hexdigest()

    @staticmethod
    def _normalize_two_factor_recovery_code(recovery_code: str) -> str:
        """标准化 2FA 恢复码文本。"""
        return str(recovery_code or "").strip().upper()

    @classmethod
    def _hash_two_factor_recovery_code(cls, recovery_code: str) -> str:
        """对 2FA 恢复码执行稳定哈希，避免持久化明文。"""
        normalized_code = cls._normalize_two_factor_recovery_code(recovery_code)
        if not normalized_code:
            return ""

        return hashlib.sha256(f"2fa-recovery:{normalized_code}".encode()).hexdigest()

    @log_method
    async def insert_bills(self, bills: list[dict[str, Any]], batch_id: str | None = None, user_id: int = 1) -> int:
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
            batch_id = datetime.now().strftime("%Y%m%d%H%M%S")
        conn = await self._get_connection()
        inserted_count = 0

        # 分批插入
        for i in range(0, len(bills), self.batch_size):
            batch = bills[i : i + self.batch_size]
            self.logger.debug(f"插入批次 {i // self.batch_size + 1}: {len(batch)} 条")

            for bill in batch:
                try:
                    # 计算哈希
                    bill_hash = self._calculate_hash(bill)

                    # 准备数据
                    now = datetime.now().isoformat()

                    await conn.execute(
                        """
                        INSERT INTO bills (
                            user_id, date, type, amount, counterparty, description,
                            payment_method, main_category, sub_category, batch_id, hash,
                            created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                        (
                            user_id,
                            bill.get("date"),
                            bill.get("type"),
                            bill.get("amount"),
                            bill.get("counterparty"),
                            bill.get("description"),
                            bill.get("payment_method"),
                            bill.get("main_category"),
                            bill.get("sub_category"),
                            batch_id,
                            bill_hash,
                            now,
                            now,
                        ),
                    )

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
    async def insert_bill(self, bill: dict[str, Any], user_id: int = 1) -> int:
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

        cursor = await conn.execute(
            """
        INSERT INTO bills (
        user_id, date, type, amount, counterparty, description, channel,
        main_category, sub_category, comment, account, tag,
        created_from_template, created_from_recurring, import_history_id,
        hash, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
            (
                user_id,
                bill.get("date"),
                bill.get("type"),
                bill.get("amount"),
                bill.get("counterparty"),
                bill.get("description"),
                bill.get("channel"),
                bill.get("category"),  # main_category
                bill.get("sub_category"),
                bill.get("comment"),
                bill.get("account"),
                bill.get("tag"),
                bill.get("created_from_template"),
                bill.get("created_from_recurring"),
                bill.get("import_history_id"),
                bill_hash,
                now,
                now,
            ),
        )

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def check_duplicate(self, bill: dict[str, Any], user_id: int = 1) -> bool:
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
            (
                user_id,
                bill_hash,
            ),
        )

        row = await cursor.fetchone()
        return row["count"] > 0

    @log_method
    @log_method
    async def get_bills_by_date_range(self, start_date: str, end_date: str, user_id: int = 1) -> list[dict[str, Any]]:
        """按日期范围查询账单（用于去重对比）

        Args:
            start_date: 开始日期 (YYYY-MM-DD)
            end_date: 结束日期 (YYYY-MM-DD)
            user_id: 用户ID

        Returns:
            List[Dict]: 账单列表
        """
        conn = await self._get_connection()

        query = """
            SELECT id, date, amount, counterparty, description,
                   type, main_category, sub_category, source_account_id
            FROM bills
            WHERE user_id = ?
              AND date >= ?
              AND date <= ?
            ORDER BY date
        """
        params = [user_id, start_date, end_date + " 23:59:59"]

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    async def get_bills(
        self, filters: dict[str, Any] | None = None, limit: int | None = None, offset: int = 0, user_id: int = 1
    ) -> list[dict[str, Any]]:
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
            if "id" in filters:
                query += " AND id = ?"
                params.append(filters["id"])

            # 日期范围筛选(支持多种参数名)
            date_from = filters.get("date_from") or filters.get("start_date")
            if date_from:
                query += " AND date >= ?"
                params.append(date_from)
                self.logger.debug(f"日期筛选: >= {date_from}")

            date_to = filters.get("date_to") or filters.get("end_date")
            if date_to:
                query += " AND date <= ?"
                params.append(date_to)
                self.logger.debug(f"日期筛选: <= {date_to}")

            # 类型筛选
            if "type" in filters:
                query += " AND type = ?"
                params.append(filters["type"])
                self.logger.debug(f"类型筛选: {filters['type']}")

            # 分类筛选
            if "main_category" in filters:
                query += " AND main_category = ?"
                params.append(filters["main_category"])
                self.logger.debug(f"主分类筛选: {filters['main_category']}")

            if "sub_category" in filters:
                query += " AND sub_category = ?"
                params.append(filters["sub_category"])
                self.logger.debug(f"子分类筛选: {filters['sub_category']}")

            # 批次ID筛选
            if "batch_id" in filters:
                query += " AND batch_id = ?"
                params.append(filters["batch_id"])

            # 交易对方模糊匹配
            if "counterparty" in filters:
                query += " AND counterparty LIKE ?"
                params.append(f"%{filters['counterparty']}%")
                self.logger.debug(f"交易对方筛选: %{filters['counterparty']}%")

            # 描述模糊匹配
            if "description" in filters:
                query += " AND description LIKE ?"
                params.append(f"%{filters['description']}%")
                self.logger.debug(f"描述筛选: %{filters['description']}%")

            # 关键词搜索(在描述和交易对方中)
            if filters.get("keyword"):
                query += " AND (description LIKE ? OR counterparty LIKE ?)"
                keyword_pattern = f"%{filters['keyword']}%"
                params.extend([keyword_pattern, keyword_pattern])
                self.logger.debug(f"关键词搜索: {keyword_pattern}")

            # 账户筛选 (列表)
            if filters.get("account_ids"):
                account_ids = filters["account_ids"]
                if isinstance(account_ids, list) and account_ids:
                    placeholders = ",".join(["?"] * len(account_ids))
                    # 只要源账户或目标账户在列表中即可
                    query += (
                        f" AND (source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
                    )
                    params.extend(account_ids)
                    params.extend(account_ids)
                    self.logger.debug(f"账户筛选: {account_ids}")

            # 分类筛选 (列表 - 包含主分类和子分类的元组或字典)
            # 格式: [{'main': '餐饮', 'sub': '早餐'}, {'main': '交通', 'sub': ''}]
            if filters.get("categories"):
                categories = filters["categories"]
                if isinstance(categories, list) and categories:
                    cat_conditions = []
                    for cat in categories:
                        main = cat.get("main")
                        sub = cat.get("sub")
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
            if filters.get("tag_ids"):
                tag_ids = filters["tag_ids"]
                if isinstance(tag_ids, list) and tag_ids:
                    placeholders = ",".join(["?"] * len(tag_ids))
                    query += f" AND id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))"
                    params.extend(tag_ids)
                    self.logger.debug(f"标签筛选: {tag_ids}")

            # 金额范围筛选
            if "min_amount" in filters and filters["min_amount"] is not None:
                query += " AND amount >= ?"
                params.append(float(filters["min_amount"]))
                self.logger.debug(f"最小金额筛选: >= {filters['min_amount']}")

            if "max_amount" in filters and filters["max_amount"] is not None:
                query += " AND amount <= ?"
                params.append(float(filters["max_amount"]))
                self.logger.debug(f"最大金额筛选: <= {filters['max_amount']}")

            # 金额过滤器(高级筛选)
            # 格式: "类型:值1[:值2]"
            # 支持类型: eq(等于), ne(不等于), gt(大于), lt(小于), gte(大于等于), lte(小于等于), between(范围)
            if filters.get("amount_filter"):
                amount_filter = filters["amount_filter"]
                self.logger.debug(f"金额过滤器: {amount_filter}")

                parts = amount_filter.split(":")
                if len(parts) >= 2:
                    filter_type = parts[0].lower()

                    try:
                        if filter_type == "eq":  # 等于
                            value = float(parts[1])
                            query += " AND amount = ?"
                            params.append(value)
                            self.logger.debug(f"金额等于: {value}")

                        elif filter_type == "ne":  # 不等于
                            value = float(parts[1])
                            query += " AND amount != ?"
                            params.append(value)
                            self.logger.debug(f"金额不等于: {value}")

                        elif filter_type == "gt":  # 大于
                            value = float(parts[1])
                            query += " AND amount > ?"
                            params.append(value)
                            self.logger.debug(f"金额大于: {value}")

                        elif filter_type == "lt":  # 小于
                            value = float(parts[1])
                            query += " AND amount < ?"
                            params.append(value)
                            self.logger.debug(f"金额小于: {value}")

                        elif filter_type == "gte":  # 大于等于
                            value = float(parts[1])
                            query += " AND amount >= ?"
                            params.append(value)
                            self.logger.debug(f"金额大于等于: {value}")

                        elif filter_type == "lte":  # 小于等于
                            value = float(parts[1])
                            query += " AND amount <= ?"
                            params.append(value)
                            self.logger.debug(f"金额小于等于: {value}")

                        elif filter_type == "between" and len(parts) >= 3:  # 范围
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
    async def create_bill(self, bill_data: dict[str, Any], user_id: int = 1) -> int | None:
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
            bill_payload = dict(bill_data)

            if "payment_method" not in bill_payload and bill_payload.get("channel") not in (None, ""):
                bill_payload["payment_method"] = bill_payload.pop("channel")

            now = datetime.now().isoformat()
            bill_payload.setdefault("created_at", now)
            bill_payload.setdefault("updated_at", now)

            # 必填字段
            required_fields = ["date", "type", "amount", "description"]
            for field in required_fields:
                if field not in bill_payload:
                    self.logger.error(f"缺少必填字段: {field}")
                    return None

            # 添加 user_id 到账单数据
            bill_payload["user_id"] = user_id

            # 构建INSERT语句
            columns = list(bill_payload.keys())
            placeholders = ", ".join(["?" for _ in columns])
            columns_str = ", ".join(columns)

            query = f"INSERT INTO bills ({columns_str}) VALUES ({placeholders})"
            values = [bill_payload[col] for col in columns]

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
    async def update_bill(self, bill_id: int, updates: dict[str, Any], user_id: int = 1) -> bool:
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
    async def move_all_transactions(self, from_account_id: int, to_account_id: int, user_id: int = 1) -> dict[str, Any]:
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
                "SELECT id, name FROM accounts WHERE user_id = ? AND id IN (?, ?)",
                (user_id, from_account_id, to_account_id),
            )
            accounts = await cursor.fetchall()

            if len(accounts) != 2:
                self.logger.error(
                    "账户不存在或无权限: from=%s, to=%s, user_id=%s", from_account_id, to_account_id, user_id
                )
                return {"success": False, "message": "账户不存在", "moved_count": 0}

            # 查询需要移动的交易数量（source_account_id或destination_account_id匹配）
            cursor = await conn.execute(
                "SELECT COUNT(*) FROM bills WHERE user_id = ? AND (source_account_id = ? OR destination_account_id = ?)",
                (user_id, from_account_id, from_account_id),
            )
            count = (await cursor.fetchone())[0]

            if count == 0:
                self.logger.info(f"源账户 {from_account_id} 没有交易记录")
                return {"success": True, "moved_count": 0}

            # 执行批量更新 - 更新source_account_id
            now = datetime.now().isoformat()
            cursor = await conn.execute(
                "UPDATE bills SET source_account_id = ?, updated_at = ? WHERE user_id = ? AND source_account_id = ?",
                (to_account_id, now, user_id, from_account_id),
            )
            source_updated = cursor.rowcount

            # 执行批量更新 - 更新destination_account_id
            cursor = await conn.execute(
                "UPDATE bills SET destination_account_id = ?, updated_at = ? WHERE user_id = ? AND destination_account_id = ?",
                (to_account_id, now, user_id, from_account_id),
            )
            dest_updated = cursor.rowcount

            await conn.commit()

            total_updated = source_updated + dest_updated
            self.logger.info(
                f"成功将 {total_updated} 条交易从账户 {from_account_id} 移动到 {to_account_id} "
                f"(源账户更新:{source_updated}, 目标账户更新:{dest_updated})"
            )

            # 同步源账户和目标账户的余额
            self.logger.info(f"开始同步账户余额: 源账户={from_account_id}, 目标账户={to_account_id}")

            # 获取同步前的余额
            cursor = await conn.execute(
                "SELECT id, name, balance FROM accounts WHERE user_id = ? AND id IN (?, ?)",
                (user_id, from_account_id, to_account_id),
            )
            accounts_before = {
                row["id"]: {"name": row["name"], "balance": row["balance"] or 0.0} for row in await cursor.fetchall()
            }

            # 同步源账户余额
            sync_result_from = await self.sync_account_balance(from_account_id)
            if sync_result_from:
                # 获取同步后的源账户余额
                cursor = await conn.execute(
                    "SELECT balance FROM accounts WHERE user_id = ? AND id = ?", (user_id, from_account_id)
                )
                row = await cursor.fetchone()
                new_balance_from = row["balance"] if row else 0.0
                old_balance_from = accounts_before.get(from_account_id, {}).get("balance", 0.0)
                self.logger.info(f"源账户 {from_account_id} 余额已同步: {old_balance_from} → {new_balance_from}")
            else:
                self.logger.warning(f"源账户 {from_account_id} 余额同步失败")

            # 同步目标账户余额
            sync_result_to = await self.sync_account_balance(to_account_id)
            if sync_result_to:
                # 获取同步后的目标账户余额
                cursor = await conn.execute(
                    "SELECT balance FROM accounts WHERE user_id = ? AND id = ?", (user_id, to_account_id)
                )
                row = await cursor.fetchone()
                new_balance_to = row["balance"] if row else 0.0
                old_balance_to = accounts_before.get(to_account_id, {}).get("balance", 0.0)
                self.logger.info(f"目标账户 {to_account_id} 余额已同步: {old_balance_to} → {new_balance_to}")
            else:
                self.logger.warning(f"目标账户 {to_account_id} 余额同步失败")

            # 清除账户映射缓存，确保前端获取最新余额
            self._clear_cache("account_mappings")

            return {"success": True, "moved_count": total_updated}

        except Exception as e:
            self.logger.error(
                "移动交易失败: from=%s, to=%s, user_id=%s, error=%s", from_account_id, to_account_id, user_id, e
            )
            await conn.rollback()
            return {"success": False, "message": str(e), "moved_count": 0}

    @log_method
    async def delete_all_transactions_by_account(self, account_id: int, user_id: int = 1) -> dict[str, Any]:
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
                "SELECT id, name FROM accounts WHERE user_id = ? AND id = ?", (user_id, account_id)
            )
            account = await cursor.fetchone()

            if not account:
                self.logger.error("账户不存在或无权限: account_id=%s, user_id=%s", account_id, user_id)
                return {"success": False, "message": "账户不存在", "deleted_count": 0}

            # 查询需要删除的交易数量（source_account_id或destination_account_id匹配）
            cursor = await conn.execute(
                "SELECT COUNT(*) FROM bills WHERE user_id = ? AND (source_account_id = ? OR destination_account_id = ?)",
                (user_id, account_id, account_id),
            )
            count = (await cursor.fetchone())[0]

            if count == 0:
                self.logger.info(f"账户 {account_id} 没有交易记录")
                return {"success": True, "deleted_count": 0}

            # 执行批量删除
            await conn.execute(
                "DELETE FROM bills WHERE user_id = ? AND (source_account_id = ? OR destination_account_id = ?)",
                (user_id, account_id, account_id),
            )

            await conn.commit()

            self.logger.info(f"成功删除账户 {account_id} 的 {count} 条交易")

            # 同步账户余额
            self.logger.info(f"开始同步账户余额: account_id={account_id}")

            # 获取同步前的余额
            cursor = await conn.execute(
                "SELECT name, balance FROM accounts WHERE user_id = ? AND id = ?", (user_id, account_id)
            )
            row = await cursor.fetchone()
            if row:
                account_name = row["name"]
                old_balance = row["balance"] or 0.0

                # 同步账户余额
                sync_result = await self.sync_account_balance(account_id)
                if sync_result:
                    # 获取同步后的余额
                    cursor = await conn.execute(
                        "SELECT balance FROM accounts WHERE user_id = ? AND id = ?", (user_id, account_id)
                    )
                    row = await cursor.fetchone()
                    new_balance = row["balance"] if row else 0.0
                    self.logger.info(f"账户 {account_id} ({account_name}) 余额已同步: {old_balance} → {new_balance}")
                else:
                    self.logger.warning(f"账户 {account_id} 余额同步失败")

                # 清除账户映射缓存，确保前端获取最新余额
                self._clear_cache("account_mappings")
            else:
                self.logger.warning(f"未找到账户 {account_id}，无法同步余额")

            return {"success": True, "deleted_count": count}

        except Exception as e:
            self.logger.error("删除账户交易失败: account_id=%s, user_id=%s, error=%s", account_id, user_id, e)
            await conn.rollback()
            return {"success": False, "message": str(e), "deleted_count": 0}

    @log_method
    async def clear_user_transactions(self, user_id: int = 1) -> dict[str, Any]:
        """清空指定用户的全部交易数据。"""
        conn = await self._get_connection()

        try:
            cursor = await conn.execute("SELECT COUNT(*) FROM bills WHERE user_id = ?", (user_id,))
            deleted_count = (await cursor.fetchone())[0]

            if deleted_count == 0:
                self.logger.info("用户 %s 没有可清除的交易数据", user_id)
                return {"success": True, "deleted_count": 0}

            await conn.execute(
                "DELETE FROM bill_tags WHERE bill_id IN (SELECT id FROM bills WHERE user_id = ?)", (user_id,)
            )
            await conn.execute("DELETE FROM bills WHERE user_id = ?", (user_id,))
            await conn.commit()

            self.logger.info("已清空用户 %s 的 %s 条交易数据", user_id, deleted_count)

            await self.sync_all_account_balances(user_id)
            self._clear_cache("account_mappings")

            return {"success": True, "deleted_count": deleted_count}

        except Exception as e:
            self.logger.error("清空用户交易失败: user_id=%s, error=%s", user_id, e)
            await conn.rollback()
            return {"success": False, "message": str(e), "deleted_count": 0}

    @log_method
    async def clear_user_data(self, user_id: int = 1) -> dict[str, Any]:
        """清空指定用户的业务数据（保留账号本身与登录态）。"""
        conn = await self._get_connection()

        try:
            counts = {}
            count_queries = {
                "bills": "SELECT COUNT(*) FROM bills WHERE user_id = ?",
                "accounts": "SELECT COUNT(*) FROM accounts WHERE user_id = ?",
                "categories": "SELECT COUNT(*) FROM categories WHERE user_id = ?",
                "tags": "SELECT COUNT(*) FROM tags WHERE user_id = ?",
                "templates": "SELECT COUNT(*) FROM bill_templates WHERE user_id = ?",
                "recurring_bills": "SELECT COUNT(*) FROM recurring_bills WHERE user_id = ?",
                "budgets": "SELECT COUNT(*) FROM budgets WHERE user_id = ?",
            }

            for key, query in count_queries.items():
                cursor = await conn.execute(query, (user_id,))
                counts[key] = (await cursor.fetchone())[0]

            await conn.execute("DELETE FROM bills_preview WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM bills_parser_template WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM import_sessions WHERE user_id = ?", (user_id,))

            await conn.execute(
                "DELETE FROM bill_tags WHERE bill_id IN (SELECT id FROM bills WHERE user_id = ?)", (user_id,)
            )
            await conn.execute("DELETE FROM bills WHERE user_id = ?", (user_id,))

            await conn.execute(
                "DELETE FROM budget_history WHERE budget_id IN (SELECT id FROM budgets WHERE user_id = ?)", (user_id,)
            )
            await conn.execute("DELETE FROM budgets WHERE user_id = ?", (user_id,))

            await conn.execute("DELETE FROM recurring_bills WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM bill_templates WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM saved_filters WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM account_transfers WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM tags WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM categories WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM account_types WHERE user_id = ?", (user_id,))
            await conn.execute("DELETE FROM accounts WHERE user_id = ?", (user_id,))

            await conn.commit()

            self._clear_cache("account_mappings")
            self._clear_cache("category_mappings")

            self.logger.info("已清空用户 %s 的业务数据: %s", user_id, counts)
            return {"success": True, "counts": counts}

        except Exception as e:
            self.logger.error("清空用户业务数据失败: user_id=%s, error=%s", user_id, e)
            await conn.rollback()
            return {"success": False, "message": str(e)}

    @log_method
    async def get_user_data_statistics(self, user_id: int = 1) -> dict[str, int]:
        """获取用户业务数据统计。

        使用 COUNT 查询直接返回统计结果，避免数据管理页为了展示数量而全量加载
        bills、templates 等大表数据，导致页面长期停留在加载状态。
        """
        conn = await self._get_connection()
        count_queries = {
            "billCount": "SELECT COUNT(*) FROM bills WHERE user_id = ?",
            "accountCount": "SELECT COUNT(*) FROM accounts WHERE user_id = ?",
            "categoryCount": "SELECT COUNT(*) FROM categories WHERE user_id = ?",
            "tagCount": "SELECT COUNT(*) FROM tags WHERE user_id = ?",
            "templateCount": "SELECT COUNT(*) FROM bill_templates WHERE user_id = ?",
        }

        statistics: dict[str, int] = {}

        for key, query in count_queries.items():
            async with conn.execute(query, (user_id,)) as cursor:
                row = await cursor.fetchone()
                statistics[key] = int(row[0] if row else 0)

        return statistics

    @log_method
    async def get_user_custom_exchange_rates(self, base_currency: str, user_id: int = 1) -> list[dict[str, Any]]:
        """获取用户自定义汇率（按目标币种取最新一条）。"""
        conn = await self._get_connection()
        normalized_base = (base_currency or "CNY").upper()

        query = """
            SELECT r1.*
            FROM user_exchange_rates r1
            INNER JOIN (
                SELECT to_currency, MAX(effective_date) AS max_effective_date
                FROM user_exchange_rates
                WHERE user_id = ? AND from_currency = ?
                GROUP BY to_currency
            ) r2 ON r1.to_currency = r2.to_currency
                AND r1.effective_date = r2.max_effective_date
            WHERE r1.user_id = ? AND r1.from_currency = ?
            ORDER BY r1.to_currency ASC
        """

        async with conn.execute(query, (user_id, normalized_base, user_id, normalized_base)) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def upsert_user_custom_exchange_rate(
        self, base_currency: str, target_currency: str, rate: float, user_id: int = 1
    ) -> dict[str, Any]:
        """更新用户自定义汇率。"""
        conn = await self._get_connection()
        normalized_base = (base_currency or "CNY").upper()
        normalized_target = (target_currency or "").upper()
        now = datetime.now().isoformat()

        try:
            await conn.execute(
                "DELETE FROM user_exchange_rates WHERE user_id = ? AND from_currency = ? AND to_currency = ?",
                (user_id, normalized_base, normalized_target),
            )
            cursor = await conn.execute(
                """
                INSERT INTO user_exchange_rates (
                    from_currency, to_currency, rate, source,
                    effective_date, created_at, updated_at, user_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (normalized_base, normalized_target, rate, "manual", now, now, now, user_id),
            )
            await conn.commit()

            return {
                "success": True,
                "id": cursor.lastrowid,
                "from_currency": normalized_base,
                "to_currency": normalized_target,
                "rate": rate,
                "update_time": int(datetime.fromisoformat(now).timestamp()),
            }
        except Exception as e:
            await conn.rollback()
            self.logger.error(
                "更新用户自定义汇率失败: user_id=%s, %s->%s, error=%s", user_id, normalized_base, normalized_target, e
            )
            return {"success": False, "message": str(e)}

    @log_method
    async def delete_user_custom_exchange_rate(
        self, base_currency: str, target_currency: str, user_id: int = 1
    ) -> bool:
        """删除用户自定义汇率。"""
        conn = await self._get_connection()
        normalized_base = (base_currency or "CNY").upper()
        normalized_target = (target_currency or "").upper()

        try:
            cursor = await conn.execute(
                "DELETE FROM user_exchange_rates WHERE user_id = ? AND from_currency = ? AND to_currency = ?",
                (user_id, normalized_base, normalized_target),
            )
            await conn.commit()
            return cursor.rowcount > 0
        except Exception as e:
            await conn.rollback()
            self.logger.error(
                "删除用户自定义汇率失败: user_id=%s, %s->%s, error=%s", user_id, normalized_base, normalized_target, e
            )
            return False

    @log_method
    async def batch_update_bills(
        self,
        bill_ids: list[int],
        updates: dict[str, Any],
        user_id: int = 1,
    ) -> dict[str, Any]:
        """
        批量更新账单

        Args:
        bill_ids: 账单ID列表
        updates: 更新字段字典
        user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
        Dict: 包含成功数、失败数和失败ID列表的统计信息
        """
        if not bill_ids:
            self.logger.warning("账单ID列表为空")
            return {"success_count": 0, "failed_count": 0, "failed_ids": []}

        if not updates:
            self.logger.warning("更新字段为空")
            return {"success_count": 0, "failed_count": 0, "failed_ids": []}

        allowed_update_fields = {
            "date",
            "type",
            "amount",
            "counterparty",
            "description",
            "payment_method",
            "main_category",
            "sub_category",
            "source_account_id",
            "destination_account_id",
            "destination_amount",
        }
        invalid_update_fields = sorted(set(updates) - allowed_update_fields)
        if invalid_update_fields:
            message = f"unsupported batch update fields: {', '.join(invalid_update_fields)}"
            self.logger.warning("批量更新字段非法: %s", ", ".join(invalid_update_fields))
            raise ValueError(message)

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
                values.append(user_id)

                query = f"UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?"

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

        result = {"success_count": success_count, "failed_count": len(failed_ids), "failed_ids": failed_ids}

        self.logger.info(f"批量更新完成: 成功={success_count}, 失败={len(failed_ids)}")
        return result

    @log_method
    async def batch_update_categories(
        self,
        bill_ids: list[int],
        main_category: str,
        sub_category: str,
        user_id: int = 1,
    ) -> dict[str, Any]:
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
            return {"success_count": 0, "failed_count": 0, "failed_ids": []}

        self.logger.info(f"准备批量修改 {len(bill_ids)} 条账单的分类")
        self.logger.debug(f"目标分类: {main_category}/{sub_category}")

        updates = {"main_category": main_category, "sub_category": sub_category}

        return await self.batch_update_bills(bill_ids, updates, user_id=user_id)

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
    async def get_statistics(self) -> dict[str, Any]:
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
            stats["total_bills"] = row[0]

        # 按类型统计
        async with conn.execute("""
            SELECT type, COUNT(*) as count, SUM(amount) as total
            FROM bills
            GROUP BY type
        """) as cursor:
            rows = await cursor.fetchall()
            stats["by_type"] = {row[0]: {"count": row[1], "total": row[2]} for row in rows}

        # 按分类统计
        async with conn.execute("""
            SELECT main_category, COUNT(*) as count, SUM(amount) as total
            FROM bills
            WHERE main_category IS NOT NULL
            GROUP BY main_category
        """) as cursor:
            rows = await cursor.fetchall()
            stats["by_category"] = {row[0]: {"count": row[1], "total": row[2]} for row in rows}

        self.logger.info("获取统计信息成功")
        return stats

    @log_method
    async def save_filter(self, name: str, filter_data: dict[str, Any], description: str | None = None) -> int:
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
            cursor = await conn.execute(
                """
                INSERT INTO saved_filters (name, filter_data, description, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?)
            """,
                (name, filter_json, description, now, now),
            )

            await conn.commit()
            filter_id = cursor.lastrowid
            self.logger.info(f"筛选条件保存成功: ID={filter_id}, 名称={name}")
            return filter_id

        except sqlite3.IntegrityError:
            # 如果名称已存在,则更新
            self.logger.info(f"筛选条件名称已存在,执行更新: {name}")
            await conn.execute(
                """
                UPDATE saved_filters
                SET filter_data = ?, description = ?, updated_at = ?
                WHERE name = ?
            """,
                (filter_json, description, now, name),
            )

            await conn.commit()

            # 获取ID
            async with conn.execute("SELECT id FROM saved_filters WHERE name = ?", (name,)) as cursor:
                row = await cursor.fetchone()
                filter_id = row[0] if row else 0

            self.logger.info(f"筛选条件更新成功: ID={filter_id}, 名称={name}")
            return filter_id

    @log_method
    async def get_saved_filters(self) -> list[dict[str, Any]]:
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
            filter_dict["filter_data"] = json.loads(filter_dict["filter_data"])
            filters.append(filter_dict)

        self.logger.info(f"查询到 {len(filters)} 个保存的筛选条件")
        return filters

    @log_method
    async def get_saved_filter(self, filter_id: int | None = None, name: str | None = None) -> dict[str, Any] | None:
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
            filter_dict["filter_data"] = json.loads(filter_dict["filter_data"])
            self.logger.info(f"查询到筛选条件: {filter_dict['name']}")
            return filter_dict

        self.logger.info("筛选条件不存在")
        return None

    def _get_first_recurring_occurrence(self, recurring: dict[str, Any], max_search_days: int = 370) -> date | None:
        """获取定时模板的首次计划日期（包含 start_date 当天）。"""
        start_date = self._parse_date_value(recurring.get("start_date"))
        if not start_date:
            return self._parse_date_value(recurring.get("next_date"))

        for offset in range(0, max_search_days + 1):
            candidate = start_date + timedelta(days=offset)
            if self._is_recurring_due_on_date(recurring, candidate):
                return candidate

        return self._parse_date_value(recurring.get("next_date"))

    @log_method
    async def delete_saved_filter(self, filter_id: int | None = None, name: str | None = None) -> bool:
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
            try:
                test_db_dir = os.environ.get(TEST_DB_DIR_ENV, "").strip()
                should_checkpoint = False

                if test_db_dir and str(self.db_path) not in {":memory:", "file::memory:?cache=shared"}:
                    try:
                        should_checkpoint = _is_relative_to(self.db_path.resolve(), Path(test_db_dir).resolve())
                    except OSError:
                        should_checkpoint = False

                if should_checkpoint:
                    await self._connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
                    await self._connection.commit()
            except (sqlite3.Error, aiosqlite.Error, OSError, RuntimeError, ValueError) as exc:
                self.logger.debug("关闭数据库前执行 WAL checkpoint 失败: %s", exc)

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
    async def query_bills(
        self, page: int = 1, page_size: int = 20, filters: dict[str, Any] | None = None, user_id: int = 1
    ) -> tuple:
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
            if "type" in filters:
                count_query += " AND type = ?"
                params.append(filters["type"])
            if "main_category" in filters:
                count_query += " AND main_category = ?"
                params.append(filters["main_category"])
            if "sub_category" in filters:
                count_query += " AND sub_category = ?"
                params.append(filters["sub_category"])
            if "start_date" in filters:
                count_query += " AND date >= ?"
                params.append(filters["start_date"])
            if "end_date" in filters:
                count_query += " AND date <= ?"
                params.append(filters["end_date"])
            if "keyword" in filters:
                count_query += " AND (description LIKE ? OR counterparty LIKE ?)"
                keyword = f"%{filters['keyword']}%"
                params.extend([keyword, keyword])

            if filters.get("account_ids"):
                account_ids = filters["account_ids"]
                if isinstance(account_ids, list) and account_ids:
                    placeholders = ",".join(["?"] * len(account_ids))
                    count_query += (
                        f" AND (source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
                    )
                    params.extend(account_ids)
                    params.extend(account_ids)

            if filters.get("categories"):
                categories = filters["categories"]
                if isinstance(categories, list) and categories:
                    cat_conditions = []
                    for cat in categories:
                        main = cat.get("main")
                        sub = cat.get("sub")
                        if main and sub:
                            cat_conditions.append("(main_category = ? AND sub_category = ?)")
                            params.extend([main, sub])
                        elif main:
                            cat_conditions.append("(main_category = ?)")
                            params.append(main)

                    if cat_conditions:
                        count_query += " AND (" + " OR ".join(cat_conditions) + ")"

            # 标签筛选
            if filters.get("tag_ids"):
                tag_ids = filters["tag_ids"]
                if isinstance(tag_ids, list) and tag_ids:
                    placeholders = ",".join(["?"] * len(tag_ids))
                    count_query += f" AND id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))"
                    params.extend(tag_ids)

            # 金额范围筛选
            if "min_amount" in filters and filters["min_amount"] is not None:
                count_query += " AND amount >= ?"
                params.append(float(filters["min_amount"]))

            if "max_amount" in filters and filters["max_amount"] is not None:
                count_query += " AND amount <= ?"
                params.append(float(filters["max_amount"]))

            # 金额过滤器(高级筛选)
            if filters.get("amount_filter"):
                amount_filter = filters["amount_filter"]
                parts = amount_filter.split(":")
                if len(parts) >= 2:
                    filter_type = parts[0].lower()
                    try:
                        if filter_type == "eq":
                            value = float(parts[1])
                            count_query += " AND amount = ?"
                            params.append(value)
                        elif filter_type == "ne":
                            value = float(parts[1])
                            count_query += " AND amount != ?"
                            params.append(value)
                        elif filter_type == "gt":
                            value = float(parts[1])
                            count_query += " AND amount > ?"
                            params.append(value)
                        elif filter_type == "lt":
                            value = float(parts[1])
                            count_query += " AND amount < ?"
                            params.append(value)
                        elif filter_type == "gte":
                            value = float(parts[1])
                            count_query += " AND amount >= ?"
                            params.append(value)
                        elif filter_type == "lte":
                            value = float(parts[1])
                            count_query += " AND amount <= ?"
                            params.append(value)
                        elif filter_type == "between" and len(parts) >= 3:
                            value1 = float(parts[1])
                            value2 = float(parts[2])
                            count_query += " AND amount BETWEEN ? AND ?"
                            params.extend([value1, value2])
                    except (ValueError, IndexError):
                        pass

        async with conn.execute(count_query, params) as cursor:
            row = await cursor.fetchone()
            total = row["total"] if row else 0

        return bills, total

    @log_method
    async def get_bill_by_id(self, bill_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """
        根据ID获取账单

        Args:
            bill_id: 账单ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            Optional[Dict]: 账单数据，不存在返回None
        """
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM bills WHERE id = ? AND user_id = ?", (bill_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def batch_delete_bills(self, bill_ids: list[int], user_id: int = 1) -> int:
        """
        批量删除账单

        Args:
            bill_ids: 账单ID列表
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            int: 删除的数量
        """
        if not bill_ids:
            return 0

        conn = await self._get_connection()
        placeholders = ",".join(["?" for _ in bill_ids])
        params = [*bill_ids, user_id]

        cursor = await conn.execute(f"DELETE FROM bills WHERE id IN ({placeholders}) AND user_id = ?", params)
        await conn.commit()

        deleted = cursor.rowcount
        self.logger.info(f"批量删除了 {deleted} 条账单 (user_id={user_id})")
        return deleted

    @log_method
    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
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
        self.logger.debug(f"[get_all_categories] 查询分类 (user_id={user_id})，排序：priority ASC")
        async with conn.execute(
            "SELECT * FROM categories WHERE user_id = ? ORDER BY priority ASC, main_category, sub_category", (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            categories = [dict(row) for row in rows]

        self.logger.debug(f"[get_all_categories] 返回 {len(categories)} 个分类")
        if categories:
            # 记录前5个分类的排序信息
            for i, cat in enumerate(categories[:5]):
                self.logger.debug(
                    f"[get_all_categories] #{i + 1} 分类: "
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
                    categories.append(
                        {
                            "id": 0,  # 虚拟ID
                            "main_category": row[0],
                            "sub_category": row[1],
                            "description": "",
                            "priority": 0,
                            "keywords": "",
                        }
                    )

        return categories

    @log_method
    async def get_category_by_name(
        self, main_category: str, sub_category: str, user_id: int = 1
    ) -> dict[str, Any] | None:
        """根据名称获取分类

        Args:
            main_category: 主分类名称
            sub_category: 子分类名称
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM categories WHERE main_category = ? AND sub_category = ? AND user_id = ?",
            (main_category, sub_category, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_category(self, category_data: dict[str, Any], user_id: int = 1) -> int | None:
        """
        创建分类

        Args:
            category_data: 分类数据字典，包含main_category, sub_category等字段
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            Optional[int]: 创建成功返回分类ID，失败返回None
        """
        conn = await self._get_connection()

        main_cat = category_data.get("main_category")
        sub_cat = category_data.get("sub_category", "")

        self.logger.info(f"开始创建分类: {main_cat}/{sub_cat} (user_id={user_id})")

        try:
            columns = [
                "type",
                "main_category",
                "sub_category",
                "description",
                "priority",
                "keywords",
                "hidden",
                "icon",
                "color",
                "created_at",
                "user_id",
            ]
            placeholders = ", ".join(["?" for _ in columns])

            values = [
                category_data.get("type", 1),
                main_cat,
                sub_cat,
                category_data.get("description", ""),
                category_data.get("priority", 0),
                category_data.get("keywords", ""),
                category_data.get("hidden", False),
                category_data.get("icon", ""),
                category_data.get("color", ""),
                datetime.now().isoformat(),
                user_id,
            ]

            self.logger.debug(f"执行插入: columns={columns}, values={values}")

            cursor = await conn.execute(
                f"INSERT INTO categories ({', '.join(columns)}) VALUES ({placeholders})", values
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
            return None

    @log_method
    async def update_category(self, category_id: int, updates: dict[str, Any], user_id: int = 1) -> bool:
        """更新分类

        Args:
            category_id: 分类ID
            updates: 更新数据字典
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        try:
            # 过滤掉无效字段
            valid_fields = [
                "type",
                "main_category",
                "sub_category",
                "description",
                "priority",
                "keywords",
                "hidden",
                "icon",
                "color",
            ]
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
                (category_id, user_id),
            ) as cursor:
                category = await cursor.fetchone()

            if not category:
                self.logger.warning(f"分类ID {category_id} 不存在或不属于用户 {user_id}")
                return False

            category = dict(category)
            main_category = category["main_category"]
            sub_category = category["sub_category"]

            # 判断是否为父级分类（sub_category为空字符串）
            if sub_category == "" or sub_category is None:
                # 父级分类：级联删除所有子分类 (限制user_id)
                self.logger.info(f"删除父级分类 '{main_category}' 及其所有子分类 (user_id={user_id})")

                # 先查询有多少子分类
                async with conn.execute(
                    "SELECT COUNT(*) as count FROM categories WHERE main_category = ? AND sub_category != '' AND user_id = ?",
                    (main_category, user_id),
                ) as cursor:
                    result = await cursor.fetchone()
                    child_count = result[0] if result else 0

                # 删除该主分类下的所有记录（包括父级和子级）
                cursor = await conn.execute(
                    "DELETE FROM categories WHERE main_category = ? AND user_id = ?", (main_category, user_id)
                )
                deleted_count = cursor.rowcount
                await conn.commit()

                self.logger.info(
                    f"已删除父级分类 '{main_category}' (ID: {category_id}) "
                    f"及其 {child_count} 个子分类，共删除 {deleted_count} 条记录 (user_id={user_id})"
                )

                # 清除缓存
                self._clear_cache("category_mappings")

                return True
            else:
                # 子分类：仅删除该子分类 (限制user_id)
                self.logger.info(f"删除子分类 '{main_category}/{sub_category}' (ID: {category_id}, user_id={user_id})")

                await conn.execute("DELETE FROM categories WHERE id = ? AND user_id = ?", (category_id, user_id))
                await conn.commit()

                self.logger.info(
                    f"已删除子分类 '{main_category}/{sub_category}' (ID: {category_id}, user_id={user_id})"
                )

                # 清除缓存
                self._clear_cache("category_mappings")

                return True

        except Exception as e:
            self.logger.error(f"删除分类失败: {e}", exc_info=True)
            return False

    @log_method
    async def get_category_by_id(self, category_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """获取单个分类

        Args:
            category_id: 分类ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        conn.row_factory = aiosqlite.Row
        async with conn.execute(
            "SELECT * FROM categories WHERE id = ? AND user_id = ?", (category_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_category_statistics(
        self, period: str = "month", start_date: str | None = None, end_date: str | None = None, user_id: int = 1
    ) -> list[dict[str, Any]]:
        """
        获取分类统计

        Args:
            period: 统计周期(目前未使用,保留以便未来扩展)
            start_date: 开始日期
            end_date: 结束日期
            user_id: 用户ID (默认1, 用于多用户数据隔离)

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
        WHERE main_category IS NOT NULL AND user_id = ?
        """
        params = [user_id]

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
    async def get_all_accounts(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取所有账户

        Args:
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM accounts WHERE user_id = ? ORDER BY display_order, name", (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_account_by_id(self, account_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """根据ID获取账户

        Args:
            account_id: 账户ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM accounts WHERE id = ? AND user_id = ?", (account_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_sub_accounts(self, parent_id: int, user_id: int = 1) -> list[dict[str, Any]]:
        """获取子账户列表

        Args:
            parent_id: 父账户ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM accounts WHERE parent_id = ? AND user_id = ?", (parent_id, user_id)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_account_alias_mapping(self, user_id: int = 1) -> dict[str, int]:
        """获取账户别名到账户ID的映射

        用于账单导入时根据别名匹配账户。返回一个字典，键为别名（小写），值为账户ID。
        同时包含账户名称作为默认别名。

        Args:
            user_id: 用户ID (默认1, 用于多用户数据隔离)

        Returns:
            Dict[str, int]: 别名 -> 账户ID 映射
            例如: {"微信零钱": 1, "支付宝余额": 2, "余额宝": 2}
        """
        import json

        conn = await self._get_connection()

        alias_map: dict[str, int] = {}

        async with conn.execute("SELECT id, name, aliases FROM accounts WHERE user_id = ?", (user_id,)) as cursor:
            rows = await cursor.fetchall()

            for row in rows:
                account_id = row["id"]
                account_name = row["name"]
                aliases_json = row["aliases"]

                # 账户名称作为默认别名
                if account_name:
                    alias_map[account_name.lower()] = account_id
                    alias_map[account_name] = account_id  # 保留原始大小写

                # 解析JSON格式的别名数组
                if aliases_json:
                    try:
                        aliases = json.loads(aliases_json)
                        if isinstance(aliases, list):
                            for alias in aliases:
                                if alias and isinstance(alias, str):
                                    alias_map[alias.lower()] = account_id
                                    alias_map[alias] = account_id  # 保留原始大小写
                    except json.JSONDecodeError:
                        self.logger.warning(f"账户 {account_id} 的别名JSON解析失败: {aliases_json}")

        self.logger.info(f"加载账户别名映射: {len(alias_map)} 条")
        return alias_map

    @log_method
    async def get_historical_source_account_suggestion(
        self,
        user_id: int = 1,
        payment_method: str = "",
        counterparty: str = "",
        description: str = "",
        bill_type: str = "",
    ) -> dict[str, Any] | None:
        """基于历史账单为源账户提供建议。"""
        normalized_payment_method = self._normalize_import_learning_text(payment_method)
        normalized_counterparty = self._normalize_import_learning_text(counterparty)
        normalized_description = self._normalize_import_learning_text(description)
        normalized_type = str(bill_type or "").strip().lower()

        if not any([normalized_payment_method, normalized_counterparty, normalized_description]):
            return None

        conn = await self._get_connection()
        clauses: list[str] = []
        params: list[Any] = [user_id]

        if normalized_payment_method:
            clauses.append("LOWER(TRIM(COALESCE(payment_method, ''))) = ?")
            params.append(normalized_payment_method)
        if normalized_counterparty:
            clauses.append("LOWER(TRIM(COALESCE(counterparty, ''))) = ?")
            params.append(normalized_counterparty)
        if normalized_description:
            clauses.append("LOWER(TRIM(COALESCE(description, ''))) = ?")
            params.append(normalized_description)

        query = f"""
            SELECT source_account_id, type, payment_method, counterparty, description, date
            FROM bills
            WHERE user_id = ?
              AND source_account_id IS NOT NULL
              AND source_account_id != 0
              AND ({" OR ".join(clauses)})
            ORDER BY date DESC, id DESC
            LIMIT 300
        """

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()

        score_by_account: dict[int, dict[str, Any]] = {}
        for row in rows:
            account_id = int(row["source_account_id"])
            score = 0
            reasons: list[str] = []

            row_payment_method = self._normalize_import_learning_text(row["payment_method"])
            row_counterparty = self._normalize_import_learning_text(row["counterparty"])
            row_description = self._normalize_import_learning_text(row["description"])
            row_type = str(row["type"] or "").strip().lower()

            if normalized_payment_method and row_payment_method == normalized_payment_method:
                score += 8
                reasons.append("payment_method")
            if normalized_counterparty and row_counterparty == normalized_counterparty:
                score += 5
                reasons.append("counterparty")
            if normalized_description and row_description == normalized_description:
                score += 3
                reasons.append("description")
            if normalized_type and row_type == normalized_type:
                score += 2
                reasons.append("type")

            if score <= 0:
                continue

            existing = score_by_account.get(account_id)
            if not existing:
                score_by_account[account_id] = {
                    "account_id": account_id,
                    "score": score,
                    "reasons": reasons,
                    "hits": 1,
                }
            else:
                existing["score"] += score
                existing["hits"] += 1
                existing["reasons"] = sorted(set(existing["reasons"] + reasons))

        if not score_by_account:
            return None

        best = max(score_by_account.values(), key=lambda item: (item["score"], item["hits"], -item["account_id"]))
        self.logger.debug(
            "[历史源账户建议] user_id=%d -> account_id=%s, score=%s, reasons=%s",
            user_id,
            best["account_id"],
            best["score"],
            ",".join(best["reasons"]),
        )
        return best

    @log_method
    async def get_historical_destination_account_suggestion(
        self,
        user_id: int = 1,
        payment_method: str = "",
        counterparty: str = "",
        description: str = "",
        bill_type: str = "",
        source_account_id: int | None = None,
    ) -> dict[str, Any] | None:
        """基于历史账单为目标账户提供建议。"""
        normalized_payment_method = self._normalize_import_learning_text(payment_method)
        normalized_counterparty = self._normalize_import_learning_text(counterparty)
        normalized_description = self._normalize_import_learning_text(description)
        normalized_type = str(bill_type or "").strip().lower()

        if not any([normalized_payment_method, normalized_counterparty, normalized_description]):
            return None

        conn = await self._get_connection()
        clauses: list[str] = []
        params: list[Any] = [user_id]

        if normalized_payment_method:
            clauses.append("LOWER(TRIM(COALESCE(payment_method, ''))) = ?")
            params.append(normalized_payment_method)
        if normalized_counterparty:
            clauses.append("LOWER(TRIM(COALESCE(counterparty, ''))) = ?")
            params.append(normalized_counterparty)
        if normalized_description:
            clauses.append("LOWER(TRIM(COALESCE(description, ''))) = ?")
            params.append(normalized_description)

        query = f"""
            SELECT destination_account_id, type, payment_method, counterparty, description, date
            FROM bills
            WHERE user_id = ?
              AND destination_account_id IS NOT NULL
              AND destination_account_id != 0
              AND ({" OR ".join(clauses)})
            ORDER BY date DESC, id DESC
            LIMIT 300
        """

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()

        score_by_account: dict[int, dict[str, Any]] = {}
        for row in rows:
            account_id = int(row["destination_account_id"])
            if source_account_id and int(source_account_id) == account_id:
                continue

            score = 0
            reasons: list[str] = []

            row_payment_method = self._normalize_import_learning_text(row["payment_method"])
            row_counterparty = self._normalize_import_learning_text(row["counterparty"])
            row_description = self._normalize_import_learning_text(row["description"])
            row_type = str(row["type"] or "").strip().lower()

            if normalized_payment_method and row_payment_method == normalized_payment_method:
                score += 6
                reasons.append("payment_method")
            if normalized_counterparty and row_counterparty == normalized_counterparty:
                score += 6
                reasons.append("counterparty")
            if normalized_description and row_description == normalized_description:
                score += 4
                reasons.append("description")
            if normalized_type and row_type == normalized_type:
                score += 2
                reasons.append("type")

            if score <= 0:
                continue

            existing = score_by_account.get(account_id)
            if not existing:
                score_by_account[account_id] = {
                    "account_id": account_id,
                    "score": score,
                    "reasons": reasons,
                    "hits": 1,
                }
            else:
                existing["score"] += score
                existing["hits"] += 1
                existing["reasons"] = sorted(set(existing["reasons"] + reasons))

        if not score_by_account:
            return None

        best = max(score_by_account.values(), key=lambda item: (item["score"], item["hits"], -item["account_id"]))
        self.logger.debug(
            "[历史目标账户建议] user_id=%d -> account_id=%s, score=%s, reasons=%s",
            user_id,
            best["account_id"],
            best["score"],
            ",".join(best["reasons"]),
        )
        return best

    @log_method
    async def create_account(self, data: dict[str, Any], user_id: int = 1) -> int:
        """创建账户

        Args:
            data: 账户数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        # 提取子账户
        sub_accounts = data.get("subAccounts", [])

        aliases_value = data.get("aliases")
        if isinstance(aliases_value, list):
            aliases_value = json.dumps(
                [str(alias).strip() for alias in aliases_value if str(alias).strip()],
                ensure_ascii=False,
            )

        # 获取 parentId (前端字段) 或 parent_id (后端字段)
        parent_id = data.get("parentId") or data.get("parent_id", 0)

        cursor = await conn.execute(
            """
            INSERT INTO accounts (
                name, type, category, currency, icon, color,
                balance, initial_balance, hidden, display_order,
                comment, aliases, parent_id, created_at, updated_at, user_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
            (
                data.get("name"),
                data.get("type", 1),
                data.get("category"),
                data.get("currency", "CNY"),
                data.get("icon"),
                data.get("color"),
                data.get("balance", 0.0),
                data.get("initial_balance", 0.0),
                1 if data.get("hidden", False) else 0,
                data.get("display_order", 0),
                data.get("comment"),
                aliases_value,  # JSON数组格式存储别名
                parent_id,
                now,
                now,
                user_id,
            ),
        )

        account_id = cursor.lastrowid
        await conn.commit()

        # 递归创建子账户
        if sub_accounts and isinstance(sub_accounts, list):
            for sub_account in sub_accounts:
                sub_account["parentId"] = account_id
                # 递归调用（传递user_id）
                await self.create_account(sub_account, user_id)

        return account_id

    @log_method
    async def update_account(self, account_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
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
        update_data["updated_at"] = datetime.now().isoformat()

        # 映射 parentId -> parent_id
        if "parentId" in update_data:
            update_data["parent_id"] = update_data.pop("parentId")

        # 移除subAccounts字段（这是嵌套数据，不应该更新到父账户表）
        if "subAccounts" in update_data:
            self.logger.warning(f"账户更新数据包含subAccounts字段，已移除: account_id={account_id}")
            del update_data["subAccounts"]

        # 定义允许更新的字段白名单
        valid_columns = {
            "name",
            "type",
            "category",
            "currency",
            "icon",
            "color",
            "balance",
            "initial_balance",
            "hidden",
            "display_order",
            "comment",
            "aliases",
            "parent_id",
            "updated_at",
        }

        # 过滤掉不在白名单中的字段
        # 这可以防止前端传递多余字段导致SQL错误（如 clientSessionId, balanceTime 等）
        filtered_data = {k: v for k, v in update_data.items() if k in valid_columns}

        if not filtered_data:
            self.logger.warning(f"账户更新数据过滤后为空: account_id={account_id}, 原始字段={list(update_data.keys())}")
            return False

        filtered_data.pop("id", None)

        set_clause = ", ".join(f"{key} = ?" for key in filtered_data.keys())
        values = list(filtered_data.values())
        values.extend([account_id, user_id])

        self.logger.info(f"更新账户: id={account_id}, user_id={user_id}, 字段={list(filtered_data.keys())}")

        cursor = await conn.execute(f"UPDATE accounts SET {set_clause} WHERE id = ? AND user_id = ?", values)
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

        cursor = await conn.execute("DELETE FROM accounts WHERE id = ? AND user_id = ?", (account_id, user_id))
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def update_account_balance(self, account_id: int, amount: float, operation: str = "add") -> bool:
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
            async with conn.execute("SELECT balance FROM accounts WHERE id = ?", (account_id,)) as cursor:
                row = await cursor.fetchone()
                if not row:
                    self.logger.warning(f"账户不存在: account_id={account_id}")
                    return False

                current_balance = row["balance"] or 0.0

            # 计算新余额
            if operation == "add":
                new_balance = current_balance + amount
                self.logger.info(f"增加余额: account_id={account_id}, 原:{current_balance} + {amount} = {new_balance}")
            elif operation == "subtract":
                new_balance = current_balance - amount
                self.logger.info(f"减少余额: account_id={account_id}, 原:{current_balance} - {amount} = {new_balance}")
            else:
                self.logger.error(f"未知操作类型: {operation}")
                return False

            # 更新余额
            cursor = await conn.execute(
                "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                (new_balance, datetime.now().isoformat(), account_id),
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
                async with conn.execute("SELECT name FROM accounts WHERE id = ?", (account_id,)) as cursor:
                    row = await cursor.fetchone()
                    if not row:
                        self.logger.warning(f"账户不存在: account_id={account_id}")
                        return 0.0
                    account_name = row["name"]

            # 获取初始余额
            async with conn.execute("SELECT initial_balance FROM accounts WHERE id = ?", (account_id,)) as cursor:
                row = await cursor.fetchone()
                initial_balance = row["initial_balance"] if row else 0.0

            # 计算所有账单的余额变动（基于source_account_id和destination_account_id字段）
            # 收入：增加余额
            # 支出：减少余额
            # 转账：源账户减少，目标账户增加
            # 投资：源账户减少，目标账户增加

            # 作为源账户的交易（收入增加，支出/转账/投资减少）
            async with conn.execute(
                """
                SELECT
                    SUM(CASE WHEN type = '收入' THEN amount ELSE 0 END) as income,
                    SUM(CASE WHEN type = '支出' THEN amount ELSE 0 END) as expense,
                    SUM(CASE WHEN type = '转账' THEN amount ELSE 0 END) as transfer_out,
                    SUM(CASE WHEN type = '投资' THEN amount ELSE 0 END) as investment_out
                FROM bills
                WHERE source_account_id = ?
            """,
                (account_id,),
            ) as cursor:
                row = await cursor.fetchone()
                income = row["income"] or 0.0
                expense = row["expense"] or 0.0
                transfer_out = row["transfer_out"] or 0.0
                investment_out = row["investment_out"] or 0.0

            # 作为目标账户的交易（转账/投资增加）
            async with conn.execute(
                """
                SELECT
                    SUM(CASE WHEN type = '转账' THEN destination_amount ELSE 0 END) as transfer_in,
                    SUM(CASE WHEN type = '投资' THEN destination_amount ELSE 0 END) as investment_in
                FROM bills
                WHERE destination_account_id = ?
            """,
                (account_id,),
            ) as cursor:
                row = await cursor.fetchone()
                transfer_in = row["transfer_in"] or 0.0
                investment_in = row["investment_in"] or 0.0

            # 余额计算: 初始余额 + 收入 - 支出 - 转账转出 + 转账转入 - 投资转出 + 投资转入
            calculated_balance = (
                initial_balance + income - expense - transfer_out + transfer_in - investment_out + investment_in
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
            async with conn.execute("SELECT name FROM accounts WHERE id = ?", (account_id,)) as cursor:
                row = await cursor.fetchone()
                if not row:
                    self.logger.warning(f"账户不存在: account_id={account_id}")
                    return False
                account_name = row["name"]

            # 计算实际余额
            calculated_balance = await self.calculate_account_balance(account_id, account_name)

            # 更新余额
            cursor = await conn.execute(
                "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                (calculated_balance, datetime.now().isoformat(), account_id),
            )
            await conn.commit()

            self.logger.info(f"同步账户余额: account_id={account_id}, balance={calculated_balance}")

            return cursor.rowcount > 0

        except Exception as e:
            self.logger.error(f"同步账户余额失败: {e}", exc_info=True)
            return False

    @log_method
    async def sync_all_account_balances(self, user_id: int = 1) -> dict[str, Any]:
        """
        同步所有账户的余额（将计算出的实际余额写入balance字段）

        v6.68: 新增批量同步所有账户余额的功能

        Args:
            user_id: 用户ID

        Returns:
            Dict包含同步结果:
            - total_accounts: 总账户数
            - synced_accounts: 成功同步的账户数
            - discrepancies: 余额差异列表 [{account_id, name, old_balance, new_balance, diff}]
            - errors: 错误信息列表
        """
        try:
            conn = await self._get_connection()

            # 获取所有账户
            async with conn.execute(
                "SELECT id, name, balance, initial_balance FROM accounts WHERE user_id = ?", (user_id,)
            ) as cursor:
                accounts = await cursor.fetchall()

            result = {"total_accounts": len(accounts), "synced_accounts": 0, "discrepancies": [], "errors": []}

            self.logger.info(f"[批量同步账户余额] 开始同步 {len(accounts)} 个账户 (user_id={user_id})")

            for account in accounts:
                account_id = account["id"]
                account_name = account["name"]
                old_balance = account["balance"] or 0.0

                try:
                    # 计算实际余额
                    new_balance = await self.calculate_account_balance(account_id, account_name)

                    # 如果余额不同，记录差异
                    if abs(old_balance - new_balance) > 0.001:
                        diff = new_balance - old_balance
                        result["discrepancies"].append(
                            {
                                "account_id": account_id,
                                "name": account_name,
                                "old_balance": round(old_balance, 2),
                                "new_balance": round(new_balance, 2),
                                "diff": round(diff, 2),
                            }
                        )
                        self.logger.info(
                            f"[余额差异] 账户 '{account_name}' (ID={account_id}): "
                            f"旧余额={old_balance:.2f}, 新余额={new_balance:.2f}, 差异={diff:.2f}"
                        )

                    # 更新余额
                    await conn.execute(
                        "UPDATE accounts SET balance = ?, updated_at = ? WHERE id = ?",
                        (new_balance, datetime.now().isoformat(), account_id),
                    )
                    result["synced_accounts"] += 1

                except Exception as e:
                    error_msg = f"账户 '{account_name}' (ID={account_id}) 同步失败: {e!s}"
                    result["errors"].append(error_msg)
                    self.logger.error(error_msg, exc_info=True)

            await conn.commit()

            # 清除账户映射缓存
            self._clear_cache("account_mappings")

            self.logger.info(
                f"[批量同步账户余额完成] 成功={result['synced_accounts']}/{result['total_accounts']}, "
                f"差异={len(result['discrepancies'])}个, 错误={len(result['errors'])}个"
            )

            return result

        except Exception as e:
            self.logger.error(f"批量同步账户余额失败: {e}", exc_info=True)
            return {"total_accounts": 0, "synced_accounts": 0, "discrepancies": [], "errors": [str(e)]}

    @log_method
    async def get_all_tags(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取所有标签

        Args:
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM tags WHERE user_id = ? ORDER BY display_order, created_at DESC", (user_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_tag_by_id(self, tag_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """根据ID获取标签

        Args:
            tag_id: 标签ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM tags WHERE id = ? AND user_id = ?", (tag_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def create_tag(self, data: dict[str, Any], user_id: int = 1) -> int:
        """创建标签

        Args:
            data: 标签数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute(
            """
            INSERT INTO tags (name, color, icon, hidden, created_at, updated_at, user_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
            (
                data.get("name"),
                data.get("color", "#000000"),
                data.get("icon", ""),
                data.get("hidden", False),
                now,
                now,
                user_id,
            ),
        )

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def update_tag(self, tag_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        """更新标签

        Args:
            tag_id: 标签ID
            data: 更新数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        if not data:
            return False

        conn = await self._get_connection()
        data["updated_at"] = datetime.now().isoformat()

        set_clause = ", ".join(f"{key} = ?" for key in data.keys())
        values = list(data.values())
        values.extend([tag_id, user_id])

        cursor = await conn.execute(f"UPDATE tags SET {set_clause} WHERE id = ? AND user_id = ?", values)
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

        cursor = await conn.execute("DELETE FROM tags WHERE id = ? AND user_id = ?", (tag_id, user_id))
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def update_tag_display_orders(self, orders: list[tuple], user_id: int = 1) -> bool:
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
                    (display_order, now, tag_id, user_id),
                )

            await conn.commit()
            self.logger.info(f"[update_tag_display_orders] 成功更新{len(orders)}个标签的显示顺序 (user_id={user_id})")
            return True
        except Exception as e:
            self.logger.error(f"更新标签显示顺序失败: {e}", exc_info=True)
            await conn.rollback()
            return False

    @log_method
    async def get_all_templates(self, user_id: int = 1, template_type: int | None = None) -> list[dict[str, Any]]:
        """获取所有模板（按模板类型统一返回前端 DTO 结构）。"""
        conn = await self._get_connection()

        templates: list[dict[str, Any]] = []

        if template_type in (None, 1):
            async with conn.execute(
                """
                SELECT * FROM bill_templates
                WHERE user_id = ?
                ORDER BY COALESCE(display_order, 0), is_favorite DESC, use_count DESC, name
                """,
                (user_id,),
            ) as cursor:
                rows = await cursor.fetchall()
                templates.extend(self._serialize_template_row(dict(row), template_type=1) for row in rows)

        if template_type in (None, 2):
            async with conn.execute(
                """
                SELECT * FROM recurring_bills
                WHERE user_id = ?
                ORDER BY COALESCE(display_order, 0), name
                """,
                (user_id,),
            ) as cursor:
                rows = await cursor.fetchall()
                templates.extend(self._serialize_template_row(dict(row), template_type=2) for row in rows)

        return templates

    @log_method
    async def get_template_by_id(
        self, template_id: int, user_id: int = 1, template_type: int | None = None
    ) -> dict[str, Any] | None:
        """根据ID获取模板。"""
        conn = await self._get_connection()

        table_candidates = []
        if template_type == 1:
            table_candidates = [("bill_templates", 1)]
        elif template_type == 2:
            table_candidates = [("recurring_bills", 2)]
        else:
            table_candidates = [("bill_templates", 1), ("recurring_bills", 2)]

        for table_name, resolved_type in table_candidates:
            async with conn.execute(
                f"SELECT * FROM {table_name} WHERE id = ? AND user_id = ?", (template_id, user_id)
            ) as cursor:
                row = await cursor.fetchone()
                if row:
                    return self._serialize_template_row(dict(row), template_type=resolved_type)

        return None

    @log_method
    async def create_template(self, data: dict[str, Any], user_id: int = 1) -> int:
        """创建模板。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        template_type = int(data.get("templateType") or 1)
        display_order = await self._get_next_template_display_order(conn, template_type, user_id)

        if template_type == 2:
            cursor = await conn.execute(
                """
                INSERT INTO recurring_bills (
                    user_id, template_id, name, description, type, category, amount,
                    account, counterparty, destination_amount, hide_amount, tag,
                    comment, frequency, scheduled_frequency_type, start_date, end_date,
                    next_date, hidden, display_order, utc_offset, enabled, auto_create,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    None,
                    data.get("name", ""),
                    data.get("description", ""),
                    data.get("type"),
                    data.get("categoryId", ""),
                    float(data.get("sourceAmount") or 0),
                    data.get("sourceAccountId", "0"),
                    data.get("destinationAccountId", "0"),
                    float(data.get("destinationAmount") or 0),
                    1 if data.get("hideAmount") else 0,
                    self._serialize_template_tag_ids(data.get("tagIds")),
                    data.get("comment", ""),
                    data.get("scheduledFrequency", ""),
                    int(data.get("scheduledFrequencyType") or 0),
                    data.get("scheduledStartDate"),
                    data.get("scheduledEndDate"),
                    data.get("scheduledStartDate") or now[:10],
                    1 if data.get("hidden") else 0,
                    display_order,
                    int(data.get("utcOffset") or 0),
                    1,
                    0,
                    now,
                    now,
                ),
            )
        else:
            cursor = await conn.execute(
                """
                INSERT INTO bill_templates (
                    user_id, name, description, type, category, amount, account,
                    counterparty, destination_amount, hide_amount, tag, comment,
                    is_favorite, display_order, hidden, utc_offset, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    data.get("name", ""),
                    data.get("description", ""),
                    data.get("type"),
                    data.get("categoryId", ""),
                    float(data.get("sourceAmount") or 0),
                    data.get("sourceAccountId", "0"),
                    data.get("destinationAccountId", "0"),
                    float(data.get("destinationAmount") or 0),
                    1 if data.get("hideAmount") else 0,
                    self._serialize_template_tag_ids(data.get("tagIds")),
                    data.get("comment", ""),
                    0,
                    display_order,
                    1 if data.get("hidden") else 0,
                    int(data.get("utcOffset") or 0),
                    now,
                    now,
                ),
            )

        await conn.commit()
        return cursor.lastrowid

    @log_method
    async def update_template(
        self, template_id: int, data: dict[str, Any], user_id: int = 1, template_type: int | None = None
    ) -> bool:
        """更新模板。"""
        if not data:
            return False

        conn = await self._get_connection()
        normalized = self._build_template_update_payload(data, template_type)
        normalized["updated_at"] = datetime.now().isoformat()

        table_name = self._get_template_table(template_type)
        set_clause = ", ".join(f"{key} = ?" for key in normalized.keys())
        values = list(normalized.values())
        values.extend([template_id, user_id])

        cursor = await conn.execute(f"UPDATE {table_name} SET {set_clause} WHERE id = ? AND user_id = ?", values)
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def delete_template(self, template_id: int, user_id: int = 1, template_type: int | None = None) -> bool:
        """删除模板。"""
        conn = await self._get_connection()

        table_name = self._get_template_table(template_type)
        cursor = await conn.execute(f"DELETE FROM {table_name} WHERE id = ? AND user_id = ?", (template_id, user_id))
        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def update_template_display_orders(self, orders: list[tuple], template_type: int, user_id: int = 1) -> bool:
        """批量更新模板显示顺序。"""
        if not orders:
            return True

        conn = await self._get_connection()
        table_name = self._get_template_table(template_type)
        now = datetime.now().isoformat()

        try:
            for template_id, display_order in orders:
                await conn.execute(
                    f"UPDATE {table_name} SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                    (display_order, now, template_id, user_id),
                )

            await conn.commit()
            return True
        except Exception as exc:
            self.logger.error("更新模板显示顺序失败: %s", exc, exc_info=True)
            await conn.rollback()
            return False

    @log_method
    async def get_recurring_candidates_for_bill(
        self, bill_id: int, user_id: int = 1, tolerance_days: int = 3
    ) -> dict[str, Any]:
        """获取账单可匹配的定时交易候选。"""
        conn = await self._get_connection()
        bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not bill:
            return {"bill": None, "linked_recurring_id": None, "linked_recurring_name": "", "candidates": []}

        linked_recurring_id = bill.get("created_from_recurring")
        linked_recurring_name = ""
        if linked_recurring_id:
            async with conn.execute(
                "SELECT name FROM recurring_bills WHERE id = ? AND user_id = ?", (linked_recurring_id, user_id)
            ) as cursor:
                row = await cursor.fetchone()
                if row:
                    linked_recurring_name = str(row["name"] or "")

        recurring_rows = await self.get_enabled_recurring_templates(user_id=user_id)
        candidates = self.build_recurring_candidates_for_bill_data(
            bill, recurring_rows, linked_recurring_id=linked_recurring_id, tolerance_days=tolerance_days
        )

        return {
            "bill": bill,
            "linked_recurring_id": linked_recurring_id,
            "linked_recurring_name": linked_recurring_name,
            "candidates": candidates,
        }

    @log_method
    async def get_enabled_recurring_templates(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取当前用户启用中的定时交易模板。"""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT * FROM recurring_bills
            WHERE user_id = ? AND enabled = 1
            ORDER BY COALESCE(display_order, 0), name
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
        return [dict(row) for row in rows]

    def build_recurring_candidates_for_bill_data(
        self,
        bill: dict[str, Any],
        recurring_rows: list[dict[str, Any]],
        linked_recurring_id: Any = None,
        tolerance_days: int = 3,
    ) -> list[dict[str, Any]]:
        """基于账单数据构建定时账单候选列表。"""
        bill_date = self._parse_date_value(bill.get("date"))
        if not bill_date:
            return []

        bill_type = self._normalize_template_transaction_type(bill.get("type"))
        bill_amount_cents = int(round(abs(float(bill.get("amount") or 0)) * 100))
        bill_source_account = str(bill.get("source_account_id") or "0")
        bill_destination_account = str(bill.get("destination_account_id") or "0")

        candidates: list[dict[str, Any]] = []
        for recurring in recurring_rows:
            recurring_type = self._normalize_template_transaction_type(recurring.get("type"))
            if recurring_type != bill_type:
                continue

            recurring_amount_cents = int(round(abs(float(recurring.get("amount") or 0))))
            if recurring_amount_cents != bill_amount_cents:
                continue

            matched_occurrence = self._find_recurring_occurrence_near_date(
                recurring, bill_date, tolerance_days=tolerance_days
            )
            if not matched_occurrence:
                continue

            score = 80
            reasons: list[str] = ["type", "amount", "schedule"]
            recurring_source_account = str(recurring.get("account") or "0")
            recurring_destination_account = str(recurring.get("counterparty") or "0")

            if recurring_source_account == bill_source_account:
                reasons.append("source_account")
                score += 10
            if bill_destination_account not in ("", "0") and recurring_destination_account == bill_destination_account:
                reasons.append("destination_account")
                score += 10

            days_offset = abs((matched_occurrence - bill_date).days)
            score += max(0, 10 - days_offset * 2)

            candidate = self._serialize_template_row(recurring, template_type=2)
            candidate.update(
                {
                    "matchScore": score,
                    "matchReasons": reasons,
                    "matchedOccurrenceDate": matched_occurrence.isoformat(),
                    "matchedDayOffset": days_offset,
                    "linked": int(linked_recurring_id or 0) == int(recurring.get("id") or 0),
                }
            )
            candidates.append(candidate)

        candidates.sort(
            key=lambda item: (
                -int(item.get("matchScore") or 0),
                int(item.get("matchedDayOffset") or 999),
                str(item.get("name") or ""),
            )
        )
        return candidates

    @log_method
    async def bind_bill_to_recurring(self, bill_id: int, recurring_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """将账单绑定到定时交易，并推进 next_date。"""
        conn = await self._get_connection()
        bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not bill:
            return None

        previous_recurring_id = bill.get("created_from_recurring")

        async with conn.execute(
            "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?", (recurring_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()
            recurring = dict(row) if row else None

        if not recurring:
            return None

        bill_date = self._parse_date_value(bill.get("date"))
        next_occurrence = None
        if bill_date:
            next_date = self._get_next_recurring_occurrence_after(recurring, bill_date)
            next_occurrence = next_date.isoformat() if next_date else None

        now = datetime.now().isoformat()
        await conn.execute(
            "UPDATE bills SET created_from_recurring = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (recurring_id, now, bill_id, user_id),
        )
        await conn.execute(
            "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (next_occurrence or recurring.get("next_date"), now, recurring_id, user_id),
        )

        if previous_recurring_id and str(previous_recurring_id) != str(recurring_id):
            await self._recalculate_recurring_next_date(conn, int(previous_recurring_id), user_id, now)

        await conn.commit()

        return {
            "billId": bill_id,
            "recurringId": recurring_id,
            "nextScheduledDate": next_occurrence or recurring.get("next_date"),
        }

    async def _recalculate_recurring_next_date(
        self, conn, recurring_id: int, user_id: int, now: str | None = None
    ) -> None:
        """根据当前已绑定账单重算定时模板的下一次计划日期。"""
        async with conn.execute(
            "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?", (recurring_id, user_id)
        ) as recurring_cursor:
            recurring_row = await recurring_cursor.fetchone()

        recurring = dict(recurring_row) if recurring_row else None
        if not recurring:
            return

        async with conn.execute(
            """
            SELECT date FROM bills
            WHERE user_id = ? AND created_from_recurring = ?
            ORDER BY date DESC
            LIMIT 1
            """,
            (user_id, recurring_id),
        ) as linked_cursor:
            latest_linked_row = await linked_cursor.fetchone()

        latest_linked_date = self._parse_date_value(latest_linked_row["date"] if latest_linked_row else None)
        if latest_linked_date:
            next_date = self._get_next_recurring_occurrence_after(recurring, latest_linked_date)
        else:
            next_date = self._get_first_recurring_occurrence(recurring)

        next_occurrence = next_date.isoformat() if next_date else recurring.get("next_date")
        await conn.execute(
            "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (next_occurrence, now or datetime.now().isoformat(), recurring_id, user_id),
        )

    @log_method
    async def unbind_bill_from_recurring(self, bill_id: int, user_id: int = 1) -> bool:
        """取消账单与定时交易的绑定。"""
        conn = await self._get_connection()
        bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not bill:
            return False

        recurring_id = bill.get("created_from_recurring")
        now = datetime.now().isoformat()
        cursor = await conn.execute(
            "UPDATE bills SET created_from_recurring = NULL, updated_at = ? WHERE id = ? AND user_id = ?",
            (now, bill_id, user_id),
        )

        if recurring_id:
            await self._recalculate_recurring_next_date(conn, int(recurring_id), user_id, now)

        await conn.commit()
        return cursor.rowcount > 0

    def _get_template_table(self, template_type: int | None) -> str:
        """根据模板类型返回表名。"""
        return "recurring_bills" if int(template_type or 1) == 2 else "bill_templates"

    async def _get_next_template_display_order(self, conn, template_type: int, user_id: int) -> int:
        """获取下一显示顺序。"""
        table_name = self._get_template_table(template_type)
        async with conn.execute(
            f"SELECT COALESCE(MAX(display_order), 0) FROM {table_name} WHERE user_id = ?", (user_id,)
        ) as cursor:
            row = await cursor.fetchone()
            max_order = row[0] if row and row[0] is not None else 0
            return int(max_order) + 1

    def _serialize_template_tag_ids(self, tag_ids: Any) -> str:
        """序列化模板标签 ID 列表。"""
        if isinstance(tag_ids, list):
            return ",".join(str(tag_id) for tag_id in tag_ids if str(tag_id).strip())
        return str(tag_ids or "")

    def _deserialize_template_tag_ids(self, raw_value: Any) -> list[str]:
        """反序列化模板标签 ID 列表。"""
        if not raw_value:
            return []
        if isinstance(raw_value, list):
            return [str(tag_id) for tag_id in raw_value if str(tag_id).strip()]
        return [item.strip() for item in str(raw_value).split(",") if item.strip()]

    def _serialize_template_row(self, row: dict[str, Any], template_type: int) -> dict[str, Any]:
        """将模板表记录统一转换为前端模板 DTO。"""
        source_amount = float(row.get("amount") or 0)
        destination_amount = float(row.get("destination_amount") or 0)
        source_account_id = str(row.get("account") or "0")
        destination_account_id = str(row.get("counterparty") or "0")

        return {
            "id": str(row.get("id")),
            "timeSequenceId": "",
            "templateType": template_type,
            "name": row.get("name", ""),
            "type": self._normalize_template_transaction_type(row.get("type")),
            "categoryId": str(row.get("category") or ""),
            "time": int(row.get("scheduled_at") or 0),
            "utcOffset": int(row.get("utc_offset") or 0),
            "sourceAccountId": source_account_id,
            "destinationAccountId": destination_account_id,
            "sourceAmount": source_amount,
            "destinationAmount": destination_amount,
            "hideAmount": bool(row.get("hide_amount")),
            "tagIds": self._deserialize_template_tag_ids(row.get("tag")),
            "comment": row.get("comment", "") or "",
            "editable": True,
            "displayOrder": int(row.get("display_order") or 0),
            "hidden": bool(row.get("hidden")),
            "scheduledFrequencyType": int(row.get("scheduled_frequency_type") or 0) if template_type == 2 else None,
            "scheduledFrequency": row.get("frequency") if template_type == 2 else None,
            "scheduledStartDate": row.get("start_date") if template_type == 2 else None,
            "scheduledEndDate": row.get("end_date") if template_type == 2 else None,
            "scheduledAt": None,
        }

    def _normalize_template_transaction_type(self, raw_value: Any) -> int:
        """将历史模板类型值统一映射到前端数字枚举。"""
        mapping = {
            "2": 2,
            "3": 3,
            "4": 4,
            "5": 5,
            "income": 2,
            "expense": 3,
            "transfer": 4,
            "investment": 5,
            "收入": 2,
            "支出": 3,
            "转账": 4,
            "投资": 5,
        }

        if raw_value is None:
            return 3

        textual = str(raw_value).strip().lower()
        return mapping.get(textual, 3)

    def _parse_date_value(self, raw_value: Any) -> date | None:
        """解析 YYYY-MM-DD 或 YYYY-MM-DD HH:MM:SS 格式日期。"""
        if not raw_value:
            return None

        text = str(raw_value).strip()
        if not text:
            return None

        try:
            return datetime.fromisoformat(text[:19]).date()
        except ValueError:
            pass

        try:
            return datetime.strptime(text[:10], "%Y-%m-%d").date()
        except ValueError:
            return None

    def _parse_schedule_frequency_values(self, raw_value: Any) -> list[int]:
        """解析定时频率值，例如 '1,15'。"""
        if not raw_value:
            return []

        values: list[int] = []
        for item in str(raw_value).split(","):
            item = item.strip()
            if not item:
                continue
            try:
                values.append(int(item))
            except ValueError:
                continue

        return sorted(set(values))

    def _weekday_sunday_first(self, target_date: date) -> int:
        """将 Python weekday(Monday=0) 转为 Sunday=0。"""
        return (target_date.weekday() + 1) % 7

    def _is_recurring_active_on_date(self, recurring: dict[str, Any], target_date: date) -> bool:
        """判断定时模板在指定日期是否生效。"""
        start_date = self._parse_date_value(recurring.get("start_date"))
        end_date = self._parse_date_value(recurring.get("end_date"))

        if start_date and target_date < start_date:
            return False
        if end_date and target_date > end_date:
            return False
        return True

    def _is_recurring_due_on_date(self, recurring: dict[str, Any], target_date: date) -> bool:
        """判断定时模板是否在某天应发生。"""
        if not self._is_recurring_active_on_date(recurring, target_date):
            return False

        frequency_type = int(recurring.get("scheduled_frequency_type") or 0)
        frequency_values = self._parse_schedule_frequency_values(recurring.get("frequency"))
        start_date = self._parse_date_value(recurring.get("start_date"))
        next_date = self._parse_date_value(recurring.get("next_date"))

        if frequency_type == 1:
            if frequency_values:
                valid_weekdays = frequency_values
            elif start_date:
                valid_weekdays = [self._weekday_sunday_first(start_date)]
            else:
                valid_weekdays = []
            return self._weekday_sunday_first(target_date) in valid_weekdays

        if frequency_type == 2:
            if frequency_values:
                valid_days = frequency_values
            elif start_date:
                valid_days = [start_date.day]
            else:
                valid_days = []
            return target_date.day in valid_days

        if next_date:
            return target_date == next_date
        return bool(start_date and target_date == start_date)

    def _find_recurring_occurrence_near_date(
        self, recurring: dict[str, Any], target_date: date, tolerance_days: int
    ) -> date | None:
        """在容差窗口内寻找最近的计划发生日期。"""
        nearest_date: date | None = None
        nearest_diff: int | None = None

        for offset in range(-tolerance_days, tolerance_days + 1):
            current_date = target_date + timedelta(days=offset)
            if not self._is_recurring_due_on_date(recurring, current_date):
                continue

            diff = abs(offset)
            if nearest_date is None or (nearest_diff is not None and diff < nearest_diff):
                nearest_date = current_date
                nearest_diff = diff

        return nearest_date

    def _get_next_recurring_occurrence_after(
        self, recurring: dict[str, Any], after_date: date, max_search_days: int = 370
    ) -> date | None:
        """获取指定日期后的下一次计划发生日期。"""
        for offset in range(1, max_search_days + 1):
            candidate = after_date + timedelta(days=offset)
            if self._is_recurring_due_on_date(recurring, candidate):
                return candidate
        return None

    def _build_template_update_payload(self, data: dict[str, Any], template_type: int | None) -> dict[str, Any]:
        """构建模板更新字段。"""
        normalized: dict[str, Any] = {}

        field_mapping = {
            "name": "name",
            "type": "type",
            "categoryId": "category",
            "sourceAccountId": "account",
            "destinationAccountId": "counterparty",
            "sourceAmount": "amount",
            "destinationAmount": "destination_amount",
            "hideAmount": "hide_amount",
            "comment": "comment",
            "hidden": "hidden",
            "displayOrder": "display_order",
            "utcOffset": "utc_offset",
        }

        for source_key, target_key in field_mapping.items():
            if source_key in data:
                value = data[source_key]
                if source_key in ("hideAmount", "hidden"):
                    normalized[target_key] = 1 if value else 0
                elif source_key in ("sourceAmount", "destinationAmount"):
                    normalized[target_key] = float(value or 0)
                elif source_key in ("displayOrder", "utcOffset"):
                    normalized[target_key] = int(value or 0)
                else:
                    normalized[target_key] = value

        if "tagIds" in data:
            normalized["tag"] = self._serialize_template_tag_ids(data.get("tagIds"))

        if int(template_type or 1) == 2:
            recurring_mapping = {
                "scheduledFrequencyType": "scheduled_frequency_type",
                "scheduledFrequency": "frequency",
                "scheduledStartDate": "start_date",
                "scheduledEndDate": "end_date",
            }
            for source_key, target_key in recurring_mapping.items():
                if source_key in data:
                    value = data[source_key]
                    if source_key == "scheduledFrequencyType":
                        normalized[target_key] = int(value or 0)
                    else:
                        normalized[target_key] = value

            if "scheduledStartDate" in data:
                normalized["next_date"] = data.get("scheduledStartDate")

        return normalized

    # ==================== 用户认证管理方法 ====================

    @log_method
    async def create_user(self, data: dict[str, Any]) -> int:
        """
        创建用户

        Args:
            data: 用户数据，必须包含 username, email, password_hash

        Returns:
            int: 新创建的用户ID
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute(
            """
            INSERT INTO users (
                username, email, password_hash, nickname, avatar,
                language, default_currency, first_day_of_week,
                is_active, email_verified, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
            (
                data.get("username"),
                data.get("email"),
                data.get("password_hash"),
                data.get("nickname", data.get("username")),
                data.get("avatar", ""),
                data.get("language", "zh_Hans"),
                data.get("default_currency", "CNY"),
                data.get("first_day_of_week", 1),
                data.get("is_active", 1),
                data.get("email_verified", 0),
                now,
                now,
            ),
        )

        await conn.commit()
        user_id = cursor.lastrowid

        self.logger.info(f"创建用户成功: ID={user_id}, username={data.get('username')}")
        return user_id

    @log_method
    async def get_user_by_username(self, username: str) -> dict[str, Any] | None:
        """根据用户名获取用户"""
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM users WHERE username = ?", (username,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_by_email(self, email: str) -> dict[str, Any] | None:
        """根据邮箱获取用户"""
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM users WHERE email = ?", (email,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_by_id(self, user_id: int) -> dict[str, Any] | None:
        """根据ID获取用户"""
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def update_user(self, user_id: int, data: dict[str, Any]) -> bool:
        """更新用户信息"""
        if not data:
            return False

        conn = await self._get_connection()
        data["updated_at"] = datetime.now().isoformat()

        set_clause = ", ".join(f"{key} = ?" for key in data.keys())
        values = list(data.values())
        values.append(user_id)

        cursor = await conn.execute(f"UPDATE users SET {set_clause} WHERE id = ?", values)
        await conn.commit()

        self.logger.info(f"更新用户成功: ID={user_id}")
        return cursor.rowcount > 0

    @log_method
    async def create_user_external_auth(self, data: dict[str, Any]) -> int:
        """创建或更新用户第三方登录绑定。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute(
            """
            INSERT INTO user_external_auths (
                user_id, external_auth_category, external_auth_type,
                external_user_id, external_username, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, external_auth_type) DO UPDATE SET
                external_auth_category = excluded.external_auth_category,
                external_user_id = excluded.external_user_id,
                external_username = excluded.external_username,
                updated_at = excluded.updated_at
            """,
            (
                data.get("user_id"),
                data.get("external_auth_category"),
                data.get("external_auth_type"),
                data.get("external_user_id"),
                data.get("external_username"),
                now,
                now,
            ),
        )
        await conn.commit()

        self.logger.info(
            "创建或更新用户第三方登录绑定: user_id=%s, type=%s", data.get("user_id"), data.get("external_auth_type")
        )
        return cursor.lastrowid or 0

    @log_method
    async def get_user_external_auths(self, user_id: int) -> list[dict[str, Any]]:
        """获取用户第三方登录绑定列表。"""
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT * FROM user_external_auths
            WHERE user_id = ?
            ORDER BY created_at DESC, id DESC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_user_external_auth(self, user_id: int, external_auth_type: str) -> dict[str, Any] | None:
        """获取单个用户第三方登录绑定。"""
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT * FROM user_external_auths
            WHERE user_id = ? AND external_auth_type = ?
            LIMIT 1
            """,
            (user_id, external_auth_type),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def delete_user_external_auth(self, user_id: int, external_auth_type: str) -> bool:
        """删除用户第三方登录绑定。"""
        conn = await self._get_connection()

        cursor = await conn.execute(
            "DELETE FROM user_external_auths WHERE user_id = ? AND external_auth_type = ?",
            (user_id, external_auth_type),
        )
        await conn.commit()

        success = cursor.rowcount > 0
        self.logger.info(
            "删除用户第三方登录绑定: user_id=%s, type=%s, success=%s", user_id, external_auth_type, success
        )
        return success

    @log_method
    async def get_user_application_cloud_settings(self, user_id: int) -> list[dict[str, Any]]:
        """获取用户应用云同步设置。"""
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT setting_key, setting_value, created_at, updated_at
            FROM user_application_cloud_settings
            WHERE user_id = ?
            ORDER BY created_at ASC, id ASC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def update_user_application_cloud_settings(
        self, user_id: int, settings: list[dict[str, Any]], full_update: bool = False
    ) -> bool:
        """创建或更新用户应用云同步设置。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()
        normalized_settings = [
            {
                "setting_key": str(setting.get("setting_key", "") or "").strip(),
                "setting_value": str(setting.get("setting_value", "") or ""),
            }
            for setting in settings
            if str(setting.get("setting_key", "") or "").strip()
        ]

        if full_update:
            if normalized_settings:
                keep_keys = [setting["setting_key"] for setting in normalized_settings]
                placeholders = ",".join("?" for _ in keep_keys)
                await conn.execute(
                    f"DELETE FROM user_application_cloud_settings WHERE user_id = ? AND setting_key NOT IN ({placeholders})",
                    [user_id, *keep_keys],
                )
            else:
                await conn.execute("DELETE FROM user_application_cloud_settings WHERE user_id = ?", (user_id,))

        for setting in normalized_settings:
            await conn.execute(
                """
                INSERT INTO user_application_cloud_settings (
                    user_id, setting_key, setting_value, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(user_id, setting_key) DO UPDATE SET
                    setting_value = excluded.setting_value,
                    updated_at = excluded.updated_at
                """,
                (
                    user_id,
                    setting["setting_key"],
                    setting["setting_value"],
                    now,
                    now,
                ),
            )

        await conn.commit()
        self.logger.info(
            "更新用户应用云同步设置: user_id=%s, count=%s, full_update=%s",
            user_id,
            len(normalized_settings),
            full_update,
        )
        return True

    @log_method
    async def delete_user_application_cloud_settings(self, user_id: int) -> bool:
        """禁用用户应用云同步设置。"""
        conn = await self._get_connection()
        cursor = await conn.execute("DELETE FROM user_application_cloud_settings WHERE user_id = ?", (user_id,))
        await conn.commit()

        self.logger.info("删除用户应用云同步设置: user_id=%s, deleted=%s", user_id, cursor.rowcount)
        return True

    @log_method
    async def update_user_last_login(self, user_id: int, ip_address: str = None):
        """更新用户最后登录时间"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        await conn.execute(
            """
            UPDATE users
            SET last_login_at = ?, last_login_ip = ?, failed_login_attempts = 0, locked_until = NULL
            WHERE id = ?
        """,
            (now, ip_address, user_id),
        )

        await conn.commit()
        self.logger.info(f"更新用户最后登录时间: ID={user_id}, IP={ip_address}")

    @log_method
    async def increment_failed_login(self, user_id: int, lockout_minutes: int = 15):
        """增加失败登录次数，超过阈值则锁定账户"""
        conn = await self._get_connection()

        # 获取当前失败次数
        async with conn.execute("SELECT failed_login_attempts FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
            if not row:
                return False

            failed_attempts = row[0] + 1

            # 如果失败次数达到5次，锁定账户
            if failed_attempts >= 5:
                locked_until = (datetime.now() + timedelta(minutes=lockout_minutes)).isoformat()
                await conn.execute(
                    """
                    UPDATE users
                    SET failed_login_attempts = ?, locked_until = ?
                    WHERE id = ?
                """,
                    (failed_attempts, locked_until, user_id),
                )
                self.logger.warning(f"用户账户已锁定: ID={user_id}, 锁定至={locked_until}")
            else:
                await conn.execute(
                    """
                    UPDATE users
                    SET failed_login_attempts = ?
                    WHERE id = ?
                """,
                    (failed_attempts, user_id),
                )

            await conn.commit()
            return True

    @log_method
    async def is_user_locked(self, user_id: int) -> bool:
        """检查用户是否被锁定"""
        conn = await self._get_connection()

        async with conn.execute("SELECT locked_until FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
            if not row or not row[0]:
                return False

            locked_until = datetime.fromisoformat(row[0])
            if datetime.now() < locked_until:
                return True

            # 锁定时间已过，清除锁定状态
            await conn.execute(
                """
                UPDATE users
                SET locked_until = NULL, failed_login_attempts = 0
                WHERE id = ?
            """,
                (user_id,),
            )
            await conn.commit()

            return False

    @log_method
    async def create_session(self, data: dict[str, Any]) -> int:
        """创建会话"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute(
            """
            INSERT INTO sessions (
                user_id, token_hash, refresh_token_hash,
                expires_at, refresh_expires_at,
                user_agent, ip_address, is_active,
                last_activity_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
            (
                data.get("user_id"),
                data.get("token_hash"),
                data.get("refresh_token_hash"),
                data.get("expires_at"),
                data.get("refresh_expires_at"),
                data.get("user_agent"),
                data.get("ip_address"),
                1,  # is_active
                now,  # last_activity_at
                now,  # created_at
            ),
        )

        await conn.commit()
        session_id = cursor.lastrowid

        self.logger.info(f"创建会话成功: ID={session_id}, user_id={data.get('user_id')}")
        return session_id

    @log_method
    async def get_session_by_token_hash(self, token_hash: str) -> dict[str, Any] | None:
        """根据token哈希获取会话"""
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT s.*, u.username, u.email, u.is_active as user_is_active
            FROM sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.token_hash = ? AND s.is_active = 1
        """,
            (token_hash,),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    @log_method
    async def get_user_sessions(self, user_id: int) -> list[dict[str, Any]]:
        """获取用户所有活跃会话"""
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT * FROM sessions
            WHERE user_id = ? AND is_active = 1
            ORDER BY created_at DESC
        """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def update_session_activity(self, session_id: int):
        """更新会话活动时间"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        await conn.execute(
            """
            UPDATE sessions
            SET last_activity_at = ?
            WHERE id = ?
        """,
            (now, session_id),
        )

        await conn.commit()

    @log_method
    async def invalidate_session(self, token_hash: str) -> bool:
        """使会话失效"""
        conn = await self._get_connection()

        cursor = await conn.execute(
            """
            UPDATE sessions
            SET is_active = 0
            WHERE token_hash = ?
        """,
            (token_hash,),
        )

        await conn.commit()
        self.logger.info(f"会话已失效: token_hash={token_hash[:16]}...")
        return cursor.rowcount > 0

    @log_method
    async def invalidate_user_sessions(self, user_id: int) -> int:
        """使用户的所有会话失效"""
        conn = await self._get_connection()

        cursor = await conn.execute(
            """
            UPDATE sessions
            SET is_active = 0
            WHERE user_id = ?
        """,
            (user_id,),
        )

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(f"用户所有会话已失效: user_id={user_id}, 数量={count}")
        return count

    @log_method
    async def invalidate_session_by_id(self, session_id: int, user_id: int) -> bool:
        """按会话ID使单个用户会话失效。"""
        conn = await self._get_connection()

        cursor = await conn.execute(
            """
            UPDATE sessions
            SET is_active = 0
            WHERE id = ? AND user_id = ?
        """,
            (session_id, user_id),
        )

        await conn.commit()
        success = cursor.rowcount > 0
        self.logger.info(
            "按ID使会话失效: session_id=%s, user_id=%s, success=%s",
            session_id,
            user_id,
            success,
        )
        return success

    @log_method
    async def invalidate_other_user_sessions(self, user_id: int, current_session_id: int) -> int:
        """使用户除当前会话外的所有会话失效。"""
        conn = await self._get_connection()

        cursor = await conn.execute(
            """
            UPDATE sessions
            SET is_active = 0
            WHERE user_id = ? AND id != ? AND is_active = 1
        """,
            (user_id, current_session_id),
        )

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(
            "用户其他会话已失效: user_id=%s, current_session_id=%s, 数量=%s",
            user_id,
            current_session_id,
            count,
        )
        return count

    @log_method
    async def cleanup_expired_sessions(self) -> int:
        """清理过期会话"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        cursor = await conn.execute(
            """
            DELETE FROM sessions
            WHERE expires_at < ? OR
                  (refresh_expires_at IS NOT NULL AND refresh_expires_at < ?)
        """,
            (now, now),
        )

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(f"清理过期会话: 数量={count}")
        return count

    @log_method
    async def create_auth_log(self, data: dict[str, Any]):
        """创建认证日志"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        await conn.execute(
            """
            INSERT INTO auth_logs (
                user_id, username, event_type, ip_address, user_agent,
                success, error_message, metadata, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
            (
                data.get("user_id"),
                data.get("username"),
                data.get("event_type"),
                data.get("ip_address"),
                data.get("user_agent"),
                data.get("success", False),
                data.get("error_message"),
                data.get("metadata"),
                now,
            ),
        )

        await conn.commit()

    @log_method
    async def replace_two_factor_recovery_codes(self, user_id: int, recovery_codes: list[str]) -> int:
        """替换用户当前有效的 2FA 恢复码（仅保存哈希）。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        try:
            seen_hashes: set[str] = set()
            hashed_payloads: list[tuple[int, str, str, str]] = []
            for recovery_code in recovery_codes:
                code_hash = self._hash_two_factor_recovery_code(recovery_code)
                if not code_hash or code_hash in seen_hashes:
                    continue

                seen_hashes.add(code_hash)
                hashed_payloads.append((user_id, code_hash, now, now))

            await conn.execute("DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?", (user_id,))

            if hashed_payloads:
                await conn.executemany(
                    """
                    INSERT INTO user_two_factor_recovery_codes (
                        user_id, code_hash, created_at, updated_at
                    ) VALUES (?, ?, ?, ?)
                    """,
                    hashed_payloads,
                )

            await conn.commit()
            self.logger.info("替换用户 2FA 恢复码: user_id=%s, count=%s", user_id, len(hashed_payloads))
            return len(hashed_payloads)
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def consume_two_factor_recovery_code(self, user_id: int, recovery_code: str) -> bool:
        """一次性消费用户恢复码；已使用或不存在时返回 False。"""
        code_hash = self._hash_two_factor_recovery_code(recovery_code)
        if not code_hash:
            return False

        conn = await self._get_connection()
        now = datetime.now().isoformat()
        cursor = await conn.execute(
            """
            UPDATE user_two_factor_recovery_codes
            SET used_at = ?, updated_at = ?
            WHERE user_id = ? AND code_hash = ? AND used_at IS NULL
            """,
            (now, now, user_id, code_hash),
        )
        await conn.commit()

        success = cursor.rowcount > 0
        self.logger.info("消费用户 2FA 恢复码: user_id=%s, success=%s", user_id, success)
        return success

    @log_method
    async def clear_two_factor_recovery_codes(self, user_id: int) -> int:
        """清空用户全部恢复码。"""
        conn = await self._get_connection()
        cursor = await conn.execute("DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?", (user_id,))
        await conn.commit()

        cleared_count = cursor.rowcount
        self.logger.info("清空用户 2FA 恢复码: user_id=%s, count=%s", user_id, cleared_count)
        return cleared_count

    @log_method
    async def count_active_two_factor_recovery_codes(self, user_id: int) -> int:
        """返回用户当前未使用的恢复码数量。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT COUNT(*) AS count FROM user_two_factor_recovery_codes WHERE user_id = ? AND used_at IS NULL",
            (user_id,),
        ) as cursor:
            row = await cursor.fetchone()
            return int((row["count"] if row else 0) or 0)

    @log_method
    async def get_auth_logs(
        self, user_id: int = None, event_type: str = None, limit: int = 100
    ) -> list[dict[str, Any]]:
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

        cursor = await conn.execute(
            """
            DELETE FROM auth_logs
            WHERE created_at < ?
        """,
            (cutoff_date,),
        )

        await conn.commit()
        count = cursor.rowcount
        self.logger.info(f"清理旧认证日志: 数量={count}, 保留天数={days}")
        return count

    # === 预算管理方法 ===
    @log_method
    async def get_budgets(self, filters: dict[str, Any] = None, user_id: int = 1) -> list[dict[str, Any]]:
        """获取预算列表

        Args:
            filters: 筛选条件
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        query = "SELECT * FROM budgets WHERE user_id = ?"
        params = [user_id]

        if filters:
            if "period_type" in filters:
                query += " AND period_type = ?"
                params.append(filters["period_type"])

            if "enabled" in filters:
                query += " AND enabled = ?"
                params.append(1 if filters["enabled"] else 0)

            if "category" in filters:
                query += " AND category = ?"
                params.append(filters["category"])

        query += " ORDER BY created_at DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_budget_by_id(self, budget_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """根据ID获取预算

        Args:
            budget_id: 预算ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM budgets WHERE id = ? AND user_id = ?", (budget_id, user_id)) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    def _build_budget_category_context(self, categories: list[dict[str, Any]]) -> dict[str, Any]:
        """构建预算分类类型/图标解析上下文。"""
        primary_by_key: dict[tuple[int, str], dict[str, Any]] = {}
        sub_by_key: dict[tuple[int, str, str], dict[str, Any]] = {}
        fallback_by_key: dict[tuple[int, str], dict[str, Any]] = {}
        types_by_name: dict[str, set[int]] = {}

        for category in categories:
            main_category = str(category.get("main_category") or "").strip()
            if not main_category:
                continue

            category_type = int(category.get("type") or 0)
            if category_type <= 0:
                continue

            normalized_sub_category = self._normalize_budget_sub_category(category.get("sub_category"))
            types_by_name.setdefault(main_category, set()).add(category_type)

            if not normalized_sub_category:
                primary_by_key[(category_type, main_category)] = category
                continue

            fallback_key = (category_type, main_category)
            sub_by_key[(category_type, main_category, normalized_sub_category)] = category

            existing_fallback = fallback_by_key.get(fallback_key)
            if existing_fallback is None:
                fallback_by_key[fallback_key] = category
                continue

            if not existing_fallback.get("icon") and category.get("icon"):
                fallback_by_key[fallback_key] = category

        return {
            "primary_by_key": primary_by_key,
            "sub_by_key": sub_by_key,
            "fallback_by_key": fallback_by_key,
            "types_by_name": {name: tuple(sorted(values)) for name, values in types_by_name.items()},
        }

    def _resolve_budget_category_type(
        self,
        category_name: str | None,
        sub_category: Any,
        category_context: dict[str, Any],
        preferred_type: int | None = None,
    ) -> int | None:
        """根据预算分类名称推导预算类型。"""
        normalized_category_name = str(category_name or "").strip()
        if not normalized_category_name:
            return None

        normalized_sub_category = self._normalize_budget_sub_category(sub_category)
        primary_by_key = category_context["primary_by_key"]
        sub_by_key = category_context["sub_by_key"]
        fallback_by_key = category_context["fallback_by_key"]
        candidate_types = list(category_context["types_by_name"].get(normalized_category_name, ()))

        if preferred_type is not None and preferred_type in candidate_types:
            if normalized_sub_category:
                if (preferred_type, normalized_category_name, normalized_sub_category) in sub_by_key:
                    return preferred_type
            elif (
                (preferred_type, normalized_category_name) in primary_by_key
                or (preferred_type, normalized_category_name) in fallback_by_key
            ):
                return preferred_type

        for candidate_type in candidate_types:
            if normalized_sub_category:
                if (candidate_type, normalized_category_name, normalized_sub_category) in sub_by_key:
                    return candidate_type
                continue

            if (candidate_type, normalized_category_name) in primary_by_key:
                return candidate_type
            if (candidate_type, normalized_category_name) in fallback_by_key:
                return candidate_type

        if not candidate_types and preferred_type is not None:
            return preferred_type

        return None

    def _resolve_budget_category_info(
        self,
        category_name: str | None,
        sub_category: Any,
        category_context: dict[str, Any],
        budget_type: int,
    ) -> dict[str, Any] | None:
        """根据预算类型解析当前预算对应的分类信息与图标。"""
        normalized_category_name = str(category_name or "").strip()
        if not normalized_category_name:
            return None

        normalized_sub_category = self._normalize_budget_sub_category(sub_category)
        primary_by_key = category_context["primary_by_key"]
        sub_by_key = category_context["sub_by_key"]
        fallback_by_key = category_context["fallback_by_key"]

        if normalized_sub_category:
            category_info = sub_by_key.get((budget_type, normalized_category_name, normalized_sub_category))
            return dict(category_info) if category_info else None

        primary_category = primary_by_key.get((budget_type, normalized_category_name))
        fallback_category = fallback_by_key.get((budget_type, normalized_category_name))

        if primary_category is None:
            return dict(fallback_category) if fallback_category else None

        if primary_category.get("icon") or fallback_category is None:
            return dict(primary_category)

        merged_category = dict(primary_category)
        merged_category["icon"] = fallback_category.get("icon", "")
        if not merged_category.get("color") and fallback_category.get("color"):
            merged_category["color"] = fallback_category["color"]
        return merged_category

    @staticmethod
    def _normalize_budget_sub_category(sub_category: Any) -> str:
        """统一一级/二级预算的子分类存储格式。"""
        if sub_category is None:
            return ""

        return str(sub_category).strip()

    async def _insert_budget_record(
        self,
        conn: aiosqlite.Connection,
        data: dict[str, Any],
        user_id: int,
    ) -> int:
        cursor = await conn.execute(
            """
            INSERT INTO budgets (
                name, category, sub_category, period_type, amount,
                start_date, end_date, alert_threshold, enabled,
                created_at, updated_at, user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
            (
                data.get("name", ""),
                data.get("category"),
                self._normalize_budget_sub_category(data.get("sub_category")),
                data["period_type"],
                data["amount"],
                data["start_date"],
                data.get("end_date"),
                data.get("alert_threshold", 80),
                data.get("enabled", 1),
                data["created_at"],
                data["updated_at"],
                user_id,
            ),
        )

        return int(cursor.lastrowid)

    async def _get_primary_category_budget_with_conn(
        self,
        conn: aiosqlite.Connection,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int,
    ) -> dict[str, Any] | None:
        async with conn.execute(
            """
            SELECT * FROM budgets
            WHERE category = ?
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
        """,
            (category, period_type, start_date, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def _get_sub_category_budgets_total_with_conn(
        self,
        conn: aiosqlite.Connection,
        category: str,
        period_type: str,
        start_date: str,
        user_id: int,
    ) -> float:
        async with conn.execute(
            """
            SELECT COALESCE(SUM(amount), 0) as total
            FROM budgets
            WHERE category = ?
              AND sub_category IS NOT NULL
              AND sub_category != ''
              AND period_type = ?
              AND start_date = ?
              AND user_id = ?
        """,
            (category, period_type, start_date, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            return float(row["total"] if row else 0)

    async def _synchronize_primary_budget_for_group(
        self,
        conn: aiosqlite.Connection,
        category: str | None,
        period_type: str | None,
        start_date: str | None,
        user_id: int,
        reference_data: dict[str, Any] | None = None,
    ) -> None:
        """确保一级预算遵循“总额不小于二级预算之和”的联动规则。"""
        if not category or not period_type or not start_date:
            return

        sub_total = await self._get_sub_category_budgets_total_with_conn(
            conn,
            category,
            period_type,
            start_date,
            user_id,
        )
        primary_budget = await self._get_primary_category_budget_with_conn(
            conn,
            category,
            period_type,
            start_date,
            user_id,
        )

        if sub_total <= 0:
            return

        now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        if not primary_budget:
            reference = reference_data or {}
            await self._insert_budget_record(
                conn,
                {
                    "name": reference.get("name", ""),
                    "category": category,
                    "sub_category": "",
                    "period_type": period_type,
                    "amount": sub_total,
                    "start_date": start_date,
                    "end_date": reference.get("end_date"),
                    "alert_threshold": reference.get("alert_threshold", 80),
                    "enabled": reference.get("enabled", 1),
                    "created_at": reference.get("created_at", now),
                    "updated_at": now,
                },
                user_id,
            )
            return

        current_amount = float(primary_budget.get("amount") or 0)
        if current_amount >= sub_total:
            return

        await conn.execute(
            "UPDATE budgets SET amount = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (sub_total, now, primary_budget["id"], user_id),
        )

    @log_method
    async def create_budget(self, data: dict[str, Any], user_id: int = 1) -> int:
        """创建预算

        Args:
            data: 预算数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        normalized_data = {
            **data,
            "name": data.get("name", ""),
            "sub_category": self._normalize_budget_sub_category(data.get("sub_category")),
        }

        budget_id = await self._insert_budget_record(conn, normalized_data, user_id)
        await self._synchronize_primary_budget_for_group(
            conn,
            normalized_data.get("category"),
            normalized_data.get("period_type"),
            normalized_data.get("start_date"),
            user_id,
            reference_data=normalized_data,
        )

        await conn.commit()
        return budget_id

    @log_method
    async def get_primary_category_budget(
        self, category: str, period_type: str, start_date: str, user_id: int = 1
    ) -> dict[str, Any] | None:
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

        return await self._get_primary_category_budget_with_conn(conn, category, period_type, start_date, user_id)

    @log_method
    async def get_budget_by_category(
        self, category: str, sub_category: str, period_type: str, start_date: str, user_id: int = 1
    ) -> dict[str, Any] | None:
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
            async with conn.execute(
                """
                SELECT * FROM budgets
                WHERE category = ?
                  AND sub_category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
            """,
                (category, sub_category, period_type, start_date, user_id),
            ) as cursor:
                row = await cursor.fetchone()
                return dict(row) if row else None
        else:
            # 查找一级分类预算
            async with conn.execute(
                """
                SELECT * FROM budgets
                WHERE category = ?
                  AND (sub_category IS NULL OR sub_category = '')
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
            """,
                (category, period_type, start_date, user_id),
            ) as cursor:
                row = await cursor.fetchone()
                return dict(row) if row else None

    @log_method
    async def get_sub_category_budgets_total(
        self, category: str, period_type: str, start_date: str, user_id: int = 1
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

        return await self._get_sub_category_budgets_total_with_conn(conn, category, period_type, start_date, user_id)

    @log_method
    async def update_budget(self, budget_id: int, data: dict[str, Any], user_id: int = 1) -> bool:
        """更新预算

        Args:
            budget_id: 预算ID
            data: 更新数据
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()
        existing_budget = await self.get_budget_by_id(budget_id, user_id=user_id)
        if not existing_budget:
            return False

        # 构建更新字段
        fields = []
        values = []
        normalized_data = {
            **data,
            "sub_category": self._normalize_budget_sub_category(data.get("sub_category", existing_budget.get("sub_category"))),
        }

        for key in [
            "name",
            "category",
            "sub_category",
            "period_type",
            "amount",
            "start_date",
            "end_date",
            "alert_threshold",
            "enabled",
        ]:
            if key in normalized_data:
                fields.append(f"{key} = ?")
                values.append(normalized_data[key])

        if "updated_at" in normalized_data:
            fields.append("updated_at = ?")
            values.append(normalized_data["updated_at"])

        if not fields:
            return False

        values.extend([budget_id, user_id])

        query = f"UPDATE budgets SET {', '.join(fields)} WHERE id = ? AND user_id = ?"
        cursor = await conn.execute(query, values)

        if cursor.rowcount > 0:
            updated_budget = {
                **existing_budget,
                **normalized_data,
            }
            group_keys = {
                (
                    existing_budget.get("category"),
                    existing_budget.get("period_type"),
                    existing_budget.get("start_date"),
                )
            }

            group_keys.add(
                (
                    updated_budget.get("category"),
                    updated_budget.get("period_type"),
                    updated_budget.get("start_date"),
                )
            )

            for category, period_type, start_date in group_keys:
                await self._synchronize_primary_budget_for_group(
                    conn,
                    category,
                    period_type,
                    start_date,
                    user_id,
                    reference_data=updated_budget or normalized_data,
                )

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

        budget = await self.get_budget_by_id(budget_id, user_id=user_id)
        if not budget:
            return False

        category = budget.get("category")
        period_type = budget.get("period_type")
        start_date = budget.get("start_date")
        is_primary_budget = not self._normalize_budget_sub_category(budget.get("sub_category"))

        if is_primary_budget:
            cursor = await conn.execute(
                """
                DELETE FROM budgets
                WHERE category = ?
                  AND period_type = ?
                  AND start_date = ?
                  AND user_id = ?
                """,
                (category, period_type, start_date, user_id),
            )
        else:
            cursor = await conn.execute("DELETE FROM budgets WHERE id = ? AND user_id = ?", (budget_id, user_id))
            await self._synchronize_primary_budget_for_group(
                conn,
                category,
                period_type,
                start_date,
                user_id,
                reference_data=budget,
            )

        await conn.commit()

        return cursor.rowcount > 0

    @log_method
    async def get_budget_execution_details(
        self,
        budget_type: int = 3,
        start_date: str = None,
        end_date: str = None,
        budget_id: int = None,
        category_id: int = None,
        account_ids: list[int] = None,
        tag_ids: list[int] = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """
        获取预算执行详情

        Args:
            budget_type: 预算类型 (3=支出, 5=投资)
            start_date: 开始日期 YYYY-MM-DD
            end_date: 结束日期 YYYY-MM-DD
            budget_id: 预算ID筛选
            category_id: 分类ID筛选
            account_ids: 账户ID列表筛选
            tag_ids: 标签ID列表筛选

        Returns:
            预算执行详情列表
        """
        self.logger.info(
            f"[get_budget_execution_details] 参数: type={budget_type}, "
            f"dates={start_date}~{end_date}, budget_id={budget_id}, category={category_id}, "
            f"accounts={account_ids}, tags={tag_ids}, user_id={user_id}"
        )

        conn = await self._get_connection()
        categories = await self.get_all_categories(user_id=user_id)
        category_context = self._build_budget_category_context(categories)

        # 1. 获取符合条件的预算 - 添加 user_id 过滤
        budget_query = "SELECT * FROM budgets WHERE enabled = 1 AND user_id = ?"
        budget_params = [user_id]

        if budget_type == 3:
            budget_query += " AND (period_type IS NOT NULL)"  # 支出预算
        elif budget_type == 5:
            budget_query += " AND (period_type IS NOT NULL)"  # 投资预算

        if budget_id:
            budget_query += " AND id = ?"
            budget_params.append(budget_id)

        if category_id:
            # 根据category_id获取分类名称
            cat_info = await self.get_category_by_id(category_id, user_id=user_id)
            if cat_info:
                budget_query += " AND category = ?"
                budget_params.append(cat_info["main_category"])
                normalized_sub_category = self._normalize_budget_sub_category(cat_info.get("sub_category"))
                if normalized_sub_category:
                    budget_query += " AND sub_category = ?"
                    budget_params.append(normalized_sub_category)

        async with conn.execute(budget_query, budget_params) as cursor:
            raw_budgets = [dict(row) for row in await cursor.fetchall()]

        budgets: list[dict[str, Any]] = []
        for budget in raw_budgets:
            resolved_budget_type = self._resolve_budget_category_type(
                budget.get("category"),
                budget.get("sub_category"),
                category_context,
                preferred_type=budget_type,
            )
            if resolved_budget_type != budget_type:
                continue

            budgets.append(
                {
                    **budget,
                    "_resolved_budget_type": resolved_budget_type,
                }
            )

        # 2. 对每个预算计算实际支出
        type_name = "支出" if budget_type == 3 else "投资"

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
            if budget.get("category"):
                bill_query += " AND main_category = ?"
                bill_params.append(budget["category"])

            if budget.get("sub_category"):
                bill_query += " AND sub_category = ?"
                bill_params.append(budget["sub_category"])

            # 日期筛选 - 使用预算自身的日期范围
            budget_start = start_date or budget.get("start_date")
            budget_end = end_date or budget.get("end_date")

            if budget_start:
                bill_query += " AND date >= ?"
                bill_params.append(budget_start)

            if budget_end:
                bill_query += " AND date <= ?"
                bill_params.append(budget_end)

            # 账户筛选
            if account_ids:
                placeholders = ",".join("?" * len(account_ids))
                bill_query += (
                    f" AND (source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
                )
                bill_params.extend(account_ids * 2)

            self.logger.debug(f"[get_budget_execution_details] 查询SQL: {bill_query}, 参数: {bill_params}")

            # 执行查询
            async with conn.execute(bill_query, bill_params) as cursor:
                row = await cursor.fetchone()
                spent = abs(row["spent"]) if row else 0

            self.logger.debug(
                f"[get_budget_execution_details] 预算 {budget.get('category')}/{budget.get('sub_category')} "
                f"查询结果: spent={spent}"
            )

            # 计算执行度
            budget_amount = budget.get("amount", 0)
            execution_rate = (spent / budget_amount * 100) if budget_amount > 0 else 0

            resolved_budget_type = int(budget.get("_resolved_budget_type") or budget_type)
            category_info = self._resolve_budget_category_info(
                budget.get("category"),
                budget.get("sub_category"),
                category_context,
                resolved_budget_type,
            )

            results.append(
                {
                    "id": budget["id"],
                    "name": budget["name"],
                    "category": budget.get("category", ""),
                    "sub_category": budget.get("sub_category", ""),
                    "category_info": category_info,
                    "category_id": str(category_info.get("id") or "") if category_info else "",
                    "period_type": budget.get("period_type", "monthly"),
                    "budget_amount": budget_amount,
                    "spent_amount": spent,
                    "remaining_amount": budget_amount - spent,
                    "execution_rate": round(execution_rate, 2),
                    "type": resolved_budget_type,
                    "alert_threshold": budget.get("alert_threshold", 80),
                    "start_date": budget.get("start_date"),
                    "end_date": budget.get("end_date"),
                    "enabled": budget.get("enabled", 1),
                }
            )

        self.logger.info(f"[get_budget_execution_details] 返回{len(results)}条预算执行详情")
        return results

    @staticmethod
    def _build_budget_history_filter_summary(
        budget_type: int | None = None,
        period_type: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
    ) -> str:
        """构建预算快照筛选摘要，便于后续历史查询复用相同口径。"""
        summary = {
            "budget_type": int(budget_type) if budget_type else None,
            "period_type": period_type or "",
            "budget_id": int(budget_id) if budget_id else None,
            "category_id": int(category_id) if category_id else None,
            "account_ids": sorted(int(item) for item in (account_ids or [])),
            "tag_ids": sorted(int(item) for item in (tag_ids or [])),
        }
        return json.dumps(summary, ensure_ascii=False, sort_keys=True)

    @log_method
    async def create_budget_execution_snapshots(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str = None,
        end_date: str = None,
        budget_id: int = None,
        category_id: int = None,
        account_ids: list[int] = None,
        tag_ids: list[int] = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """创建预算执行快照。"""
        snapshots = await self.get_budget_execution_details(
            budget_type=budget_type,
            start_date=start_date,
            end_date=end_date,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=account_ids,
            tag_ids=tag_ids,
            user_id=user_id,
        )

        conn = await self._get_connection()
        calculated_at = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        filter_summary = self._build_budget_history_filter_summary(
            budget_type=budget_type,
            period_type=period_type,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=account_ids,
            tag_ids=tag_ids,
        )

        created_count = 0
        for snapshot in snapshots:
            await conn.execute(
                """
                DELETE FROM budget_history
                WHERE user_id = ? AND budget_id = ? AND period_start = ? AND period_end = ?
                  AND filter_summary = ?
                """,
                (user_id, snapshot["id"], start_date, end_date, filter_summary),
            )

            status = "over_budget" if snapshot["spent_amount"] > snapshot["budget_amount"] else "within_budget"
            await conn.execute(
                """
                INSERT INTO budget_history (
                    user_id, budget_id, period_start, period_end,
                    budget_amount, spent_amount, remaining_amount,
                    execution_rate, status, filter_summary, calculated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    snapshot["id"],
                    start_date,
                    end_date,
                    snapshot["budget_amount"],
                    snapshot["spent_amount"],
                    snapshot["remaining_amount"],
                    snapshot["execution_rate"],
                    status,
                    filter_summary,
                    calculated_at,
                ),
            )
            created_count += 1

        await conn.commit()
        return {
            "created_count": created_count,
            "period_start": start_date,
            "period_end": end_date,
            "filter_summary": filter_summary,
            "calculated_at": calculated_at,
        }

    @log_method
    async def get_budget_execution_history(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str = None,
        end_date: str = None,
        budget_id: int = None,
        category_id: int = None,
        account_ids: list[int] = None,
        tag_ids: list[int] = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """获取预算执行快照历史。"""
        if start_date and end_date:
            history_items = await self._build_budget_execution_history_on_demand(
                budget_type=budget_type,
                period_type=period_type,
                start_date=start_date,
                end_date=end_date,
                budget_id=budget_id,
                category_id=category_id,
                account_ids=account_ids,
                tag_ids=tag_ids,
                user_id=user_id,
            )
            if history_items:
                return history_items

        conn = await self._get_connection()
        filter_summary = self._build_budget_history_filter_summary(
            budget_type=budget_type,
            period_type=period_type,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=account_ids,
            tag_ids=tag_ids,
        )

        query = """
            SELECT
                bh.id,
                bh.budget_id,
                bh.period_start,
                bh.period_end,
                bh.budget_amount,
                bh.spent_amount,
                bh.remaining_amount,
                bh.execution_rate,
                bh.status,
                bh.filter_summary,
                bh.calculated_at,
                b.name,
                b.category,
                b.sub_category,
                b.period_type,
                b.alert_threshold,
                b.enabled
            FROM budget_history bh
            INNER JOIN budgets b ON b.id = bh.budget_id
            WHERE bh.user_id = ? AND b.user_id = ?
        """
        params: list[Any] = [user_id, user_id]

        if budget_id:
            query += " AND bh.budget_id = ?"
            params.append(budget_id)

        if start_date:
            query += " AND bh.period_start >= ?"
            params.append(start_date)

        if end_date:
            query += " AND bh.period_end <= ?"
            params.append(end_date)

        query += " AND bh.filter_summary = ?"
        params.append(filter_summary)
        query += " ORDER BY bh.period_start DESC, bh.calculated_at DESC, bh.budget_id ASC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        return [dict(row) for row in rows]

    @staticmethod
    def _parse_budget_history_date(date_text: str | None) -> date | None:
        """解析预算历史使用的日期字符串。"""
        if not date_text:
            return None

        try:
            return datetime.strptime(str(date_text)[:10], "%Y-%m-%d").date()
        except (TypeError, ValueError):
            return None

    @staticmethod
    def _add_months(source_date: date, months: int) -> date:
        """为日期增加指定月份数。"""
        total_month = (source_date.year * 12 + source_date.month - 1) + months
        year = total_month // 12
        month = total_month % 12 + 1
        return date(year, month, 1)

    @classmethod
    def _iter_budget_history_period_ranges(
        cls, period_type: str, start_date: str | None, end_date: str | None
    ) -> list[dict[str, str]]:
        """根据周期类型生成历史查询区间。"""
        start = cls._parse_budget_history_date(start_date)
        end = cls._parse_budget_history_date(end_date)

        if not start or not end or start > end:
            return []

        period_ranges: list[dict[str, str]] = []

        if period_type == "yearly":
            current_start = date(start.year, 1, 1)
            while current_start <= end:
                next_start = date(current_start.year + 1, 1, 1)
                current_end = next_start - timedelta(days=1)
                period_ranges.append(
                    {"start_date": current_start.strftime("%Y-%m-%d"), "end_date": current_end.strftime("%Y-%m-%d")}
                )
                current_start = next_start
            return period_ranges

        if period_type == "quarterly":
            quarter_start_month = ((start.month - 1) // 3) * 3 + 1
            current_start = date(start.year, quarter_start_month, 1)
            while current_start <= end:
                next_start = cls._add_months(current_start, 3)
                current_end = next_start - timedelta(days=1)
                period_ranges.append(
                    {"start_date": current_start.strftime("%Y-%m-%d"), "end_date": current_end.strftime("%Y-%m-%d")}
                )
                current_start = next_start
            return period_ranges

        current_start = date(start.year, start.month, 1)
        while current_start <= end:
            next_start = cls._add_months(current_start, 1)
            current_end = next_start - timedelta(days=1)
            period_ranges.append(
                {"start_date": current_start.strftime("%Y-%m-%d"), "end_date": current_end.strftime("%Y-%m-%d")}
            )
            current_start = next_start

        return period_ranges

    @classmethod
    def _budget_overlaps_period(
        cls, budget_start: str | None, budget_end: str | None, period_start: str, period_end: str
    ) -> bool:
        """判断预算定义是否覆盖指定历史周期。"""
        parsed_budget_start = cls._parse_budget_history_date(budget_start)
        parsed_budget_end = cls._parse_budget_history_date(budget_end)
        parsed_period_start = cls._parse_budget_history_date(period_start)
        parsed_period_end = cls._parse_budget_history_date(period_end)

        if not parsed_period_start or not parsed_period_end:
            return False

        if parsed_budget_start and parsed_budget_start > parsed_period_end:
            return False

        if parsed_budget_end and parsed_budget_end < parsed_period_start:
            return False

        return True

    @log_method
    async def _build_budget_execution_history_on_demand(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str = None,
        end_date: str = None,
        budget_id: int = None,
        category_id: int = None,
        account_ids: list[int] = None,
        tag_ids: list[int] = None,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """按查询周期动态计算预算历史，避免仅依赖已落库快照。"""
        period_ranges = self._iter_budget_history_period_ranges(period_type, start_date, end_date)
        if not period_ranges:
            return []

        filter_summary = self._build_budget_history_filter_summary(
            budget_type=budget_type,
            period_type=period_type,
            budget_id=budget_id,
            category_id=category_id,
            account_ids=account_ids,
            tag_ids=tag_ids,
        )

        history_items: list[dict[str, Any]] = []
        self.logger.info(
            "[预算历史] 动态计算区间数=%d, period_type=%s, range=%s~%s",
            len(period_ranges),
            period_type,
            start_date,
            end_date,
        )

        for period_range in period_ranges:
            period_start = period_range["start_date"]
            period_end = period_range["end_date"]
            execution_details = await self.get_budget_execution_details(
                budget_type=budget_type,
                start_date=period_start,
                end_date=period_end,
                budget_id=budget_id,
                category_id=category_id,
                account_ids=account_ids,
                tag_ids=tag_ids,
                user_id=user_id,
            )

            for detail in execution_details:
                if not self._budget_overlaps_period(
                    detail.get("start_date"), detail.get("end_date"), period_start, period_end
                ):
                    continue

                history_items.append(
                    {
                        "id": f"{detail.get('id', '')}_{period_start}_{period_end}",
                        "budget_id": detail.get("id"),
                        "period_start": period_start,
                        "period_end": period_end,
                        "budget_amount": detail.get("budget_amount", 0),
                        "spent_amount": detail.get("spent_amount", 0),
                        "remaining_amount": detail.get("remaining_amount", 0),
                        "execution_rate": detail.get("execution_rate", 0),
                        "status": "over_budget"
                        if detail.get("spent_amount", 0) > detail.get("budget_amount", 0)
                        else "within_budget",
                        "filter_summary": filter_summary,
                        "calculated_at": "",
                        "name": detail.get("name", ""),
                        "category": detail.get("category", ""),
                        "sub_category": detail.get("sub_category", ""),
                        "category_id": detail.get("category_id", ""),
                        "category_info": detail.get("category_info"),
                        "type": detail.get("type", budget_type),
                        "period_type": detail.get("period_type", period_type),
                        "alert_threshold": detail.get("alert_threshold", 80),
                        "enabled": detail.get("enabled", 1),
                    }
                )

        history_items.sort(
            key=lambda item: (
                item.get("period_start", ""),
                item.get("period_end", ""),
                str(item.get("category", "")),
                str(item.get("sub_category", "")),
                int(item.get("budget_id", 0) or 0),
            ),
            reverse=True,
        )
        return history_items

    @log_method
    async def get_period_forecast(
        self,
        budget_type: int = 3,
        period_type: str = "monthly",
        start_date: str = None,
        end_date: str = None,
        forecast_strategy: str = "historical_average",
        history_periods: int = 6,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
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
            f"period={period_type}, dates={start_date}~{end_date}, "
            f"strategy={forecast_strategy}, history_periods={history_periods}, user_id={user_id}"
        )

        conn = await self._get_connection()
        type_name = "支出" if budget_type == 3 else "投资"

        # 根据周期类型确定分组方式
        if period_type == "daily":
            date_format = "%Y-%m-%d"
            group_by = "date"
        elif period_type == "weekly":
            date_format = "%Y-%W"
            group_by = "strftime('%Y-%W', date)"
        elif period_type == "monthly":
            date_format = "%Y-%m"
            group_by = "strftime('%Y-%m', date)"
        else:  # yearly
            date_format = "%Y"
            group_by = "strftime('%Y', date)"

        # 查询历史数据
        query = f"""
            SELECT
                {group_by} as period,
                main_category,
                COALESCE(SUM(amount), 0) as total_amount,
                COUNT(*) as transaction_count
            FROM bills
            WHERE type = ? AND user_id = ?
        """
        params = [type_name, user_id]

        if start_date:
            query += " AND date >= ?"
            params.append(start_date)

        if end_date:
            query += " AND date <= ?"
            params.append(end_date)

        query += f" GROUP BY {group_by}, main_category ORDER BY period DESC, total_amount DESC"

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        normalized_strategy = forecast_strategy or "historical_average"
        normalized_history_periods = max(int(history_periods or 0), 1)

        # 按分类汇总
        category_totals = {}
        period_count = set()

        for row in rows:
            period = row["period"]
            category = row["main_category"] or "未分类"
            amount = abs(row["total_amount"])

            period_count.add(period)

            if category not in category_totals:
                category_totals[category] = {"total": 0, "periods": []}
            category_totals[category]["total"] += amount
            category_totals[category]["periods"].append({"period": period, "amount": amount})

        categories = await self.get_all_categories(user_id=user_id)
        category_context = self._build_budget_category_context(categories)

        budget_query = """
            SELECT category, sub_category, amount
            FROM budgets
            WHERE period_type = ? AND enabled = 1 AND user_id = ?
        """
        budget_params = [period_type, user_id]

        if start_date:
            budget_query += " AND (end_date IS NULL OR end_date = '' OR end_date >= ?)"
            budget_params.append(start_date)

        if end_date:
            budget_query += " AND (start_date IS NULL OR start_date = '' OR start_date <= ?)"
            budget_params.append(end_date)

        async with conn.execute(budget_query, budget_params) as cursor:
            budget_rows = await cursor.fetchall()

        budget_map = {}
        for row in budget_rows:
            category_name = row["category"] or "未分类"
            resolved_budget_type = self._resolve_budget_category_type(
                category_name,
                row["sub_category"],
                category_context,
                preferred_type=budget_type,
            )
            if int(resolved_budget_type or 0) != int(budget_type):
                continue
            budget_map.setdefault(category_name, {"primary": 0.0, "sub_total": 0.0})
            amount_value = float(row["amount"] or 0)
            if not row["sub_category"]:
                budget_map[category_name]["primary"] += amount_value
            else:
                budget_map[category_name]["sub_total"] += amount_value

        # 计算平均值和预测
        num_periods = len(period_count) if period_count else 1
        results = []

        for category, data in category_totals.items():
            sorted_periods = sorted(data["periods"], key=lambda item: item["period"])
            recent_periods = sorted_periods[-normalized_history_periods:]

            if not recent_periods:
                continue

            recent_amounts = [float(item["amount"]) for item in recent_periods]
            avg_amount = sum(recent_amounts) / len(recent_amounts)
            moving_window_size = min(3, len(recent_amounts))
            moving_average_amount = (
                sum(recent_amounts[-moving_window_size:]) / moving_window_size if moving_window_size > 0 else 0
            )

            if normalized_strategy == "moving_average":
                forecast_amount = moving_average_amount
                strategy_explanation = f"基于最近{moving_window_size}个周期的移动平均"
            else:
                forecast_amount = avg_amount
                strategy_explanation = f"基于最近{len(recent_amounts)}个周期的历史均值"

            backtest_errors = []
            for index in range(1, len(recent_amounts)):
                actual_amount = recent_amounts[index]
                if actual_amount <= 0:
                    continue

                history_slice = recent_amounts[:index]
                if normalized_strategy == "moving_average":
                    window_size = min(3, len(history_slice))
                    predicted_amount = sum(history_slice[-window_size:]) / window_size if window_size > 0 else 0
                else:
                    predicted_amount = sum(history_slice) / len(history_slice)

                backtest_errors.append(abs(actual_amount - predicted_amount) / actual_amount)

            backtest_mape = round((sum(backtest_errors) / len(backtest_errors)) * 100, 2) if backtest_errors else None
            if backtest_mape is None:
                confidence = "low"
            elif backtest_mape <= 10:
                confidence = "high"
            elif backtest_mape <= 20:
                confidence = "medium"
            else:
                confidence = "low"

            latest_amount = recent_amounts[-1] if recent_amounts else 0
            if len(recent_amounts) >= 2:
                baseline_amounts = recent_amounts[:-1]
                baseline_avg = sum(baseline_amounts) / len(baseline_amounts)
                if baseline_avg > 0 and latest_amount > baseline_avg * 1.05:
                    trend = "up"
                elif baseline_avg > 0 and latest_amount < baseline_avg * 0.95:
                    trend = "down"
                else:
                    trend = "stable"
            else:
                trend = "stable"

            category_info = self._resolve_budget_category_info(category, "", category_context, budget_type)

            budget_amount = 0.0
            if category in budget_map:
                budget_info = budget_map[category]
                budget_amount = budget_info["primary"] if budget_info["primary"] > 0 else budget_info["sub_total"]

            current_spent = latest_amount
            projected_over_budget = budget_amount > 0 and forecast_amount > budget_amount

            results.append(
                {
                    "category": category,
                    "category_info": category_info,
                    "total_amount": round(sum(recent_amounts), 2),
                    "average_amount": round(avg_amount, 2),
                    "period_count": num_periods,
                    "sample_periods": len(recent_amounts),
                    "current_spent": round(current_spent, 2),
                    "budget_amount": round(budget_amount, 2),
                    "forecast_amount": round(forecast_amount, 2),
                    "projected_over_budget": projected_over_budget,
                    "forecast_strategy": normalized_strategy,
                    "strategy_explanation": strategy_explanation,
                    "backtest_mape": backtest_mape,
                    "confidence": confidence,
                    "trend": trend,
                    "periods": recent_periods,
                }
            )

        # 按预测金额排序
        results.sort(key=lambda x: x["forecast_amount"], reverse=True)

        self.logger.info(f"[get_period_forecast] 返回{len(results)}条预测数据")
        return results

    @log_method
    async def import_budgets(self, budgets_data: list[dict[str, Any]], user_id: int = 1) -> dict[str, Any]:
        """
        批量导入预算

        Args:
            budgets_data: 预算数据列表

        Returns:
            导入结果统计
        """
        self.logger.info(f"[import_budgets] 开始导入{len(budgets_data)}条预算")

        conn = await self._get_connection()
        now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        created_count = 0
        updated_count = 0
        error_count = 0
        errors = []

        for idx, data in enumerate(budgets_data):
            try:
                # 检查必填字段
                if not data.get("name") or not data.get("amount"):
                    errors.append(f"第{idx + 1}条: 缺少必填字段(name或amount)")
                    error_count += 1
                    continue

                # 检查是否存在同名预算
                existing = None
                async with conn.execute(
                    "SELECT id FROM budgets WHERE name = ? AND user_id = ?", (data["name"], user_id)
                ) as cursor:
                    existing = await cursor.fetchone()

                if existing:
                    # 更新现有预算
                    await conn.execute(
                        """
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
                        WHERE id = ? AND user_id = ?
                    """,
                        (
                            data.get("category"),
                            data.get("sub_category"),
                            data.get("period_type", "monthly"),
                            data["amount"],
                            data.get("start_date"),
                            data.get("end_date"),
                            data.get("alert_threshold", 80),
                            data.get("enabled", 1),
                            now,
                            existing["id"],
                            user_id,
                        ),
                    )
                    updated_count += 1
                else:
                    # 创建新预算
                    await conn.execute(
                        """
                        INSERT INTO budgets (
                            name, category, sub_category, period_type, amount,
                            start_date, end_date, alert_threshold, enabled,
                            created_at, updated_at, user_id
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                        (
                            data["name"],
                            data.get("category"),
                            data.get("sub_category"),
                            data.get("period_type", "monthly"),
                            data["amount"],
                            data.get("start_date"),
                            data.get("end_date"),
                            data.get("alert_threshold", 80),
                            data.get("enabled", 1),
                            now,
                            now,
                            user_id,
                        ),
                    )
                    created_count += 1

            except Exception as e:
                errors.append(f"第{idx + 1}条: {e!s}")
                error_count += 1
                self.logger.error(f"导入预算失败: {e}")

        await conn.commit()

        result = {"created": created_count, "updated": updated_count, "errors": error_count, "error_details": errors}

        self.logger.info(f"[import_budgets] 导入完成: 创建={created_count}, 更新={updated_count}, 错误={error_count}")
        return result

    @log_method
    async def export_budgets(self, user_id: int = 1) -> list[dict[str, Any]]:
        """
        导出所有预算

        Returns:
            预算数据列表
        """
        self.logger.info("[export_budgets] 开始导出预算")

        conn = await self._get_connection()
        export_fields = [
            "name",
            "category",
            "sub_category",
            "period_type",
            "amount",
            "start_date",
            "end_date",
            "alert_threshold",
            "enabled",
        ]

        async with conn.execute("SELECT * FROM budgets WHERE user_id = ? ORDER BY created_at", (user_id,)) as cursor:
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
            await conn.execute("UPDATE categories SET main_category = ? WHERE main_category = ?", (new_name, old_name))
            await conn.commit()
            self.logger.info(f"已更新主分类名称: {old_name} -> {new_name}")

            # 清除缓存
            self._clear_cache("account_mappings")
            self._clear_cache("category_mappings")

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
    async def get_account_mappings(self) -> dict[str, Any]:
        """获取账户映射(带缓存)

        Returns:
            Dict: {
                'id_to_account': {id: account_dict},
                'name_to_id': {name: id},
                'id_to_name': {id: name}
            }
        """
        cache_key = "account_mappings"

        # 检查缓存
        if self._is_cache_valid(cache_key):
            self.logger.debug("使用缓存的账户映射")
            return self._cache[cache_key]

        # 查询数据库
        accounts = await self.get_all_accounts()

        # 构建映射
        mappings = {
            "id_to_account": {acc["id"]: acc for acc in accounts},
            "name_to_id": {acc["name"]: acc["id"] for acc in accounts},
            "id_to_name": {acc["id"]: acc["name"] for acc in accounts},
        }

        # 更新缓存
        self._cache[cache_key] = mappings
        self._cache_expiry[cache_key] = datetime.now() + timedelta(seconds=self._cache_ttl)

        self.logger.info(f"已构建账户映射: {len(accounts)} 个账户")
        return mappings

    @log_method
    async def get_category_mappings(self) -> dict[str, Any]:
        """获取分类映射(带缓存)

        Returns:
            Dict: {
                'id_to_category': {id: category_dict},
                'name_to_id': {(main, sub): id},
                'id_to_name': {id: (main, sub)}
            }
        """
        cache_key = "category_mappings"

        # 检查缓存
        if self._is_cache_valid(cache_key):
            self.logger.debug("使用缓存的分类映射")
            return self._cache[cache_key]

        # 查询数据库
        categories = await self.get_all_categories()

        # 构建映射
        mappings = {
            "id_to_category": {cat["id"]: cat for cat in categories},
            "name_to_id": {(cat["main_category"], cat["sub_category"]): cat["id"] for cat in categories},
            "id_to_name": {cat["id"]: (cat["main_category"], cat["sub_category"]) for cat in categories},
        }

        # 更新缓存
        self._cache[cache_key] = mappings
        self._cache_expiry[cache_key] = datetime.now() + timedelta(seconds=self._cache_ttl)

        self.logger.info(f"已构建分类映射: {len(categories)} 个分类")
        return mappings

    @log_method
    async def get_balances_before_date(self, date_str: str, user_id: int = 1) -> dict[int, float]:
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
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:  # 忽略ID为0或None
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) + amount

        # 2. 支出 (source_account_id)
        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills WHERE date < ? AND type = '支出' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) - abs(amount)

        # 3. 转出 (source_account_id)
        async with conn.execute(
            "SELECT source_account_id, SUM(amount) FROM bills WHERE date < ? AND type = '转账' AND user_id = ? GROUP BY source_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) - abs(amount)

        # 4. 转入 (destination_account_id)
        async with conn.execute(
            "SELECT destination_account_id, SUM(destination_amount) FROM bills WHERE date < ? AND type = '转账' AND user_id = ? GROUP BY destination_account_id",
            (date_str, user_id),
        ) as cursor:
            async for row in cursor:
                acc_id = row[0]
                if acc_id:
                    amount = row[1] or 0
                    balances[acc_id] = balances.get(acc_id, 0) + abs(amount)

        return balances

    @log_method
    async def add_tags_to_bill(self, bill_id: int, tag_ids: list[int], user_id: int = 1) -> bool:
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
                "INSERT OR IGNORE INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)", values
            )
            await conn.commit()
            self.logger.info(f"[add_tags_to_bill] 成功添加{len(tag_ids)}个标签")
            return True
        except Exception as e:
            self.logger.error(f"添加标签失败: {e}", exc_info=True)
            return False

    @log_method
    async def get_tags_for_bill(self, bill_id: int, user_id: int = 1) -> list[dict[str, Any]]:
        """获取账单的所有标签

        Args:
            bill_id: 账单ID
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT t.*
            FROM tags t
            JOIN bill_tags bt ON t.id = bt.tag_id
            WHERE bt.bill_id = ?
            ORDER BY t.name
        """,
            (bill_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    @log_method
    async def get_tags_for_bills(self, bill_ids: list[int], user_id: int = 1) -> dict[int, list[dict[str, Any]]]:
        """批量获取账单标签

        Args:
            bill_ids: 账单ID列表
            user_id: 用户ID (默认1, 用于多用户数据隔离)
        """
        if not bill_ids:
            return {}

        conn = await self._get_connection()
        placeholders = ",".join("?" * len(bill_ids))

        async with conn.execute(
            f"""
            SELECT bt.bill_id, t.*
            FROM tags t
            JOIN bill_tags bt ON t.id = bt.tag_id
            WHERE bt.bill_id IN ({placeholders})
            ORDER BY t.name
        """,
            bill_ids,
        ) as cursor:
            rows = await cursor.fetchall()

            result = {}
            for row in rows:
                bill_id = row["bill_id"]
                tag = dict(row)
                del tag["bill_id"]  # remove bill_id from tag object

                if bill_id not in result:
                    result[bill_id] = []
                result[bill_id].append(tag)

            return result

    @log_method
    async def update_bill_tags(self, bill_id: int, tag_ids: list[int], user_id: int = 1) -> bool:
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
                await conn.executemany("INSERT INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)", values)
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
        target_id: int | None = None,
        details: dict[str, Any] | None = None,
        affected_count: int = 0,
        ip_address: str | None = None,
        user_agent: str | None = None,
        session_id: str | None = None,
        status: str = "success",
        error_message: str | None = None,
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
                operation_type,
                operation_target,
                target_id,
                details_json,
                affected_count,
                ip_address,
                user_agent,
                session_id,
                status,
                error_message,
                now,
            ),
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
        operation_type: str | None = None,
        operation_target: str | None = None,
        target_id: int | None = None,
        status: str | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
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
                if log_dict.get("details"):
                    try:
                        log_dict["details"] = json.loads(log_dict["details"])
                    except json.JSONDecodeError:
                        pass
                logs.append(log_dict)
            return logs

    @log_method
    async def create_backup_record(self, payload: dict[str, Any]) -> int:
        """创建本地备份记录。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()
        metadata_json = json.dumps(payload.get("metadata", {}), ensure_ascii=False)

        cursor = await conn.execute(
            """
            INSERT INTO backup_records (
                backup_name, storage_type, file_path, checksum,
                encrypted, status, metadata_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(backup_name) DO UPDATE SET
                storage_type = excluded.storage_type,
                file_path = excluded.file_path,
                checksum = excluded.checksum,
                encrypted = excluded.encrypted,
                status = excluded.status,
                metadata_json = excluded.metadata_json,
                updated_at = excluded.updated_at
            """,
            (
                payload.get("backup_name"),
                payload.get("storage_type", "local"),
                payload.get("file_path", ""),
                payload.get("checksum", ""),
                1 if payload.get("encrypted", False) else 0,
                payload.get("status", "created"),
                metadata_json,
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    @log_method
    async def get_backup_records(self) -> list[dict[str, Any]]:
        """获取备份记录列表。"""
        conn = await self._get_connection()
        async with conn.execute(
            "SELECT * FROM backup_records ORDER BY created_at DESC, id DESC"
        ) as cursor:
            rows = await cursor.fetchall()
            result: list[dict[str, Any]] = []
            for row in rows:
                record = dict(row)
                try:
                    record["metadata"] = json.loads(record.get("metadata_json") or "{}")
                except (TypeError, ValueError, json.JSONDecodeError):
                    record["metadata"] = {}
                record["encrypted"] = bool(record.get("encrypted", 0))
                result.append(record)
            return result

    @log_method
    async def update_backup_record_by_filename(self, filename: str, updates: dict[str, Any]) -> bool:
        """按备份文件名更新备份记录。"""
        if not filename or not updates:
            return False

        conn = await self._get_connection()
        update_payload = dict(updates)
        update_payload["updated_at"] = datetime.now().isoformat()
        if "metadata" in update_payload:
            metadata = dict(update_payload.pop("metadata") or {})
            async with conn.execute(
                "SELECT metadata_json FROM backup_records WHERE backup_name = ?",
                (filename,),
            ) as cursor:
                row = await cursor.fetchone()

            existing_metadata: dict[str, Any] = {}
            if row and row[0]:
                try:
                    existing_metadata = json.loads(row[0])
                except (TypeError, ValueError, json.JSONDecodeError):
                    existing_metadata = {}

            existing_metadata.update(metadata)
            update_payload["metadata_json"] = json.dumps(existing_metadata, ensure_ascii=False)

        set_clause = ", ".join(f"{key} = ?" for key in update_payload.keys())
        values = list(update_payload.values())
        values.append(filename)
        cursor = await conn.execute(
            f"UPDATE backup_records SET {set_clause} WHERE backup_name = ?",
            values,
        )
        await conn.commit()
        return cursor.rowcount > 0

    @log_method
    async def get_backup_jobs(self) -> list[dict[str, Any]]:
        """获取备份任务配置列表。"""
        conn = await self._get_connection()
        async with conn.execute("SELECT * FROM backup_jobs ORDER BY created_at DESC, id DESC") as cursor:
            rows = await cursor.fetchall()
            result: list[dict[str, Any]] = []
            for row in rows:
                record = dict(row)
                record["enabled"] = bool(record.get("enabled", 0))
                result.append(record)
            return result

    @log_method
    async def create_or_update_backup_job(self, payload: dict[str, Any]) -> int:
        """创建或更新备份任务配置。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()
        job_id = int(payload.get("id") or 0)

        if job_id:
            await conn.execute(
                """
                UPDATE backup_jobs
                SET job_type = ?, schedule_expr = ?, retention_days = ?, retention_count = ?,
                    enabled = ?, last_status = ?, updated_at = ?
                WHERE id = ?
                """,
                (
                    payload.get("job_type"),
                    payload.get("schedule_expr"),
                    payload.get("retention_days", 30),
                    payload.get("retention_count", 10),
                    1 if payload.get("enabled", True) else 0,
                    payload.get("last_status"),
                    now,
                    job_id,
                ),
            )
            await conn.commit()
            return job_id

        cursor = await conn.execute(
            """
            INSERT INTO backup_jobs (
                job_type, schedule_expr, retention_days, retention_count,
                enabled, last_run_at, last_status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                payload.get("job_type"),
                payload.get("schedule_expr"),
                payload.get("retention_days", 30),
                payload.get("retention_count", 10),
                1 if payload.get("enabled", True) else 0,
                payload.get("last_run_at"),
                payload.get("last_status"),
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid or 0)

    # ==================== 密码验证相关方法 ====================

    @log_method
    async def get_app_setting(self, key: str) -> str | None:
        """
        获取应用配置

        Args:
            key: 配置键

        Returns:
            Optional[str]: 配置值，不存在返回None
        """
        conn = await self._get_connection()

        async with conn.execute("SELECT value, is_encrypted FROM app_settings WHERE key = ?", (key,)) as cursor:
            row = await cursor.fetchone()
            if row:
                value = row["value"]
                # TODO: 如果is_encrypted为True，解密value
                return value
            return None

    @log_method
    async def set_app_setting(
        self,
        key: str,
        value: str,
        value_type: str = "string",
        description: str | None = None,
        is_encrypted: bool = False,
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
                (key, encrypted_value, value_type, description, is_encrypted, now, now),
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
        env_password = os.getenv("BILL_ANALYSER_OPERATION_PASSWORD")
        if env_password:
            result = password == env_password
            self.logger.info(f"使用环境变量密码验证: {'成功' if result else '失败'}")
            return result

        # 2. 从数据库读取配置
        stored_password = await self.get_app_setting("operation_password")

        # 3. 如果没有配置密码，默认接受任何密码（开发模式）
        if not stored_password:
            self.logger.warning("未配置操作密码，默认允许操作（不安全！）")
            return True

        # 4. 验证密码
        result = password == stored_password
        self.logger.info(f"数据库密码验证: {'成功' if result else '失败'}")
        return result

    @staticmethod
    def _normalize_import_config_header(value: Any) -> str:
        """标准化导入模板表头文本。"""
        if value is None:
            return ""
        return " ".join(str(value).strip().lower().split())

    def _normalize_import_config_headers(self, headers: list[Any] | None) -> list[str]:
        """标准化表头列表，过滤空值。"""
        if not headers:
            return []

        normalized_headers = []
        for header in headers:
            normalized = self._normalize_import_config_header(header)
            if normalized:
                normalized_headers.append(normalized)

        return normalized_headers

    def _build_import_config_header_signature(self, headers: list[Any] | None) -> str:
        """构建稳定的表头签名。"""
        normalized_headers = self._normalize_import_config_headers(headers)
        if not normalized_headers:
            return ""
        return "||".join(normalized_headers)

    @staticmethod
    def _parse_json_object(raw_value: Any, default: dict[str, Any] | None = None) -> dict[str, Any]:
        """安全解析 JSON 对象。"""
        if raw_value in (None, ""):
            return default.copy() if default else {}

        if isinstance(raw_value, dict):
            return dict(raw_value)

        try:
            parsed = json.loads(raw_value)
            if isinstance(parsed, dict):
                return parsed
        except (TypeError, ValueError, json.JSONDecodeError):
            pass

        return default.copy() if default else {}

    def _serialize_import_config_custom_rules(self, custom_rules: Any, sample_headers: list[Any] | None = None) -> str:
        """序列化导入模板附加规则，并补充表头学习元数据。"""
        custom_rules_data = self._parse_json_object(custom_rules)
        normalized_headers = self._normalize_import_config_headers(sample_headers)

        if normalized_headers:
            custom_rules_data["sample_headers"] = normalized_headers
            custom_rules_data["header_signature"] = self._build_import_config_header_signature(normalized_headers)
            custom_rules_data["header_count"] = len(normalized_headers)

        return json.dumps(custom_rules_data, ensure_ascii=False)

    def _deserialize_import_config_row(self, row: aiosqlite.Row) -> dict[str, Any]:
        """将 import_configs 表行反序列化为前端友好的结构。"""
        result = dict(row)
        result["field_mappings"] = self._parse_json_object(result.get("field_mappings"))
        result["custom_rules"] = self._parse_json_object(result.get("custom_rules"))
        result["sample_headers"] = result["custom_rules"].get("sample_headers", [])
        result["header_signature"] = result["custom_rules"].get("header_signature", "")
        result["header_count"] = int(result["custom_rules"].get("header_count", 0) or 0)
        result["has_header"] = bool(result.get("has_header", 1))
        result["is_default"] = bool(result.get("is_default", 0))
        result["description_summary"] = self._build_import_config_description_summary(result)
        result["default_recommendation"] = False
        return result

    @staticmethod
    def _build_import_config_description_summary(config: dict[str, Any]) -> str:
        """基于字段映射和样本表头生成可读摘要，供前端在空描述时回退展示。"""
        field_mappings = config.get("field_mappings") or {}
        sample_headers = config.get("sample_headers") or []

        display_labels = {
            "date": "时间",
            "type": "类型",
            "amount": "金额",
            "description": "描述",
            "account": "账户",
            "category": "分类",
            "counterparty": "交易对方",
            "paymentMethod": "支付方式",
            "payment_method": "支付方式",
        }
        display_order = {
            "date": 1,
            "type": 2,
            "amount": 3,
            "description": 4,
            "account": 5,
            "category": 6,
            "counterparty": 7,
            "paymentMethod": 8,
            "payment_method": 8,
        }

        summary_parts: list[str] = []

        if isinstance(field_mappings, dict) and field_mappings:
            mapped_entries = []
            sorted_items = sorted(
                field_mappings.items(), key=lambda item: (display_order.get(str(item[0]), 99), str(item[0]))
            )

            for field_name, header_name in sorted_items:
                header_text = str(header_name or "").strip()
                if not header_text:
                    continue

                mapped_entries.append(f"{display_labels.get(str(field_name), str(field_name))}->{header_text}")
                if len(mapped_entries) >= 4:
                    break

            if mapped_entries:
                summary_parts.append(f"映射: {' / '.join(mapped_entries)}")

        normalized_headers = []
        for header in sample_headers[:4]:
            header_text = str(header or "").strip()
            if header_text:
                normalized_headers.append(header_text)

        if normalized_headers:
            summary_parts.append(f"表头: {' / '.join(normalized_headers)}")

        return " | ".join(summary_parts)

    @staticmethod
    def _mark_import_config_default_recommendation(configs: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """在没有默认模板时，为最合适的模板打上默认推荐标记。"""
        if not configs:
            return configs

        if any(bool(config.get("is_default")) for config in configs):
            return configs

        def _sort_key(config: dict[str, Any]) -> tuple:
            use_count = int(config.get("use_count", 0) or 0)
            last_used_at = str(config.get("last_used_at", "") or "")
            updated_at = str(config.get("updated_at", "") or "")
            created_at = str(config.get("created_at", "") or "")
            return (use_count, last_used_at, updated_at, created_at)

        recommended = max(configs, key=_sort_key)
        recommended["default_recommendation"] = True
        return configs

    @log_method
    async def save_import_config(self, data: dict[str, Any], user_id: int = 1) -> int:
        """保存导入列映射模板。"""
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        config_id = int(data.get("id", 0) or 0)
        name = str(data.get("name", "")).strip()
        file_format = str(data.get("file_format", "")).strip().lower()
        field_mappings = data.get("field_mappings") or {}

        if not name:
            raise ValueError("name is required")
        if not file_format:
            raise ValueError("file_format is required")
        if not isinstance(field_mappings, dict) or not field_mappings:
            raise ValueError("field_mappings is required")

        description = str(data.get("description", "") or "").strip()
        date_format = str(data.get("date_format", "") or "").strip()
        encoding = str(data.get("encoding", "utf-8") or "utf-8").strip()
        delimiter = data.get("delimiter")
        skip_rows = int(data.get("skip_rows", 0) or 0)
        has_header = 1 if bool(data.get("has_header", True)) else 0
        is_default = 1 if bool(data.get("is_default", False)) else 0
        sample_headers = data.get("sample_headers") or data.get("headers") or []
        custom_rules_json = self._serialize_import_config_custom_rules(
            data.get("custom_rules"), sample_headers=sample_headers
        )
        field_mappings_json = json.dumps(field_mappings, ensure_ascii=False)

        if is_default:
            await conn.execute(
                "UPDATE import_configs SET is_default = 0, updated_at = ? WHERE user_id = ? AND file_format = ?",
                (now, user_id, file_format),
            )

        if config_id:
            cursor = await conn.execute(
                """
                UPDATE import_configs
                SET name = ?, file_format = ?, description = ?, field_mappings = ?,
                    date_format = ?, encoding = ?, delimiter = ?, skip_rows = ?,
                    has_header = ?, custom_rules = ?, is_default = ?, updated_at = ?
                WHERE id = ? AND user_id = ?
                """,
                (
                    name,
                    file_format,
                    description,
                    field_mappings_json,
                    date_format,
                    encoding,
                    delimiter,
                    skip_rows,
                    has_header,
                    custom_rules_json,
                    is_default,
                    now,
                    config_id,
                    user_id,
                ),
            )
            if cursor.rowcount == 0:
                raise ValueError("import config not found")
            saved_id = config_id
        else:
            cursor = await conn.execute(
                """
                INSERT INTO import_configs (
                    user_id, name, file_format, description, field_mappings,
                    date_format, encoding, delimiter, skip_rows, has_header,
                    custom_rules, is_default, use_count, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    name,
                    file_format,
                    description,
                    field_mappings_json,
                    date_format,
                    encoding,
                    delimiter,
                    skip_rows,
                    has_header,
                    custom_rules_json,
                    is_default,
                    0,
                    now,
                    now,
                ),
            )
            saved_id = int(cursor.lastrowid)

        await conn.commit()
        self.logger.info("[导入模板] 已保存 config_id=%s, user_id=%s, file_format=%s", saved_id, user_id, file_format)
        return saved_id

    @log_method
    async def get_import_configs(
        self, user_id: int = 1, file_format: str | None = None, limit: int = 100
    ) -> list[dict[str, Any]]:
        """获取用户的导入列映射模板列表。"""
        conn = await self._get_connection()
        params: list[Any] = [user_id]
        query = "SELECT * FROM import_configs WHERE user_id = ?"

        if file_format:
            query += " AND file_format = ?"
            params.append(str(file_format).strip().lower())

        query += " ORDER BY is_default DESC, updated_at DESC LIMIT ?"
        params.append(int(limit))

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        configs = [self._deserialize_import_config_row(row) for row in rows]
        return self._mark_import_config_default_recommendation(configs)

    @log_method
    async def find_matching_import_config(
        self, file_format: str, headers: list[Any], user_id: int = 1, min_score: float = 0.6
    ) -> dict[str, Any] | None:
        """根据文件格式与表头，匹配最合适的导入模板。"""
        conn = await self._get_connection()
        normalized_headers = self._normalize_import_config_headers(headers)
        if not normalized_headers:
            return None

        configs = await self.get_import_configs(user_id=user_id, file_format=file_format, limit=200)
        if not configs:
            return None

        incoming_signature = self._build_import_config_header_signature(normalized_headers)
        incoming_set = set(normalized_headers)
        best_match = None
        best_score = 0.0
        best_reason = ""
        default_match = None

        for config in configs:
            if config.get("is_default") and default_match is None:
                default_match = config

            stored_headers = self._normalize_import_config_headers(config.get("sample_headers"))
            stored_signature = config.get("header_signature", "")

            if stored_signature and stored_signature == incoming_signature:
                best_match = config
                best_score = 1.0
                best_reason = "exact_header_signature"
                break

            if not stored_headers:
                continue

            stored_set = set(stored_headers)
            overlap_count = len(incoming_set & stored_set)
            if overlap_count == 0:
                continue

            overlap_score = overlap_count / max(len(incoming_set), len(stored_set))
            if normalized_headers[:1] == stored_headers[:1]:
                overlap_score += 0.05

            if overlap_score > best_score:
                best_match = config
                best_score = overlap_score
                best_reason = "header_overlap"

        if not best_match or best_score < min_score:
            if not default_match:
                return None

            matched = dict(default_match)
            matched["match_score"] = 0.0
            matched["match_reason"] = "default_template_fallback"
            matched["matched_header_count"] = 0

            now = datetime.now().isoformat()
            await conn.execute(
                """
                UPDATE import_configs
                SET use_count = COALESCE(use_count, 0) + 1,
                    last_used_at = ?,
                    updated_at = ?
                WHERE id = ? AND user_id = ?
                """,
                (now, now, int(default_match["id"]), user_id),
            )
            await conn.commit()
            return matched

        matched = dict(best_match)
        matched["match_score"] = round(min(best_score, 1.0), 4)
        matched["match_reason"] = best_reason
        matched["matched_header_count"] = len(
            incoming_set & set(self._normalize_import_config_headers(best_match.get("sample_headers")))
        )

        now = datetime.now().isoformat()
        await conn.execute(
            """
            UPDATE import_configs
            SET use_count = COALESCE(use_count, 0) + 1,
                last_used_at = ?,
                updated_at = ?
            WHERE id = ? AND user_id = ?
            """,
            (now, now, int(best_match["id"]), user_id),
        )
        await conn.commit()
        return matched

    @log_method
    async def delete_import_config(self, config_id: int, user_id: int = 1) -> bool:
        """删除导入列映射模板。"""
        conn = await self._get_connection()
        cursor = await conn.execute("DELETE FROM import_configs WHERE id = ? AND user_id = ?", (config_id, user_id))
        await conn.commit()
        return cursor.rowcount > 0

    # ==================== v6.47: 账单导入三阶段系统方法 ====================

    @log_method
    async def create_import_session(self, session_id: str, user_id: int = 1, file_count: int = 0) -> int:
        """
        创建导入会话

        Args:
            session_id: 会话唯一标识（UUID格式）
            user_id: 用户ID
            file_count: 待导入文件数量

        Returns:
            int: 会话数据库ID
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        self.logger.info(f"[创建导入会话] session_id={session_id}, user_id={user_id}, file_count={file_count}")

        cursor = await conn.execute(
            """
            INSERT INTO import_sessions (
                session_id, user_id, status, file_count,
                total_parsed, total_preview, total_confirmed,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (session_id, user_id, "parsing", file_count, 0, 0, 0, now, now),
        )

        await conn.commit()
        session_db_id = cursor.lastrowid

        self.logger.info(f"[导入会话创建成功] id={session_db_id}, session_id={session_id}")
        return session_db_id

    @log_method
    async def update_import_session_status(
        self,
        session_id: str,
        status: str,
        total_parsed: int | None = None,
        total_preview: int | None = None,
        total_confirmed: int | None = None,
    ) -> bool:
        """
        更新导入会话状态

        Args:
            session_id: 会话唯一标识
            status: 状态（parsing/deduping/previewing/confirming/completed/failed）
            total_parsed: 解析总数
            total_preview: 预览总数
            total_confirmed: 确认总数

        Returns:
            bool: 是否成功
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        self.logger.info(
            f"[更新导入会话] session_id={session_id}, status={status}, "
            f"parsed={total_parsed}, preview={total_preview}, "
            f"confirmed={total_confirmed}"
        )

        # 构建动态更新语句
        update_parts = ["status = ?", "updated_at = ?"]
        params = [status, now]

        if total_parsed is not None:
            update_parts.append("total_parsed = ?")
            params.append(total_parsed)

        if total_preview is not None:
            update_parts.append("total_preview = ?")
            params.append(total_preview)

        if total_confirmed is not None:
            update_parts.append("total_confirmed = ?")
            params.append(total_confirmed)

        params.append(session_id)

        await conn.execute(f"UPDATE import_sessions SET {', '.join(update_parts)} WHERE session_id = ?", tuple(params))

        await conn.commit()
        self.logger.info(f"[导入会话更新成功] session_id={session_id}")
        return True

    @log_method
    async def get_import_session(self, session_id: str) -> dict[str, Any] | None:
        """
        获取导入会话信息

        Args:
            session_id: 会话唯一标识

        Returns:
            Optional[Dict]: 会话信息
        """
        conn = await self._get_connection()

        async with conn.execute("SELECT * FROM import_sessions WHERE session_id = ?", (session_id,)) as cursor:
            row = await cursor.fetchone()
            if row:
                return dict(row)
            return None

    @log_method
    async def insert_parser_templates(
        self, session_id: str, bills: list[dict[str, Any]], parser_id: str, user_id: int = 1
    ) -> int:
        """
        批量插入解析器模板数据（阶段1）

        Args:
            session_id: 会话唯一标识
            bills: 解析后的账单列表
            parser_id: 解析器标识（wechat/alipay/icbc等）
            user_id: 用户ID

        Returns:
            int: 成功插入的数量
        """
        if not bills:
            self.logger.warning("[插入解析模板] 账单列表为空")
            return 0

        conn = await self._get_connection()
        now = datetime.now().isoformat()
        inserted_count = 0

        self.logger.info(
            f"[插入解析模板] session={session_id}, parser={parser_id}, count={len(bills)}, user_id={user_id}"
        )

        # 分批插入
        for i in range(0, len(bills), self.batch_size):
            batch = bills[i : i + self.batch_size]
            self.logger.debug(f"[解析模板批次] {i // self.batch_size + 1}: {len(batch)} 条")

            for bill in batch:
                try:
                    # 根据金额正负确定类型
                    amount = float(bill.get("amount", 0))
                    if bill.get("type"):
                        bill_type = bill.get("type")
                    elif amount > 0:
                        bill_type = "收入"
                    elif amount < 0:
                        bill_type = "支出"
                    else:
                        bill_type = "其他"

                    await conn.execute(
                        """
                        INSERT INTO bills_parser_template (
                            session_id, user_id, parser_date, parser_amount,
                            parser_type, parser_description, parser_id,
                            parser_counterparty, parser_payment_method,
                            parser_original_type, parser_original_category,
                            parser_account_id, parser_is_processed, created_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            session_id,
                            user_id,
                            bill.get("date", ""),
                            amount,
                            bill_type,
                            bill.get("description", ""),
                            parser_id,
                            bill.get("counterparty", ""),
                            bill.get("payment_method", ""),
                            bill.get("original_type", ""),
                            bill.get("original_category", ""),
                            bill.get("account_id", ""),
                            "0",  # parser_is_processed默认为"0"
                            now,
                        ),
                    )
                    inserted_count += 1

                except Exception as e:
                    self.logger.error(f"[插入解析模板失败] {bill.get('date')} - {e}")

            await conn.commit()

        self.logger.info(f"[解析模板插入完成] session={session_id}, 成功={inserted_count}/{len(bills)}")
        return inserted_count

    @log_method
    async def get_parser_templates_by_session(
        self, session_id: str, processed_only: bool | None = None
    ) -> list[dict[str, Any]]:
        """
        获取会话的解析模板数据

        Args:
            session_id: 会话唯一标识
            processed_only: True只获取已处理，False只获取未处理，None获取全部

        Returns:
            List[Dict]: 解析模板列表
        """
        conn = await self._get_connection()

        query = "SELECT * FROM bills_parser_template WHERE session_id = ?"
        params = [session_id]

        if processed_only is True:
            query += " AND parser_is_processed = '1'"
        elif processed_only is False:
            query += " AND parser_is_processed = '0'"

        query += " ORDER BY parser_date ASC"

        self.logger.debug(f"[获取解析模板] session={session_id}, processed={processed_only}")

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
            templates = [dict(row) for row in rows]

        self.logger.info(f"[解析模板获取完成] session={session_id}, count={len(templates)}")
        return templates

    @log_method
    async def update_parser_template_status(
        self, template_ids: list[int], processed: bool = True, account_id: str | None = None
    ) -> int:
        """
        更新解析模板处理状态

        Args:
            template_ids: 模板ID列表
            processed: 是否已处理
            account_id: 匹配到的账户ID

        Returns:
            int: 更新的记录数
        """
        if not template_ids:
            return 0

        conn = await self._get_connection()

        processed_value = "1" if processed else "0"

        self.logger.info(f"[更新解析模板状态] ids={template_ids}, processed={processed}, account_id={account_id}")

        # 构建动态更新语句
        update_parts = ["parser_is_processed = ?"]
        params = [processed_value]

        if account_id is not None:
            update_parts.append("parser_account_id = ?")
            params.append(account_id)

        # 构建IN子句
        placeholders = ",".join(["?" for _ in template_ids])
        params.extend(template_ids)

        await conn.execute(
            f"UPDATE bills_parser_template SET {', '.join(update_parts)} WHERE id IN ({placeholders})", tuple(params)
        )

        await conn.commit()
        self.logger.info(f"[解析模板状态更新完成] count={len(template_ids)}")
        return len(template_ids)

    @log_method
    async def insert_preview_bill(
        self,
        session_id: str,
        preview_data: dict[str, Any],
        user_id: int = 1,
        dedup_type: str | None = None,
        dedup_source_ids: list[int] | None = None,
    ) -> int:
        """
        插入预览账单（阶段2）

        Args:
            session_id: 会话唯一标识
            preview_data: 预览数据
            user_id: 用户ID
            dedup_type: 去重类型（transfer/platform_bank/similar/split_merge/remaining）
            dedup_source_ids: 去重来源模板ID列表

        Returns:
            int: 预览账单ID
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()

        source_ids_str = ",".join(map(str, dedup_source_ids)) if dedup_source_ids else ""

        self.logger.debug(f"[插入预览账单] session={session_id}, type={dedup_type}, source_ids={source_ids_str}")

        cursor = await conn.execute(
            """
            INSERT INTO bills_preview (
                session_id, user_id, preview_date, preview_type,
                preview_amount, preview_destination_amount,
                preview_main_category, preview_sub_category,
                preview_source_account_id, preview_destination_account_id,
                preview_counterparty, preview_payment_method, preview_description,
                preview_recurring_id, preview_recurring_name,
                preview_recurring_candidate_count, preview_recurring_match_score,
                preview_recurring_match_reasons, preview_recurring_matched_date,
                preview_selected, dedup_type, dedup_source_ids, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                session_id,
                user_id,
                preview_data.get("preview_date", ""),
                preview_data.get("preview_type", ""),
                preview_data.get("preview_amount", 0),
                preview_data.get("preview_destination_amount", 0),
                preview_data.get("preview_main_category", ""),
                preview_data.get("preview_sub_category", ""),
                preview_data.get("preview_source_account_id"),
                preview_data.get("preview_destination_account_id"),
                preview_data.get("preview_counterparty", ""),
                preview_data.get("preview_payment_method", ""),
                preview_data.get("preview_description", ""),
                preview_data.get("preview_recurring_id"),
                preview_data.get("preview_recurring_name", ""),
                preview_data.get("preview_recurring_candidate_count", 0),
                preview_data.get("preview_recurring_match_score", 0),
                preview_data.get("preview_recurring_match_reasons", ""),
                preview_data.get("preview_recurring_matched_date", ""),
                1,  # preview_selected默认选中
                dedup_type,
                source_ids_str,
                now,
            ),
        )

        await conn.commit()
        preview_id = cursor.lastrowid

        self.logger.debug(f"[预览账单插入成功] id={preview_id}")
        return preview_id

    @log_method
    async def insert_preview_bills_batch(
        self, session_id: str, preview_list: list[dict[str, Any]], user_id: int = 1
    ) -> int:
        """
        批量插入预览账单

        Args:
            session_id: 会话唯一标识
            preview_list: 预览数据列表，每项包含preview_data, dedup_type, dedup_source_ids
            user_id: 用户ID

        Returns:
            int: 成功插入的数量
        """
        if not preview_list:
            return 0

        conn = await self._get_connection()
        now = datetime.now().isoformat()
        inserted_count = 0

        self.logger.info(f"[批量插入预览账单] session={session_id}, count={len(preview_list)}")

        for i in range(0, len(preview_list), self.batch_size):
            batch = preview_list[i : i + self.batch_size]

            for item in batch:
                try:
                    preview_data = item.get("preview_data", item)
                    dedup_type = item.get("dedup_type", "remaining")
                    dedup_source_ids = item.get("dedup_source_ids", [])
                    source_ids_str = ",".join(map(str, dedup_source_ids)) if dedup_source_ids else ""

                    await conn.execute(
                        """
                        INSERT INTO bills_preview (
                            session_id, user_id, preview_date, preview_type,
                            preview_amount, preview_destination_amount,
                            preview_main_category, preview_sub_category,
                            preview_source_account_id, preview_destination_account_id,
                            preview_counterparty, preview_payment_method, preview_description,
                            preview_parser_id,
                            preview_recurring_id, preview_recurring_name,
                            preview_recurring_candidate_count, preview_recurring_match_score,
                            preview_recurring_match_reasons, preview_recurring_matched_date,
                            preview_selected, dedup_type, dedup_source_ids, created_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        """,
                        (
                            session_id,
                            user_id,
                            preview_data.get("preview_date", ""),
                            preview_data.get("preview_type", ""),
                            preview_data.get("preview_amount", 0),
                            preview_data.get("preview_destination_amount", 0),
                            preview_data.get("preview_main_category", ""),
                            preview_data.get("preview_sub_category", ""),
                            preview_data.get("preview_source_account_id"),
                            preview_data.get("preview_destination_account_id"),
                            preview_data.get("preview_counterparty", ""),
                            preview_data.get("preview_payment_method", ""),
                            preview_data.get("preview_description", ""),
                            preview_data.get("preview_parser_id", ""),
                            preview_data.get("preview_recurring_id"),
                            preview_data.get("preview_recurring_name", ""),
                            preview_data.get("preview_recurring_candidate_count", 0),
                            preview_data.get("preview_recurring_match_score", 0),
                            preview_data.get("preview_recurring_match_reasons", ""),
                            preview_data.get("preview_recurring_matched_date", ""),
                            1,
                            dedup_type,
                            source_ids_str,
                            now,
                        ),
                    )
                    inserted_count += 1

                except Exception as e:
                    self.logger.error(f"[批量插入预览失败] {e}")

            await conn.commit()

        self.logger.info(f"[批量预览账单插入完成] session={session_id}, 成功={inserted_count}/{len(preview_list)}")
        return inserted_count

    @log_method
    async def get_preview_by_session(self, session_id: str, selected_only: bool = False) -> list[dict[str, Any]]:
        """
        获取会话的预览账单数据

        Args:
            session_id: 会话唯一标识
            selected_only: 是否只获取选中的账单

        Returns:
            List[Dict]: 预览账单列表
        """
        conn = await self._get_connection()

        query = "SELECT * FROM bills_preview WHERE session_id = ?"
        params = [session_id]

        if selected_only:
            query += " AND preview_selected = 1"

        query += " ORDER BY preview_date ASC"

        self.logger.debug(f"[获取预览账单] session={session_id}, selected_only={selected_only}")

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()
            previews = [dict(row) for row in rows]

        self.logger.info(f"[预览账单获取完成] session={session_id}, count={len(previews)}")
        return previews

    @log_method
    async def get_preview_bill_by_id(self, preview_id: int, user_id: int = 1) -> dict[str, Any] | None:
        """按ID获取单条预览账单。"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM bills_preview WHERE id = ? AND user_id = ?", (preview_id, user_id)
        ) as cursor:
            row = await cursor.fetchone()

        return dict(row) if row else None

    @log_method
    async def get_recurring_candidates_for_preview(
        self, preview_id: int, user_id: int = 1, tolerance_days: int = 3
    ) -> dict[str, Any]:
        """获取预览账单可匹配的定时交易候选。"""
        preview = await self.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return {"preview": None, "linked_recurring_id": None, "candidates": []}

        recurring_rows = await self.get_enabled_recurring_templates(user_id=user_id)
        bill_data = {
            "date": preview.get("preview_date"),
            "type": preview.get("preview_type"),
            "amount": preview.get("preview_amount"),
            "source_account_id": preview.get("preview_source_account_id"),
            "destination_account_id": preview.get("preview_destination_account_id"),
        }
        candidates = self.build_recurring_candidates_for_bill_data(
            bill_data,
            recurring_rows,
            linked_recurring_id=preview.get("preview_recurring_id"),
            tolerance_days=tolerance_days,
        )
        return {
            "preview": preview,
            "linked_recurring_id": preview.get("preview_recurring_id"),
            "candidates": candidates,
        }

    @log_method
    async def update_preview_selection(self, preview_ids: list[int], selected: bool) -> int:
        """
        更新预览账单选中状态

        Args:
            preview_ids: 预览账单ID列表
            selected: 是否选中

        Returns:
            int: 更新的记录数
        """
        if not preview_ids:
            return 0

        conn = await self._get_connection()
        selected_value = 1 if selected else 0

        self.logger.info(f"[更新预览选中状态] ids={preview_ids}, selected={selected}")

        placeholders = ",".join(["?" for _ in preview_ids])
        await conn.execute(
            f"UPDATE bills_preview SET preview_selected = ? WHERE id IN ({placeholders})",
            tuple([selected_value] + preview_ids),
        )

        await conn.commit()
        self.logger.info(f"[预览选中状态更新完成] count={len(preview_ids)}")
        return len(preview_ids)

    @log_method
    async def reset_session_preview_selection(self, session_id: str) -> int:
        """
        重置会话中所有预览账单的选中状态为未选中

        在阶段3确认导入前调用，确保只有前端传入的选中账单会被标记为选中。

        Args:
            session_id: 会话唯一标识

        Returns:
            int: 更新的记录数
        """
        conn = await self._get_connection()

        self.logger.info(f"[重置会话预览选中状态] session={session_id}")

        cursor = await conn.execute("UPDATE bills_preview SET preview_selected = 0 WHERE session_id = ?", (session_id,))
        updated_count = cursor.rowcount

        await conn.commit()
        self.logger.info(f"[会话预览选中状态重置完成] session={session_id}, count={updated_count}")
        return updated_count

    @log_method
    async def update_preview_bill(self, preview_id: int, update_data: dict[str, Any]) -> bool:
        """
        更新预览账单数据

        Args:
            preview_id: 预览账单ID
            update_data: 更新数据字典

        Returns:
            bool: 是否成功
        """
        if not update_data:
            return False

        conn = await self._get_connection()

        # 构建动态更新语句
        update_parts = []
        params = []

        field_mapping = {
            "preview_date": "preview_date",
            "preview_type": "preview_type",
            "preview_amount": "preview_amount",
            "preview_destination_amount": "preview_destination_amount",
            "preview_main_category": "preview_main_category",
            "preview_sub_category": "preview_sub_category",
            "preview_source_account_id": "preview_source_account_id",
            "preview_destination_account_id": "preview_destination_account_id",
            "preview_counterparty": "preview_counterparty",
            "preview_payment_method": "preview_payment_method",
            "preview_description": "preview_description",
            "preview_recurring_id": "preview_recurring_id",
            "preview_recurring_name": "preview_recurring_name",
            "preview_recurring_candidate_count": "preview_recurring_candidate_count",
            "preview_recurring_match_score": "preview_recurring_match_score",
            "preview_recurring_match_reasons": "preview_recurring_match_reasons",
            "preview_recurring_matched_date": "preview_recurring_matched_date",
            "preview_selected": "preview_selected",
        }

        for key, column in field_mapping.items():
            if key in update_data:
                update_parts.append(f"{column} = ?")
                params.append(update_data[key])

        if not update_parts:
            return False

        params.append(preview_id)

        self.logger.debug(f"[更新预览账单] id={preview_id}, fields={list(update_data.keys())}")

        await conn.execute(f"UPDATE bills_preview SET {', '.join(update_parts)} WHERE id = ?", tuple(params))

        await conn.commit()
        self.logger.info(f"[预览账单更新成功] id={preview_id}")
        return True

    @log_method
    async def update_preview_bills_batch(self, session_id: str, updates: list[dict[str, Any]], user_id: int = 1) -> int:
        """
        批量更新预览账单数据（阶段3用户编辑后保存）

        Args:
            session_id: 会话唯一标识
            updates: 更新数据列表，每项包含id和需要更新的字段
            user_id: 用户ID

        Returns:
            int: 成功更新的数量
        """
        if not updates:
            return 0

        conn = await self._get_connection()
        updated_count = 0

        self.logger.info(f"[批量更新预览] session={session_id}, count={len(updates)}, user_id={user_id}")

        for update_item in updates:
            try:
                preview_id = update_item.get("id")
                if not preview_id:
                    continue

                # 构建动态更新语句
                update_parts = []
                params = []

                field_mapping = {
                    "preview_type": "preview_type",
                    "preview_amount": "preview_amount",
                    "preview_destination_amount": "preview_destination_amount",
                    "preview_source_account_id": "preview_source_account_id",
                    "preview_destination_account_id": "preview_destination_account_id",
                    "preview_recurring_id": "preview_recurring_id",
                    "preview_recurring_name": "preview_recurring_name",
                    "preview_recurring_candidate_count": "preview_recurring_candidate_count",
                    "preview_recurring_match_score": "preview_recurring_match_score",
                    "preview_recurring_match_reasons": "preview_recurring_match_reasons",
                    "preview_recurring_matched_date": "preview_recurring_matched_date",
                    "category_id": None,  # 需要特殊处理
                    "selected": "preview_selected",
                }

                for key, column in field_mapping.items():
                    if key not in update_item:
                        continue

                    value = update_item[key]

                    if key == "category_id":
                        # 如果提供了category_id，需要查询对应的分类名称
                        cat_id = value
                        if cat_id:
                            category = await self.get_category_by_id(cat_id)
                            if category:
                                update_parts.append("preview_main_category = ?")
                                params.append(category.get("main_category", ""))
                                update_parts.append("preview_sub_category = ?")
                                params.append(category.get("sub_category", ""))
                    elif key == "selected":
                        update_parts.append("preview_selected = ?")
                        params.append(1 if value else 0)
                    else:
                        update_parts.append(f"{column} = ?")
                        params.append(value)

                if update_parts:
                    params.append(preview_id)
                    await conn.execute(
                        f"UPDATE bills_preview SET {', '.join(update_parts)} WHERE id = ?", tuple(params)
                    )
                    updated_count += 1

            except Exception as e:
                self.logger.error(f"[批量更新预览失败] id={update_item.get('id')}, error={e}")

        await conn.commit()
        self.logger.info(f"[批量更新预览完成] updated={updated_count}/{len(updates)}")
        return updated_count

    @log_method
    async def confirm_preview_to_bills(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        """
        将选中的预览账单确认写入正式账单表（阶段3）

        Args:
            session_id: 会话唯一标识
            user_id: 用户ID

        Returns:
            Dict: 确认结果，包含confirmed_count, skipped_count, errors
        """
        conn = await self._get_connection()
        now = datetime.now().isoformat()
        batch_id = datetime.now().strftime("%Y%m%d%H%M%S")

        self.logger.info(f"[确认预览账单] session={session_id}, user_id={user_id}, batch_id={batch_id}")

        result = {"confirmed_count": 0, "skipped_count": 0, "duplicate_count": 0, "errors": []}
        recurring_advances: dict[int, date] = {}

        # 获取选中的预览账单
        previews = await self.get_preview_by_session(session_id, selected_only=True)

        self.logger.info(f"[确认预览账单] 选中 {len(previews)} 条待确认")

        for preview in previews:
            try:
                # 准备账单数据
                bill_type = preview.get("preview_type", "")
                amount = abs(float(preview.get("preview_amount", 0)))

                # 根据类型确定金额符号
                if bill_type in ["支出", "expense"]:
                    amount = -abs(amount)
                elif bill_type in ["收入", "income"]:
                    amount = abs(amount)
                elif bill_type in ["转账", "transfer", "投资", "investment"]:
                    amount = abs(amount)

                # 计算哈希用于去重
                bill_data = {
                    "date": preview.get("preview_date", ""),
                    "type": bill_type,
                    "amount": amount,
                    "counterparty": preview.get("preview_counterparty", ""),
                    "description": preview.get("preview_description", ""),
                }
                bill_hash = self._calculate_hash(bill_data)

                # 插入正式账单表
                await conn.execute(
                    """
                    INSERT INTO bills (
                        user_id, date, type, amount, counterparty, description,
                        payment_method, main_category, sub_category,
                        source_account_id, destination_account_id, destination_amount,
                        batch_id, hash, created_from_recurring, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        user_id,
                        preview.get("preview_date", ""),
                        bill_type,
                        amount,
                        preview.get("preview_counterparty", ""),
                        preview.get("preview_description", ""),
                        preview.get("preview_payment_method", ""),
                        preview.get("preview_main_category", ""),
                        preview.get("preview_sub_category", ""),
                        preview.get("preview_source_account_id"),
                        preview.get("preview_destination_account_id"),
                        preview.get("preview_destination_amount", 0),
                        batch_id,
                        bill_hash,
                        preview.get("preview_recurring_id"),
                        now,
                        now,
                    ),
                )
                result["confirmed_count"] += 1

                recurring_id = preview.get("preview_recurring_id")
                preview_date = self._parse_date_value(preview.get("preview_date"))
                if recurring_id and preview_date:
                    recurring_id_int = int(recurring_id)
                    recorded_date = recurring_advances.get(recurring_id_int)
                    if recorded_date is None or preview_date > recorded_date:
                        recurring_advances[recurring_id_int] = preview_date

            except sqlite3.IntegrityError:
                # 哈希重复，跳过
                result["duplicate_count"] += 1
                self.logger.debug(f"[确认跳过重复] date={preview.get('preview_date')}")

            except Exception as e:
                result["errors"].append(str(e))
                self.logger.error(f"[确认失败] {e}")

        for recurring_id, matched_date in recurring_advances.items():
            async with conn.execute(
                "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?", (recurring_id, user_id)
            ) as cursor:
                recurring_row = await cursor.fetchone()

            recurring = dict(recurring_row) if recurring_row else None
            if not recurring:
                self.logger.warning("[确认推进定时账单] 未找到 recurring_id=%s, 跳过推进", recurring_id)
                continue

            next_date = self._get_next_recurring_occurrence_after(recurring, matched_date)
            next_occurrence = next_date.isoformat() if next_date else recurring.get("next_date")

            await conn.execute(
                "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                (next_occurrence, now, recurring_id, user_id),
            )
            self.logger.info(
                "[确认推进定时账单] recurring_id=%s, matched_date=%s, next_date=%s",
                recurring_id,
                matched_date.isoformat(),
                next_occurrence,
            )

        await conn.commit()

        # 更新会话状态
        await self.update_import_session_status(session_id, "completed", total_confirmed=result["confirmed_count"])

        self.logger.info(
            f"[确认完成] session={session_id}, confirmed={result['confirmed_count']}, "
            f"duplicate={result['duplicate_count']}, errors={len(result['errors'])}"
        )

        return result

    @log_method
    async def clear_session_data(self, session_id: str, user_id: int = None) -> dict[str, int]:
        """
        清空会话相关的临时数据

        Args:
            session_id: 会话唯一标识
            user_id: 用户ID (可选，用于安全验证)

        Returns:
            Dict: 删除统计，包含parser_count, preview_count
        """
        conn = await self._get_connection()

        self.logger.info(f"[清空会话数据] session={session_id}, user_id={user_id}")

        result = {"parser_count": 0, "preview_count": 0, "annotation_count": 0}

        # 删除解析模板数据
        cursor = await conn.execute("DELETE FROM bills_parser_template WHERE session_id = ?", (session_id,))
        result["parser_count"] = cursor.rowcount

        # 删除预览数据
        cursor = await conn.execute("DELETE FROM bills_preview WHERE session_id = ?", (session_id,))
        result["preview_count"] = cursor.rowcount

        cursor = await conn.execute("DELETE FROM import_annotation_samples WHERE session_id = ?", (session_id,))
        result["annotation_count"] = cursor.rowcount

        await conn.commit()

        self.logger.info(
            f"[会话数据清空完成] session={session_id}, "
            f"parser={result['parser_count']}, preview={result['preview_count']}, "
            f"annotation={result['annotation_count']}"
        )

        return result

    @log_method
    async def get_unprocessed_templates_for_dedup(self, session_id: str) -> list[dict[str, Any]]:
        """
        获取未处理的解析模板用于去重

        按日期排序，便于时间窗口匹配

        Args:
            session_id: 会话唯一标识

        Returns:
            List[Dict]: 未处理的模板列表
        """
        conn = await self._get_connection()

        self.logger.debug(f"[获取未处理模板] session={session_id}")

        async with conn.execute(
            """
            SELECT * FROM bills_parser_template
            WHERE session_id = ? AND parser_is_processed = '0'
            ORDER BY parser_date ASC, id ASC
            """,
            (session_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            templates = [dict(row) for row in rows]

        self.logger.info(f"[未处理模板] session={session_id}, count={len(templates)}")
        return templates

    @log_method
    async def get_existing_bills_for_dedup(self, user_id: int, start_date: str, end_date: str) -> list[dict[str, Any]]:
        """
        获取指定日期范围内的已有账单用于数据库去重

        Args:
            user_id: 用户ID
            start_date: 开始日期 (YYYY-MM-DD)
            end_date: 结束日期 (YYYY-MM-DD)

        Returns:
            List[Dict]: 已有账单列表
        """
        conn = await self._get_connection()

        self.logger.debug(f"[获取已有账单] user_id={user_id}, range={start_date} ~ {end_date}")

        async with conn.execute(
            """
            SELECT id, date, type, amount, counterparty, description,
                   payment_method, main_category, sub_category,
                   source_account_id, destination_account_id
            FROM bills
            WHERE user_id = ?
              AND date >= ?
              AND date <= ?
            ORDER BY date ASC
            """,
            (user_id, start_date, end_date + " 23:59:59"),
        ) as cursor:
            rows = await cursor.fetchall()
            bills = [dict(row) for row in rows]

        self.logger.info(f"[已有账单] user_id={user_id}, count={len(bills)}")
        return bills

    @log_method
    async def save_import_annotation_samples(
        self, session_id: str, samples: list[dict[str, Any]], user_id: int = 1
    ) -> int:
        """保存当前导入会话中的人工标注样本。"""
        if not samples:
            return 0

        conn = await self._get_connection()
        now = datetime.now().isoformat()
        saved_count = 0

        for sample in samples:
            preview_id = sample.get("preview_id") or sample.get("id")
            if not preview_id:
                continue

            await conn.execute(
                """
                INSERT INTO import_annotation_samples (
                    session_id, user_id, preview_id,
                    annotated_type, annotated_category_id,
                    annotated_source_account_id, annotated_destination_account_id,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id, preview_id) DO UPDATE SET
                    annotated_type = excluded.annotated_type,
                    annotated_category_id = excluded.annotated_category_id,
                    annotated_source_account_id = excluded.annotated_source_account_id,
                    annotated_destination_account_id = excluded.annotated_destination_account_id,
                    updated_at = excluded.updated_at
                """,
                (
                    session_id,
                    user_id,
                    preview_id,
                    sample.get("preview_type") or sample.get("annotated_type"),
                    sample.get("category_id") or sample.get("annotated_category_id"),
                    sample.get("preview_source_account_id") or sample.get("annotated_source_account_id"),
                    sample.get("preview_destination_account_id") or sample.get("annotated_destination_account_id"),
                    now,
                    now,
                ),
            )
            saved_count += 1

        await conn.commit()
        self.logger.info("[保存会话标注样本] session=%s, count=%d, user_id=%d", session_id, saved_count, user_id)
        return saved_count

    @log_method
    async def get_import_annotation_samples(self, session_id: str, user_id: int = 1) -> list[dict[str, Any]]:
        """读取当前导入会话中的人工标注样本。"""
        conn = await self._get_connection()

        async with conn.execute(
            """
            SELECT * FROM import_annotation_samples
            WHERE session_id = ? AND user_id = ?
            ORDER BY updated_at ASC, id ASC
            """,
            (session_id, user_id),
        ) as cursor:
            rows = await cursor.fetchall()

        samples = [dict(row) for row in rows]
        self.logger.info("[读取会话标注样本] session=%s, count=%d, user_id=%d", session_id, len(samples), user_id)
        return samples

    @log_method
    async def promote_import_annotation_samples_to_learning(
        self, session_id: str, preview_ids: list[int] | None = None, user_id: int = 1
    ) -> dict[str, int]:
        """将当前会话标注样本提升为长期导入学习规则。"""
        previews = await self.get_preview_by_session(session_id)
        preview_map = {int(preview["id"]): preview for preview in previews if preview.get("id")}
        samples = await self.get_import_annotation_samples(session_id, user_id=user_id)

        selected_preview_ids = {int(pid) for pid in (preview_ids or []) if pid}
        if selected_preview_ids:
            samples = [sample for sample in samples if int(sample.get("preview_id", 0) or 0) in selected_preview_ids]

        if not samples:
            return {
                "selected_samples": 0,
                "rules_total": 0,
                "created": 0,
                "updated": 0,
            }

        pending_rules: dict[tuple, dict[str, Any]] = {}

        for sample in samples:
            preview_id = int(sample.get("preview_id", 0) or 0)
            preview = preview_map.get(preview_id)
            if not preview:
                continue

            learned_type = sample.get("annotated_type") or preview.get("preview_type")
            learned_category_id = sample.get("annotated_category_id")
            learned_source_account_id = sample.get("annotated_source_account_id")
            learned_destination_account_id = sample.get("annotated_destination_account_id")

            # 仅保留复合匹配规则，避免生成过于宽松的单字段长期学习。
            composite_hash = self.build_composite_match_hash(
                parser_id=preview.get("preview_parser_id", ""),
                counterparty=preview.get("preview_counterparty", ""),
                description=preview.get("preview_description", ""),
                payment_method=preview.get("preview_payment_method", ""),
            )
            match_features = self.build_composite_match_features(
                parser_id=preview.get("preview_parser_id", ""),
                counterparty=preview.get("preview_counterparty", ""),
                description=preview.get("preview_description", ""),
                payment_method=preview.get("preview_payment_method", ""),
            )
            if composite_hash and match_features:
                pending_rules[("composite", composite_hash)] = {
                    "match_type": "composite",
                    "match_value": composite_hash,
                    "normalized_match_value": composite_hash,
                    "learned_type": learned_type,
                    "learned_category_id": learned_category_id,
                    "learned_source_account_id": learned_source_account_id,
                    "learned_destination_account_id": learned_destination_account_id,
                    "source_session_id": session_id,
                    "source_preview_id": preview_id,
                    "parser_id": preview.get("preview_parser_id", ""),
                    "composite_match_hash": composite_hash,
                    "match_features_json": json.dumps(match_features, ensure_ascii=False, sort_keys=True),
                }

        if not pending_rules:
            return {
                "selected_samples": len(samples),
                "rules_total": 0,
                "created": 0,
                "updated": 0,
            }

        conn = await self._get_connection()
        now = datetime.now().isoformat()
        created_count = 0
        updated_count = 0

        for rule_data in pending_rules.values():
            async with conn.execute(
                """
                SELECT id FROM import_learning_rules
                WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                LIMIT 1
                """,
                (
                    user_id,
                    rule_data["match_type"],
                    rule_data["normalized_match_value"],
                ),
            ) as cursor:
                existing = await cursor.fetchone()

            await conn.execute(
                """
                INSERT INTO import_learning_rules (
                    user_id, match_type, match_value, normalized_match_value,
                    learned_type, learned_category_id,
                    learned_source_account_id, learned_destination_account_id,
                    enabled, source_session_id, source_preview_id,
                    parser_id, composite_match_hash, match_features_json,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    match_value = excluded.match_value,
                    learned_type = excluded.learned_type,
                    learned_category_id = excluded.learned_category_id,
                    learned_source_account_id = excluded.learned_source_account_id,
                    learned_destination_account_id = excluded.learned_destination_account_id,
                    enabled = 1,
                    source_session_id = excluded.source_session_id,
                    source_preview_id = excluded.source_preview_id,
                    parser_id = excluded.parser_id,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    updated_at = excluded.updated_at
                """,
                (
                    user_id,
                    rule_data["match_type"],
                    rule_data["match_value"],
                    rule_data["normalized_match_value"],
                    rule_data["learned_type"],
                    rule_data["learned_category_id"],
                    rule_data["learned_source_account_id"],
                    rule_data["learned_destination_account_id"],
                    rule_data["source_session_id"],
                    rule_data["source_preview_id"],
                    rule_data.get("parser_id"),
                    rule_data.get("composite_match_hash"),
                    rule_data.get("match_features_json"),
                    now,
                    now,
                ),
            )

            async with conn.execute(
                """
                SELECT id FROM import_learning_rules
                WHERE user_id = ? AND match_type = ? AND normalized_match_value = ?
                LIMIT 1
                """,
                (
                    user_id,
                    rule_data["match_type"],
                    rule_data["normalized_match_value"],
                ),
            ) as cursor:
                saved_row = await cursor.fetchone()

            rule_id = int(saved_row["id"]) if saved_row else None
            action = "updated" if existing else "created"
            if existing:
                updated_count += 1
            else:
                created_count += 1

            await self._record_import_learning_rule_log(
                conn,
                rule_id=rule_id,
                user_id=user_id,
                action=action,
                match_type=rule_data["match_type"],
                match_value=rule_data["match_value"],
                normalized_match_value=rule_data["normalized_match_value"],
                session_id=session_id,
                preview_id=rule_data["source_preview_id"],
                payload=rule_data,
            )

        await conn.commit()

        self.logger.info(
            "[长期学习提升] session=%s, selected_samples=%d, rules_total=%d, created=%d, updated=%d",
            session_id,
            len(samples),
            len(pending_rules),
            created_count,
            updated_count,
        )

        return {
            "selected_samples": len(samples),
            "rules_total": len(pending_rules),
            "created": created_count,
            "updated": updated_count,
        }

    @log_method
    async def get_import_learning_rules(
        self, user_id: int = 1, enabled_only: bool = False, limit: int | None = 200, offset: int = 0
    ) -> list[dict[str, Any]]:
        """获取导入学习规则列表。"""
        conn = await self._get_connection()
        query = "SELECT * FROM import_learning_rules WHERE user_id = ?"
        params: list[Any] = [user_id]

        if enabled_only:
            query += " AND enabled = 1"

        query += " ORDER BY updated_at DESC, id DESC"

        if limit is not None and limit > 0:
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, max(offset, 0)])

        async with conn.execute(query, tuple(params)) as cursor:
            rows = await cursor.fetchall()

        return [dict(row) for row in rows]

    @log_method
    async def count_import_learning_rules(self, user_id: int = 1, enabled_only: bool = False) -> int:
        """统计导入学习规则总数。"""
        conn = await self._get_connection()
        query = "SELECT COUNT(*) AS total_count FROM import_learning_rules WHERE user_id = ?"
        params: list[Any] = [user_id]

        if enabled_only:
            query += " AND enabled = 1"

        async with conn.execute(query, tuple(params)) as cursor:
            row = await cursor.fetchone()

        if not row:
            return 0

        return int(row["total_count"] or 0)

    @log_method
    async def set_import_learning_rule_enabled(self, rule_id: int, enabled: bool, user_id: int = 1) -> bool:
        """启用或禁用导入学习规则。"""
        conn = await self._get_connection()
        enabled_value = 1 if enabled else 0
        now = datetime.now().isoformat()

        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1", (rule_id, user_id)
        ) as cursor:
            existing = await cursor.fetchone()

        if not existing:
            return False

        await conn.execute(
            """
            UPDATE import_learning_rules
            SET enabled = ?, updated_at = ?
            WHERE id = ? AND user_id = ?
            """,
            (enabled_value, now, rule_id, user_id),
        )

        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="enabled" if enabled else "disabled",
            match_type=existing["match_type"],
            match_value=existing["match_value"],
            normalized_match_value=existing["normalized_match_value"],
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload={"enabled": enabled_value},
        )

        await conn.commit()
        return True

    @log_method
    async def delete_import_learning_rule(self, rule_id: int, user_id: int = 1) -> bool:
        """删除导入学习规则。"""
        conn = await self._get_connection()

        async with conn.execute(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1", (rule_id, user_id)
        ) as cursor:
            existing = await cursor.fetchone()

        if not existing:
            return False

        await self._record_import_learning_rule_log(
            conn,
            rule_id=rule_id,
            user_id=user_id,
            action="deleted",
            match_type=existing["match_type"],
            match_value=existing["match_value"],
            normalized_match_value=existing["normalized_match_value"],
            session_id=existing["source_session_id"],
            preview_id=existing["source_preview_id"],
            payload=dict(existing),
        )

        await conn.execute("DELETE FROM import_learning_rules WHERE id = ? AND user_id = ?", (rule_id, user_id))
        await conn.commit()
        return True

    @log_method
    async def increment_import_learning_rule_usage(self, rule_ids: list[int], user_id: int = 1) -> int:
        """批量增加导入学习规则命中次数。"""
        unique_ids = [int(rule_id) for rule_id in sorted(set(rule_ids)) if rule_id]
        if not unique_ids:
            return 0

        conn = await self._get_connection()
        now = datetime.now().isoformat()
        placeholders = ",".join(["?" for _ in unique_ids])
        cursor = await conn.execute(
            f"""
            UPDATE import_learning_rules
            SET applied_count = applied_count + 1,
                last_applied_at = ?,
                updated_at = ?
            WHERE user_id = ? AND id IN ({placeholders})
            """,
            tuple([now, now, user_id] + unique_ids),
        )
        await conn.commit()
        return cursor.rowcount

    @log_method
    async def batch_update_preview_classification(self, updates: list[dict[str, Any]]) -> int:
        """
        v6.55: 批量更新预览账单的分类和账户信息

        用于重新分类功能，仅更新分类和账户字段

        Args:
            updates: 更新数据列表，每项包含:
                - id: 预览记录ID
                - preview_type: 预览类型
                - preview_main_category: 主分类
                - preview_sub_category: 子分类
                - preview_source_account_id: 源账户ID
                - preview_destination_account_id: 目标账户ID

        Returns:
            int: 成功更新的数量
        """
        if not updates:
            return 0

        conn = await self._get_connection()
        updated_count = 0

        self.logger.info(f"[重新分类-批量更新] count={len(updates)}")

        for update_item in updates:
            try:
                preview_id = update_item.get("id")
                if not preview_id:
                    continue

                await conn.execute(
                    """
                    UPDATE bills_preview SET
                        preview_type = ?,
                        preview_main_category = ?,
                        preview_sub_category = ?,
                        preview_source_account_id = ?,
                        preview_destination_account_id = ?
                    WHERE id = ?
                    """,
                    (
                        update_item.get("preview_type", ""),
                        update_item.get("preview_main_category", ""),
                        update_item.get("preview_sub_category", ""),
                        update_item.get("preview_source_account_id"),
                        update_item.get("preview_destination_account_id"),
                        preview_id,
                    ),
                )
                updated_count += 1

            except Exception as e:
                self.logger.error(f"[重新分类-更新失败] id={update_item.get('id')}, error={e}")

        await conn.commit()
        self.logger.info(f"[重新分类-批量更新完成] updated={updated_count}/{len(updates)}")
        return updated_count

    # ==================== v6.88: ML 训练数据 ====================

    @log_method
    async def get_ml_training_data(self, user_id: int = 1) -> list[dict[str, Any]]:
        """获取用于ML分类器训练的已分类账单数据

        只返回有主分类的账单，用于训练文本分类模型。

        Args:
            user_id: 用户ID

        Returns:
            List[Dict]: 训练数据列表，每条包含:
                - counterparty: 交易对方
                - description: 描述
                - main_category: 主分类
                - sub_category: 子分类
        """
        conn = await self._get_connection()

        query = """
            SELECT counterparty, description, main_category, sub_category
            FROM bills
            WHERE user_id = ?
              AND main_category IS NOT NULL
              AND main_category != ''
            ORDER BY date DESC
        """

        async with conn.execute(query, (user_id,)) as cursor:
            rows = await cursor.fetchall()
            result = [dict(row) for row in rows]
            self.logger.info("[ML训练数据] user_id=%d, 获取 %d 条已分类账单", user_id, len(result))
            return result
