ALTER TABLE import_confirm_operations ADD COLUMN IF NOT EXISTS operation_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_import_confirm_operations_group_decision_idempotency
ON import_confirm_operations (user_id, session_id, operation_id)
WHERE operation_kind = 'decision_group' AND operation_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_import_confirm_operations_history_rewrite_idempotency
ON import_confirm_operations (user_id, session_id, operation_id)
WHERE operation_kind = 'history_rewrite' AND operation_id IS NOT NULL;
