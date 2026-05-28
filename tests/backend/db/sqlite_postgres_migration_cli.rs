use std::{
    error::Error,
    process::{Command, Output},
};

use bill_analyser_db::{
    run_postgres_migrations, write_sqlite_to_postgres_json, SqliteToPostgresExportBundle,
};
use rusqlite::Connection;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;

#[test]
fn sqlite_to_postgres_cli_runs_help_export_and_import_check() -> Result<(), Box<dyn Error>> {
    let bin = env!("CARGO_BIN_EXE_bill_sqlite_to_postgres_migrate");

    let help = Command::new(bin).arg("--help").output()?;
    assert!(
        help.status.success(),
        "help failed: {}",
        String::from_utf8_lossy(&help.stderr)
    );
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));

    let temp_dir = tempfile::tempdir()?;
    let sqlite_path = temp_dir.path().join("source.db");
    Connection::open(&sqlite_path)?;

    let dry_run = run_cli([
        "--mode",
        "dry-run",
        "--sqlite",
        sqlite_path.to_str().expect("sqlite path is utf-8"),
    ])?;
    let dry_run_json: Value = serde_json::from_slice(&dry_run.stdout)?;
    assert_eq!(dry_run_json["schema_version"], 1);

    let bundle_path = temp_dir.path().join("bundle.json");
    let export = run_cli([
        "--mode",
        "export",
        "--sqlite",
        sqlite_path.to_str().expect("sqlite path is utf-8"),
        "--output",
        bundle_path.to_str().expect("bundle path is utf-8"),
    ])?;
    assert!(export.stdout.is_empty());
    assert!(bundle_path.exists());

    let import_check = run_cli([
        "--mode",
        "import-check",
        "--bundle",
        bundle_path.to_str().expect("bundle path is utf-8"),
    ])?;
    let import_check_json: Value = serde_json::from_slice(&import_check.stdout)?;
    assert_eq!(import_check_json["imported_rows"], 0);

    let missing_url = run_cli_error([
        "--mode",
        "import",
        "--bundle",
        bundle_path.to_str().expect("bundle path is utf-8"),
    ])?;
    assert!(missing_url.contains("import requires --postgres-url"));

    let connection_error = run_cli_error([
        "--mode",
        "import",
        "--bundle",
        bundle_path.to_str().expect("bundle path is utf-8"),
        "--postgres-url",
        "not-a-postgres-url",
    ])?;
    assert!(connection_error.contains("connect Postgres for import"));

    let error = Command::new(bin).args(["--mode", "apply"]).output()?;
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("unsupported --mode"));
    Ok(())
}

#[tokio::test]
async fn sqlite_to_postgres_cli_imports_when_test_url_is_set() -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;
    sqlx::query("TRUNCATE migration_audit_events RESTART IDENTITY")
        .execute(&pool)
        .await?;

    let temp_dir = tempfile::tempdir()?;
    let bundle_path = temp_dir.path().join("empty-bundle.json");
    write_sqlite_to_postgres_json(
        &bundle_path,
        &SqliteToPostgresExportBundle {
            schema_version: 1,
            table_count: 0,
            total_rows: 0,
            checksum: "empty".to_string(),
            tables: Vec::new(),
        },
    )?;

    let output = run_cli([
        "--mode",
        "import",
        "--bundle",
        bundle_path.to_str().expect("bundle path is utf-8"),
        "--postgres-url",
        postgres_url.as_str(),
    ])?;
    let import_json: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(import_json["imported_rows"], 0);

    let audit_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM migration_audit_events WHERE status = 'succeeded'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(audit_count, 1);
    Ok(())
}

fn run_cli<const N: usize>(args: [&str; N]) -> Result<Output, Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_bill_sqlite_to_postgres_migrate"))
        .args(args)
        .output()?;
    assert!(
        output.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

fn run_cli_error<const N: usize>(args: [&str; N]) -> Result<String, Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_bill_sqlite_to_postgres_migrate"))
        .args(args)
        .output()?;
    assert!(!output.status.success());
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}
