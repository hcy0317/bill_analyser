-- no-transaction
CREATE INDEX CONCURRENTLY idx_matching_pairs_left_bill_id
    ON matching_pairs (left_bill_id);
