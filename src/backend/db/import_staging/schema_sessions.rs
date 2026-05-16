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
            preview_selected INTEGER DEFAULT 1,
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

pub fn create_import_session(connection: &Connection, draft: &ImportSessionDraft) -> DbResult<i64> {
    let now = now_text();
    let user_id = user_id_i64(draft.user_id)?;
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

pub fn update_import_session_status(
    connection: &Connection,
    update: &ImportSessionStatusUpdate,
) -> DbResult<bool> {
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
