use std::{env, error::Error, io, process::Command};

use bill_analyser_db::{
    audit_import_preview_signal_performance, backfill_import_preview_signal_projection_batch,
    ImportPreviewSignalPerformanceAuditConfig, PostgresPool,
    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS,
};
use serde_json::json;

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal performance audit tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal performance audit tests",
            )
            .into()
        })
}

async fn create_materialized_session(
    pool: &PostgresPool,
    suffix: &str,
    rows: usize,
) -> Result<(i64, i64), Box<dyn Error>> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("signal-performance-audit-{suffix}"))
        .fetch_one(pool)
        .await?;
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("signal-performance-audit-{suffix}"))
    .fetch_one(pool)
    .await?;
    for index in 0..rows {
        sqlx::query(
            "INSERT INTO import_preview_rows (session_id,user_id,page_sort_key,operation_kind,occurred_at,amount_cents,direction,preview_payload) VALUES ($1,$2,$3,'insert',now(),-100,'expense',$4)",
        )
        .bind(session_id)
        .bind(user_id)
        .bind(format!("row-{index:04}"))
        .bind(json!({
            "preview_parser_id": "wechat",
            "preview_source_account_id": 11,
            "preview_matching_feedback": {}
        }))
        .execute(pool)
        .await?;
    }
    let batch_size = u32::try_from(rows + 10)?;
    let backfill = backfill_import_preview_signal_projection_batch(pool, 0, batch_size).await?;
    assert_eq!(backfill.updated_rows, u32::try_from(rows)?);
    Ok((user_id, session_id))
}

async fn create_bulk_materialized_session(
    pool: &PostgresPool,
    suffix: &str,
    rows: i64,
) -> Result<(i64, i64), Box<dyn Error>> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("signal-performance-audit-{suffix}"))
        .fetch_one(pool)
        .await?;
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("signal-performance-audit-{suffix}"))
    .fetch_one(pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO import_preview_rows (
               session_id, user_id, page_sort_key, operation_kind, occurred_at,
               amount_cents, direction, preview_payload,
               signal_parser, signal_platform_duplicate, signal_transfer,
               signal_history, signal_learning, signal_llm, signal_projection_version
           )
           SELECT $1, $2, lpad(series::text, 8, '0'), 'insert', now(), -100, 'expense',
                  jsonb_build_object(
                      'preview_parser_id', 'wechat',
                      'preview_source_account_id', 11,
                      'preview_matching_feedback', '{}'::jsonb
                  ),
                  true, false, false, false, false, false, 1
           FROM generate_series(1, $3) AS series"#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(rows)
    .execute(pool)
    .await?;
    Ok((user_id, session_id))
}

async fn business_row_checksum(pool: &PostgresPool) -> Result<(i64, i64, String), sqlx::Error> {
    sqlx::query_as(
        r#"SELECT COUNT(*)::BIGINT,
                  COALESCE(SUM(version), 0)::BIGINT,
                  md5(COALESCE(string_agg(id::text || ':' || preview_payload::text, ',' ORDER BY id), ''))
           FROM import_preview_rows"#,
    )
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn performance_audit_measures_real_query_paths_and_rolls_back_temporary_writes(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_performance_audit").await?;
    let result = async {
        let (_user_id, session_id) =
            create_materialized_session(&isolated.pool, "match", 8).await?;
        let before = business_row_checksum(&isolated.pool).await?;

        let report = audit_import_preview_signal_performance(
            &isolated.pool,
            &ImportPreviewSignalPerformanceAuditConfig {
                warmup_iterations: 1,
                measured_iterations: 5,
                query_page_size: 25,
                maximum_query_regression_percent: 15.0,
                maximum_write_regression_percent: 15.0,
            },
        )
        .await?;

        assert_eq!(report.expected_migration_version, 28);
        assert_eq!(report.actual_migration_version, 28);
        assert_eq!(report.migration_count, 28);
        assert!(!report.snapshot_token.is_empty());
        assert_eq!(report.corpus_sessions, 1);
        assert_eq!(report.query_cases.len(), 7);
        assert_eq!(report.query_mismatch_cases, 0);
        assert_eq!(report.timing_contract.clock, "rust_std_instant");
        assert!(report.timing_contract.jit_disabled);
        assert!(report.timing_contract.warmup_excluded);
        assert_eq!(report.timing_contract.percentile_method, "percentile_cont");
        assert_eq!(report.timing_contract.statement_timeout_ms, 60_000);
        assert_eq!(report.timing_contract.lock_timeout_ms, 5_000);
        for query_case in &report.query_cases {
            assert_eq!(query_case.session_id, session_id);
            assert_eq!(query_case.preview_rows, 8);
            assert!(query_case.parity_match);
            assert_eq!(query_case.legacy.samples, 5);
            assert_eq!(query_case.typed.samples, 5);
            assert!(query_case.legacy.p95_ms >= 0.0);
            assert!(query_case.typed.p95_ms >= 0.0);
        }
        assert_eq!(report.write.source_session_id, session_id);
        assert_eq!(report.write.source_session_rows, 8);
        assert_eq!(report.write.rows, 8);
        assert_eq!(
            report.write.maximum_rows,
            IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS
        );
        assert!(!report.write.truncated);
        assert_eq!(report.write.base.samples, 5);
        assert_eq!(report.write.typed.samples, 5);
        assert!(report.transaction_rolled_back);

        let after = business_row_checksum(&isolated.pool).await?;
        assert_eq!(
            after, before,
            "performance audit must not mutate business rows"
        );

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn performance_audit_bounds_large_write_corpus_without_mutating_source_rows(
) -> Result<(), Box<dyn Error>> {
    let isolated =
        strict_isolated_postgres_database("import_signal_performance_audit_bound").await?;
    let result = async {
        let source_rows = i64::from(IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS) + 5;
        let (_user_id, session_id) =
            create_bulk_materialized_session(&isolated.pool, "bound", source_rows).await?;
        let before = business_row_checksum(&isolated.pool).await?;

        let report = audit_import_preview_signal_performance(
            &isolated.pool,
            &ImportPreviewSignalPerformanceAuditConfig {
                warmup_iterations: 0,
                measured_iterations: 1,
                query_page_size: 25,
                maximum_query_regression_percent: 15.0,
                maximum_write_regression_percent: 15.0,
            },
        )
        .await?;
        assert_eq!(report.write.source_session_id, session_id);
        assert_eq!(report.write.source_session_rows, source_rows);
        assert_eq!(
            report.write.rows,
            i64::from(IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS)
        );
        assert!(report.write.truncated);
        assert_eq!(business_row_checksum(&isolated.pool).await?, before);

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn performance_audit_cli_emits_one_rollback_report_and_exits_with_gate_result(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("import_signal_performance_audit_cli").await?;
    let result = async {
        create_bulk_materialized_session(&isolated.pool, "cli", 5_000).await?;
        let before = business_row_checksum(&isolated.pool).await?;
        let base_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
        let (server_url, _) = base_url
            .rsplit_once('/')
            .ok_or("test PostgreSQL URL must include a database path")?;
        let isolated_url = format!("{server_url}/{}", isolated.db_name);

        let output = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_performance_audit"))
            .args([
                "--warmup-iterations=1",
                "--measured-iterations=5",
                "--query-page-size=25",
                "--maximum-query-regression-percent=15",
                "--maximum-write-regression-percent=15",
            ])
            .env("BILL_ANALYSER_POSTGRES_URL", &isolated_url)
            .output()?;
        let lines = String::from_utf8(output.stdout)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines.len(), 1);
        let report = &lines[0];
        assert_eq!(report["event"], "import_signal_performance_target_audit");
        let performance_gate_passed = report["performance_gate_passed"]
            .as_bool()
            .ok_or("performance gate result must be boolean")?;
        assert_eq!(output.status.success(), performance_gate_passed);
        if !performance_gate_passed {
            let gate_error: serde_json::Value = serde_json::from_slice(&output.stderr)?;
            assert_eq!(gate_error["event"], "error");
            assert!(gate_error["message"]
                .as_str()
                .is_some_and(|message| message.contains("performance audit failed")));
        }
        assert_eq!(report["transaction_rolled_back"], true);
        assert_eq!(report["config"]["warmup_iterations"], 1);
        assert_eq!(report["config"]["measured_iterations"], 5);
        assert_eq!(report["query_cases"].as_array().map(Vec::len), Some(7));
        assert_eq!(report["write"]["rows"], 5_000);
        assert_eq!(business_row_checksum(&isolated.pool).await?, before);

        let help = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_performance_audit"))
            .arg("--help")
            .env_remove("BILL_ANALYSER_POSTGRES_URL")
            .output()?;
        assert!(help.status.success());
        let help_json: serde_json::Value = serde_json::from_slice(&help.stdout)?;
        assert_eq!(help_json["env"], json!(["BILL_ANALYSER_POSTGRES_URL"]));
        assert_eq!(help_json["writes_business_data"], false);
        assert_eq!(help_json["temporary_tables_rolled_back"], true);

        let missing_env = Command::new(env!("CARGO_BIN_EXE_bill_import_signal_performance_audit"))
            .env_remove("BILL_ANALYSER_POSTGRES_URL")
            .output()?;
        assert!(!missing_env.status.success());
        let error: serde_json::Value = serde_json::from_slice(&missing_env.stderr)?;
        assert_eq!(error["event"], "error");
        assert_eq!(error["message"], "BILL_ANALYSER_POSTGRES_URL is required");
        assert!(!String::from_utf8_lossy(&missing_env.stderr).contains(&isolated_url));

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn performance_audit_rejects_unmaterialized_rows_and_invalid_limits_before_measurement(
) -> Result<(), Box<dyn Error>> {
    let isolated =
        strict_isolated_postgres_database("import_signal_performance_audit_reject").await?;
    let result = async {
        let user_id: i64 =
            sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
                .bind("signal-performance-audit-unmaterialized")
                .fetch_one(&isolated.pool)
                .await?;
        let session_id: i64 = sqlx::query_scalar(
            "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
        )
        .bind(user_id)
        .bind("signal-performance-audit-unmaterialized")
        .fetch_one(&isolated.pool)
        .await?;
        sqlx::query(
            "INSERT INTO import_preview_rows (session_id,user_id,page_sort_key,operation_kind,occurred_at,amount_cents,direction,preview_payload) VALUES ($1,$2,'row-0001','insert',now(),-100,'expense',$3)",
        )
        .bind(session_id)
        .bind(user_id)
        .bind(json!({"preview_parser_id":"wechat","preview_matching_feedback":{}}))
        .execute(&isolated.pool)
        .await?;
        let before = business_row_checksum(&isolated.pool).await?;

        let error = audit_import_preview_signal_performance(
            &isolated.pool,
            &ImportPreviewSignalPerformanceAuditConfig::default(),
        )
        .await
        .expect_err("unmaterialized rows must fail before performance measurement");
        assert!(error.to_string().contains("unmaterialized rows"));
        assert_eq!(business_row_checksum(&isolated.pool).await?, before);

        for config in [
            ImportPreviewSignalPerformanceAuditConfig {
                measured_iterations: 0,
                ..ImportPreviewSignalPerformanceAuditConfig::default()
            },
            ImportPreviewSignalPerformanceAuditConfig {
                warmup_iterations: 4,
                ..ImportPreviewSignalPerformanceAuditConfig::default()
            },
            ImportPreviewSignalPerformanceAuditConfig {
                query_page_size: 0,
                ..ImportPreviewSignalPerformanceAuditConfig::default()
            },
            ImportPreviewSignalPerformanceAuditConfig {
                maximum_query_regression_percent: f64::NAN,
                ..ImportPreviewSignalPerformanceAuditConfig::default()
            },
        ] {
            assert!(
                audit_import_preview_signal_performance(&isolated.pool, &config)
                    .await
                    .is_err(),
                "invalid performance config must fail closed: {config:?}"
            );
        }

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
