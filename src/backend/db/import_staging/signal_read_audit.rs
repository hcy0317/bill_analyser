pub const IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_ROW_BATCH_SIZE: u32 = 10_000;
pub const IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewSignalTargetAuditBatchReport {
    pub start_after_row_id: i64,
    pub next_after_row_id: i64,
    pub observed_rows: u64,
    pub unmaterialized_rows: u64,
    pub mismatch_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewSignalTargetAuditReport {
    pub expected_migration_version: i64,
    pub actual_migration_version: i64,
    pub migration_count: u64,
    pub snapshot_token: String,
    pub row_batch_size: u32,
    pub query_page_size: u32,
    pub preview_rows: u64,
    pub sessions: u64,
    pub row_batches: Vec<ImportPreviewSignalTargetAuditBatchReport>,
    pub unmaterialized_rows: u64,
    pub row_mismatch_rows: u64,
    pub query_audit_sessions: u64,
    pub query_audit_cases: u64,
    pub query_mismatch_cases: u64,
    pub query_mismatch_dimensions: u64,
    pub duration_ms: u64,
}

impl ImportPreviewSignalTargetAuditReport {
    pub fn is_match(&self) -> bool {
        self.actual_migration_version == self.expected_migration_version
            && self.migration_count
                == u64::try_from(self.expected_migration_version).unwrap_or(u64::MAX)
            && self.unmaterialized_rows == 0
            && self.row_mismatch_rows == 0
            && self.query_mismatch_cases == 0
            && self.query_mismatch_dimensions == 0
    }
}

/// 迁移期只读 shadow：同一请求分别走 legacy payload 与 typed v1 columns，生产入口不调用它。
#[tracing::instrument(level = "debug", skip_all)]
pub fn audit_import_preview_signal_read_parity(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    request: &ImportPreviewPageRequest,
) -> DbResult<ImportPreviewSignalReadParityReport> {
    block_on_db(async move {
        let user_id_i64 = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let session_db_id = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2",
        )
        .bind(session_id)
        .bind(user_id_i64)
        .fetch_one(&mut *tx)
        .await?
        .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))?;
        ensure_preview_signal_projection_v1(&mut tx, session_db_id, user_id_i64).await?;
        let report = query_import_preview_signal_read_parity_on_connection(
            &mut tx,
            session_db_id,
            user_id_i64,
            request,
        )
        .await?;
        tx.commit().await?;
        Ok(report)
    })
}

#[derive(Debug)]
struct ImportPreviewSignalAuditSession {
    session_id: i64,
    user_id: i64,
    row_count: i64,
    families: [bool; 6],
}

/// 在一个只读可重复读快照中完成目标库全量行 parity 与有界查询语义审计。
pub async fn audit_import_preview_signal_target_snapshot(
    pool: &PostgresPool,
    row_batch_size: u32,
    query_page_size: u32,
) -> DbResult<ImportPreviewSignalTargetAuditReport> {
    validate_import_preview_signal_target_audit_limits(row_batch_size, query_page_size)?;
    let started_at = std::time::Instant::now();
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let migration_ledger = validate_import_audit_migration_ledger(
        &mut tx,
        "import signal target audit",
    )
    .await?;
    let snapshot_token: String =
        sqlx::query_scalar("SELECT txid_current_snapshot()::text")
            .fetch_one(&mut *tx)
            .await?;

    let mut row_batches = Vec::new();
    let mut after_row_id = 0_i64;
    let mut preview_rows = 0_u64;
    let mut unmaterialized_rows = 0_u64;
    let mut row_mismatch_rows = 0_u64;
    loop {
        let row = sqlx::query(
            r#"WITH audit_batch AS MATERIALIZED (
                   SELECT preview.id,
                          preview.signal_projection_version,
                          preview.signal_parser,
                          preview.signal_platform_duplicate,
                          preview.signal_transfer,
                          preview.signal_history,
                          preview.signal_learning,
                          preview.signal_llm,
                          legacy.parser AS legacy_parser,
                          legacy.platform_duplicate AS legacy_platform_duplicate,
                          legacy.transfer AS legacy_transfer,
                          legacy.history AS legacy_history,
                          legacy.learning AS legacy_learning,
                          legacy.llm AS legacy_llm
                   FROM import_preview_rows preview
                   CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(preview.preview_payload))
                       AS legacy(parser BOOLEAN, platform_duplicate BOOLEAN, transfer BOOLEAN,
                                 history BOOLEAN, learning BOOLEAN, llm BOOLEAN)
                   WHERE preview.id > $1
                   ORDER BY preview.id
                   LIMIT $2
               )
               SELECT MAX(id) AS next_after_row_id,
                      COUNT(*)::BIGINT AS observed_rows,
                      COUNT(*) FILTER (
                          WHERE signal_projection_version <> 1
                             OR signal_parser IS NULL
                             OR signal_platform_duplicate IS NULL
                             OR signal_transfer IS NULL
                             OR signal_history IS NULL
                             OR signal_learning IS NULL
                             OR signal_llm IS NULL
                      )::BIGINT AS unmaterialized_rows,
                      COUNT(*) FILTER (
                          WHERE signal_projection_version = 1
                            AND (signal_parser IS DISTINCT FROM legacy_parser
                              OR signal_platform_duplicate IS DISTINCT FROM legacy_platform_duplicate
                              OR signal_transfer IS DISTINCT FROM legacy_transfer
                              OR signal_history IS DISTINCT FROM legacy_history
                              OR signal_learning IS DISTINCT FROM legacy_learning
                              OR signal_llm IS DISTINCT FROM legacy_llm)
                      )::BIGINT AS mismatch_rows
               FROM audit_batch"#,
        )
        .bind(after_row_id)
        .bind(i64::from(row_batch_size))
        .fetch_one(&mut *tx)
        .await?;
        let Some(next_after_row_id) = row.try_get::<Option<i64>, _>("next_after_row_id")? else {
            break;
        };
        let observed = import_audit_count(&row, "observed_rows", "import signal target audit")?;
        let unmaterialized = import_audit_count(
            &row,
            "unmaterialized_rows",
            "import signal target audit",
        )?;
        let mismatches =
            import_audit_count(&row, "mismatch_rows", "import signal target audit")?;
        row_batches.push(ImportPreviewSignalTargetAuditBatchReport {
            start_after_row_id: after_row_id,
            next_after_row_id,
            observed_rows: observed,
            unmaterialized_rows: unmaterialized,
            mismatch_rows: mismatches,
        });
        preview_rows = preview_rows.saturating_add(observed);
        unmaterialized_rows = unmaterialized_rows.saturating_add(unmaterialized);
        row_mismatch_rows = row_mismatch_rows.saturating_add(mismatches);
        after_row_id = next_after_row_id;
        if observed < u64::from(row_batch_size) {
            break;
        }
    }

    let sessions = load_import_preview_signal_audit_sessions(&mut tx).await?;
    let session_count = u64::try_from(sessions.len()).unwrap_or(u64::MAX);
    let mut query_audit_sessions = 0_u64;
    let mut query_audit_cases = 0_u64;
    let mut query_mismatch_cases = 0_u64;
    let mut query_mismatch_dimensions = 0_u64;
    if unmaterialized_rows == 0 && row_mismatch_rows == 0 {
        for session in select_import_preview_signal_audit_corpus(&sessions) {
            ensure_preview_signal_projection_v1(&mut tx, session.session_id, session.user_id)
                .await?;
            query_audit_sessions = query_audit_sessions.saturating_add(1);
            for signal in std::iter::once(None).chain(
                IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES
                    .iter()
                    .map(|family| Some((*family).to_string())),
            ) {
                let request = ImportPreviewPageRequest {
                    page_size: usize::try_from(query_page_size).unwrap_or(usize::MAX),
                    filters: ImportPreviewQueryFilters {
                        signal,
                        ..ImportPreviewQueryFilters::default()
                    },
                    ..ImportPreviewPageRequest::default()
                };
                let parity = query_import_preview_signal_read_parity_on_connection(
                    &mut tx,
                    session.session_id,
                    session.user_id,
                    &request,
                )
                .await?;
                query_audit_cases = query_audit_cases.saturating_add(1);
                if !parity.is_match() {
                    query_mismatch_cases = query_mismatch_cases.saturating_add(1);
                    query_mismatch_dimensions = query_mismatch_dimensions
                        .saturating_add(u64::try_from(parity.mismatch_count()).unwrap_or(u64::MAX));
                }
            }
        }
    }
    tx.commit().await?;

    Ok(ImportPreviewSignalTargetAuditReport {
        expected_migration_version: migration_ledger.expected_migration_version,
        actual_migration_version: migration_ledger.actual_migration_version,
        migration_count: migration_ledger.migration_count,
        snapshot_token,
        row_batch_size,
        query_page_size,
        preview_rows,
        sessions: session_count,
        row_batches,
        unmaterialized_rows,
        row_mismatch_rows,
        query_audit_sessions,
        query_audit_cases,
        query_mismatch_cases,
        query_mismatch_dimensions,
        duration_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}

fn validate_import_preview_signal_target_audit_limits(
    row_batch_size: u32,
    query_page_size: u32,
) -> DbResult<()> {
    if row_batch_size == 0 || row_batch_size > IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_ROW_BATCH_SIZE
    {
        return Err(DbError::InvalidOperation(format!(
            "import signal target audit row batch size must be between 1 and {IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_ROW_BATCH_SIZE}"
        )));
    }
    if query_page_size == 0 || query_page_size > IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE {
        return Err(DbError::InvalidOperation(format!(
            "import signal target audit query page size must be between 1 and {IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE}"
        )));
    }
    Ok(())
}

async fn load_import_preview_signal_audit_sessions(
    connection: &mut sqlx::PgConnection,
) -> DbResult<Vec<ImportPreviewSignalAuditSession>> {
    let rows = sqlx::query(
        r#"SELECT preview.session_id,
                  preview.user_id,
                  COUNT(*)::BIGINT AS row_count,
                  COALESCE(BOOL_OR(preview.signal_parser), false) AS has_parser,
                  COALESCE(BOOL_OR(preview.signal_platform_duplicate), false) AS has_platform_duplicate,
                  COALESCE(BOOL_OR(preview.signal_transfer), false) AS has_transfer,
                  COALESCE(BOOL_OR(preview.signal_history), false) AS has_history,
                  COALESCE(BOOL_OR(preview.signal_learning), false) AS has_learning,
                  COALESCE(BOOL_OR(preview.signal_llm), false) AS has_llm
           FROM import_preview_rows preview
           GROUP BY preview.session_id, preview.user_id
           ORDER BY row_count DESC, preview.session_id ASC"#,
    )
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportPreviewSignalAuditSession {
                session_id: row.try_get("session_id")?,
                user_id: row.try_get("user_id")?,
                row_count: row.try_get("row_count")?,
                families: [
                    row.try_get("has_parser")?,
                    row.try_get("has_platform_duplicate")?,
                    row.try_get("has_transfer")?,
                    row.try_get("has_history")?,
                    row.try_get("has_learning")?,
                    row.try_get("has_llm")?,
                ],
            })
        })
        .collect()
}

fn select_import_preview_signal_audit_corpus(
    sessions: &[ImportPreviewSignalAuditSession],
) -> Vec<&ImportPreviewSignalAuditSession> {
    let mut selected = Vec::new();
    let mut selected_ids = BTreeSet::new();
    if let Some(largest) = sessions.first() {
        selected.push(largest);
        selected_ids.insert(largest.session_id);
    }
    for family_index in 0..IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES.len() {
        if let Some(session) = sessions.iter().find(|session| {
            session.families[family_index] && !selected_ids.contains(&session.session_id)
        }) {
            selected.push(session);
            selected_ids.insert(session.session_id);
        }
    }
    selected.sort_by_key(|session| (std::cmp::Reverse(session.row_count), session.session_id));
    selected
}
