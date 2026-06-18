-- Add indexes used by import staging cleanup and foreign-key checks.
-- Existing deployments may already create these at runtime; this migration
-- keeps the authoritative schema aligned for fresh databases.

CREATE INDEX IF NOT EXISTS idx_import_sessions_user_id
    ON import_sessions (user_id, id);
CREATE INDEX IF NOT EXISTS idx_import_sources_user_session
    ON import_sources (user_id, session_id);
CREATE INDEX IF NOT EXISTS idx_import_standard_rows_user_session
    ON import_standard_rows (user_id, session_id);
CREATE INDEX IF NOT EXISTS idx_import_preview_rows_user_session
    ON import_preview_rows (user_id, session_id);
CREATE INDEX IF NOT EXISTS idx_import_preview_rows_base_standard_row_id
    ON import_preview_rows (base_standard_row_id);
CREATE INDEX IF NOT EXISTS idx_import_decision_groups_base_preview_row_id
    ON import_decision_groups (base_preview_row_id);
CREATE INDEX IF NOT EXISTS idx_import_decision_group_members_preview_row_id
    ON import_decision_group_members (preview_row_id);
CREATE INDEX IF NOT EXISTS idx_import_decision_group_members_standard_row_id
    ON import_decision_group_members (standard_row_id);
CREATE INDEX IF NOT EXISTS idx_import_confirm_operations_session_id
    ON import_confirm_operations (session_id);
CREATE INDEX IF NOT EXISTS idx_import_confirm_operations_preview_row_id
    ON import_confirm_operations (preview_row_id);
CREATE INDEX IF NOT EXISTS idx_preview_matching_feedback_session_id
    ON preview_matching_feedback (session_id);
CREATE INDEX IF NOT EXISTS idx_preview_matching_feedback_preview_row_id
    ON preview_matching_feedback (preview_row_id);
CREATE INDEX IF NOT EXISTS idx_preview_matching_feedback_group_id
    ON preview_matching_feedback (group_id);
CREATE INDEX IF NOT EXISTS idx_import_learning_samples_preview_row_id
    ON import_learning_samples (preview_row_id);
CREATE INDEX IF NOT EXISTS idx_import_learning_suggestions_session_id
    ON import_learning_suggestions (session_id);
CREATE INDEX IF NOT EXISTS idx_import_learning_suggestions_preview_row_id
    ON import_learning_suggestions (preview_row_id);
CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_suggestion_id
    ON import_learning_feedback_events (suggestion_id);
