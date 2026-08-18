const IMPORT_PREVIEW_SIGNAL_QUERY_REGRESSION_TOLERANCE_MS: f64 = 1.0;
const IMPORT_PREVIEW_SIGNAL_PERFORMANCE_STATEMENT_TIMEOUT_MS: i64 = 60_000;
const IMPORT_PREVIEW_SIGNAL_PERFORMANCE_LOCK_TIMEOUT_MS: i64 = 5_000;

struct ImportPreviewSignalMeasuredQuery {
    result: ImportPreviewPageResult,
    elapsed_nanos: u64,
}

/// 使用真实 legacy/typed 查询路径和事务级临时表，采集不落业务数据的目标库 p95 证据。
pub async fn audit_import_preview_signal_performance(
    pool: &PostgresPool,
    config: &ImportPreviewSignalPerformanceAuditConfig,
) -> DbResult<ImportPreviewSignalPerformanceAuditReport> {
    validate_import_preview_signal_performance_config(config)?;
    let started_at = std::time::Instant::now();
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL jit = off")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(IMPORT_PREVIEW_SIGNAL_PERFORMANCE_STATEMENT_TIMEOUT_MS.to_string())
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT set_config('lock_timeout', $1, true)")
        .bind(IMPORT_PREVIEW_SIGNAL_PERFORMANCE_LOCK_TIMEOUT_MS.to_string())
        .execute(&mut *tx)
        .await?;
    let migration_ledger = validate_import_preview_signal_audit_migration_ledger(
        &mut tx,
        "import signal performance audit",
    )
    .await?;
    let unmaterialized_rows: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::BIGINT
           FROM import_preview_rows
           WHERE signal_projection_version <> 1
              OR signal_parser IS NULL
              OR signal_platform_duplicate IS NULL
              OR signal_transfer IS NULL
              OR signal_history IS NULL
              OR signal_learning IS NULL
              OR signal_llm IS NULL"#,
    )
    .fetch_one(&mut *tx)
    .await?;
    if unmaterialized_rows != 0 {
        return Err(DbError::InvalidOperation(format!(
            "import signal performance audit requires every preview row at projection version 1; found {unmaterialized_rows} unmaterialized rows"
        )));
    }
    let snapshot_token: String =
        sqlx::query_scalar("SELECT txid_current_snapshot()::text")
            .fetch_one(&mut *tx)
            .await?;
    let sessions = load_import_preview_signal_audit_sessions(&mut tx).await?;
    let corpus = select_import_preview_signal_audit_corpus(&sessions);
    let Some(write_source) = corpus.first().copied() else {
        return Err(DbError::InvalidOperation(
            "import signal performance audit requires at least one preview session".to_string(),
        ));
    };

    let mut query_cases = Vec::new();
    let mut query_mismatch_cases = 0_u64;
    let mut query_regression_cases = 0_u64;
    for session in &corpus {
        ensure_preview_signal_projection_v1(&mut tx, session.session_id, session.user_id).await?;
        for signal in std::iter::once(None).chain(
            IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES
                .iter()
                .map(|family| Some((*family).to_string())),
        ) {
            let request = ImportPreviewPageRequest {
                page_size: usize::try_from(config.query_page_size).unwrap_or(usize::MAX),
                filters: ImportPreviewQueryFilters {
                    signal: signal.clone(),
                    ..ImportPreviewQueryFilters::default()
                },
                ..ImportPreviewPageRequest::default()
            };
            let measured = measure_import_preview_signal_query_case(
                &mut tx,
                session,
                &request,
                config,
            )
            .await?;
            if !measured.parity_match {
                query_mismatch_cases = query_mismatch_cases.saturating_add(1);
            }
            if measured.regression_exceeded {
                query_regression_cases = query_regression_cases.saturating_add(1);
            }
            query_cases.push(measured);
        }
    }

    let write = measure_import_preview_signal_write_cost(&mut tx, write_source, config).await?;
    tx.rollback().await?;

    Ok(ImportPreviewSignalPerformanceAuditReport {
        expected_migration_version: migration_ledger.expected_migration_version,
        actual_migration_version: migration_ledger.actual_migration_version,
        migration_count: migration_ledger.migration_count,
        snapshot_token,
        config: config.clone(),
        timing_contract: ImportPreviewSignalPerformanceTimingContract {
            clock: "rust_std_instant".to_string(),
            jit_disabled: true,
            warmup_excluded: true,
            legacy_typed_order: "alternating_by_iteration".to_string(),
            percentile_method: "percentile_cont".to_string(),
            statement_timeout_ms: u64::try_from(
                IMPORT_PREVIEW_SIGNAL_PERFORMANCE_STATEMENT_TIMEOUT_MS,
            )
            .unwrap_or(u64::MAX),
            lock_timeout_ms: u64::try_from(IMPORT_PREVIEW_SIGNAL_PERFORMANCE_LOCK_TIMEOUT_MS)
                .unwrap_or(u64::MAX),
        },
        corpus_sessions: u64::try_from(corpus.len()).unwrap_or(u64::MAX),
        query_cases,
        query_mismatch_cases,
        query_regression_cases,
        query_regression_tolerance_ms: IMPORT_PREVIEW_SIGNAL_QUERY_REGRESSION_TOLERANCE_MS,
        write,
        transaction_rolled_back: true,
        duration_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}

async fn measure_import_preview_signal_query_case(
    connection: &mut sqlx::PgConnection,
    session: &ImportPreviewSignalAuditSession,
    request: &ImportPreviewPageRequest,
    config: &ImportPreviewSignalPerformanceAuditConfig,
) -> DbResult<ImportPreviewSignalQueryPerformanceCase> {
    let total_iterations = config
        .warmup_iterations
        .saturating_add(config.measured_iterations);
    let mut legacy_samples = Vec::with_capacity(config.measured_iterations as usize);
    let mut typed_samples = Vec::with_capacity(config.measured_iterations as usize);
    let mut parity_match = true;
    for iteration in 0..total_iterations {
        let (legacy, typed) = if iteration % 2 == 0 {
            let legacy = measure_import_preview_signal_query(
                connection,
                session,
                request,
                PreviewSignalReadSource::LegacyPayload,
            )
            .await?;
            let typed = measure_import_preview_signal_query(
                connection,
                session,
                request,
                PreviewSignalReadSource::TypedV1,
            )
            .await?;
            (legacy, typed)
        } else {
            let typed = measure_import_preview_signal_query(
                connection,
                session,
                request,
                PreviewSignalReadSource::TypedV1,
            )
            .await?;
            let legacy = measure_import_preview_signal_query(
                connection,
                session,
                request,
                PreviewSignalReadSource::LegacyPayload,
            )
            .await?;
            (legacy, typed)
        };
        parity_match &= build_import_preview_signal_read_parity_report(
            legacy.result,
            typed.result,
        )
        .is_match();
        if iteration >= config.warmup_iterations {
            legacy_samples.push(legacy.elapsed_nanos);
            typed_samples.push(typed.elapsed_nanos);
        }
    }
    let legacy = import_preview_signal_duration_summary(&legacy_samples)?;
    let typed = import_preview_signal_duration_summary(&typed_samples)?;
    let p95_regression_percent = regression_percent(legacy.p95_ms, typed.p95_ms);
    let regression_exceeded = regression_exceeded(
        legacy.p95_ms,
        typed.p95_ms,
        config.maximum_query_regression_percent,
    );
    Ok(ImportPreviewSignalQueryPerformanceCase {
        session_id: session.session_id,
        preview_rows: session.row_count,
        signal: request
            .filters
            .signal
            .clone()
            .unwrap_or_else(|| "unfiltered".to_string()),
        parity_match,
        legacy,
        typed,
        p95_regression_percent,
        regression_exceeded,
    })
}

async fn measure_import_preview_signal_query(
    connection: &mut sqlx::PgConnection,
    session: &ImportPreviewSignalAuditSession,
    request: &ImportPreviewPageRequest,
    source: PreviewSignalReadSource,
) -> DbResult<ImportPreviewSignalMeasuredQuery> {
    let started_at = std::time::Instant::now();
    let result = query_preview_page_from_connection(
        connection,
        session.session_id,
        session.user_id,
        request,
        source,
    )
    .await?;
    Ok(ImportPreviewSignalMeasuredQuery {
        result,
        elapsed_nanos: started_at
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}

fn validate_import_preview_signal_performance_config(
    config: &ImportPreviewSignalPerformanceAuditConfig,
) -> DbResult<()> {
    if config.warmup_iterations > IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WARMUP_ITERATIONS {
        return Err(DbError::InvalidOperation(format!(
            "import signal performance warmup iterations must be between 0 and {IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WARMUP_ITERATIONS}"
        )));
    }
    if !(1..=IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_MEASURED_ITERATIONS)
        .contains(&config.measured_iterations)
    {
        return Err(DbError::InvalidOperation(format!(
            "import signal performance measured iterations must be between 1 and {IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_MEASURED_ITERATIONS}"
        )));
    }
    if config.query_page_size == 0
        || config.query_page_size > IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE
    {
        return Err(DbError::InvalidOperation(format!(
            "import signal performance query page size must be between 1 and {IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE}"
        )));
    }
    for (label, value) in [
        (
            "maximum query regression percent",
            config.maximum_query_regression_percent,
        ),
        (
            "maximum write regression percent",
            config.maximum_write_regression_percent,
        ),
    ] {
        if !value.is_finite()
            || !(0.0..=IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT).contains(&value)
        {
            return Err(DbError::InvalidOperation(format!(
                "import signal performance {label} must be between 0 and {IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT}"
            )));
        }
    }
    Ok(())
}

fn import_preview_signal_duration_summary(
    samples_nanos: &[u64],
) -> DbResult<ImportPreviewSignalDurationSummary> {
    if samples_nanos.is_empty() {
        return Err(DbError::InvalidOperation(
            "import signal performance duration samples must not be empty".to_string(),
        ));
    }
    let mut sorted = samples_nanos.to_vec();
    sorted.sort_unstable();
    Ok(ImportPreviewSignalDurationSummary {
        samples: u32::try_from(sorted.len()).unwrap_or(u32::MAX),
        min_ms: nanos_to_millis(*sorted.first().unwrap_or(&0)),
        p50_ms: percentile_cont_millis(&sorted, 0.50),
        p95_ms: percentile_cont_millis(&sorted, 0.95),
        max_ms: nanos_to_millis(*sorted.last().unwrap_or(&0)),
    })
}

fn percentile_cont_millis(sorted_nanos: &[u64], percentile: f64) -> f64 {
    let maximum_index = sorted_nanos.len().saturating_sub(1);
    let position = maximum_index as f64 * percentile;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let lower_value = sorted_nanos[lower] as f64;
    let upper_value = sorted_nanos[upper] as f64;
    (lower_value + (upper_value - lower_value) * (position - lower as f64)) / 1_000_000.0
}

fn nanos_to_millis(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000.0
}

fn regression_percent(base_ms: f64, candidate_ms: f64) -> f64 {
    if base_ms <= f64::EPSILON {
        return if candidate_ms <= f64::EPSILON {
            0.0
        } else {
            IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT + 1.0
        };
    }
    ((candidate_ms - base_ms) / base_ms) * 100.0
}

fn regression_exceeded(base_ms: f64, candidate_ms: f64, maximum_percent: f64) -> bool {
    candidate_ms > base_ms + IMPORT_PREVIEW_SIGNAL_QUERY_REGRESSION_TOLERANCE_MS
        && regression_percent(base_ms, candidate_ms) > maximum_percent
}

#[cfg(test)]
mod import_preview_signal_performance_tests {
    use super::*;

    #[test]
    fn duration_summary_uses_continuous_percentiles() {
        let summary = import_preview_signal_duration_summary(&[
            1_000_000, 2_000_000, 3_000_000, 4_000_000, 5_000_000,
        ])
        .unwrap();
        assert_eq!(summary.samples, 5);
        assert_eq!(summary.min_ms, 1.0);
        assert_eq!(summary.p50_ms, 3.0);
        assert_eq!(summary.p95_ms, 4.8);
        assert_eq!(summary.max_ms, 5.0);
    }

    #[test]
    fn regression_gate_keeps_one_millisecond_noise_floor() {
        assert!(!regression_exceeded(0.10, 0.50, 15.0));
        assert!(regression_exceeded(10.0, 12.0, 15.0));
        assert!(!regression_exceeded(10.0, 11.0, 15.0));
    }

    #[test]
    fn regression_percent_remains_json_finite_for_zero_baselines() {
        assert_eq!(regression_percent(0.0, 0.0), 0.0);
        assert_eq!(
            regression_percent(0.0, 0.1),
            IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT + 1.0
        );
    }

    #[test]
    fn performance_config_rejects_unbounded_values() {
        let invalid = [
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
                maximum_write_regression_percent: f64::NAN,
                ..ImportPreviewSignalPerformanceAuditConfig::default()
            },
        ];
        for config in invalid {
            assert!(validate_import_preview_signal_performance_config(&config).is_err());
        }
    }
}
