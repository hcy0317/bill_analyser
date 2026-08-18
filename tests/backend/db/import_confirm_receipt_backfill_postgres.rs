use std::{env, error::Error, io, process::Command};

use bill_analyser_db::{backfill_import_confirm_receipt_batch, PostgresPool};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for confirm receipt backfill tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for confirm receipt backfill tests",
            )
            .into()
        })
}

async fn insert_metadata_only_receipt_session(
    pool: &PostgresPool,
    suffix: &str,
) -> Result<(i64, i64, DateTime<Utc>, Value), sqlx::Error> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("receipt-backfill-{suffix}"))
        .fetch_one(pool)
        .await?;
    let receipt = json!({
        "receipt_schema_version": 1,
        "command_fingerprint": "a".repeat(64),
        "request_session_version": 3,
        "response_schema_version": 1,
        "http_status": 200,
        "success_envelope": {
            "success": true,
            "data": {
                "imported_count": 2,
                "skipped_count": 1,
                "errors": []
            }
        }
    });
    let row = sqlx::query(
        r#"
        INSERT INTO import_sessions (
            user_id, session_key, status, import_mode, metadata, version, updated_at
        )
        VALUES ($1, $2, 'confirmed', 'preview', jsonb_build_object('confirm_receipt', $3::jsonb), 4, $4)
        RETURNING id, updated_at
        "#,
    )
    .bind(user_id)
    .bind(format!("receipt-backfill-{suffix}"))
    .bind(receipt.to_string())
    .bind(DateTime::parse_from_rfc3339("2026-08-01T12:34:56+08:00").unwrap())
    .fetch_one(pool)
    .await?;
    Ok((
        user_id,
        row.try_get("id")?,
        row.try_get("updated_at")?,
        receipt,
    ))
}

#[tokio::test]
async fn historical_metadata_receipt_backfills_one_typed_row_with_confirm_timestamp(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_backfill_first").await?;
    let result = async {
        let (user_id, session_id, confirmed_at, receipt) =
            insert_metadata_only_receipt_session(&isolated.pool, "first").await?;

        let report = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 10).await?;

        assert_eq!(report.start_after_session_id, 0);
        assert_eq!(report.next_after_session_id, session_id);
        assert_eq!(report.scanned_sessions, 1);
        assert_eq!(report.inserted_receipts, 1);
        assert_eq!(report.already_materialized_receipts, 0);
        assert_eq!(report.mismatch_receipts, 0);
        assert!(!report.has_remaining_sessions);

        let typed = sqlx::query(
            r#"
            SELECT user_id, receipt_schema_version, command_fingerprint,
                   request_session_version, response_schema_version, http_status,
                   success_envelope, created_at
            FROM import_confirm_receipts
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(typed.try_get::<i64, _>("user_id")?, user_id);
        assert_eq!(typed.try_get::<i16, _>("receipt_schema_version")?, 1);
        assert_eq!(
            typed.try_get::<String, _>("command_fingerprint")?,
            "a".repeat(64)
        );
        assert_eq!(typed.try_get::<i64, _>("request_session_version")?, 3);
        assert_eq!(typed.try_get::<i16, _>("response_schema_version")?, 1);
        assert_eq!(typed.try_get::<i16, _>("http_status")?, 200);
        assert_eq!(
            typed.try_get::<Value, _>("success_envelope")?,
            receipt["success_envelope"]
        );
        assert_eq!(
            typed.try_get::<DateTime<Utc>, _>("created_at")?,
            confirmed_at
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn backfill_resumes_reports_existing_rows_and_rejects_a_skipped_gap(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_backfill_resume").await?;
    let result = async {
        let (_, first_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "resume-first").await?;
        let (_, second_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "resume-second").await?;
        let (_, third_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "resume-third").await?;

        let first = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 2).await?;
        assert_eq!(first.next_after_session_id, second_id);
        assert_eq!(first.scanned_sessions, 2);
        assert_eq!(first.inserted_receipts, 2);
        assert!(first.has_remaining_sessions);

        let second = backfill_import_confirm_receipt_batch(&isolated.pool, second_id, 2).await?;
        assert_eq!(second.next_after_session_id, third_id);
        assert_eq!(second.scanned_sessions, 1);
        assert_eq!(second.inserted_receipts, 1);
        assert!(!second.has_remaining_sessions);

        let replay = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 10).await?;
        assert_eq!(replay.scanned_sessions, 3);
        assert_eq!(replay.inserted_receipts, 0);
        assert_eq!(replay.already_materialized_receipts, 3);

        let exhausted = backfill_import_confirm_receipt_batch(&isolated.pool, third_id, 10).await?;
        assert_eq!(exhausted.scanned_sessions, 0);
        assert_eq!(exhausted.next_after_session_id, third_id);

        sqlx::query("DELETE FROM import_confirm_receipts WHERE session_id=$1")
            .bind(first_id)
            .execute(&isolated.pool)
            .await?;
        let gap = backfill_import_confirm_receipt_batch(&isolated.pool, second_id, 10)
            .await
            .expect_err("checkpoint must not skip a deleted lower receipt");
        assert!(gap.to_string().contains("would skip an unfilled receipt"));
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn invalid_metadata_receipt_rolls_back_the_whole_batch() -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_backfill_invalid").await?;
    let result = async {
        let (_, valid_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "invalid-valid").await?;
        let (_, invalid_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "invalid-schema").await?;
        sqlx::query(
            "UPDATE import_sessions SET metadata=jsonb_set(metadata,'{confirm_receipt,receipt_schema_version}','2'::jsonb) WHERE id=$1",
        )
        .bind(invalid_id)
        .execute(&isolated.pool)
        .await?;

        let error = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 10)
            .await
            .expect_err("invalid receipt schema must fail closed");
        assert!(error
            .to_string()
            .contains("unsupported import confirm receipt schema"));
        let typed_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM import_confirm_receipts WHERE session_id=ANY($1)",
        )
        .bind(vec![valid_id, invalid_id])
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(typed_rows, 0);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn existing_typed_mismatch_fails_without_materializing_other_sessions(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_backfill_mismatch").await?;
    let result = async {
        let (user_id, mismatch_id, confirmed_at, receipt) =
            insert_metadata_only_receipt_session(&isolated.pool, "mismatch-existing").await?;
        let (_, missing_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "mismatch-missing").await?;
        sqlx::query(
            r#"
            INSERT INTO import_confirm_receipts (
                session_id,user_id,receipt_schema_version,command_fingerprint,
                request_session_version,response_schema_version,http_status,
                success_envelope,created_at
            )
            VALUES ($1,$2,1,$3,3,1,200,$4,$5)
            "#,
        )
        .bind(mismatch_id)
        .bind(user_id)
        .bind("b".repeat(64))
        .bind(receipt["success_envelope"].clone())
        .bind(confirmed_at)
        .execute(&isolated.pool)
        .await?;

        let error = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 10)
            .await
            .expect_err("typed mismatch must fail closed");
        assert!(error.to_string().contains("metadata/typed mismatches"));
        let missing_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM import_confirm_receipts WHERE session_id=$1",
        )
        .bind(missing_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(missing_rows, 0);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn post_insert_parity_mismatch_rolls_back_the_insert() -> Result<(), Box<dyn Error>> {
    let isolated =
        strict_isolated_postgres_database("confirm_receipt_backfill_post_parity").await?;
    let result = async {
        let (_, session_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "post-parity").await?;
        sqlx::query(
            r#"
            CREATE FUNCTION c6c_corrupt_backfill_insert() RETURNS trigger AS $$
            BEGIN
                NEW.success_envelope = '{"success":false}'::jsonb;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql
            "#,
        )
        .execute(&isolated.pool)
        .await?;
        sqlx::query(
            "CREATE TRIGGER c6c_corrupt_backfill_insert BEFORE INSERT ON import_confirm_receipts FOR EACH ROW EXECUTE FUNCTION c6c_corrupt_backfill_insert()",
        )
        .execute(&isolated.pool)
        .await?;

        let error = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 10)
            .await
            .expect_err("stored typed values must be re-read before commit");
        assert!(error.to_string().contains("post-insert parity"));
        let typed_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM import_confirm_receipts WHERE session_id=$1",
        )
        .bind(session_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(typed_rows, 0);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn backfill_requires_the_exact_binary_migration_ledger() -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_backfill_ledger").await?;
    let result = async {
        insert_metadata_only_receipt_session(&isolated.pool, "ledger").await?;
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version=27")
            .execute(&isolated.pool)
            .await?;

        let error = backfill_import_confirm_receipt_batch(&isolated.pool, 0, 10)
            .await
            .expect_err("backfill must not run against a stale migration ledger");
        assert!(error
            .to_string()
            .contains("requires exact migration ledger"));
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[test]
fn backfill_cli_help_is_json_and_does_not_require_database_access() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_bill_import_confirm_receipt_backfill"))
        .arg("--help")
        .env_remove("BILL_ANALYSER_POSTGRES_URL")
        .output()?;
    assert!(output.status.success());
    let help: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        help["usage"],
        "bill_import_confirm_receipt_backfill [--after-session-id <id>] [--batch-size <1..1000>] [--max-batches <count>] [--sleep-ms <milliseconds>]"
    );
    assert_eq!(help["env"], json!(["BILL_ANALYSER_POSTGRES_URL"]));
    assert_eq!(help["runs_migrations"], false);
    assert_eq!(help["writes_business_data"], true);
    Ok(())
}

#[tokio::test]
async fn backfill_cli_emits_one_committed_json_checkpoint() -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_backfill_cli").await?;
    let result = async {
        let (_, session_id, _, _) =
            insert_metadata_only_receipt_session(&isolated.pool, "cli").await?;
        let base_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
        let (server_url, _) = base_url
            .rsplit_once('/')
            .ok_or("test PostgreSQL URL must include a database path")?;
        let isolated_url = format!("{server_url}/{}", isolated.db_name);

        let output = Command::new(env!("CARGO_BIN_EXE_bill_import_confirm_receipt_backfill"))
            .args(["--batch-size=1", "--max-batches=1", "--sleep-ms=0"])
            .env("BILL_ANALYSER_POSTGRES_URL", isolated_url)
            .output()?;
        assert!(
            output.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let lines = String::from_utf8(output.stdout)?
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        let checkpoint: Value = serde_json::from_str(&lines[0])?;
        assert_eq!(checkpoint["event"], "import_confirm_receipt_backfill_batch");
        assert_eq!(checkpoint["batch_number"], 1);
        assert_eq!(checkpoint["next_after_session_id"], session_id);
        assert_eq!(checkpoint["scanned_sessions"], 1);
        assert_eq!(checkpoint["inserted_receipts"], 1);
        assert_eq!(checkpoint["already_materialized_receipts"], 0);
        assert_eq!(checkpoint["mismatch_receipts"], 0);
        assert_eq!(checkpoint["has_remaining_sessions"], false);
        assert!(checkpoint["duration_ms"].is_u64());
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
