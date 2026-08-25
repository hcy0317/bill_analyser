-- no-transaction
CREATE INDEX CONCURRENTLY idx_import_confirm_operations_deleted_bill_id
    ON import_confirm_operations (deleted_bill_id);
