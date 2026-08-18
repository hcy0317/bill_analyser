pub const IMPORT_CONFIRM_RECEIPT_READ_AUDIT_MAX_BATCH_SIZE: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportConfirmReceiptReadAuditBatchReport {
    pub start_after_session_id: i64,
    pub next_after_session_id: i64,
    pub target_sessions: u64,
    pub materialized_target_receipts: u64,
    pub unmaterialized_target_receipts: u64,
    pub mismatch_receipts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportConfirmReceiptReadAuditReport {
    pub expected_migration_version: i64,
    pub expected_migration_count: u64,
    pub actual_migration_version: i64,
    pub migration_count: u64,
    pub snapshot_token: String,
    pub batch_size: u32,
    pub target_sessions: u64,
    pub typed_receipts: u64,
    pub materialized_target_receipts: u64,
    pub unmaterialized_target_receipts: u64,
    pub unexpected_typed_receipts: u64,
    pub mismatch_receipts: u64,
    pub batches: Vec<ImportConfirmReceiptReadAuditBatchReport>,
    pub duration_ms: u64,
}

impl ImportConfirmReceiptReadAuditReport {
    pub fn is_match(&self) -> bool {
        self.actual_migration_version == self.expected_migration_version
            && self.migration_count == self.expected_migration_count
            && self.unmaterialized_target_receipts == 0
            && self.unexpected_typed_receipts == 0
            && self.mismatch_receipts == 0
    }
}

/// 在一个只读可重复读快照中审计 legacy metadata receipt 与 typed receipt 的全量 parity。
pub async fn audit_import_confirm_receipt_target_snapshot(
    pool: &PostgresPool,
    batch_size: u32,
) -> DbResult<ImportConfirmReceiptReadAuditReport> {
    if batch_size == 0 || batch_size > IMPORT_CONFIRM_RECEIPT_READ_AUDIT_MAX_BATCH_SIZE {
        return Err(DbError::InvalidOperation(format!(
            "confirm receipt read audit batch size must be between 1 and {IMPORT_CONFIRM_RECEIPT_READ_AUDIT_MAX_BATCH_SIZE}"
        )));
    }

    let started_at = std::time::Instant::now();
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let migration_ledger = validate_import_audit_migration_ledger(
        &mut tx,
        "confirm receipt read audit",
    )
    .await?;
    let snapshot_token: String =
        sqlx::query_scalar("SELECT txid_current_snapshot()::text")
            .fetch_one(&mut *tx)
            .await?;

    let mut after_session_id = 0_i64;
    let mut batches = Vec::new();
    let mut target_sessions = 0_u64;
    let mut materialized_target_receipts = 0_u64;
    let mut unmaterialized_target_receipts = 0_u64;
    let mut mismatch_receipts = 0_u64;
    loop {
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
            "#,
        )
        .bind(after_session_id)
        .bind(i64::from(batch_size))
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            break;
        }

        let start_after_session_id = after_session_id;
        let mut batch_materialized = 0_u64;
        let mut batch_unmaterialized = 0_u64;
        let mut batch_mismatches = 0_u64;
        for row in &rows {
            let receipt = import_confirm_receipt_projection_row(row)?;
            after_session_id = receipt.session_id;
            match receipt.typed_receipt.as_ref() {
                Some(typed) => {
                    batch_materialized = batch_materialized.saturating_add(1);
                    if typed != &receipt.metadata_receipt {
                        batch_mismatches = batch_mismatches.saturating_add(1);
                    }
                }
                None => batch_unmaterialized = batch_unmaterialized.saturating_add(1),
            }
        }
        let batch_target_sessions = u64::try_from(rows.len()).unwrap_or(u64::MAX);
        batches.push(ImportConfirmReceiptReadAuditBatchReport {
            start_after_session_id,
            next_after_session_id: after_session_id,
            target_sessions: batch_target_sessions,
            materialized_target_receipts: batch_materialized,
            unmaterialized_target_receipts: batch_unmaterialized,
            mismatch_receipts: batch_mismatches,
        });
        target_sessions = target_sessions.saturating_add(batch_target_sessions);
        materialized_target_receipts =
            materialized_target_receipts.saturating_add(batch_materialized);
        unmaterialized_target_receipts =
            unmaterialized_target_receipts.saturating_add(batch_unmaterialized);
        mismatch_receipts = mismatch_receipts.saturating_add(batch_mismatches);
        if rows.len() < usize::try_from(batch_size).unwrap_or(usize::MAX) {
            break;
        }
    }

    let typed_counts = sqlx::query(
        r#"
        SELECT COUNT(*)::BIGINT AS typed_receipts,
               COUNT(*) FILTER (
                   WHERE session.status <> 'confirmed'
                      OR NOT (session.metadata ? 'confirm_receipt')
               )::BIGINT AS unexpected_typed_receipts
        FROM import_confirm_receipts receipt
        JOIN import_sessions session
          ON session.id = receipt.session_id AND session.user_id = receipt.user_id
        "#,
    )
    .fetch_one(&mut *tx)
    .await?;
    let typed_receipts =
        import_audit_count(&typed_counts, "typed_receipts", "confirm receipt read audit")?;
    let unexpected_typed_receipts = import_audit_count(
        &typed_counts,
        "unexpected_typed_receipts",
        "confirm receipt read audit",
    )?;
    tx.commit().await?;

    Ok(ImportConfirmReceiptReadAuditReport {
        expected_migration_version: migration_ledger.expected_migration_version,
        expected_migration_count: migration_ledger.expected_migration_count,
        actual_migration_version: migration_ledger.actual_migration_version,
        migration_count: migration_ledger.migration_count,
        snapshot_token,
        batch_size,
        target_sessions,
        typed_receipts,
        materialized_target_receipts,
        unmaterialized_target_receipts,
        unexpected_typed_receipts,
        mismatch_receipts,
        batches,
        duration_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}
