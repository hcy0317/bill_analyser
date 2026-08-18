pub const IMPORT_PREVIEW_SIGNAL_BACKFILL_MAX_BATCH_SIZE: u32 = 1_000;
const IMPORT_PREVIEW_SIGNAL_BACKFILL_ADVISORY_LOCK: i64 = 0x4241_5349_474E_414C;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportPreviewSignalBackfillBatchReport {
    pub start_after_id: i64,
    pub next_after_id: i64,
    pub scanned_rows: u32,
    pub updated_rows: u32,
    pub mismatch_rows: i64,
    pub has_remaining_rows: bool,
    pub duration_ms: u64,
}

pub async fn backfill_import_preview_signal_projection_batch(
    pool: &PostgresPool,
    after_id: i64,
    batch_size: u32,
) -> DbResult<ImportPreviewSignalBackfillBatchReport> {
    if after_id < 0 {
        return Err(DbError::InvalidOperation(
            "import signal backfill watermark must be non-negative".to_string(),
        ));
    }
    if batch_size == 0 || batch_size > IMPORT_PREVIEW_SIGNAL_BACKFILL_MAX_BATCH_SIZE {
        return Err(DbError::InvalidOperation(format!(
            "import signal backfill batch size must be between 1 and {IMPORT_PREVIEW_SIGNAL_BACKFILL_MAX_BATCH_SIZE}"
        )));
    }

    let started_at = std::time::Instant::now();
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(IMPORT_PREVIEW_SIGNAL_BACKFILL_ADVISORY_LOCK)
        .execute(&mut *tx)
        .await?;

    // A persisted checkpoint may only move past rows already materialized by another batch.
    let has_unfilled_gap: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM import_preview_rows WHERE signal_projection_version=0 AND id <= $1)",
    )
    .bind(after_id)
    .fetch_one(&mut *tx)
    .await?;
    if has_unfilled_gap {
        return Err(DbError::InvalidOperation(format!(
            "import signal backfill watermark {after_id} would skip an unfilled row"
        )));
    }

    // Do not use SKIP LOCKED: advancing beyond a skipped lower id would make that gap permanent.
    let rows = sqlx::query(
        "SELECT id,preview_payload FROM import_preview_rows WHERE signal_projection_version=0 AND id > $1 ORDER BY id LIMIT $2 FOR UPDATE",
    )
    .bind(after_id)
    .bind(i64::from(batch_size))
    .fetch_all(&mut *tx)
    .await?;
    if rows.is_empty() {
        tx.commit().await?;
        return Ok(ImportPreviewSignalBackfillBatchReport {
            start_after_id: after_id,
            next_after_id: after_id,
            scanned_rows: 0,
            updated_rows: 0,
            mismatch_rows: 0,
            has_remaining_rows: false,
            duration_ms: started_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        });
    }

    let mut ids = Vec::with_capacity(rows.len());
    let mut parser = Vec::with_capacity(rows.len());
    let mut platform_duplicate = Vec::with_capacity(rows.len());
    let mut transfer = Vec::with_capacity(rows.len());
    let mut history = Vec::with_capacity(rows.len());
    let mut learning = Vec::with_capacity(rows.len());
    let mut llm = Vec::with_capacity(rows.len());
    let mut versions = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.try_get("id")?;
        let payload: Value = row.try_get("preview_payload")?;
        let projection = import_preview_signal_projection_from_payload(&payload)?;
        ids.push(id);
        parser.push(projection.parser);
        platform_duplicate.push(projection.platform_duplicate);
        transfer.push(projection.transfer);
        history.push(projection.history);
        learning.push(projection.learning);
        llm.push(projection.llm);
        versions.push(projection.version);
    }

    let updated_ids: Vec<i64> = sqlx::query_scalar(
        r#"UPDATE import_preview_rows AS preview
           SET signal_parser = projected.parser,
               signal_platform_duplicate = projected.platform_duplicate,
               signal_transfer = projected.transfer,
               signal_history = projected.history,
               signal_learning = projected.learning,
               signal_llm = projected.llm,
               signal_projection_version = projected.version
           FROM UNNEST(
               $1::bigint[], $2::boolean[], $3::boolean[], $4::boolean[],
               $5::boolean[], $6::boolean[], $7::boolean[], $8::smallint[]
           ) AS projected(id,parser,platform_duplicate,transfer,history,learning,llm,version)
           WHERE preview.id = projected.id
             AND preview.signal_projection_version = 0
           RETURNING preview.id"#,
    )
    .bind(&ids)
    .bind(&parser)
    .bind(&platform_duplicate)
    .bind(&transfer)
    .bind(&history)
    .bind(&learning)
    .bind(&llm)
    .bind(&versions)
    .fetch_all(&mut *tx)
    .await?;
    if updated_ids.len() != ids.len() {
        return Err(DbError::InvalidOperation(format!(
            "import signal backfill CAS mismatch: claimed {}, updated {}",
            ids.len(),
            updated_ids.len()
        )));
    }

    let (observed_rows, mismatch_rows) =
        import_preview_signal_projection_parity_counts(&mut tx, &ids).await?;
    if observed_rows != i64::try_from(ids.len()).unwrap_or(i64::MAX) {
        return Err(DbError::InvalidOperation(format!(
            "import signal backfill parity observed {observed_rows} of {} rows",
            ids.len()
        )));
    }
    let next_after_id = ids.last().copied().unwrap_or(after_id);
    let has_remaining_rows: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM import_preview_rows WHERE signal_projection_version=0 AND id > $1)",
    )
    .bind(next_after_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(ImportPreviewSignalBackfillBatchReport {
        start_after_id: after_id,
        next_after_id,
        scanned_rows: u32::try_from(ids.len()).unwrap_or(u32::MAX),
        updated_rows: u32::try_from(updated_ids.len()).unwrap_or(u32::MAX),
        mismatch_rows,
        has_remaining_rows,
        duration_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}
