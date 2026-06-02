use std::{
    error::Error,
    process::{Command, Output},
};

use bill_analyser_db::{
    apply_postgres_account_recovery, build_postgres_account_recovery_dry_run,
    run_postgres_migrations, AccountRecoveryApplyOptions, AccountRecoveryTargetRef,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;

#[test]
fn postgres_account_recovery_cli_runs_help_and_preflight() -> Result<(), Box<dyn Error>> {
    let bin = env!("CARGO_BIN_EXE_bill_postgres_account_recovery");

    let help = Command::new(bin).arg("--help").output()?;
    assert!(
        help.status.success(),
        "help failed: {}",
        String::from_utf8_lossy(&help.stderr)
    );
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));

    let temp_dir = tempfile::tempdir()?;
    let sqlite_path = temp_dir.path().join("source.db");
    let connection = Connection::open(&sqlite_path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT,
            password_hash TEXT
        );
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            aliases TEXT
        );
        INSERT INTO users VALUES (5, 'source', 'source@example.test', 'source-hash');
        INSERT INTO accounts VALUES (42, 5, 'Wallet', '["Cash", "零钱"]');
        "#,
    )?;
    drop(connection);

    let preflight = run_cli([
        "--mode",
        "preflight",
        "--sqlite",
        sqlite_path.to_str().expect("sqlite path is utf-8"),
    ])?;
    let preflight_json: Value = serde_json::from_slice(&preflight.stdout)?;
    assert_eq!(preflight_json["source_user_id"], 5);
    assert_eq!(preflight_json["counts"]["accounts"], 1);
    assert_eq!(preflight_json["counts"]["generated_account_rules"], 2);
    assert!(preflight_json["source_identity_hash"]
        .as_str()
        .is_some_and(|value| value.len() == 64));

    let missing_target = run_cli_error([
        "--mode",
        "dry-run",
        "--sqlite",
        sqlite_path.to_str().expect("sqlite path is utf-8"),
        "--postgres-url",
        "not-a-postgres-url",
    ])?;
    assert!(missing_target.contains("dry-run requires --target-user"));

    let missing_manifest = run_cli_error([
        "--mode",
        "apply",
        "--sqlite",
        sqlite_path.to_str().expect("sqlite path is utf-8"),
        "--postgres-url",
        "not-a-postgres-url",
        "--target-user",
        "target",
    ])?;
    assert!(missing_manifest.contains("apply requires --manifest-id"));
    Ok(())
}

fn run_cli<const N: usize>(args: [&str; N]) -> Result<Output, Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_bill_postgres_account_recovery"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_bill_postgres_account_recovery"))
        .args(args)
        .output()?;
    assert!(!output.status.success());
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

#[tokio::test]
async fn postgres_account_recovery_apply_runs_in_target_user_scope_when_test_url_is_set(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;
    refresh_users_identity_sequence_for_test(&pool).await?;

    let suffix = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let target_username = format!("recovery-target-{suffix}");
    let target_email = format!("recovery-target-{suffix}@example.test");
    let other_username = format!("recovery-other-{suffix}");
    let other_email = format!("recovery-other-{suffix}@example.test");
    let target_user_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, email, password_hash, metadata) VALUES ($1, $2, 'target-hash', $3) RETURNING id",
    )
    .bind(&target_username)
    .bind(&target_email)
    .bind(Json(json!({
        "is_active": true,
        "failed_login_attempts": 5,
        "locked_until": "2026-06-02T15:40:26.917016700"
    })))
    .fetch_one(&pool)
    .await?;
    let other_user_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, 'hash') RETURNING id",
    )
    .bind(&other_username)
    .bind(&other_email)
    .fetch_one(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO accounts (user_id, name, currency, balance_cents) VALUES ($1, 'old target account', 'CNY', 1)",
    )
    .bind(target_user_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO parser_templates (user_id, parser_id, feature_signature) VALUES ($1, 'manual-parser', 'manual-parser:keep')",
    )
    .bind(target_user_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO transaction_templates (user_id, legacy_id, template_type, name) VALUES ($1, 10001, 1, 'keep template')",
    )
    .bind(target_user_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO accounts (user_id, name, currency, balance_cents) VALUES ($1, 'other account', 'CNY', 2)",
    )
    .bind(other_user_id)
    .execute(&pool)
    .await?;

    let temp_dir = tempfile::tempdir()?;
    let sqlite_path = temp_dir.path().join("source.db");
    create_apply_fixture(&sqlite_path)?;
    let target_ref = AccountRecoveryTargetRef::new(target_user_id.to_string())?;
    let dry_run =
        build_postgres_account_recovery_dry_run(&sqlite_path, 5, &pool, &target_ref).await?;
    let workspace_root = temp_dir.path().join("workspace");
    let snapshot_dir = workspace_root
        .join(".git")
        .join("ai")
        .join("recovery-snapshots")
        .join(&dry_run.manifest_id);

    let sync_output = Command::new(env!("CARGO_BIN_EXE_bill_postgres_account_recovery"))
        .env("BILL_ANALYSER_POSTGRES_URL", &postgres_url)
        .arg("--mode")
        .arg("sync-auth")
        .arg("--sqlite")
        .arg(&sqlite_path)
        .arg("--target-user")
        .arg(target_user_id.to_string())
        .arg("--confirm-target-user-id")
        .arg(target_user_id.to_string())
        .arg("--confirm-target-username")
        .arg(&target_username)
        .arg("--confirm-target-email")
        .arg(&target_email)
        .output()?;
    assert!(
        sync_output.status.success(),
        "sync-auth failed: {}",
        String::from_utf8_lossy(&sync_output.stderr)
    );
    let sync_report: Value = serde_json::from_slice(&sync_output.stdout)?;
    assert_eq!(sync_report["source_user_id"], 5);
    assert_eq!(sync_report["source_password_hash_present"], true);
    assert_eq!(sync_report["target_password_hash_replaced"], true);
    assert_eq!(sync_report["target_lockout_metadata_cleared"], true);
    let (synced_hash, synced_attempts, synced_locked_until): (String, i64, String) =
        sqlx::query_as(
            "SELECT password_hash, COALESCE((metadata->>'failed_login_attempts')::BIGINT, -1), COALESCE(metadata->>'locked_until', '') FROM users WHERE id = $1",
        )
        .bind(target_user_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(synced_hash, "source-hash");
    assert_eq!(synced_attempts, 0);
    assert_eq!(synced_locked_until, "");

    let report = apply_postgres_account_recovery(
        &sqlite_path,
        &pool,
        AccountRecoveryApplyOptions {
            source_user_id: 5,
            target_user_ref: target_ref,
            manifest_id: dry_run.manifest_id,
            confirm_target_user_id: target_user_id,
            confirm_target_username: target_username.clone(),
            confirm_target_email: target_email.clone(),
            snapshot_dir: snapshot_dir.clone(),
            workspace_root,
            allow_unexpected_source_shape: true,
        },
    )
    .await?;

    assert_eq!(report.source_counts.accounts, 1);
    assert!(report.allow_unexpected_source_shape);
    assert_eq!(report.target_post_counts.accounts, 1);
    assert_eq!(report.target_post_counts.bills, 1);
    assert_eq!(report.target_post_counts.account_rules, 1);
    assert!(snapshot_dir.join("manifest.redacted.json").exists());
    let post_apply_hash =
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE id = $1")
            .bind(target_user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(post_apply_hash, "source-hash");
    let target_old_accounts = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM accounts WHERE user_id = $1 AND name = 'old target account'",
    )
    .bind(target_user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(target_old_accounts, 0);
    let target_parser_templates =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM parser_templates WHERE user_id = $1")
            .bind(target_user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(target_parser_templates, 1);
    let target_transaction_templates = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transaction_templates WHERE user_id = $1",
    )
    .bind(target_user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(target_transaction_templates, 1);
    let other_accounts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts WHERE user_id = $1")
            .bind(other_user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(other_accounts, 1);

    sqlx::query("DELETE FROM users WHERE id IN ($1, $2)")
        .bind(target_user_id)
        .bind(other_user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn postgres_account_recovery_sync_auth_creates_missing_target_user(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;
    refresh_users_identity_sequence_for_test(&pool).await?;

    let suffix = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let target_user_id = 1_000_000 + suffix[suffix.len().saturating_sub(6)..].parse::<i64>()?;
    let target_username = format!("recovery-missing-target-{suffix}");
    let target_email = format!("recovery-missing-target-{suffix}@example.test");
    sqlx::query("DELETE FROM users WHERE id = $1 OR username = $2 OR email = $3")
        .bind(target_user_id)
        .bind(&target_username)
        .bind(&target_email)
        .execute(&pool)
        .await?;

    let temp_dir = tempfile::tempdir()?;
    let sqlite_path = temp_dir.path().join("source.db");
    create_apply_fixture(&sqlite_path)?;
    let sync_output = Command::new(env!("CARGO_BIN_EXE_bill_postgres_account_recovery"))
        .env("BILL_ANALYSER_POSTGRES_URL", &postgres_url)
        .arg("--mode")
        .arg("sync-auth")
        .arg("--sqlite")
        .arg(&sqlite_path)
        .arg("--target-user")
        .arg(&target_username)
        .arg("--confirm-target-user-id")
        .arg(target_user_id.to_string())
        .arg("--confirm-target-username")
        .arg(&target_username)
        .arg("--confirm-target-email")
        .arg(&target_email)
        .output()?;
    assert!(
        sync_output.status.success(),
        "sync-auth failed: {}",
        String::from_utf8_lossy(&sync_output.stderr)
    );
    let sync_report: Value = serde_json::from_slice(&sync_output.stdout)?;
    assert_eq!(sync_report["target"]["user_id"], target_user_id);
    assert_eq!(sync_report["source_password_hash_present"], true);
    assert_eq!(sync_report["target_password_hash_replaced"], true);

    let (username, email, password_hash, is_active, failed_attempts, locked_until): (
        String,
        Option<String>,
        String,
        bool,
        i64,
        String,
    ) = sqlx::query_as(
        "SELECT username, email, password_hash, COALESCE((metadata->>'is_active')::boolean, FALSE), COALESCE((metadata->>'failed_login_attempts')::BIGINT, -1), COALESCE(metadata->>'locked_until', '') FROM users WHERE id = $1",
    )
    .bind(target_user_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(username, target_username);
    assert_eq!(email.as_deref(), Some(target_email.as_str()));
    assert_eq!(password_hash, "source-hash");
    assert!(is_active);
    assert_eq!(failed_attempts, 0);
    assert_eq!(locked_until, "");

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(target_user_id)
        .execute(&pool)
        .await?;
    Ok(())
}

async fn refresh_users_identity_sequence_for_test(
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn Error>> {
    let sequence_name: Option<String> =
        sqlx::query_scalar("SELECT pg_get_serial_sequence('users', 'id')")
            .fetch_one(pool)
            .await?;
    if let Some(sequence_name) = sequence_name {
        sqlx::query(
            "SELECT setval($1, COALESCE((SELECT MAX(id) FROM users), 1), (SELECT MAX(id) FROM users) IS NOT NULL)",
        )
        .bind(sequence_name)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn create_apply_fixture(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT,
            password_hash TEXT
        );
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            type INTEGER,
            currency TEXT,
            balance REAL,
            aliases TEXT,
            hidden INTEGER,
            display_order INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            type INTEGER,
            main_category TEXT,
            sub_category TEXT,
            priority INTEGER,
            hidden INTEGER,
            created_at TEXT NOT NULL
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT,
            description TEXT,
            payment_method TEXT,
            main_category TEXT,
            sub_category TEXT,
            hash TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_account_id INTEGER,
            destination_account_id INTEGER
        );
        INSERT INTO users VALUES (5, 'source', 'source@example.test', 'source-hash');
        INSERT INTO accounts VALUES (42, 5, 'Wallet', 1, 'CNY', 123.45, '["Cash"]', 0, 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
        INSERT INTO categories VALUES (50, 5, 1, 'Food', 'Lunch', 1, 0, '2026-01-01T00:00:00Z');
        INSERT INTO bills VALUES (70, 5, '2026-01-03T12:00:00Z', '支出', 19.99, 'Cafe', 'Lunch', 'Wallet', 'Food', 'Lunch', 'hash70', '2026-01-03T12:01:00Z', '2026-01-03T12:02:00Z', 42, 0);
        "#,
    )?;
    Ok(())
}
