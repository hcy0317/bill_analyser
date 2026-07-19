ALTER TABLE llm_candidates
    ADD COLUMN IF NOT EXISTS suggested_account_id BIGINT REFERENCES accounts(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS suggested_account_name TEXT NOT NULL DEFAULT '';

