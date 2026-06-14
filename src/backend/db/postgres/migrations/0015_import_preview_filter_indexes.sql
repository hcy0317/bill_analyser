CREATE INDEX IF NOT EXISTS idx_import_preview_rows_session_category
    ON import_preview_rows (session_id, user_id, category_id)
    WHERE category_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_import_preview_rows_session_account
    ON import_preview_rows (session_id, user_id, account_id)
    WHERE account_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_import_preview_rows_session_transfer_account
    ON import_preview_rows (session_id, user_id, transfer_target_account_id)
    WHERE transfer_target_account_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_import_preview_rows_session_selected
    ON import_preview_rows (session_id, user_id, selected);

CREATE INDEX IF NOT EXISTS idx_import_preview_rows_session_type
    ON import_preview_rows (session_id, user_id, transaction_type);
