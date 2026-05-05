use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::{DbResult, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaResponsibility {
    pub python_path: &'static str,
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
                python_path: "src/bill_analyser/core/database/runtime.py",
                rust_mapping:
                    "crates/bill-analyser-db/src/connection.rs; crates/bill-analyser-db/src/path.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/shared.py",
                rust_mapping: "crates/bill-analyser-db/src/user_scope.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/time.py",
                rust_mapping: "deferred to shared primitives and future domain migrations",
                status: "deferred",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/encryption.py",
                rust_mapping: "deferred; SQLCipher remains Python runtime opt-in",
                status: "deferred",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/schema/__init__.py",
                rust_mapping: "crates/bill-analyser-db/src/schema.rs",
                status: "foundational",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/schema/core/business.py",
                rust_mapping: "deferred; table DDL remains Python-owned",
                status: "deferred",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/schema/core/indexes.py",
                rust_mapping: "deferred; index DDL remains Python-owned",
                status: "deferred",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/schema/core/migrations.py",
                rust_mapping: "deferred; legacy ALTER migrations remain Python-owned",
                status: "deferred",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/schema/templates_imports",
                rust_mapping: "deferred to import/template domain slices",
                status: "deferred",
            },
            SchemaResponsibility {
                python_path: "src/bill_analyser/core/database/schema/users_security.py",
                rust_mapping: "deferred to auth/security domain slice",
                status: "deferred",
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
