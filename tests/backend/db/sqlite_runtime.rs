use std::error::Error;
use std::path::{Path, PathBuf};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    init_auth_security_schema, init_foundational_schema, migrate_categories_unique_constraint,
    migrate_user_id_field, run_transaction, schema_inventory, DbError, SchemaDryRun,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime, UserScope,
};
use rusqlite::Connection;

fn runtime_for(path: &std::path::Path) -> Result<SqliteRuntime, Box<dyn Error>> {
    let db_path = SqliteDbPath::temporary_file(path)?;
    Ok(SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: std::time::Duration::from_secs(1),
    })?)
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}-{suffix}", path.display()))
}

fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row?);
    }
    Ok(columns)
}

fn table_indexes(connection: &Connection, table: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut statement = connection.prepare(&format!("PRAGMA index_list({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut indexes = Vec::new();
    for row in rows {
        indexes.push(row?);
    }
    Ok(indexes)
}

fn collect_rust_files(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(collect_rust_files(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }

    Ok(files)
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "CARGO_MANIFEST_DIR is not under src/backend/db",
            )
        })?;
    Ok(workspace_root.to_path_buf())
}

#[test]
fn safe_path_guard_rejects_real_data_bills_db() -> Result<(), Box<dyn Error>> {
    let real_db = std::env::current_dir()
        .unwrap()
        .join("data")
        .join("bills.db");

    let error = SqliteDbPath::temporary_file(&real_db).unwrap_err();

    assert!(error.to_string().contains("data/bills.db"));
    let temp_dir = tempfile::tempdir()?;
    let copied_fixture = temp_dir.path().join("data").join("bills.db");
    assert!(SqliteDbPath::temporary_file(&copied_fixture).is_ok());
    Ok(())
}

#[test]
fn application_path_guard_accepts_temp_application_file_for_runtime() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir)?;
    let application_db = data_dir.join("bills.db");
    std::fs::File::create(&application_db)?;

    let db_path = SqliteDbPath::application_file(&application_db)?;

    assert_eq!(db_path.as_path(), application_db.as_path());
    Ok(())
}

#[test]
fn application_path_guard_rejects_missing_parent_and_directory_target() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let missing_parent = temp_dir.path().join("missing").join("runtime.db");
    let missing_parent_error = SqliteDbPath::application_file(&missing_parent).unwrap_err();
    assert!(missing_parent_error
        .to_string()
        .contains("parent directory must exist"));

    let directory_target = temp_dir.path().join("directory-target");
    std::fs::create_dir(&directory_target)?;
    let directory_target_error = SqliteDbPath::application_file(&directory_target).unwrap_err();
    assert!(directory_target_error
        .to_string()
        .contains("not a directory"));
    Ok(())
}

#[test]
fn safe_path_guard_rejects_temp_symlink_to_real_data_bills_db() -> Result<(), Box<dyn Error>> {
    let real_db = std::env::current_dir()?.join("data").join("bills.db");
    if !real_db.is_file() {
        return Ok(());
    }
    let temp_dir = tempfile::tempdir()?;
    let link_path = temp_dir.path().join("linked_real.db");

    if let Err(error) = symlink_file(&real_db, &link_path) {
        if cfg!(windows)
            && (error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314))
        {
            return Ok(());
        }
        return Err(Box::new(error));
    }

    let error = SqliteDbPath::temporary_file(&link_path).unwrap_err();

    assert!(
        matches!(&error, DbError::UnsafePath(_)),
        "expected unsafe path error, got {error}"
    );
    Ok(())
}

#[test]
fn backend_db_tests_do_not_create_repo_data_bills_db() -> Result<(), Box<dyn Error>> {
    let db_tests_root = workspace_root()?.join("tests").join("backend").join("db");
    let this_file = db_tests_root.join("sqlite_runtime.rs").canonicalize()?;
    let repo_join_pattern = [".join(\"data\")", ".join(\"bills.db\")"].concat();
    let old_real_db_guard_pattern = ["Created", "RealDb"].concat();

    for file in collect_rust_files(&db_tests_root)? {
        let canonical = file.canonicalize()?;
        let source = std::fs::read_to_string(&file)?;

        assert!(
            !source.contains(&old_real_db_guard_pattern),
            "{} must not create or clean up repository data/bills.db as a test fixture",
            file.display()
        );

        if canonical == this_file {
            continue;
        }

        assert!(
            !source.contains(&repo_join_pattern),
            "{} must use temp DB fixtures and leave repository data/bills.db untouched",
            file.display()
        );
    }

    Ok(())
}

#[test]
fn connection_applies_wal_foreign_keys_and_runtime_pragmas() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("test_runtime.db"))?;

    let pragmas = runtime.pragma_snapshot()?;
    assert_eq!(pragmas.journal_mode, "wal");
    assert!(pragmas.foreign_keys);
    assert_eq!(pragmas.synchronous, 1);

    runtime.connection().execute_batch(
        "CREATE TABLE parent(id INTEGER PRIMARY KEY);
         CREATE TABLE child(parent_id INTEGER NOT NULL REFERENCES parent(id));",
    )?;
    let foreign_key_error = runtime
        .connection()
        .execute("INSERT INTO child(parent_id) VALUES (404)", []);
    assert!(foreign_key_error.is_err());
    Ok(())
}

#[test]
fn foundational_schema_initializes_core_tables_indexes_and_runtime_contracts(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("test_foundational_schema.db"))?;

    init_foundational_schema(runtime.connection())?;
    init_foundational_schema(runtime.connection())?;

    for table in [
        "users",
        "bills",
        "categories",
        "category_rules",
        "accounts",
        "tags",
        "budgets",
        "budget_history",
        "user_exchange_rates",
    ] {
        assert!(
            !table_columns(runtime.connection(), table)?.is_empty(),
            "{table}"
        );
    }

    assert!(
        table_columns(runtime.connection(), "bills")?.contains(&"destination_amount".to_string())
    );
    assert!(table_columns(runtime.connection(), "accounts")?.contains(&"currency".to_string()));
    assert!(table_columns(runtime.connection(), "account_transfers")?.contains(&"note".to_string()));
    assert!(!table_columns(runtime.connection(), "account_transfers")?
        .contains(&"description".to_string()));
    assert!(table_columns(runtime.connection(), "budget_history")?
        .contains(&"filter_summary".to_string()));
    assert!(
        table_columns(runtime.connection(), "saved_filters")?.contains(&"filter_data".to_string())
    );
    assert!(
        table_columns(runtime.connection(), "saved_filters")?.contains(&"description".to_string())
    );
    assert!(
        !table_columns(runtime.connection(), "saved_filters")?.contains(&"filter_json".to_string())
    );
    assert!(table_indexes(runtime.connection(), "bills")?
        .contains(&"idx_bills_user_hash_unique".to_string()));
    assert!(table_indexes(runtime.connection(), "categories")?
        .contains(&"idx_categories_user".to_string()));
    assert!(table_indexes(runtime.connection(), "saved_filters")?
        .contains(&"idx_saved_filters_user_name_unique".to_string()));
    assert!(!table_columns(runtime.connection(), "bill_tags")?.contains(&"id".to_string()));
    assert!(table_columns(runtime.connection(), "users")?.contains(&"email".to_string()));
    assert!(table_columns(runtime.connection(), "users")?.contains(&"avatar".to_string()));
    assert!(table_columns(runtime.connection(), "users")?
        .contains(&"import_learning_enabled".to_string()));
    assert!(table_indexes(runtime.connection(), "users")?.contains(&"idx_users_email".to_string()));

    let missing_email_user = runtime.connection().execute(
        "INSERT INTO users(id, username, password_hash, created_at, updated_at)
         VALUES (99, 'missing-email-user', 'hash', '2026-05-09T00:00:00', '2026-05-09T00:00:00')",
        [],
    );
    assert!(missing_email_user.is_err());

    runtime.connection().execute(
        "INSERT INTO users(id, username, email, password_hash, created_at, updated_at)
         VALUES (1, 'schema-user', 'schema-user@example.test', 'hash',
                 '2026-05-09T00:00:00', '2026-05-09T00:00:00')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO categories(user_id, type, main_category, sub_category, created_at)
         VALUES (1, 1, '餐饮', '午餐', '2026-05-09T00:00:00')",
        [],
    )?;
    let foreign_key_error = runtime.connection().execute(
        "INSERT INTO categories(user_id, type, main_category, sub_category, created_at)
         VALUES (404, 1, '餐饮', '晚餐', '2026-05-09T00:00:00')",
        [],
    );
    assert!(foreign_key_error.is_err());

    let account_transfer_foreign_key_error = runtime.connection().execute(
        "INSERT INTO account_transfers(
             user_id, from_account_id, to_account_id, amount, transfer_date, note, created_at
         ) VALUES (1, 404, 405, 12.3, '2026-05-09', 'bad account', '2026-05-09T00:00:00')",
        [],
    );
    assert!(account_transfer_foreign_key_error.is_err());

    runtime.connection().execute(
        "INSERT INTO saved_filters(user_id, name, description, filter_data, created_at, updated_at)
         VALUES (1, 'recent-food', 'desc', '{}', '2026-05-09T00:00:00', '2026-05-09T00:00:00')",
        [],
    )?;
    let duplicate_filter = runtime.connection().execute(
        "INSERT INTO saved_filters(user_id, name, description, filter_data, created_at, updated_at)
         VALUES (1, 'recent-food', 'other', '{}', '2026-05-09T00:01:00', '2026-05-09T00:01:00')",
        [],
    );
    assert!(duplicate_filter.is_err());
    Ok(())
}

#[test]
fn auth_security_schema_initializes_tables_and_migrates_legacy_user_columns(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("test_auth_security_schema.db"))?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL
        );
        INSERT INTO users(id, username) VALUES (42, 'legacy-auth-user');
        ",
    )?;

    init_auth_security_schema(runtime.connection())?;
    init_auth_security_schema(runtime.connection())?;

    for column in [
        "email",
        "password_hash",
        "nickname",
        "avatar",
        "cash_account_id",
        "cash_transfer_category_id",
        "import_learning_enabled",
        "investment_platform_keywords",
        "investment_product_keywords",
        "investment_exclude_keywords",
        "is_active",
        "email_verified",
        "two_factor_enabled",
        "failed_login_attempts",
        "locked_until",
        "last_login_at",
        "last_login_ip",
        "created_at",
        "updated_at",
    ] {
        assert!(
            table_columns(runtime.connection(), "users")?.contains(&column.to_string()),
            "{column}"
        );
    }
    for table in [
        "sessions",
        "auth_logs",
        "user_two_factor_recovery_codes",
        "user_external_auths",
        "user_application_cloud_settings",
    ] {
        assert!(
            !table_columns(runtime.connection(), table)?.is_empty(),
            "{table}"
        );
    }

    assert!(table_indexes(runtime.connection(), "sessions")?
        .contains(&"idx_sessions_token".to_string()));
    assert!(table_indexes(runtime.connection(), "auth_logs")?
        .contains(&"idx_auth_logs_event".to_string()));
    assert!(table_indexes(runtime.connection(), "users")?
        .contains(&"idx_users_username_unique".to_string()));
    assert!(table_indexes(runtime.connection(), "users")?
        .contains(&"idx_users_email_unique".to_string()));
    assert!(
        table_indexes(runtime.connection(), "user_application_cloud_settings")?
            .contains(&"idx_user_application_cloud_settings_user".to_string())
    );

    let defaults: (String, i64, i64, i64) = runtime.connection().query_row(
        "SELECT COALESCE(email, ''), is_active, import_learning_enabled, failed_login_attempts
         FROM users WHERE id = 42",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    assert_eq!(defaults, ("".to_string(), 1, 1, 0));

    runtime.connection().execute(
        "UPDATE users SET email = 'legacy@example.test' WHERE id = 42",
        [],
    )?;
    let duplicate_username = runtime.connection().execute(
        "INSERT INTO users(id, username, email) VALUES (43, 'legacy-auth-user', 'other@example.test')",
        [],
    );
    assert!(duplicate_username.is_err());
    let duplicate_email = runtime.connection().execute(
        "INSERT INTO users(id, username, email) VALUES (44, 'other-auth-user', 'legacy@example.test')",
        [],
    );
    assert!(duplicate_email.is_err());

    runtime.connection().execute(
        "INSERT INTO sessions(user_id, token_hash, expires_at, created_at)
         VALUES (42, 'token-hash', '2026-05-09T19:00:00', '2026-05-09T18:00:00')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO auth_logs(user_id, username, event_type, success, created_at)
         VALUES (42, 'legacy-auth-user', 'schema_smoke', 1, '2026-05-09T18:00:00')",
        [],
    )?;
    Ok(())
}

#[test]
fn foundational_schema_migrates_legacy_user_scoped_unique_constraints() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("test_foundational_legacy.db"))?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE users(id INTEGER PRIMARY KEY, username TEXT NOT NULL, password_hash TEXT NOT NULL);
        INSERT INTO users(id, username, password_hash) VALUES
            (1, 'legacy-one', 'hash'),
            (2, 'legacy-two', 'hash');

        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(main_category, sub_category)
        );
        INSERT INTO categories(main_category, sub_category, description, created_at)
            VALUES ('收入', '工资', 'legacy', '2026-05-09T01:00:00');

        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            hash TEXT UNIQUE,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        INSERT INTO bills(id, date, type, amount, counterparty, description, hash, created_at, updated_at)
            VALUES
                (7, '2026-05-09 08:00:00', '支出', -18.5, 'legacy shop', 'legacy bill', 'hash-a',
                 '2026-05-09T08:00:00', '2026-05-09T08:00:00'),
                (42, '2026-05-09 08:30:00', '支出', -8.5, 'legacy child shop', 'legacy child bill', 'hash-child',
                 '2026-05-09T08:30:00', '2026-05-09T08:30:00');

        CREATE TABLE tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        INSERT INTO tags(id, user_id, name, created_at, updated_at)
            VALUES (5, 1, 'legacy-tag', '2026-05-09T08:00:00', '2026-05-09T08:00:00');

        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL REFERENCES bills(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            created_at TEXT NOT NULL,
            PRIMARY KEY (bill_id, tag_id)
        );
        INSERT INTO bill_tags(bill_id, tag_id, created_at)
            VALUES (42, 5, '2026-05-09T08:40:00');

        CREATE TABLE user_exchange_rates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_currency TEXT NOT NULL,
            to_currency TEXT NOT NULL,
            rate REAL NOT NULL,
            source TEXT DEFAULT 'manual',
            effective_date TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(from_currency, to_currency, effective_date)
        );
        INSERT INTO user_exchange_rates(from_currency, to_currency, rate, source, effective_date)
            VALUES ('CNY', 'USD', 7.21, 'manual', '2026-05-09');
        ",
    )?;

    init_foundational_schema(runtime.connection())?;

    assert!(table_columns(runtime.connection(), "categories")?.contains(&"user_id".to_string()));
    assert!(table_columns(runtime.connection(), "bills")?.contains(&"user_id".to_string()));
    assert!(table_columns(runtime.connection(), "user_exchange_rates")?
        .contains(&"user_id".to_string()));
    assert!(table_columns(runtime.connection(), "bills")?.contains(&"id".to_string()));

    runtime.connection().execute(
        "INSERT INTO categories(user_id, main_category, sub_category, created_at)
         VALUES (2, '收入', '工资', '2026-05-09T09:00:00')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO bills(user_id, date, type, amount, counterparty, description, hash, created_at, updated_at)
         VALUES (2, '2026-05-09 09:00:00', '支出', -25.0, 'second shop', 'second bill', 'hash-a',
                 '2026-05-09T09:00:00', '2026-05-09T09:00:00')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO user_exchange_rates(user_id, from_currency, to_currency, rate, source, effective_date)
         VALUES (2, 'CNY', 'USD', 7.19, 'manual', '2026-05-09')",
        [],
    )?;

    let category_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM categories WHERE main_category = '收入' AND sub_category = '工资'",
        [],
        |row| row.get(0),
    )?;
    let bill_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE hash = 'hash-a'",
        [],
        |row| row.get(0),
    )?;
    let child_bill_tag_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bill_tags WHERE bill_id = 42 AND tag_id = 5",
        [],
        |row| row.get(0),
    )?;
    let legacy_bill_id_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE id IN (7, 42)",
        [],
        |row| row.get(0),
    )?;
    let rate_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM user_exchange_rates WHERE from_currency = 'CNY'
         AND to_currency = 'USD' AND effective_date = '2026-05-09'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(category_count, 2);
    assert_eq!(bill_count, 2);
    assert_eq!(child_bill_tag_count, 1);
    assert_eq!(legacy_bill_id_count, 2);
    assert_eq!(rate_count, 2);
    Ok(())
}

#[test]
fn schema_rebuild_fk_failure_rolls_back_table_swap() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("test_rebuild_fk_rollback.db"))?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT NOT NULL,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        INSERT INTO users(id, username, email, password_hash, created_at, updated_at)
            VALUES (2, 'existing-user', 'existing@example.test', 'hash',
                    '2026-05-09T00:00:00', '2026-05-09T00:00:00');

        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(main_category, sub_category)
        );
        INSERT INTO categories(user_id, main_category, sub_category, description, created_at)
            VALUES (1, 'orphaned', 'legacy', 'must rollback', '2026-05-09T01:00:00');
        ",
    )?;

    let result = migrate_categories_unique_constraint(runtime.connection());
    assert!(result.is_err());

    let table_sql: String = runtime.connection().query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'categories'",
        [],
        |row| row.get(0),
    )?;
    let legacy_row_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM categories
         WHERE user_id = 1 AND main_category = 'orphaned' AND sub_category = 'legacy'",
        [],
        |row| row.get(0),
    )?;
    assert!(table_sql.contains("UNIQUE(main_category, sub_category)"));
    assert!(!table_sql.contains("FOREIGN KEY (user_id)"));
    assert!(!table_indexes(runtime.connection(), "categories")?
        .contains(&"idx_categories_user".to_string()));
    assert_eq!(legacy_row_count, 1);
    Ok(())
}

#[test]
fn migrate_user_id_field_is_idempotent_and_ignores_missing_tables() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("test_user_scope_migration.db"))?;
    runtime.connection().execute(
        "CREATE TABLE legacy_items(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        [],
    )?;

    migrate_user_id_field(runtime.connection(), "legacy_items")?;
    migrate_user_id_field(runtime.connection(), "legacy_items")?;
    migrate_user_id_field(runtime.connection(), "missing_table")?;

    assert!(table_columns(runtime.connection(), "legacy_items")?.contains(&"user_id".to_string()));
    assert!(table_indexes(runtime.connection(), "legacy_items")?
        .contains(&"idx_legacy_items_user_id".to_string()));
    Ok(())
}

#[test]
fn connection_does_not_create_parent_dirs_without_create_flag() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let missing_parent = temp_dir.path().join("missing-parent");
    let db_path = SqliteDbPath::temporary_file(missing_parent.join("runtime.db"))?;

    let result = SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: false,
        busy_timeout: std::time::Duration::from_secs(1),
    });

    assert!(result.is_err());
    assert!(!missing_parent.exists());
    Ok(())
}

#[test]
fn transaction_helper_commits_success_and_rolls_back_error() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("test_transaction.db"))?;
    runtime.connection().execute(
        "CREATE TABLE audit(id INTEGER PRIMARY KEY, label TEXT NOT NULL)",
        [],
    )?;

    run_transaction(runtime.connection_mut(), |tx| {
        tx.execute("INSERT INTO audit(label) VALUES ('committed')", [])?;
        Ok(())
    })?;
    let committed_count: i64 =
        runtime
            .connection()
            .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))?;
    assert_eq!(committed_count, 1);

    let failed: bill_analyser_db::DbResult<()> = run_transaction(runtime.connection_mut(), |tx| {
        tx.execute("INSERT INTO audit(label) VALUES ('rolled-back')", [])?;
        Err(bill_analyser_db::DbError::InvalidOperation(
            "force rollback".to_string(),
        ))
    });
    assert!(failed.is_err());
    let final_count: i64 =
        runtime
            .connection()
            .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))?;
    assert_eq!(final_count, 1);
    Ok(())
}

#[test]
fn schema_dry_run_uses_copy_database_and_is_idempotent() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let source = temp_dir.path().join("source_fixture.db");
    let copy = temp_dir.path().join("copy_fixture.db");
    {
        let source_runtime = runtime_for(&source)?;
        source_runtime.connection().execute_batch(
            "CREATE TABLE users(id INTEGER PRIMARY KEY);
             CREATE TABLE bills(id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id));",
        )?;
    }

    let first = SchemaDryRun::copy_and_validate(&source, &copy)?;
    let second = SchemaDryRun::copy_and_validate(&source, &copy)?;

    assert_eq!(first.copy_path, second.copy_path);
    assert_eq!(first.foreign_key_violations, 0);
    assert!(first.existing_tables.contains(&"users".to_string()));
    assert!(first.existing_tables.contains(&"bills".to_string()));
    assert_eq!(first.pragma_snapshot.journal_mode, "wal");
    assert!(first.pragma_snapshot.foreign_keys);
    Ok(())
}

#[test]
fn schema_dry_run_does_not_checkpoint_or_rewrite_source_wal() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let source = temp_dir.path().join("source_wal_fixture.db");
    let copy = temp_dir.path().join("copy_wal_fixture.db");
    let source_connection = Connection::open(&source)?;
    source_connection.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA user_version=77;
         CREATE TABLE source_marker(id INTEGER PRIMARY KEY, label TEXT NOT NULL);
         INSERT INTO source_marker(label) VALUES ('kept-in-wal');",
    )?;
    let source_wal = sqlite_sidecar_path(&source, "wal");
    let source_wal_len_before = std::fs::metadata(&source_wal)?.len();
    let user_version_before: i64 =
        source_connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    let report = SchemaDryRun::copy_and_validate(&source, &copy)?;

    let source_wal_len_after = std::fs::metadata(&source_wal)?.len();
    let user_version_after: i64 =
        source_connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(source_wal_len_after, source_wal_len_before);
    assert_eq!(user_version_after, user_version_before);
    assert!(report
        .existing_tables
        .contains(&"source_marker".to_string()));
    Ok(())
}

#[test]
fn user_scope_uses_positive_core_user_id_without_inline_sql_values() -> Result<(), Box<dyn Error>> {
    let user_id = UserId::new(42).unwrap();
    let scope = UserScope::new(user_id);

    assert_eq!(scope.user_id().get(), 42);
    assert_eq!(scope.where_clause("bills"), "bills.user_id = ?");
    assert_eq!(scope.where_clause(""), "user_id = ?");
    assert_eq!(scope.bind_value()?, 42);
    assert!(UserId::new(0).is_err());
    Ok(())
}

#[test]
fn schema_inventory_maps_rust_runtime_schema_responsibilities_as_foundational_only() {
    let inventory = schema_inventory();
    let areas: Vec<_> = inventory
        .responsibilities
        .iter()
        .map(|item| item.legacy_area)
        .collect();

    assert!(areas.contains(&"runtime connection"));
    assert!(areas.contains(&"schema initializer"));
    assert!(inventory
        .responsibilities
        .iter()
        .all(|item| item.status == "foundational" || item.status == "deferred"));
    assert!(inventory.responsibilities.iter().any(|item| item
        .rust_mapping
        .contains("auth/security users/session/cloud schema")));
}
