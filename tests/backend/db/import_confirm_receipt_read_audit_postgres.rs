use std::{env, error::Error, io, process::Command};

use bill_analyser_db::{audit_import_confirm_receipt_target_snapshot, PostgresPool};
use serde_json::{json, Value};

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for confirm receipt read audit tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for confirm receipt read audit tests",
            )
            .into()
        })
}

fn receipt_payload(fingerprint_character: char) -> Value {
    json!({
        "receipt_schema_version": 1,
        "command_fingerprint": fingerprint_character.to_string().repeat(64),
        "request_session_version": 3,
        "response_schema_version": 1,
        "http_status": 200,
        "success_envelope": {
            "success": true,
            "data": {"imported_count": 2, "skipped_count": 1, "errors": []}
        }
    })
}

async fn insert_receipt_session(
    pool: &PostgresPool,
    suffix: &str,
    fingerprint_character: char,
    include_metadata_receipt: bool,
) -> Result<(i64, i64, Value), sqlx::Error> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("receipt-read-audit-{suffix}"))
        .fetch_one(pool)
        .await?;
    let receipt = receipt_payload(fingerprint_character);
    let metadata = if include_metadata_receipt {
        json!({"confirm_receipt": receipt.clone()})
    } else {
        json!({})
    };
    let session_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO import_sessions (
            user_id, session_key, status, import_mode, metadata, version
        )
        VALUES ($1, $2, 'confirmed', 'preview', $3, 4)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(format!("receipt-read-audit-{suffix}"))
    .bind(metadata)
    .fetch_one(pool)
    .await?;
    Ok((user_id, session_id, receipt))
}

async fn insert_typed_receipt(
    pool: &PostgresPool,
    user_id: i64,
    session_id: i64,
    receipt: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO import_confirm_receipts (
            session_id, user_id, receipt_schema_version, command_fingerprint,
            request_session_version, response_schema_version, http_status, success_envelope
        )
        VALUES ($1, $2, 1, $3, 3, 1, 200, $4)
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(receipt["command_fingerprint"].as_str().unwrap())
    .bind(receipt["success_envelope"].clone())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_target_receipt(
    pool: &PostgresPool,
    suffix: &str,
    fingerprint_character: char,
) -> Result<i64, sqlx::Error> {
    let (user_id, session_id, receipt) =
        insert_receipt_session(pool, suffix, fingerprint_character, true).await?;
    insert_typed_receipt(pool, user_id, session_id, &receipt).await?;
    Ok(session_id)
}

fn isolated_database_url(database_name: &str) -> Result<String, Box<dyn Error>> {
    let base_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
    let (server_url, _) = base_url
        .rsplit_once('/')
        .ok_or("test PostgreSQL URL must include a database path")?;
    Ok(format!("{server_url}/{database_name}"))
}

#[tokio::test]
async fn target_audit_covers_matching_receipts_in_bounded_read_only_batches(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_read_audit_match").await?;
    let result = async {
        let first_id = insert_target_receipt(&isolated.pool, "first", 'a').await?;
        insert_target_receipt(&isolated.pool, "second", 'b').await?;
        let third_id = insert_target_receipt(&isolated.pool, "third", 'c').await?;
        let before: Value = sqlx::query_scalar(
            "SELECT jsonb_build_object('sessions', (SELECT COUNT(*) FROM import_sessions), 'receipts', (SELECT COUNT(*) FROM import_confirm_receipts))",
        )
        .fetch_one(&isolated.pool)
        .await?;

        let report = audit_import_confirm_receipt_target_snapshot(&isolated.pool, 2).await?;

        assert!(report.is_match());
        assert_eq!(report.expected_migration_version, 28);
        assert_eq!(report.expected_migration_count, 28);
        assert_eq!(report.actual_migration_version, 28);
        assert_eq!(report.migration_count, 28);
        assert!(!report.snapshot_token.is_empty());
        assert_eq!(report.batch_size, 2);
        assert_eq!(report.target_sessions, 3);
        assert_eq!(report.typed_receipts, 3);
        assert_eq!(report.materialized_target_receipts, 3);
        assert_eq!(report.unmaterialized_target_receipts, 0);
        assert_eq!(report.unexpected_typed_receipts, 0);
        assert_eq!(report.mismatch_receipts, 0);
        assert_eq!(report.batches.len(), 2);
        assert_eq!(report.batches[0].start_after_session_id, 0);
        assert_eq!(report.batches[0].target_sessions, 2);
        assert_eq!(report.batches[1].target_sessions, 1);
        assert_eq!(report.batches[1].next_after_session_id, third_id);
        assert!(report.batches[0].next_after_session_id > first_id);
        assert!(report.duration_ms <= 60_000);

        let after: Value = sqlx::query_scalar(
            "SELECT jsonb_build_object('sessions', (SELECT COUNT(*) FROM import_sessions), 'receipts', (SELECT COUNT(*) FROM import_confirm_receipts))",
        )
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(after, before, "audit must remain read-only");
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn target_audit_separates_missing_mismatched_and_unexpected_typed_receipts(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_read_audit_drift").await?;
    let result = async {
        insert_target_receipt(&isolated.pool, "matching", 'a').await?;
        insert_receipt_session(&isolated.pool, "missing", 'b', true).await?;

        let (mismatch_user, mismatch_session, _) =
            insert_receipt_session(&isolated.pool, "mismatch", 'c', true).await?;
        let mismatched_typed = receipt_payload('d');
        insert_typed_receipt(
            &isolated.pool,
            mismatch_user,
            mismatch_session,
            &mismatched_typed,
        )
        .await?;

        let (unexpected_user, unexpected_session, unexpected_receipt) =
            insert_receipt_session(&isolated.pool, "unexpected", 'e', false).await?;
        insert_typed_receipt(
            &isolated.pool,
            unexpected_user,
            unexpected_session,
            &unexpected_receipt,
        )
        .await?;

        let report = audit_import_confirm_receipt_target_snapshot(&isolated.pool, 10).await?;

        assert!(!report.is_match());
        assert_eq!(report.target_sessions, 3);
        assert_eq!(report.typed_receipts, 3);
        assert_eq!(report.materialized_target_receipts, 2);
        assert_eq!(report.unmaterialized_target_receipts, 1);
        assert_eq!(report.unexpected_typed_receipts, 1);
        assert_eq!(report.mismatch_receipts, 1);
        assert_eq!(report.batches.len(), 1);
        assert_eq!(report.batches[0].target_sessions, 3);
        assert_eq!(report.batches[0].materialized_target_receipts, 2);
        assert_eq!(report.batches[0].unmaterialized_target_receipts, 1);
        assert_eq!(report.batches[0].mismatch_receipts, 1);
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[test]
fn read_audit_cli_help_is_json_and_does_not_require_database_access() -> Result<(), Box<dyn Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_bill_import_confirm_receipt_read_audit"))
        .arg("--help")
        .env_remove("BILL_ANALYSER_POSTGRES_URL")
        .output()?;
    assert!(output.status.success());
    let help: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        help["usage"],
        "bill_import_confirm_receipt_read_audit [--batch-size <1..10000>]"
    );
    assert_eq!(help["env"], json!(["BILL_ANALYSER_POSTGRES_URL"]));
    assert_eq!(help["runs_migrations"], false);
    assert_eq!(help["writes_business_data"], false);
    assert_eq!(
        help["snapshot"],
        "One REPEATABLE READ, READ ONLY transaction; no checkpoint resume."
    );
    Ok(())
}

#[tokio::test]
async fn target_audit_fails_closed_for_invalid_metadata_ledger_and_batch_limits(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_read_audit_invalid").await?;
    let result = async {
        let user_id: i64 =
            sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
                .bind("receipt-read-audit-invalid")
                .fetch_one(&isolated.pool)
                .await?;
        sqlx::query(
            r#"
            INSERT INTO import_sessions (
                user_id, session_key, status, import_mode, metadata, version
            )
            VALUES ($1, 'receipt-read-audit-invalid', 'confirmed', 'preview',
                    '{"confirm_receipt":{"receipt_schema_version":1}}'::jsonb, 4)
            "#,
        )
        .bind(user_id)
        .execute(&isolated.pool)
        .await?;

        let invalid = audit_import_confirm_receipt_target_snapshot(&isolated.pool, 10)
            .await
            .expect_err("invalid legacy receipt must fail closed");
        assert!(invalid
            .to_string()
            .contains("confirmed import session receipt is invalid"));

        for batch_size in [0, 10_001] {
            assert!(
                audit_import_confirm_receipt_target_snapshot(&isolated.pool, batch_size)
                    .await
                    .is_err(),
                "invalid batch size must fail closed: {batch_size}"
            );
        }

        sqlx::query("DELETE FROM _sqlx_migrations WHERE version=27")
            .execute(&isolated.pool)
            .await?;
        let ledger = audit_import_confirm_receipt_target_snapshot(&isolated.pool, 10)
            .await
            .expect_err("audit must reject a stale migration ledger before scanning receipts");
        assert!(ledger
            .to_string()
            .contains("requires exact migration ledger"));
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn read_audit_cli_emits_one_safe_report_and_fails_nonzero_on_drift(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_read_audit_cli").await?;
    let result = async {
        insert_target_receipt(&isolated.pool, "cli-match", 'f').await?;
        let database_url = isolated_database_url(&isolated.db_name)?;
        let binary = env!("CARGO_BIN_EXE_bill_import_confirm_receipt_read_audit");

        let success = Command::new(binary)
            .arg("--batch-size=1")
            .env("BILL_ANALYSER_POSTGRES_URL", &database_url)
            .output()?;
        assert!(
            success.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&success.stderr)
        );
        let success_lines = String::from_utf8(success.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(success_lines.len(), 1);
        assert_eq!(
            success_lines[0]["event"],
            "import_confirm_receipt_read_target_audit"
        );
        assert_eq!(success_lines[0]["eligible_for_typed_read_cutover"], true);
        assert_eq!(success_lines[0]["target_sessions"], 1);
        assert_eq!(success_lines[0]["typed_receipts"], 1);
        let success_stdout = serde_json::to_string(&success_lines[0])?;
        assert!(!success_stdout.contains("command_fingerprint"));
        assert!(!success_stdout.contains("success_envelope"));

        insert_receipt_session(&isolated.pool, "cli-missing", 'e', true).await?;
        let drift = Command::new(binary)
            .arg("--batch-size=1")
            .env("BILL_ANALYSER_POSTGRES_URL", &database_url)
            .output()?;
        assert!(!drift.status.success());
        let drift_stdout = String::from_utf8(drift.stdout)?;
        let drift_lines = drift_stdout
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(drift_lines.len(), 1, "one snapshot emits one final report");
        assert_eq!(drift_lines[0]["eligible_for_typed_read_cutover"], false);
        assert_eq!(drift_lines[0]["unmaterialized_target_receipts"], 1);
        let drift_stderr = String::from_utf8(drift.stderr)?;
        for secret in [&database_url, &"e".repeat(64), &"f".repeat(64)] {
            assert!(!drift_stdout.contains(secret));
            assert!(!drift_stderr.contains(secret));
        }

        let missing_env = Command::new(binary)
            .env_remove("BILL_ANALYSER_POSTGRES_URL")
            .output()?;
        assert!(!missing_env.status.success());
        let error: Value = serde_json::from_slice(&missing_env.stderr)?;
        assert_eq!(error["event"], "error");
        assert_eq!(error["message"], "BILL_ANALYSER_POSTGRES_URL is required");
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
