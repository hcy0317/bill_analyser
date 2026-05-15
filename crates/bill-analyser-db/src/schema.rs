use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::{DbError, DbResult, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaResponsibility {
    pub legacy_area: &'static str,
    pub rust_mapping: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaInventory {
    pub responsibilities: Vec<SchemaResponsibility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDryRunReport {
    pub source_path: String,
    pub copy_path: String,
    pub existing_tables: Vec<String>,
    pub foreign_key_violations: usize,
    pub pragma_snapshot: crate::PragmaSnapshot,
}

pub struct SchemaDryRun;

impl SchemaDryRun {
    pub fn copy_and_validate(
        source_path: impl AsRef<Path>,
        copy_path: impl AsRef<Path>,
    ) -> DbResult<SchemaDryRunReport> {
        let source = SqliteDbPath::temporary_file(source_path)?;
        let copy = SqliteDbPath::temporary_file(copy_path)?;

        copy_database_files(source.as_path(), copy.as_path())?;

        let runtime = SqliteRuntime::open(SqliteConnectionConfig {
            path: copy.clone(),
            create_if_missing: false,
            busy_timeout: std::time::Duration::from_secs(1),
        })?;
        let pragma_snapshot = runtime.pragma_snapshot()?;
        let existing_tables = list_tables(runtime.connection())?;
        let foreign_key_violations = count_foreign_key_violations(runtime.connection())?;

        Ok(SchemaDryRunReport {
            source_path: source.as_path().display().to_string(),
            copy_path: copy.as_path().display().to_string(),
            existing_tables,
            foreign_key_violations,
            pragma_snapshot,
        })
    }
}

pub fn init_foundational_schema(connection: &Connection) -> DbResult<()> {
    create_core_tables(connection)?;
    migrate_core_legacy_columns(connection)?;
    migrate_core_user_scope_constraints(connection)?;
    create_core_indexes(connection)?;
    Ok(())
}

pub fn init_auth_security_schema(connection: &Connection) -> DbResult<()> {
    create_auth_security_tables(connection)?;
    migrate_legacy_user_security_columns(connection)?;
    create_auth_security_indexes(connection)?;
    Ok(())
}

pub fn migrate_core_user_scope_constraints(connection: &Connection) -> DbResult<()> {
    for table_name in USER_SCOPED_TABLES {
        migrate_user_id_field(connection, table_name)?;
    }
    migrate_bills_hash_unique_constraint(connection)?;
    migrate_categories_unique_constraint(connection)?;
    migrate_user_exchange_rates_unique_constraint(connection)?;
    Ok(())
}

pub fn migrate_user_id_field(connection: &Connection, table_name: &str) -> DbResult<()> {
    ensure_safe_identifier(table_name)?;
    if !table_exists(connection, table_name)? {
        return Ok(());
    }

    let columns = table_column_names(connection, table_name)?;
    if columns.iter().any(|column| column == "user_id") {
        return Ok(());
    }

    connection.execute(
        &format!("ALTER TABLE {table_name} ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1"),
        [],
    )?;
    connection.execute(
        &format!("CREATE INDEX IF NOT EXISTS idx_{table_name}_user_id ON {table_name}(user_id)"),
        [],
    )?;
    Ok(())
}

pub fn migrate_categories_unique_constraint(connection: &Connection) -> DbResult<()> {
    if !table_exists(connection, "categories")?
        || !unique_index_exists_on(connection, "categories", &["main_category", "sub_category"])?
    {
        return Ok(());
    }
    migrate_user_id_field(connection, "categories")?;
    let column_sql = copy_columns_for_rebuild(connection, "categories", true)?;

    run_schema_rebuild_with_fk_guard(connection, |connection| {
        connection.execute_batch(
            "
        DROP TABLE IF EXISTS categories_new;
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
        );
        ",
        )?;
        connection.execute(
            &format!(
                "INSERT OR IGNORE INTO categories_new ({column_sql}) SELECT {column_sql} FROM categories"
            ),
            [],
        )?;
        connection.execute_batch(
            "
        DROP TABLE categories;
        ALTER TABLE categories_new RENAME TO categories;
        CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id);
        ",
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn migrate_bills_hash_unique_constraint(connection: &Connection) -> DbResult<()> {
    if !table_exists(connection, "bills")?
        || !unique_index_exists_on(connection, "bills", &["hash"])?
    {
        return Ok(());
    }
    migrate_user_id_field(connection, "bills")?;
    let column_sql = copy_columns_for_rebuild(connection, "bills", false)?;

    run_schema_rebuild_with_fk_guard(connection, |connection| {
        connection.execute_batch(
            "
        DROP TABLE IF EXISTS bills_new;
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
        );
        ",
        )?;
        connection.execute(
            &format!("INSERT INTO bills_new ({column_sql}) SELECT {column_sql} FROM bills"),
            [],
        )?;
        connection.execute_batch(
            "
        DROP TABLE bills;
        ALTER TABLE bills_new RENAME TO bills;
        CREATE INDEX IF NOT EXISTS idx_bills_user ON bills(user_id);
        CREATE INDEX IF NOT EXISTS idx_bills_date ON bills(date);
        CREATE INDEX IF NOT EXISTS idx_bills_type ON bills(type);
        CREATE INDEX IF NOT EXISTS idx_bills_category ON bills(main_category, sub_category);
        CREATE INDEX IF NOT EXISTS idx_bills_batch ON bills(batch_id);
        CREATE INDEX IF NOT EXISTS idx_bills_hash ON bills(hash);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash);
        ",
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn migrate_user_exchange_rates_unique_constraint(connection: &Connection) -> DbResult<()> {
    if !table_exists(connection, "user_exchange_rates")?
        || !unique_index_exists_on(
            connection,
            "user_exchange_rates",
            &["from_currency", "to_currency", "effective_date"],
        )?
    {
        return Ok(());
    }
    migrate_user_id_field(connection, "user_exchange_rates")?;
    let column_sql = copy_columns_for_rebuild(connection, "user_exchange_rates", true)?;

    run_schema_rebuild_with_fk_guard(connection, |connection| {
        connection.execute_batch(
            "
        DROP TABLE IF EXISTS user_exchange_rates_new;
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
        );
        ",
        )?;
        connection.execute(
            &format!(
                "INSERT OR IGNORE INTO user_exchange_rates_new ({column_sql}) SELECT {column_sql} FROM user_exchange_rates"
            ),
            [],
        )?;
        connection.execute_batch(
            "
        DROP TABLE user_exchange_rates;
        ALTER TABLE user_exchange_rates_new RENAME TO user_exchange_rates;
        CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies
            ON user_exchange_rates(from_currency, to_currency);
        CREATE INDEX IF NOT EXISTS idx_exchange_rates_date
            ON user_exchange_rates(effective_date DESC);
        CREATE INDEX IF NOT EXISTS idx_user_exchange_rates_user_id ON user_exchange_rates(user_id);
        ",
        )?;
        Ok(())
    })?;
    Ok(())
}

const USER_SCOPED_TABLES: &[&str] = &[
    "bills",
    "categories",
    "account_types",
    "accounts",
    "account_transfers",
    "tags",
    "budgets",
    "budget_history",
    "saved_filters",
    "bill_templates",
    "recurring_bills",
    "import_configs",
    "import_history",
    "llm_candidates",
    "llm_configs",
    "user_exchange_rates",
];

fn create_core_tables(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
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
        );

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
        );

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
        );

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
        );

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
        );

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
        );

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
        );

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
        );

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
        );

        CREATE TABLE IF NOT EXISTS bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (bill_id, tag_id),
            FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS budgets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            category TEXT,
            sub_category TEXT,
            budget_type INTEGER NOT NULL DEFAULT 1,
            period_type TEXT NOT NULL DEFAULT 'monthly',
            amount REAL NOT NULL,
            spent_amount REAL DEFAULT 0,
            alert_threshold REAL DEFAULT 80,
            start_date TEXT NOT NULL,
            end_date TEXT,
            enabled BOOLEAN DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

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
            calculated_at TEXT NOT NULL,
            filter_summary TEXT DEFAULT '',
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY (budget_id) REFERENCES budgets(id) ON DELETE CASCADE
        );

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
        );

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
        );

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
            created_at TEXT DEFAULT (datetime('now','localtime')),
            reviewed_at TEXT
        );

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
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            UNIQUE(user_id, name)
        );
        ",
    )?;
    Ok(())
}

fn create_core_indexes(connection: &Connection) -> DbResult<()> {
    if table_exists(connection, "users")? {
        let user_columns = table_column_names(connection, "users")?;
        if user_columns.iter().any(|column| column == "username") {
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)",
                [],
            )?;
        }
        if user_columns.iter().any(|column| column == "email") {
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
                [],
            )?;
        }
        if user_columns.iter().any(|column| column == "is_active") {
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active)",
                [],
            )?;
        }
    }
    connection.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_bills_user ON bills(user_id);
        CREATE INDEX IF NOT EXISTS idx_bills_date ON bills(date);
        CREATE INDEX IF NOT EXISTS idx_bills_type ON bills(type);
        CREATE INDEX IF NOT EXISTS idx_bills_category ON bills(main_category, sub_category);
        CREATE INDEX IF NOT EXISTS idx_bills_batch ON bills(batch_id);
        CREATE INDEX IF NOT EXISTS idx_bills_hash ON bills(hash);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash);
        CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id);
        CREATE INDEX IF NOT EXISTS idx_category_rules_user_priority
            ON category_rules(user_id, enabled, priority);
        CREATE INDEX IF NOT EXISTS idx_accounts_user ON accounts(user_id);
        CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(type);
        CREATE INDEX IF NOT EXISTS idx_accounts_hidden ON accounts(hidden);
        CREATE INDEX IF NOT EXISTS idx_account_types_user ON account_types(user_id);
        CREATE INDEX IF NOT EXISTS idx_account_types_type ON account_types(type);
        CREATE INDEX IF NOT EXISTS idx_account_transfers_from ON account_transfers(from_account_id);
        CREATE INDEX IF NOT EXISTS idx_account_transfers_to ON account_transfers(to_account_id);
        CREATE INDEX IF NOT EXISTS idx_account_transfers_date ON account_transfers(transfer_date);
        CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
        CREATE INDEX IF NOT EXISTS idx_bill_tags_bill ON bill_tags(bill_id);
        CREATE INDEX IF NOT EXISTS idx_bill_tags_tag ON bill_tags(tag_id);
        CREATE INDEX IF NOT EXISTS idx_budgets_period ON budgets(period_type);
        CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category, sub_category);
        CREATE INDEX IF NOT EXISTS idx_budgets_dates ON budgets(start_date, end_date);
        CREATE INDEX IF NOT EXISTS idx_budget_history_budget ON budget_history(budget_id);
        CREATE INDEX IF NOT EXISTS idx_budget_history_period
            ON budget_history(period_start, period_end);
        CREATE INDEX IF NOT EXISTS idx_budget_history_user_period
            ON budget_history(user_id, period_start, period_end);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_saved_filters_user_name_unique
            ON saved_filters(user_id, name);
        CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies
            ON user_exchange_rates(from_currency, to_currency);
        CREATE INDEX IF NOT EXISTS idx_exchange_rates_date
            ON user_exchange_rates(effective_date DESC);
        CREATE INDEX IF NOT EXISTS idx_user_exchange_rates_user_id ON user_exchange_rates(user_id);
        CREATE INDEX IF NOT EXISTS idx_llm_candidates_user_status
            ON llm_candidates(user_id, status);
        CREATE INDEX IF NOT EXISTS idx_llm_configs_user_active
            ON llm_configs(user_id, is_active);
        CREATE INDEX IF NOT EXISTS idx_user_external_auths_user ON user_external_auths(user_id);
        CREATE INDEX IF NOT EXISTS idx_user_external_auths_type ON user_external_auths(external_auth_type);
        ",
    )?;
    Ok(())
}

fn create_auth_security_tables(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
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
        );

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
        );

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
        );

        CREATE TABLE IF NOT EXISTS user_two_factor_recovery_codes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            code_hash TEXT NOT NULL,
            used_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, code_hash),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

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
        );

        CREATE TABLE IF NOT EXISTS user_application_cloud_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, setting_key),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        ",
    )?;
    Ok(())
}

fn create_auth_security_indexes(connection: &Connection) -> DbResult<()> {
    if table_exists(connection, "users")? {
        let user_columns = table_column_names(connection, "users")?;
        if user_columns.iter().any(|column| column == "username") {
            connection.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_unique
                 ON users(username)
                 WHERE username IS NOT NULL AND username <> ''",
                [],
            )?;
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)",
                [],
            )?;
        }
        if user_columns.iter().any(|column| column == "email") {
            connection.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_unique
                 ON users(email)
                 WHERE email IS NOT NULL AND email <> ''",
                [],
            )?;
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
                [],
            )?;
        }
        if user_columns.iter().any(|column| column == "is_active") {
            connection.execute(
                "CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active)",
                [],
            )?;
        }
    }
    connection.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);
        CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(is_active, expires_at);
        CREATE INDEX IF NOT EXISTS idx_auth_logs_user ON auth_logs(user_id);
        CREATE INDEX IF NOT EXISTS idx_auth_logs_event ON auth_logs(event_type, created_at);
        CREATE INDEX IF NOT EXISTS idx_auth_logs_created ON auth_logs(created_at);
        CREATE INDEX IF NOT EXISTS idx_user_two_factor_recovery_codes_user
            ON user_two_factor_recovery_codes(user_id, used_at);
        CREATE INDEX IF NOT EXISTS idx_user_two_factor_recovery_codes_hash
            ON user_two_factor_recovery_codes(user_id, code_hash);
        CREATE INDEX IF NOT EXISTS idx_user_external_auths_user ON user_external_auths(user_id);
        CREATE INDEX IF NOT EXISTS idx_user_external_auths_type
            ON user_external_auths(external_auth_type);
        CREATE INDEX IF NOT EXISTS idx_user_application_cloud_settings_user
            ON user_application_cloud_settings(user_id);
        CREATE INDEX IF NOT EXISTS idx_user_application_cloud_settings_key
            ON user_application_cloud_settings(setting_key);
        ",
    )?;
    Ok(())
}

fn migrate_legacy_user_security_columns(connection: &Connection) -> DbResult<()> {
    for (column, definition) in [
        ("email", "TEXT DEFAULT ''"),
        ("password_hash", "TEXT DEFAULT ''"),
        ("nickname", "TEXT"),
        ("avatar", "TEXT"),
        ("default_account_id", "INTEGER"),
        ("transaction_edit_scope", "INTEGER DEFAULT 0"),
        ("language", "TEXT DEFAULT 'zh_Hans'"),
        ("default_currency", "TEXT DEFAULT 'CNY'"),
        ("first_day_of_week", "INTEGER DEFAULT 1"),
        ("fiscal_year_start", "INTEGER DEFAULT 1"),
        ("calendar_display_type", "INTEGER DEFAULT 0"),
        ("date_display_type", "INTEGER DEFAULT 0"),
        ("long_date_format", "INTEGER DEFAULT 0"),
        ("short_date_format", "INTEGER DEFAULT 0"),
        ("long_time_format", "INTEGER DEFAULT 0"),
        ("short_time_format", "INTEGER DEFAULT 0"),
        ("fiscal_year_format", "INTEGER DEFAULT 0"),
        ("currency_display_type", "INTEGER DEFAULT 0"),
        ("numeral_system", "INTEGER DEFAULT 0"),
        ("decimal_separator", "INTEGER DEFAULT 0"),
        ("digit_grouping_symbol", "INTEGER DEFAULT 0"),
        ("digit_grouping", "INTEGER DEFAULT 0"),
        ("coordinate_display_type", "INTEGER DEFAULT 0"),
        ("expense_amount_color", "INTEGER DEFAULT 0"),
        ("income_amount_color", "INTEGER DEFAULT 0"),
        ("cash_account_id", "INTEGER"),
        ("cash_transfer_category_id", "INTEGER"),
        ("import_learning_enabled", "BOOLEAN DEFAULT 1"),
        ("investment_platform_keywords", "TEXT"),
        ("investment_product_keywords", "TEXT"),
        ("investment_exclude_keywords", "TEXT"),
        ("is_active", "BOOLEAN DEFAULT 1"),
        ("email_verified", "BOOLEAN DEFAULT 0"),
        ("two_factor_enabled", "BOOLEAN DEFAULT 0"),
        ("two_factor_secret", "TEXT"),
        ("failed_login_attempts", "INTEGER DEFAULT 0"),
        ("locked_until", "TEXT"),
        ("last_login_at", "TEXT"),
        ("last_login_ip", "TEXT"),
        ("created_at", "TEXT DEFAULT ''"),
        ("updated_at", "TEXT DEFAULT ''"),
    ] {
        add_column_if_missing(connection, "users", column, definition)?;
    }
    Ok(())
}

fn migrate_core_legacy_columns(connection: &Connection) -> DbResult<()> {
    for (table, column, definition) in [
        ("categories", "type", "INTEGER DEFAULT 1"),
        ("categories", "priority", "INTEGER DEFAULT 0"),
        ("categories", "keywords", "TEXT"),
        ("categories", "hidden", "BOOLEAN DEFAULT 0"),
        ("categories", "icon", "TEXT"),
        ("categories", "color", "TEXT"),
        ("accounts", "currency", "TEXT DEFAULT 'CNY'"),
        ("accounts", "parent_id", "INTEGER DEFAULT 0"),
        ("accounts", "aliases", "TEXT"),
        ("account_transfers", "note", "TEXT"),
        ("tags", "hidden", "BOOLEAN DEFAULT 0"),
        ("budget_history", "budget_amount", "REAL DEFAULT 0"),
        ("budget_history", "execution_rate", "REAL DEFAULT 0"),
        ("budget_history", "filter_summary", "TEXT DEFAULT ''"),
        ("saved_filters", "description", "TEXT"),
        ("saved_filters", "filter_data", "TEXT DEFAULT '{}'"),
        ("bills", "payment_method", "TEXT DEFAULT ''"),
        ("bills", "source_account_id", "INTEGER DEFAULT 0"),
        ("bills", "destination_account_id", "INTEGER DEFAULT 0"),
        ("bills", "destination_amount", "REAL DEFAULT 0"),
        ("bills", "created_from_template", "INTEGER"),
        ("bills", "created_from_recurring", "INTEGER"),
        ("bills", "import_history_id", "INTEGER"),
    ] {
        add_column_if_missing(connection, table, column, definition)?;
    }
    connection.execute(
        "UPDATE categories SET type = 2 WHERE main_category = '收入' AND type = 1",
        [],
    )?;
    connection.execute(
        "UPDATE categories SET type = 3 WHERE main_category = '转账' AND type = 1",
        [],
    )?;
    Ok(())
}

fn run_schema_rebuild_with_fk_guard(
    connection: &Connection,
    operation: impl FnOnce(&Connection) -> DbResult<()>,
) -> DbResult<()> {
    let restore_foreign_keys = foreign_keys_enabled(connection)?;
    if restore_foreign_keys {
        connection.pragma_update(None, "foreign_keys", "OFF")?;
    }
    if let Err(error) = connection.execute_batch("BEGIN IMMEDIATE") {
        if restore_foreign_keys {
            let _ = connection.pragma_update(None, "foreign_keys", "ON");
        }
        return Err(error.into());
    }

    let operation_result = operation(connection);
    match operation_result {
        Ok(()) => {
            let violations = match count_foreign_key_violations(connection) {
                Ok(violations) => violations,
                Err(error) => {
                    let _ = connection.execute_batch("ROLLBACK");
                    if restore_foreign_keys {
                        let _ = connection.pragma_update(None, "foreign_keys", "ON");
                    }
                    return Err(error);
                }
            };
            if violations > 0 {
                let _ = connection.execute_batch("ROLLBACK");
                if restore_foreign_keys {
                    let _ = connection.pragma_update(None, "foreign_keys", "ON");
                }
                return Err(DbError::InvalidOperation(format!(
                    "foreign_key_check failed during schema rebuild: {violations} violations"
                )));
            }
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                if restore_foreign_keys {
                    let _ = connection.pragma_update(None, "foreign_keys", "ON");
                }
                return Err(error.into());
            }
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            if restore_foreign_keys {
                let _ = connection.pragma_update(None, "foreign_keys", "ON");
            }
            return Err(error);
        }
    }

    if restore_foreign_keys {
        connection.pragma_update(None, "foreign_keys", "ON")?;
    }
    let violations = count_foreign_key_violations(connection)?;
    if violations > 0 {
        return Err(DbError::InvalidOperation(format!(
            "foreign_key_check failed after schema rebuild: {violations} violations"
        )));
    }
    Ok(())
}

fn add_column_if_missing(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> DbResult<()> {
    ensure_safe_identifier(table_name)?;
    ensure_safe_identifier(column_name)?;
    if !table_exists(connection, table_name)? {
        return Ok(());
    }
    let columns = table_column_names(connection, table_name)?;
    if columns.iter().any(|column| column == column_name) {
        return Ok(());
    }
    connection.execute(
        &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"),
        [],
    )?;
    Ok(())
}

fn copy_columns_for_rebuild(
    connection: &Connection,
    table_name: &str,
    append_user_id: bool,
) -> DbResult<String> {
    let mut columns = table_column_names(connection, table_name)?
        .into_iter()
        .collect::<Vec<_>>();
    if append_user_id && !columns.iter().any(|column| column == "user_id") {
        columns.push("user_id".to_string());
    }
    if columns.is_empty() {
        return Err(DbError::InvalidOperation(format!(
            "{table_name} has no copyable columns"
        )));
    }
    Ok(columns.join(", "))
}

fn foreign_keys_enabled(connection: &Connection) -> DbResult<bool> {
    let foreign_keys: i64 =
        connection.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))?;
    Ok(foreign_keys == 1)
}

fn unique_index_exists_on(
    connection: &Connection,
    table_name: &str,
    expected_columns: &[&str],
) -> DbResult<bool> {
    ensure_safe_identifier(table_name)?;
    let mut statement = connection.prepare(&format!("PRAGMA index_list({table_name})"))?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? == 1))
    })?;
    for row in rows {
        let (index_name, is_unique) = row?;
        if is_unique && index_columns(connection, &index_name)? == expected_columns {
            return Ok(true);
        }
    }
    Ok(false)
}

fn index_columns(connection: &Connection, index_name: &str) -> DbResult<Vec<String>> {
    ensure_safe_identifier(index_name)?;
    let mut statement = connection.prepare(&format!("PRAGMA index_info({index_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(2))?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row?);
    }
    Ok(columns)
}

fn table_column_names(connection: &Connection, table_name: &str) -> DbResult<Vec<String>> {
    ensure_safe_identifier(table_name)?;
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row?);
    }
    Ok(columns)
}

fn table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            [table_name],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

fn ensure_safe_identifier(value: &str) -> DbResult<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(DbError::InvalidOperation(format!(
            "unsafe sqlite identifier: {value}"
        )));
    }
    Ok(())
}

fn copy_database_files(source: &Path, copy: &Path) -> DbResult<()> {
    remove_database_files(copy)?;
    std::fs::copy(source, copy)?;
    copy_sidecar_if_exists(source, copy, "wal")?;
    copy_sidecar_if_exists(source, copy, "shm")?;
    Ok(())
}

fn remove_database_files(path: &Path) -> DbResult<()> {
    remove_file_if_exists(path)?;
    remove_file_if_exists(&sqlite_sidecar_path(path, "wal"))?;
    remove_file_if_exists(&sqlite_sidecar_path(path, "shm"))?;
    Ok(())
}

fn copy_sidecar_if_exists(source: &Path, copy: &Path, suffix: &str) -> DbResult<()> {
    let source_sidecar = sqlite_sidecar_path(source, suffix);
    if source_sidecar.exists() {
        std::fs::copy(source_sidecar, sqlite_sidecar_path(copy, suffix))?;
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> DbResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut file_name = OsString::from(path.as_os_str());
    file_name.push(format!("-{suffix}"));
    PathBuf::from(file_name)
}

pub fn schema_inventory() -> SchemaInventory {
    SchemaInventory {
        responsibilities: vec![
            SchemaResponsibility {
                legacy_area: "runtime connection",
                rust_mapping:
                    "crates/bill-analyser-db/src/connection.rs; crates/bill-analyser-db/src/path.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "user scope helpers",
                rust_mapping: "crates/bill-analyser-db/src/user_scope.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "time normalization",
                rust_mapping: "crates/bill-analyser-core/src/time.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "encryption provider",
                rust_mapping: "deferred SQLCipher provider integration",
                status: "deferred",
            },
            SchemaResponsibility {
                legacy_area: "schema initializer",
                rust_mapping: "crates/bill-analyser-db/src/schema.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "business table DDL",
                rust_mapping: "crates/bill-analyser-db/src/schema.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "core indexes",
                rust_mapping: "crates/bill-analyser-db/src/schema.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "legacy ALTER migrations",
                rust_mapping: "crates/bill-analyser-db/src/schema.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "templates and import staging",
                rust_mapping: "crates/bill-analyser-db/src/import_staging.rs; crates/bill-analyser-db/src/taxonomy",
                status: "foundational",
            },
            SchemaResponsibility {
                legacy_area: "users and security schema",
                rust_mapping: "crates/bill-analyser-db/src/schema.rs covers auth/security users/session/cloud schema; crates/bill-analyser-db/src/app_settings.rs covers app_settings/OCR config subset; crates/bill-analyser-db/src/backup.rs covers audit_logs/backup_records/backup_jobs for ops runtime",
                status: "foundational",
            },
        ],
    }
}

fn list_tables(connection: &Connection) -> DbResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut tables = Vec::new();
    for row in rows {
        tables.push(row?);
    }
    Ok(tables)
}

fn count_foreign_key_violations(connection: &Connection) -> DbResult<usize> {
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    let mut count = 0;
    while rows.next()?.is_some() {
        count += 1;
    }
    Ok(count)
}
