-- Remove deprecated account-rule persisted scope columns.
-- Account recognition now derives role, transaction type, and field bundle from
-- the stabilized import/runtime context; account_rules stores expression rules
-- only.

DROP INDEX IF EXISTS idx_account_rules_user_enabled_type_priority;

ALTER TABLE account_rules
    DROP COLUMN IF EXISTS account_role_scope,
    DROP COLUMN IF EXISTS transaction_type_scope,
    DROP COLUMN IF EXISTS field_scope;

CREATE INDEX IF NOT EXISTS idx_account_rules_user_enabled_priority
    ON account_rules (user_id, enabled, priority, id);
