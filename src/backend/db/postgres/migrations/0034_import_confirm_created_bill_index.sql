-- no-transaction
CREATE INDEX CONCURRENTLY idx_import_confirm_operations_created_bill_id
    ON import_confirm_operations (created_bill_id);
