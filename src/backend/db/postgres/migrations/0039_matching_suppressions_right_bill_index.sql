-- no-transaction
CREATE INDEX CONCURRENTLY idx_matching_suppressions_right_bill_id
    ON matching_suppressions (right_bill_id);
