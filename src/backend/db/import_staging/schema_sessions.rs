// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
pub fn init_import_staging_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS import_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            status TEXT NOT NULL DEFAULT 'parsing',
            file_count INTEGER DEFAULT 0,
            total_parsed INTEGER DEFAULT 0,
            total_preview INTEGER DEFAULT 0,
            total_confirmed INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_import_sessions_user_session
            ON import_sessions(user_id, session_id);
        CREATE INDEX IF NOT EXISTS idx_import_sessions_session ON import_sessions(session_id);
        CREATE INDEX IF NOT EXISTS idx_import_sessions_user ON import_sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_import_sessions_status ON import_sessions(status);

        CREATE TABLE IF NOT EXISTS import_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            source_index INTEGER NOT NULL,
            original_file_name TEXT,
            parser_id TEXT NOT NULL,
            parser_name TEXT NOT NULL,
            parser_signal TEXT,
            parser_confidence REAL DEFAULT 0,
            feature_signature TEXT NOT NULL,
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(session_id, user_id, source_index),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_import_sources_session_user
            ON import_sources(session_id, user_id, source_index);
        CREATE INDEX IF NOT EXISTS idx_import_sources_parser_id
            ON import_sources(user_id, parser_id);

        CREATE TABLE IF NOT EXISTS import_standard_rows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            source_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            source_row_index INTEGER NOT NULL,
            occurred_at TEXT NOT NULL,
            amount_cents INTEGER NOT NULL,
            direction TEXT NOT NULL,
            transaction_type TEXT NOT NULL,
            merchant TEXT,
            payment_method TEXT,
            description TEXT,
            parser_payload_json TEXT,
            standard_payload_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(source_id, source_row_index),
            FOREIGN KEY (source_id) REFERENCES import_sources(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_import_standard_rows_session_user
            ON import_standard_rows(session_id, user_id, occurred_at, id);
        CREATE INDEX IF NOT EXISTS idx_import_standard_rows_match_key
            ON import_standard_rows(user_id, occurred_at, amount_cents, direction);

        CREATE TABLE IF NOT EXISTS import_decision_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            group_type TEXT NOT NULL,
            group_key TEXT NOT NULL,
            decision_status TEXT NOT NULL DEFAULT 'pending',
            base_preview_row_id INTEGER,
            signal_payload_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(session_id, user_id, group_type, group_key),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_import_decision_groups_session_group_type
            ON import_decision_groups(session_id, user_id, group_type);

        CREATE TABLE IF NOT EXISTS import_decision_group_members (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER NOT NULL,
            preview_row_id INTEGER,
            standard_row_id INTEGER,
            history_bill_id INTEGER,
            member_role TEXT NOT NULL,
            parser_name TEXT,
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(group_id, preview_row_id, standard_row_id, history_bill_id, member_role),
            FOREIGN KEY (group_id) REFERENCES import_decision_groups(id) ON DELETE CASCADE,
            FOREIGN KEY (standard_row_id) REFERENCES import_standard_rows(id) ON DELETE SET NULL
        );
        CREATE INDEX IF NOT EXISTS idx_import_decision_group_members_group
            ON import_decision_group_members(group_id, member_role);

        CREATE TABLE IF NOT EXISTS import_history_materializations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            history_bill_id INTEGER NOT NULL,
            history_bill_version INTEGER NOT NULL DEFAULT 1,
            materialized_payload_json TEXT,
            rewrite_reason TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(session_id, user_id, history_bill_id),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_import_history_materializations_session_history_bill
            ON import_history_materializations(session_id, user_id, history_bill_id);

        CREATE TABLE IF NOT EXISTS import_confirm_operations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            operation_kind TEXT NOT NULL,
            preview_row_id INTEGER,
            history_bill_id INTEGER,
            created_bill_id INTEGER,
            deleted_bill_id INTEGER,
            status TEXT NOT NULL DEFAULT 'pending',
            payload_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(session_id, user_id, operation_kind, preview_row_id, history_bill_id)
        );

        CREATE TABLE IF NOT EXISTS bills_preview (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            preview_date TEXT NOT NULL,
            preview_type TEXT NOT NULL,
            preview_amount REAL NOT NULL,
            preview_destination_amount REAL DEFAULT 0,
            preview_main_category TEXT,
            preview_sub_category TEXT,
            preview_source_account_id INTEGER,
            preview_destination_account_id INTEGER,
            preview_counterparty TEXT,
            preview_payment_method TEXT,
            preview_description TEXT,
            preview_parser_id TEXT,
            preview_parser_tags_json TEXT,
            preview_recurring_id INTEGER,
            preview_recurring_name TEXT,
            preview_recurring_candidate_count INTEGER DEFAULT 0,
            preview_recurring_match_score REAL DEFAULT 0,
            preview_recurring_match_reasons TEXT,
            preview_recurring_matched_date TEXT,
            preview_selected INTEGER DEFAULT 0,
            dedup_type TEXT,
            dedup_source_ids TEXT,
            preview_matching_feedback_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_preview_session ON bills_preview(session_id);
        CREATE INDEX IF NOT EXISTS idx_preview_selected ON bills_preview(preview_selected);
        CREATE INDEX IF NOT EXISTS idx_preview_type ON bills_preview(preview_type);
        CREATE INDEX IF NOT EXISTS idx_preview_date ON bills_preview(preview_date);
        CREATE INDEX IF NOT EXISTS idx_preview_session_user_order
            ON bills_preview(session_id, user_id, preview_date, id);
        CREATE INDEX IF NOT EXISTS idx_preview_session_user_selected_order
            ON bills_preview(session_id, user_id, preview_selected, preview_date, id);

        CREATE TABLE IF NOT EXISTS bills_parser_template (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            parser_date TEXT NOT NULL,
            parser_amount REAL NOT NULL,
            parser_type TEXT NOT NULL,
            parser_description TEXT,
            parser_id TEXT NOT NULL,
            parser_tags_json TEXT,
            parser_counterparty TEXT,
            parser_payment_method TEXT,
            parser_original_type TEXT,
            parser_original_category TEXT,
            parser_account_id TEXT,
            parser_is_processed TEXT DEFAULT '0',
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_parser_template_session ON bills_parser_template(session_id);
        CREATE INDEX IF NOT EXISTS idx_parser_template_processed ON bills_parser_template(parser_is_processed);
        CREATE INDEX IF NOT EXISTS idx_parser_template_date ON bills_parser_template(parser_date);
        CREATE INDEX IF NOT EXISTS idx_parser_template_parser_id ON bills_parser_template(parser_id);
        CREATE INDEX IF NOT EXISTS idx_parser_template_session_user_processed_order
            ON bills_parser_template(session_id, user_id, parser_is_processed, parser_date, id);

        CREATE TABLE IF NOT EXISTS import_annotation_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            preview_id INTEGER NOT NULL,
            annotated_type TEXT,
            annotated_category_id INTEGER,
            annotated_source_account_id INTEGER,
            annotated_destination_account_id INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(session_id, preview_id),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_annotation_samples_session ON import_annotation_samples(session_id);
        CREATE INDEX IF NOT EXISTS idx_annotation_samples_user ON import_annotation_samples(user_id);

        CREATE TABLE IF NOT EXISTS llm_memory_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            session_id TEXT,
            preview_id INTEGER,
            event_type TEXT NOT NULL DEFAULT 'recommendation',
            decision TEXT,
            prompt_text TEXT,
            llm_response_raw TEXT,
            llm_provider TEXT,
            llm_model TEXT,
            suggested_main_category TEXT,
            suggested_sub_category TEXT,
            suggested_source_account TEXT,
            suggested_destination_account TEXT,
            confidence REAL DEFAULT 0.0,
            user_correction_category TEXT,
            user_correction_account TEXT,
            snapshot_before TEXT,
            snapshot_after TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_user
            ON llm_memory_events(user_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_user_order
            ON llm_memory_events(user_id, created_at DESC, id DESC);
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session
            ON llm_memory_events(user_id, session_id);
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session_order
            ON llm_memory_events(user_id, session_id, created_at DESC, id DESC);
        ",
    )?;
    apply_legacy_staging_alters(connection)?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn create_import_session(connection: &Connection, draft: &ImportSessionDraft) -> DbResult<i64> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "create_import_session", "business operation entered");
    let now = now_text();
    let user_id = user_id_i64(draft.user_id)?;
    clear_user_import_staging_data(connection, user_id)?;
    connection.execute(
        "
        INSERT INTO import_sessions (
            session_id, user_id, status, file_count,
            total_parsed, total_preview, total_confirmed,
            created_at, updated_at
        ) VALUES (?1, ?2, 'parsing', ?3, 0, 0, 0, ?4, ?4)
        ",
        params![draft.session_id, user_id, draft.file_count, now],
    )?;
    Ok(connection.last_insert_rowid())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn clear_user_import_staging_data(connection: &Connection, user_id: i64) -> DbResult<usize> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "clear_user_import_staging_data", "business operation entered");
    let annotation_count = connection.execute(
        "DELETE FROM import_annotation_samples WHERE user_id = ?1",
        [user_id],
    )?;
    let group_ids = {
        let mut statement =
            connection.prepare("SELECT id FROM import_decision_groups WHERE user_id = ?1")?;
        let rows = statement.query_map([user_id], |row| row.get::<_, i64>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let decision_member_count = if group_ids.is_empty() {
        0
    } else {
        let placeholders = std::iter::repeat_n("?", group_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        connection.execute(
            &format!("DELETE FROM import_decision_group_members WHERE group_id IN ({placeholders})"),
            rusqlite::params_from_iter(group_ids.iter().copied()),
        )?
    };
    let decision_group_count =
        connection.execute("DELETE FROM import_decision_groups WHERE user_id = ?1", [user_id])?;
    let history_materialization_count = connection.execute(
        "DELETE FROM import_history_materializations WHERE user_id = ?1",
        [user_id],
    )?;
    let confirm_operation_count = connection.execute(
        "DELETE FROM import_confirm_operations WHERE user_id = ?1",
        [user_id],
    )?;
    let standard_count =
        connection.execute("DELETE FROM import_standard_rows WHERE user_id = ?1", [user_id])?;
    let source_count =
        connection.execute("DELETE FROM import_sources WHERE user_id = ?1", [user_id])?;
    let preview_count = connection.execute("DELETE FROM bills_preview WHERE user_id = ?1", [user_id])?;
    let parser_count =
        connection.execute("DELETE FROM bills_parser_template WHERE user_id = ?1", [user_id])?;
    let session_count =
        connection.execute("DELETE FROM import_sessions WHERE user_id = ?1", [user_id])?;
    Ok(annotation_count
        + decision_member_count
        + decision_group_count
        + history_materialization_count
        + confirm_operation_count
        + standard_count
        + source_count
        + preview_count
        + parser_count
        + session_count)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_import_session_status(
    connection: &Connection,
    update: &ImportSessionStatusUpdate,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "update_import_session_status", "business operation entered");
    let changed = connection.execute(
        "
        UPDATE import_sessions
        SET status = ?1,
            updated_at = ?2,
            total_parsed = COALESCE(?3, total_parsed),
            total_preview = COALESCE(?4, total_preview),
            total_confirmed = COALESCE(?5, total_confirmed)
        WHERE session_id = ?6 AND user_id = ?7
        ",
        params![
            update.status,
            now_text(),
            update.total_parsed,
            update.total_preview,
            update.total_confirmed,
            update.session_id,
            user_id_i64(update.user_id)?,
        ],
    )?;
    Ok(changed > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Option<ImportSessionRow>> {
    connection
        .query_row(
            "SELECT * FROM import_sessions WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id_i64(user_id)?],
            import_session_from_row,
        )
        .optional()
        .map_err(DbError::from)
}
