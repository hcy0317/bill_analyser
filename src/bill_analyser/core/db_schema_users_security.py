"""User, security, audit, and backup schema helpers."""

# pylint: disable=line-too-long,wrong-import-position

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite

from .db_shared import DatabaseFacadeBase


class DatabaseSchemaUsersSecurityMixin(DatabaseFacadeBase):
    """User, session, auth, settings, audit, and backup schema helpers."""

    async def _init_users_security_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

        await conn.execute(
            """
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
            """
        )

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
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_logs_type ON audit_logs(operation_type, created_at)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_logs_target "
            "ON audit_logs(operation_target, target_id)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at DESC)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_audit_logs_status ON audit_logs(status)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_app_settings_key ON app_settings(key)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_backup_records_status_created ON backup_records(status, created_at DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_backup_jobs_type_enabled ON backup_jobs(job_type, enabled)")

    async def _migrate_users_cash_fields(self, conn: aiosqlite.Connection) -> None:
        """为 users 表补齐现金存取相关字段。"""
        async with conn.execute("PRAGMA table_info(users)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
        if "cash_account_id" not in columns:
            self.logger.info("为 users 表添加 cash_account_id 字段")
            await conn.execute("ALTER TABLE users ADD COLUMN cash_account_id INTEGER")
        if "cash_transfer_category_id" not in columns:
            self.logger.info("为 users 表添加 cash_transfer_category_id 字段")
            await conn.execute("ALTER TABLE users ADD COLUMN cash_transfer_category_id INTEGER")

    async def _migrate_users_import_learning_fields(self, conn: aiosqlite.Connection) -> None:
        """为 users 表补齐长期导入学习开关字段。"""
        async with conn.execute("PRAGMA table_info(users)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]
        if "import_learning_enabled" in columns:
            return

        self.logger.info("为 users 表添加 import_learning_enabled 字段")
        await conn.execute("ALTER TABLE users ADD COLUMN import_learning_enabled BOOLEAN DEFAULT 1")
        await conn.execute("UPDATE users SET import_learning_enabled = 1 WHERE import_learning_enabled IS NULL")

    async def _migrate_users_investment_keyword_fields(self, conn: aiosqlite.Connection) -> None:
        """为 users 表补齐投资识别关键词配置字段。"""
        async with conn.execute("PRAGMA table_info(users)") as cursor:
            columns = [row[1] for row in await cursor.fetchall()]

        for field_name in [
            "investment_platform_keywords",
            "investment_product_keywords",
            "investment_exclude_keywords",
        ]:
            if field_name in columns:
                continue
            self.logger.info("为 users 表添加 %s 字段", field_name)
            await conn.execute(f"ALTER TABLE users ADD COLUMN {field_name} TEXT")
