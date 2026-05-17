fn apply_legacy_staging_alters(connection: &Connection) -> DbResult<()> {
    for statement in [
        "ALTER TABLE bills_parser_template ADD COLUMN parser_tags_json TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_parser_id TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_parser_tags_json TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_id INTEGER",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_name TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_candidate_count INTEGER DEFAULT 0",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_score REAL DEFAULT 0",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_reasons TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_matched_date TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_matching_feedback_json TEXT",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_type TEXT",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_category_id INTEGER",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_source_account_id INTEGER",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_destination_account_id INTEGER",
        "ALTER TABLE import_annotation_samples ADD COLUMN created_at TEXT",
        "ALTER TABLE import_annotation_samples ADD COLUMN updated_at TEXT",
    ] {
        match connection.execute(statement, []) {
            Ok(_) => {}
            Err(error) if is_duplicate_column_error(&error) => {}
            Err(error) => return Err(DbError::from(error)),
        }
    }
    connection.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_annotation_samples_session_preview_unique \
         ON import_annotation_samples(session_id, preview_id)",
        [],
    )?;
    Ok(())
}

fn is_duplicate_column_error(error: &rusqlite::Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("duplicate column")
}

fn is_sqlite_constraint_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(sqlite_error, _)
            if sqlite_error.code == ErrorCode::ConstraintViolation
                && matches!(
                    sqlite_error.extended_code,
                    SQLITE_CONSTRAINT_UNIQUE | SQLITE_CONSTRAINT_PRIMARYKEY
                )
    )
}

fn usize_to_i64_saturating(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("user id exceeds sqlite integer range".to_string()))
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}
