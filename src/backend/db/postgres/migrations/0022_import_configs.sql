CREATE TABLE IF NOT EXISTS import_configs (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    normalized_name TEXT GENERATED ALWAYS AS (
        lower(regexp_replace(btrim(name), '[[:space:]]+', ' ', 'g'))
    ) STORED,
    file_format TEXT NOT NULL CHECK (file_format IN ('csv', 'excel')),
    description TEXT NOT NULL DEFAULT '',
    field_mappings JSONB NOT NULL DEFAULT '{}'::JSONB
        CHECK (jsonb_typeof(field_mappings) = 'object'),
    sample_headers JSONB NOT NULL DEFAULT '[]'::JSONB
        CHECK (jsonb_typeof(sample_headers) = 'array'),
    date_format TEXT NOT NULL DEFAULT '',
    delimiter TEXT,
    encoding TEXT NOT NULL DEFAULT 'utf-8',
    skip_rows INTEGER NOT NULL DEFAULT 0 CHECK (skip_rows >= 0),
    has_header BOOLEAN NOT NULL DEFAULT TRUE,
    custom_rules JSONB NOT NULL DEFAULT '{}'::JSONB
        CHECK (jsonb_typeof(custom_rules) = 'object'),
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_import_configs_user_name_format
    ON import_configs (user_id, normalized_name, file_format);

CREATE UNIQUE INDEX IF NOT EXISTS uq_import_configs_user_format_default
    ON import_configs (user_id, file_format)
    WHERE is_default;

CREATE INDEX IF NOT EXISTS idx_import_configs_user_format_list
    ON import_configs (user_id, file_format, is_default DESC, updated_at DESC, id ASC);
