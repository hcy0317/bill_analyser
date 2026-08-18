CREATE TABLE IF NOT EXISTS import_confirm_receipts (
    session_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    receipt_schema_version SMALLINT NOT NULL,
    command_fingerprint TEXT NOT NULL,
    request_session_version BIGINT NOT NULL,
    response_schema_version SMALLINT NOT NULL,
    http_status SMALLINT NOT NULL,
    success_envelope JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT pk_import_confirm_receipts PRIMARY KEY (session_id),
    CONSTRAINT fk_import_confirm_receipts_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE,
    CONSTRAINT chk_import_confirm_receipts_receipt_schema_version
        CHECK (receipt_schema_version = 1),
    CONSTRAINT chk_import_confirm_receipts_command_fingerprint
        CHECK (command_fingerprint ~ '^[0-9a-f]{64}$'),
    CONSTRAINT chk_import_confirm_receipts_request_session_version
        CHECK (request_session_version > 0),
    CONSTRAINT chk_import_confirm_receipts_response_schema_version
        CHECK (response_schema_version = 1),
    CONSTRAINT chk_import_confirm_receipts_http_status
        CHECK (http_status = 200),
    CONSTRAINT chk_import_confirm_receipts_success_envelope
        CHECK (jsonb_typeof(success_envelope) = 'object')
);

CREATE OR REPLACE FUNCTION reject_import_confirm_receipt_update()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'import confirm receipts are immutable'
        USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER trg_import_confirm_receipts_immutable
BEFORE UPDATE ON import_confirm_receipts
FOR EACH ROW
EXECUTE FUNCTION reject_import_confirm_receipt_update();
