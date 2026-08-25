-- no-transaction
CREATE INDEX CONCURRENTLY idx_import_history_materializations_history_bill_id
    ON import_history_materializations (history_bill_id);
