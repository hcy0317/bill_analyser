pub const IMPORT_CONFIRM_RECEIPT_BACKFILL_MAX_BATCH_SIZE: u32 = 1_000;
const IMPORT_CONFIRM_RECEIPT_BACKFILL_ADVISORY_LOCK: i64 = 0x4241_5245_4345_4950;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportConfirmReceiptBackfillBatchReport {
    pub start_after_session_id: i64,
    pub next_after_session_id: i64,
    pub scanned_sessions: u32,
    pub inserted_receipts: u32,
    pub already_materialized_receipts: u32,
    pub mismatch_receipts: u32,
    pub has_remaining_sessions: bool,
    pub duration_ms: u64,
}

struct ImportConfirmReceiptBackfillRow {
    session_id: i64,
    user_id: i64,
    confirmed_at: DateTime<Utc>,
    metadata_receipt: StoredConfirmReceipt,
    typed_receipt: Option<StoredConfirmReceipt>,
}

pub async fn backfill_import_confirm_receipt_batch(
    pool: &PostgresPool,
    after_session_id: i64,
    batch_size: u32,
) -> DbResult<ImportConfirmReceiptBackfillBatchReport> {
    if after_session_id < 0 {
        return Err(DbError::InvalidOperation(
            "confirm receipt backfill watermark must be non-negative".to_string(),
        ));
    }
    if batch_size == 0 || batch_size > IMPORT_CONFIRM_RECEIPT_BACKFILL_MAX_BATCH_SIZE {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt backfill batch size must be between 1 and {IMPORT_CONFIRM_RECEIPT_BACKFILL_MAX_BATCH_SIZE}"
        )));
    }

    let started_at = std::time::Instant::now();
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(IMPORT_CONFIRM_RECEIPT_BACKFILL_ADVISORY_LOCK)
        .execute(&mut *tx)
        .await?;
    validate_import_confirm_receipt_backfill_migration_ledger(&mut tx).await?;

    let has_unfilled_gap: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM import_sessions session
            LEFT JOIN import_confirm_receipts receipt
              ON receipt.session_id = session.id AND receipt.user_id = session.user_id
            WHERE session.id <= $1
              AND session.status = 'confirmed'
              AND session.metadata ? 'confirm_receipt'
              AND receipt.session_id IS NULL
        )
        "#,
    )
    .bind(after_session_id)
    .fetch_one(&mut *tx)
    .await?;
    if has_unfilled_gap {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt backfill watermark {after_session_id} would skip an unfilled receipt"
        )));
    }

    let rows = sqlx::query(
        r#"
        SELECT session.id AS session_id,
               session.user_id,
               session.metadata,
               session.updated_at AS confirmed_at,
               receipt.session_id AS typed_session_id,
               receipt.receipt_schema_version AS typed_receipt_schema_version,
               receipt.command_fingerprint AS typed_command_fingerprint,
               receipt.request_session_version AS typed_request_session_version,
               receipt.response_schema_version AS typed_response_schema_version,
               receipt.http_status AS typed_http_status,
               receipt.success_envelope AS typed_success_envelope
        FROM import_sessions session
        LEFT JOIN import_confirm_receipts receipt
          ON receipt.session_id = session.id AND receipt.user_id = session.user_id
        WHERE session.id > $1
          AND session.status = 'confirmed'
          AND session.metadata ? 'confirm_receipt'
        ORDER BY session.id
        LIMIT $2
        FOR UPDATE OF session
        "#,
    )
    .bind(after_session_id)
    .bind(i64::from(batch_size))
    .fetch_all(&mut *tx)
    .await?;

    if rows.is_empty() {
        tx.commit().await?;
        return Ok(import_confirm_receipt_backfill_report(
            after_session_id,
            after_session_id,
            0,
            0,
            0,
            false,
            started_at,
        ));
    }

    let mut batch = Vec::with_capacity(rows.len());
    for row in &rows {
        batch.push(import_confirm_receipt_backfill_row(row)?);
    }
    let mismatch_receipts = batch
        .iter()
        .filter(|row| {
            row.typed_receipt
                .as_ref()
                .is_some_and(|typed| typed != &row.metadata_receipt)
        })
        .count();
    if mismatch_receipts > 0 {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt backfill found {mismatch_receipts} metadata/typed mismatches"
        )));
    }

    let already_materialized_receipts = batch
        .iter()
        .filter(|row| row.typed_receipt.is_some())
        .count();
    let mut inserted_receipts = 0_usize;
    for row in batch.iter().filter(|row| row.typed_receipt.is_none()) {
        insert_confirm_receipt_projection(
            &mut tx,
            row.session_id,
            row.user_id,
            &row.metadata_receipt,
            Some(row.confirmed_at),
        )
        .await?;
        inserted_receipts += 1;
    }

    let session_ids = batch.iter().map(|row| row.session_id).collect::<Vec<_>>();
    let persisted_rows = sqlx::query(
        r#"
        SELECT session.id AS session_id,
               session.user_id,
               session.metadata,
               session.updated_at AS confirmed_at,
               receipt.session_id AS typed_session_id,
               receipt.receipt_schema_version AS typed_receipt_schema_version,
               receipt.command_fingerprint AS typed_command_fingerprint,
               receipt.request_session_version AS typed_request_session_version,
               receipt.response_schema_version AS typed_response_schema_version,
               receipt.http_status AS typed_http_status,
               receipt.success_envelope AS typed_success_envelope
        FROM import_sessions session
        JOIN import_confirm_receipts receipt
          ON receipt.session_id = session.id AND receipt.user_id = session.user_id
        WHERE session.id = ANY($1)
        ORDER BY session.id
        "#,
    )
    .bind(&session_ids)
    .fetch_all(&mut *tx)
    .await?;
    if persisted_rows.len() != batch.len() {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt backfill parity observed {} of {} sessions",
            persisted_rows.len(),
            batch.len()
        )));
    }
    let persisted_mismatches = persisted_rows
        .iter()
        .map(import_confirm_receipt_backfill_row)
        .collect::<DbResult<Vec<_>>>()?
        .into_iter()
        .filter(|row| row.typed_receipt.as_ref() != Some(&row.metadata_receipt))
        .count();
    if persisted_mismatches > 0 {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt backfill post-insert parity found {persisted_mismatches} mismatches"
        )));
    }

    let next_after_session_id = session_ids.last().copied().unwrap_or(after_session_id);
    let has_remaining_sessions: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM import_sessions
            WHERE id > $1
              AND status = 'confirmed'
              AND metadata ? 'confirm_receipt'
        )
        "#,
    )
    .bind(next_after_session_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(import_confirm_receipt_backfill_report(
        after_session_id,
        next_after_session_id,
        batch.len(),
        inserted_receipts,
        already_materialized_receipts,
        has_remaining_sessions,
        started_at,
    ))
}

async fn validate_import_confirm_receipt_backfill_migration_ledger(
    connection: &mut sqlx::PgConnection,
) -> DbResult<()> {
    let expected_versions = crate::postgres_migration_manifest()
        .iter()
        .map(|migration| migration.version)
        .collect::<Vec<_>>();
    let actual_versions: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success=true ORDER BY version",
    )
    .fetch_all(connection)
    .await?;
    if actual_versions != expected_versions {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt backfill requires exact migration ledger {:?}; found {:?}",
            expected_versions, actual_versions
        )));
    }
    Ok(())
}

fn import_confirm_receipt_backfill_row(row: &PgRow) -> DbResult<ImportConfirmReceiptBackfillRow> {
    let metadata: Value = row.try_get("metadata")?;
    let metadata_receipt = stored_confirm_receipt(&metadata)?;
    let typed_receipt = row
        .try_get::<Option<i64>, _>("typed_session_id")?
        .map(|_| -> DbResult<StoredConfirmReceipt> {
            Ok(StoredConfirmReceipt {
                receipt_schema_version: row.try_get("typed_receipt_schema_version")?,
                command_fingerprint: row.try_get("typed_command_fingerprint")?,
                request_session_version: row.try_get("typed_request_session_version")?,
                response_schema_version: row.try_get("typed_response_schema_version")?,
                http_status: row.try_get("typed_http_status")?,
                success_envelope: row.try_get("typed_success_envelope")?,
            })
        })
        .transpose()?;
    Ok(ImportConfirmReceiptBackfillRow {
        session_id: row.try_get("session_id")?,
        user_id: row.try_get("user_id")?,
        confirmed_at: row.try_get("confirmed_at")?,
        metadata_receipt,
        typed_receipt,
    })
}

fn import_confirm_receipt_backfill_report(
    start_after_session_id: i64,
    next_after_session_id: i64,
    scanned_sessions: usize,
    inserted_receipts: usize,
    already_materialized_receipts: usize,
    has_remaining_sessions: bool,
    started_at: std::time::Instant,
) -> ImportConfirmReceiptBackfillBatchReport {
    ImportConfirmReceiptBackfillBatchReport {
        start_after_session_id,
        next_after_session_id,
        scanned_sessions: u32::try_from(scanned_sessions).unwrap_or(u32::MAX),
        inserted_receipts: u32::try_from(inserted_receipts).unwrap_or(u32::MAX),
        already_materialized_receipts: u32::try_from(already_materialized_receipts)
            .unwrap_or(u32::MAX),
        mismatch_receipts: 0,
        has_remaining_sessions,
        duration_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    }
}
