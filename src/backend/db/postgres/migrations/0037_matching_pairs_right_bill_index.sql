-- no-transaction
CREATE INDEX CONCURRENTLY idx_matching_pairs_right_bill_id
    ON matching_pairs (right_bill_id);
