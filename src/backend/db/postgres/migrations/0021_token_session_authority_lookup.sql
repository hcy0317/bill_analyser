CREATE INDEX IF NOT EXISTS idx_token_sessions_authority_lookup
    ON token_sessions (token_hash, user_id, is_active, expires_at)
    INCLUDE (id);
