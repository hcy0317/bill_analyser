-- no-transaction
CREATE INDEX CONCURRENTLY idx_import_confirm_operations_history_bill_id
    ON import_confirm_operations (history_bill_id);
