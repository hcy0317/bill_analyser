-- no-transaction
CREATE INDEX CONCURRENTLY idx_import_learning_samples_bill_id
    ON import_learning_samples (bill_id);
