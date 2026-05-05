use std::error::Error;
use std::path::{Path, PathBuf};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    run_transaction, schema_inventory, SchemaDryRun, SqliteConnectionConfig, SqliteDbPath,
    SqliteRuntime, UserScope,
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

struct CreatedRealDb {
    path: PathBuf,
    remove_file_on_drop: bool,
}

impl CreatedRealDb {
    fn ensure() -> Result<Self, Box<dyn Error>> {
        let path = std::env::current_dir()?.join("data").join("bills.db");
        if path.exists() {
            return Ok(Self {
                path,
                remove_file_on_drop: false,
            });
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::File::create(&path)?;
        Ok(Self {
            path,
            remove_file_on_drop: true,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CreatedRealDb {
    fn drop(&mut self) {
        if self.remove_file_on_drop {
            let _ = std::fs::remove_file(&self.path);
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
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
fn safe_path_guard_rejects_temp_symlink_to_real_data_bills_db() -> Result<(), Box<dyn Error>> {
    let real_db = CreatedRealDb::ensure()?;
    let temp_dir = tempfile::tempdir()?;
    let link_path = temp_dir.path().join("linked_real.db");

    if let Err(error) = symlink_file(real_db.path(), &link_path) {
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
        error.to_string().contains("data/bills.db")
            || error
                .to_string()
                .contains("target must stay under temp root")
    );
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
fn schema_inventory_maps_python_runtime_schema_responsibilities_as_foundational_only() {
    let inventory = schema_inventory();
    let paths: Vec<_> = inventory
        .responsibilities
        .iter()
        .map(|item| item.python_path)
        .collect();

    assert!(paths.contains(&"src/bill_analyser/core/database/runtime.py"));
    assert!(paths.contains(&"src/bill_analyser/core/database/schema/__init__.py"));
    assert!(inventory
        .responsibilities
        .iter()
        .all(|item| item.status == "foundational" || item.status == "deferred"));
    assert!(inventory
        .responsibilities
        .iter()
        .any(|item| item.rust_mapping.contains("crates/bill-analyser-db/src")));
}
