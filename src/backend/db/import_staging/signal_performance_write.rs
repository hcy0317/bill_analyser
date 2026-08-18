async fn measure_import_preview_signal_write_cost(
    connection: &mut sqlx::PgConnection,
    source: &ImportPreviewSignalAuditSession,
    config: &ImportPreviewSignalPerformanceAuditConfig,
) -> DbResult<ImportPreviewSignalWritePerformanceReport> {
    sqlx::query(
        r#"CREATE TEMP TABLE bill_import_signal_perf_source ON COMMIT DROP AS
           SELECT id, session_id, user_id, page_sort_key,
                  signal_parser, signal_platform_duplicate, signal_transfer,
                  signal_history, signal_learning, signal_llm,
                  signal_projection_version
           FROM import_preview_rows
           WHERE session_id=$1 AND user_id=$2
           ORDER BY id
           LIMIT $3"#,
    )
    .bind(source.session_id)
    .bind(source.user_id)
    .bind(i64::from(
        IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS,
    ))
    .execute(&mut *connection)
    .await?;
    sqlx::query("ANALYZE bill_import_signal_perf_source")
        .execute(&mut *connection)
        .await?;
    sqlx::query(
        r#"CREATE TEMP TABLE bill_import_signal_perf_base (
               id BIGINT PRIMARY KEY,
               session_id BIGINT NOT NULL,
               user_id BIGINT NOT NULL,
               page_sort_key TEXT NOT NULL
           ) ON COMMIT DROP"#,
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query(
        "CREATE INDEX bill_import_signal_perf_base_session_idx ON bill_import_signal_perf_base (session_id, page_sort_key)",
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query(
        r#"CREATE TEMP TABLE bill_import_signal_perf_typed (
               id BIGINT PRIMARY KEY,
               session_id BIGINT NOT NULL,
               user_id BIGINT NOT NULL,
               page_sort_key TEXT NOT NULL,
               signal_parser BOOLEAN NOT NULL,
               signal_platform_duplicate BOOLEAN NOT NULL,
               signal_transfer BOOLEAN NOT NULL,
               signal_history BOOLEAN NOT NULL,
               signal_learning BOOLEAN NOT NULL,
               signal_llm BOOLEAN NOT NULL,
               signal_projection_version SMALLINT NOT NULL
           ) ON COMMIT DROP"#,
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query(
        "CREATE INDEX bill_import_signal_perf_typed_session_idx ON bill_import_signal_perf_typed (session_id, page_sort_key)",
    )
    .execute(&mut *connection)
    .await?;

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bill_import_signal_perf_source")
        .fetch_one(&mut *connection)
        .await?;
    if rows <= 0 {
        return Err(DbError::InvalidOperation(
            "import signal performance write corpus is empty".to_string(),
        ));
    }
    let total_iterations = config
        .warmup_iterations
        .saturating_add(config.measured_iterations);
    let mut base_samples = Vec::with_capacity(config.measured_iterations as usize);
    let mut typed_samples = Vec::with_capacity(config.measured_iterations as usize);
    for iteration in 0..total_iterations {
        let (base_nanos, typed_nanos) = if iteration % 2 == 0 {
            let base = measure_import_preview_signal_base_insert(connection).await?;
            let typed = measure_import_preview_signal_typed_insert(connection).await?;
            (base, typed)
        } else {
            let typed = measure_import_preview_signal_typed_insert(connection).await?;
            let base = measure_import_preview_signal_base_insert(connection).await?;
            (base, typed)
        };
        if iteration >= config.warmup_iterations {
            base_samples.push(base_nanos);
            typed_samples.push(typed_nanos);
        }
    }
    let base = import_preview_signal_duration_summary(&base_samples)?;
    let typed = import_preview_signal_duration_summary(&typed_samples)?;
    let p95_regression_percent = regression_percent(base.p95_ms, typed.p95_ms);
    let passed = !regression_exceeded(
        base.p95_ms,
        typed.p95_ms,
        config.maximum_write_regression_percent,
    );
    Ok(ImportPreviewSignalWritePerformanceReport {
        source_session_id: source.session_id,
        source_session_rows: source.row_count,
        maximum_rows: IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS,
        rows,
        truncated: rows < source.row_count,
        base,
        typed,
        p95_regression_percent,
        maximum_regression_percent: config.maximum_write_regression_percent,
        passed,
    })
}

async fn measure_import_preview_signal_base_insert(
    connection: &mut sqlx::PgConnection,
) -> DbResult<u64> {
    sqlx::query("TRUNCATE bill_import_signal_perf_base")
        .execute(&mut *connection)
        .await?;
    let started_at = std::time::Instant::now();
    sqlx::query(
        r#"INSERT INTO bill_import_signal_perf_base (id, session_id, user_id, page_sort_key)
           SELECT id, session_id, user_id, page_sort_key
           FROM bill_import_signal_perf_source"#,
    )
    .execute(&mut *connection)
    .await?;
    Ok(started_at
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX))
}

async fn measure_import_preview_signal_typed_insert(
    connection: &mut sqlx::PgConnection,
) -> DbResult<u64> {
    sqlx::query("TRUNCATE bill_import_signal_perf_typed")
        .execute(&mut *connection)
        .await?;
    let started_at = std::time::Instant::now();
    sqlx::query(
        r#"INSERT INTO bill_import_signal_perf_typed (
               id, session_id, user_id, page_sort_key,
               signal_parser, signal_platform_duplicate, signal_transfer,
               signal_history, signal_learning, signal_llm, signal_projection_version
           )
           SELECT id, session_id, user_id, page_sort_key,
                  signal_parser, signal_platform_duplicate, signal_transfer,
                  signal_history, signal_learning, signal_llm, signal_projection_version
           FROM bill_import_signal_perf_source"#,
    )
    .execute(&mut *connection)
    .await?;
    Ok(started_at
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX))
}
