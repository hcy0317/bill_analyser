CREATE INDEX IF NOT EXISTS idx_import_preview_rows_session_occurred_at
    ON import_preview_rows (session_id, user_id, occurred_at, id);
