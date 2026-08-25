-- no-transaction
CREATE INDEX CONCURRENTLY idx_matching_suppressions_left_bill_id
    ON matching_suppressions (left_bill_id);
