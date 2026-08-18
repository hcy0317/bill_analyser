ALTER TABLE import_preview_rows
    ADD COLUMN IF NOT EXISTS signal_parser BOOLEAN,
    ADD COLUMN IF NOT EXISTS signal_platform_duplicate BOOLEAN,
    ADD COLUMN IF NOT EXISTS signal_transfer BOOLEAN,
    ADD COLUMN IF NOT EXISTS signal_history BOOLEAN,
    ADD COLUMN IF NOT EXISTS signal_learning BOOLEAN,
    ADD COLUMN IF NOT EXISTS signal_llm BOOLEAN,
    ADD COLUMN IF NOT EXISTS signal_projection_version SMALLINT NOT NULL DEFAULT 0;

ALTER TABLE import_preview_rows
    ADD CONSTRAINT chk_import_preview_rows_signal_projection_version
        CHECK (signal_projection_version IN (0, 1)) NOT VALID;

ALTER TABLE import_preview_rows
    VALIDATE CONSTRAINT chk_import_preview_rows_signal_projection_version;

COMMENT ON COLUMN import_preview_rows.signal_projection_version IS
    '0=legacy or unmaterialized; 1=PreviewStateKernel v1';
