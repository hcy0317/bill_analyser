-- no-transaction
-- PostgreSQL does not create indexes for referencing foreign-key columns.
-- Keep one concurrent index per migration because PostgreSQL wraps a
-- multi-statement simple query in an implicit transaction.

CREATE INDEX CONCURRENTLY idx_import_preview_rows_history_bill_id
    ON import_preview_rows (history_bill_id);
