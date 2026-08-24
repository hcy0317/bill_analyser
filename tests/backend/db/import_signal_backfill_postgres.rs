use std::{env, error::Error, io, process::Command};

use bill_analyser_db::{backfill_import_preview_signal_projection_batch, DbError, PostgresPool};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal backfill tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal backfill tests",
            )
            .into()
        })
}

async fn create_session(pool: &PostgresPool, suffix: &str) -> Result<(i64, i64), sqlx::Error> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("signal-backfill-{suffix}"))
        .fetch_one(pool)
        .await?;
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("signal-backfill-{suffix}"))
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
async fn signal_backfill_is_resumable_and_never_overwrites_online_projection(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_backfill_resume").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "resume").await?;
        let payloads = [
            json!({"preview_parser_id":"wechat","preview_source_account_id":11,"preview_matching_feedback":{}}),
            json!({"preview_parser_id":"wechat","preview_source_account_id":11,"dedup_type":"platform_bank","preview_matching_feedback":{}}),
            json!({"preview_type":"支出","category_id":10,"preview_source_account_id":11,"preview_matching_feedback":{"transfer":{"review_status":"pending","candidate_type":"transfer","learning_level":"green"},"learning":{"review_status":"skipped","reason":"transfer preview is protected from learning type/category overrides"}}}),
            json!({"preview_matching_feedback":{}}),
        ];
        let mut legacy_ids = Vec::new();
        for (index, payload) in payloads.into_iter().enumerate() {
            legacy_ids.push(
                insert_legacy_preview(
                    &isolated.pool,
                    user_id,
                    session_id,
                    &format!("legacy-{index}"),
                    payload,
                )
                .await?,
            );
        }
        let protected_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "online-v1",
            json!({"preview_matching_feedback":{}}),
        )
        .await?;
        sqlx::query(
            "UPDATE import_preview_rows SET signal_parser=true,signal_platform_duplicate=true,signal_transfer=true,signal_history=true,signal_learning=true,signal_llm=true,signal_projection_version=1 WHERE id=$1",
        )
        .bind(protected_id)
        .execute(&isolated.pool)
        .await?;

        let first = backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 2).await?;
        assert_eq!(first.start_after_id, 0);
        assert_eq!(first.next_after_id, legacy_ids[1]);
        assert_eq!(first.scanned_rows, 2);
        assert_eq!(first.updated_rows, 2);
        assert_eq!(first.mismatch_rows, 0);
        assert!(first.has_remaining_rows);

        let second = backfill_import_preview_signal_projection_batch(
            &isolated.pool,
            first.next_after_id,
            2,
        )
        .await?;
        assert_eq!(second.next_after_id, legacy_ids[3]);
        assert_eq!(second.scanned_rows, 2);
        assert_eq!(second.updated_rows, 2);
        assert_eq!(second.mismatch_rows, 0);
        assert!(!second.has_remaining_rows);

        let terminal = backfill_import_preview_signal_projection_batch(
            &isolated.pool,
            second.next_after_id,
            2,
        )
        .await?;
        assert_eq!(terminal.next_after_id, second.next_after_id);
        assert_eq!(terminal.scanned_rows, 0);
        assert_eq!(terminal.updated_rows, 0);
        assert_eq!(terminal.mismatch_rows, 0);
        assert!(!terminal.has_remaining_rows);

        let rows = sqlx::query(
            "SELECT id,signal_parser,signal_platform_duplicate,signal_transfer,signal_history,signal_learning,signal_llm,signal_projection_version FROM import_preview_rows WHERE session_id=$1 ORDER BY id",
        )
        .bind(session_id)
        .fetch_all(&isolated.pool)
        .await?;
        assert_eq!(rows.len(), 5);
        assert!(rows[0].try_get::<bool, _>("signal_parser")?);
        assert!(rows[1].try_get::<bool, _>("signal_platform_duplicate")?);
        assert!(rows[2].try_get::<bool, _>("signal_transfer")?);
        assert!(rows[2].try_get::<bool, _>("signal_learning")?);
        for row in &rows {
            assert_eq!(row.try_get::<i16, _>("signal_projection_version")?, 1);
        }
        let protected = rows
            .iter()
            .find(|row| {
                row.try_get::<i64, _>("id")
                    .is_ok_and(|id| id == protected_id)
            })
            .expect("online projection row");
        for column in [
            "signal_parser",
            "signal_platform_duplicate",
            "signal_transfer",
            "signal_history",
            "signal_learning",
            "signal_llm",
        ] {
            assert!(protected.try_get::<bool, _>(column)?);
        }

        let mismatch_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "mismatch",
            json!({"preview_parser_id":"wechat","preview_source_account_id":11,"preview_matching_feedback":{}}),
        )
        .await?;
        sqlx::query(
            r#"CREATE OR REPLACE FUNCTION import_preview_signal_flags(payload JSONB)
               RETURNS JSONB LANGUAGE SQL IMMUTABLE PARALLEL SAFE
               AS $$ SELECT jsonb_build_object(
                   'parser',false,'platform_duplicate',false,'transfer',false,
                   'history',false,'learning',false,'llm',false
               ) $$"#,
        )
        .execute(&isolated.pool)
        .await?;
        let mismatch = backfill_import_preview_signal_projection_batch(
            &isolated.pool,
            terminal.next_after_id,
            1,
        )
        .await?;
        assert_eq!(mismatch.next_after_id, mismatch_id);
        assert_eq!(mismatch.updated_rows, 1);
        assert_eq!(mismatch.mismatch_rows, 1);
        assert!(!mismatch.has_remaining_rows);

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn signal_backfill_ignores_parser_details_without_parser_identity(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_parser_identity").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "parser-identity").await?;
        let preview_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "parser-detail-only",
            json!({
                "preview_parser_id": "",
                "preview_parser_tags": [],
                "preview_matching_feedback": {
                    "parser": {
                        "parser_id": "",
                        "parser_tags": [],
                        "counterparty": "legacy parser detail"
                    }
                }
            }),
        )
        .await?;

        let report =
            backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 10).await?;
        assert_eq!(report.updated_rows, 1);
        assert_eq!(report.mismatch_rows, 0);

        let row = sqlx::query(
            "SELECT signal_parser, import_preview_signal_flags(preview_payload)->>'parser' AS legacy_parser FROM import_preview_rows WHERE id=$1",
        )
        .bind(preview_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert!(!row.try_get::<bool, _>("signal_parser")?);
        assert_eq!(row.try_get::<String, _>("legacy_parser")?, "false");

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn signal_backfill_rejects_unsafe_watermarks_limits_and_poison_rows(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_backfill_guards").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "guards").await?;
        let first_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "first",
            json!({"preview_parser_id":"wechat","preview_source_account_id":11,"preview_matching_feedback":{}}),
        )
        .await?;
        let poison_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "poison",
            json!(["not", "a", "canonical", "row"]),
        )
        .await?;

        for invalid_limit in [0, 1_001] {
            let error = backfill_import_preview_signal_projection_batch(
                &isolated.pool,
                0,
                invalid_limit,
            )
            .await
            .expect_err("invalid limits must fail closed");
            assert!(matches!(error, DbError::InvalidOperation(_)));
        }
        let negative_watermark =
            backfill_import_preview_signal_projection_batch(&isolated.pool, -1, 1)
                .await
                .expect_err("negative watermarks must fail closed");
        assert!(matches!(negative_watermark, DbError::InvalidOperation(_)));

        let watermark_error = backfill_import_preview_signal_projection_batch(
            &isolated.pool,
            first_id,
            10,
        )
        .await
        .expect_err("a watermark above an unfilled row must fail closed");
        assert!(matches!(watermark_error, DbError::InvalidOperation(_)));

        let poison_error = backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 10)
            .await
            .expect_err("a malformed payload must roll back the whole batch");
        assert!(matches!(poison_error, DbError::InvalidOperation(_)));
        let versions: Vec<i16> = sqlx::query_scalar(
            "SELECT signal_projection_version FROM import_preview_rows WHERE id = ANY($1::bigint[]) ORDER BY id",
        )
        .bind(vec![first_id, poison_id])
        .fetch_all(&isolated.pool)
        .await?;
        assert_eq!(versions, vec![0, 0]);

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn signal_backfill_cli_emits_committed_json_checkpoint() -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_backfill_cli").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "cli").await?;
        let preview_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_id,
            "cli-row",
            json!({"preview_parser_id":"wechat","preview_source_account_id":11,"preview_matching_feedback":{}}),
        )
        .await?;
        let base_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
        let (server_url, _) = base_url
            .rsplit_once('/')
            .ok_or("test PostgreSQL URL must include a database path")?;
        let isolated_url = format!("{server_url}/{}", isolated.db_name);

        let output = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_backfill"))
            .args([
                "--batch-size=1",
                "--max-batches=1",
                "--sleep-ms=0",
            ])
            .env("BILL_ANALYSER_POSTGRES_URL", isolated_url)
            .output()?;
        assert!(
            output.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout)?;
        let lines = stdout.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "one committed batch emits one checkpoint");
        let checkpoint: Value = serde_json::from_str(lines[0])?;
        assert_eq!(checkpoint["event"], "import_signal_backfill_batch");
        assert_eq!(checkpoint["batch_number"], 1);
        assert_eq!(checkpoint["next_after_id"], preview_id);
        assert_eq!(checkpoint["scanned_rows"], 1);
        assert_eq!(checkpoint["updated_rows"], 1);
        assert_eq!(checkpoint["mismatch_rows"], 0);
        assert_eq!(checkpoint["has_remaining_rows"], false);
        assert!(checkpoint["duration_ms"].is_u64());

        let version: i16 = sqlx::query_scalar(
            "SELECT signal_projection_version FROM import_preview_rows WHERE id=$1",
        )
        .bind(preview_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(version, 1);

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn concurrent_signal_backfill_batches_serialize_without_duplicate_writes(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_backfill_concurrent").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "concurrent").await?;
        for index in 0..4 {
            insert_legacy_preview(
                &isolated.pool,
                user_id,
                session_id,
                &format!("concurrent-{index}"),
                json!({"preview_parser_id":"wechat","preview_source_account_id":11,"preview_matching_feedback":{}}),
            )
            .await?;
        }

        let (left, right) = tokio::join!(
            backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 2),
            backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 2),
        );
        let left = left?;
        let right = right?;
        assert_eq!(left.updated_rows + right.updated_rows, 4);
        assert_ne!(left.next_after_id, right.next_after_id);
        assert_eq!(u8::from(left.has_remaining_rows) + u8::from(right.has_remaining_rows), 1);
        assert_eq!(left.mismatch_rows + right.mismatch_rows, 0);

        let versions: Vec<i16> = sqlx::query_scalar(
            "SELECT signal_projection_version FROM import_preview_rows WHERE session_id=$1 ORDER BY id",
        )
        .bind(session_id)
        .fetch_all(&isolated.pool)
        .await?;
        assert_eq!(versions, vec![1, 1, 1, 1]);

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn signal_backfill_cli_runs_multiple_rate_limited_batches() -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_backfill_cli_multi").await?;
    let result = async {
        let (user_id, session_id) = create_session(&isolated.pool, "cli-multi").await?;
        for index in 0..2 {
            insert_legacy_preview(
                &isolated.pool,
                user_id,
                session_id,
                &format!("cli-multi-{index}"),
                json!({"preview_parser_id":"wechat","preview_source_account_id":11,"preview_matching_feedback":{}}),
            )
            .await?;
        }
        let base_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
        let (server_url, _) = base_url
            .rsplit_once('/')
            .ok_or("test PostgreSQL URL must include a database path")?;
        let isolated_url = format!("{server_url}/{}", isolated.db_name);

        let output = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_backfill"))
            .args(["--batch-size=1", "--sleep-ms=1"])
            .env("BILL_ANALYSER_POSTGRES_URL", isolated_url)
            .output()?;
        assert!(
            output.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let checkpoints = String::from_utf8(output.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0]["batch_number"], 1);
        assert_eq!(checkpoints[0]["has_remaining_rows"], true);
        assert_eq!(checkpoints[1]["batch_number"], 2);
        assert_eq!(checkpoints[1]["has_remaining_rows"], false);

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[test]
fn signal_backfill_cli_help_and_missing_database_url_are_secret_safe() -> Result<(), Box<dyn Error>>
{
    let help = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_backfill"))
        .arg("--help")
        .env_remove("BILL_ANALYSER_POSTGRES_URL")
        .output()?;
    assert!(help.status.success());
    let help_json: Value = serde_json::from_slice(&help.stdout)?;
    assert_eq!(help_json["env"], json!(["BILL_ANALYSER_POSTGRES_URL"]));

    let missing_env = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_backfill"))
        .env_remove("BILL_ANALYSER_POSTGRES_URL")
        .output()?;
    assert!(!missing_env.status.success());
    let error: Value = serde_json::from_slice(&missing_env.stderr)?;
    assert_eq!(error["event"], "error");
    assert_eq!(error["message"], "BILL_ANALYSER_POSTGRES_URL is required");

    Ok(())
}
