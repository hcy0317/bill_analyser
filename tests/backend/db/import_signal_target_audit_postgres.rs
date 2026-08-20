use std::{env, error::Error, io, process::Command};

use bill_analyser_db::{
    audit_import_preview_signal_target_snapshot, backfill_import_preview_signal_projection_batch,
    PostgresPool,
};
use serde_json::{json, Value};

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal target audit tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal target audit tests",
            )
            .into()
        })
}

async fn create_session(pool: &PostgresPool, suffix: &str) -> Result<(i64, i64), sqlx::Error> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("signal-target-audit-{suffix}"))
        .fetch_one(pool)
        .await?;
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("signal-target-audit-{suffix}"))
    .fetch_one(pool)
    .await?;
    Ok((user_id, session_id))
}

async fn insert_legacy_preview(
    pool: &PostgresPool,
    user_id: i64,
    session_id: i64,
    sort_key: &str,
    payload: Value,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO import_preview_rows (session_id,user_id,page_sort_key,operation_kind,occurred_at,amount_cents,direction,preview_payload) VALUES ($1,$2,$3,'insert',now(),-100,'expense',$4) RETURNING id",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(sort_key)
    .bind(payload)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn target_audit_covers_every_row_and_bounded_query_corpus_in_one_snapshot(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_target_audit_match").await?;
    let result = async {
        let (parser_user, parser_session) = create_session(&isolated.pool, "parser").await?;
        for index in 0..2 {
            insert_legacy_preview(
                &isolated.pool,
                parser_user,
                parser_session,
                &format!("parser-{index}"),
                json!({
                    "preview_parser_id": "wechat",
                    "preview_source_account_id": 11,
                    "preview_matching_feedback": {}
                }),
            )
            .await?;
        }
        let (transfer_user, transfer_session) = create_session(&isolated.pool, "transfer").await?;
        insert_legacy_preview(
            &isolated.pool,
            transfer_user,
            transfer_session,
            "transfer",
            json!({
                "preview_type": "支出",
                "preview_source_account_id": 11,
                "preview_matching_feedback": {
                    "transfer": {
                        "review_status": "pending",
                        "candidate_type": "transfer",
                        "learning_level": "green"
                    },
                    "learning": {
                        "review_status": "skipped",
                        "reason": "transfer preview is protected from learning type/category overrides"
                    }
                }
            }),
        )
        .await?;
        let backfill =
            backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 100).await?;
        assert_eq!(backfill.updated_rows, 3);

        let report =
            audit_import_preview_signal_target_snapshot(&isolated.pool, 2, 25).await?;

        assert!(report.is_match());
        assert_eq!(report.expected_migration_version, 28);
        assert_eq!(report.actual_migration_version, 28);
        assert_eq!(report.migration_count, 28);
        assert!(!report.snapshot_token.is_empty());
        assert_eq!(report.preview_rows, 3);
        assert_eq!(report.sessions, 2);
        assert_eq!(report.row_batches.len(), 2);
        assert_eq!(report.row_batches[0].observed_rows, 2);
        assert_eq!(report.row_batches[1].observed_rows, 1);
        assert_eq!(report.unmaterialized_rows, 0);
        assert_eq!(report.row_mismatch_rows, 0);
        assert_eq!(report.query_audit_sessions, 2);
        assert_eq!(report.query_audit_cases, 14);
        assert_eq!(report.query_mismatch_cases, 0);
        assert_eq!(report.query_mismatch_dimensions, 0);

        let versions: Vec<i16> = sqlx::query_scalar(
            "SELECT signal_projection_version FROM import_preview_rows ORDER BY id",
        )
        .fetch_all(&isolated.pool)
        .await?;
        assert_eq!(versions, vec![1, 1, 1], "audit must remain read-only");

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn target_audit_reports_unmaterialized_and_drift_without_claiming_query_parity(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_target_audit_drift").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "drift").await?;
        let preview_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "parser",
            json!({
                "preview_parser_id": "wechat",
                "preview_source_account_id": 11,
                "preview_matching_feedback": {}
            }),
        )
        .await?;

        let unmaterialized =
            audit_import_preview_signal_target_snapshot(&isolated.pool, 10, 25).await?;
        assert!(!unmaterialized.is_match());
        assert_eq!(unmaterialized.preview_rows, 1);
        assert_eq!(unmaterialized.unmaterialized_rows, 1);
        assert_eq!(unmaterialized.row_mismatch_rows, 0);
        assert_eq!(unmaterialized.query_audit_sessions, 0);
        assert_eq!(unmaterialized.query_audit_cases, 0);

        backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 100).await?;
        sqlx::query("UPDATE import_preview_rows SET signal_parser=false WHERE id=$1")
            .bind(preview_id)
            .execute(&isolated.pool)
            .await?;

        let drift = audit_import_preview_signal_target_snapshot(&isolated.pool, 10, 25).await?;
        assert!(!drift.is_match());
        assert_eq!(drift.unmaterialized_rows, 0);
        assert_eq!(drift.row_mismatch_rows, 1);
        assert_eq!(drift.query_audit_sessions, 0);
        assert_eq!(drift.query_audit_cases, 0);

        for (row_batch_size, query_page_size) in [(0, 25), (10_001, 25), (10, 0), (10, 1_001)] {
            assert!(
                audit_import_preview_signal_target_snapshot(
                    &isolated.pool,
                    row_batch_size,
                    query_page_size,
                )
                .await
                .is_err(),
                "invalid audit limits must fail closed: {row_batch_size}/{query_page_size}"
            );
        }

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn target_audit_cli_emits_one_committed_snapshot_report_and_is_secret_safe(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_target_audit_cli").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "cli").await?;
        insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "parser",
            json!({
                "preview_parser_id": "wechat",
                "preview_source_account_id": 11,
                "preview_matching_feedback": {}
            }),
        )
        .await?;
        backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 100).await?;

        let base_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
        let (server_url, _) = base_url
            .rsplit_once('/')
            .ok_or("test PostgreSQL URL must include a database path")?;
        let isolated_url = format!("{server_url}/{}", isolated.db_name);
        let output = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_read_audit"))
            .args(["--row-batch-size=1", "--query-page-size", "25"])
            .env("BILL_ANALYSER_POSTGRES_URL", &isolated_url)
            .output()?;
        assert!(
            output.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let lines = String::from_utf8(output.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines.len(), 1, "one snapshot must emit one final report");
        let report = &lines[0];
        assert_eq!(report["event"], "import_signal_read_target_audit");
        assert_eq!(report["eligible_for_read_cutover"], true);
        assert_eq!(report["row_batch_size"], 1);
        assert_eq!(report["query_page_size"], 25);
        assert_eq!(report["preview_rows"], 1);
        assert_eq!(report["unmaterialized_rows"], 0);
        assert_eq!(report["row_mismatch_rows"], 0);

        let help = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_read_audit"))
            .arg("--help")
            .env_remove("BILL_ANALYSER_POSTGRES_URL")
            .output()?;
        assert!(help.status.success());
        let help_json: Value = serde_json::from_slice(&help.stdout)?;
        assert_eq!(help_json["env"], json!(["BILL_ANALYSER_POSTGRES_URL"]));
        assert_eq!(help_json["writes_business_data"], false);

        let missing_env = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_read_audit"))
            .env_remove("BILL_ANALYSER_POSTGRES_URL")
            .output()?;
        assert!(!missing_env.status.success());
        let error: Value = serde_json::from_slice(&missing_env.stderr)?;
        assert_eq!(error["event"], "error");
        assert_eq!(error["message"], "BILL_ANALYSER_POSTGRES_URL is required");
        assert!(!String::from_utf8_lossy(&missing_env.stderr).contains(&isolated_url));

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
