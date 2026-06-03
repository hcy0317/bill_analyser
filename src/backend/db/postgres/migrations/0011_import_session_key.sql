ALTER TABLE import_sessions
    ADD COLUMN IF NOT EXISTS session_key TEXT;

UPDATE import_sessions
SET session_key = id::text
WHERE session_key IS NULL OR btrim(session_key) = '';

ALTER TABLE import_sessions
    ALTER COLUMN session_key SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_import_sessions_user_session_key
    ON import_sessions (user_id, session_key);

ALTER TABLE import_sessions
    ADD COLUMN IF NOT EXISTS file_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS total_parsed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS total_preview BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS total_confirmed BIGINT NOT NULL DEFAULT 0;
