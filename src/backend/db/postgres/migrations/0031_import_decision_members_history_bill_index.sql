-- no-transaction
CREATE INDEX CONCURRENTLY idx_import_decision_group_members_history_bill_id
    ON import_decision_group_members (history_bill_id);
