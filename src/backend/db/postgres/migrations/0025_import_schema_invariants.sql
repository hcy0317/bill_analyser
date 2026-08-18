ALTER TABLE import_sessions
    ADD CONSTRAINT uq_import_sessions_id_user UNIQUE (id, user_id);

ALTER TABLE import_sessions
    ADD CONSTRAINT chk_import_sessions_status
        CHECK (status IN ('created', 'parsing', 'preview', 'confirmed')) NOT VALID,
    ADD CONSTRAINT chk_import_sessions_import_mode
        CHECK (import_mode = 'preview') NOT VALID;

ALTER TABLE import_preview_rows
    ADD CONSTRAINT chk_import_preview_rows_operation_kind
        CHECK (operation_kind = 'insert') NOT VALID;

ALTER TABLE import_decision_groups
    ADD CONSTRAINT chk_import_decision_groups_group_type
        CHECK (group_type IN (
            'duplicate',
            'historical_duplicate',
            'same_batch_transfer',
            'historical_transfer'
        )) NOT VALID,
    ADD CONSTRAINT chk_import_decision_groups_decision_status
        CHECK (decision_status IN (
            'pending',
            'matched',
            'merged',
            'accepted',
            'rejected',
            'suppressed'
        )) NOT VALID;

ALTER TABLE import_confirm_operations
    ADD CONSTRAINT chk_import_confirm_operations_operation_kind
        CHECK (operation_kind IN ('decision_group', 'history_rewrite')) NOT VALID,
    ADD CONSTRAINT chk_import_confirm_operations_status
        CHECK (status IN ('pending', 'completed', 'failed')) NOT VALID;

ALTER TABLE import_learning_lifecycle
    ADD CONSTRAINT chk_import_learning_lifecycle_status
        CHECK (status IN (
            'yellow',
            'green',
            'auto_applied',
            'downgraded',
            'suppressed',
            'pending',
            'accepted',
            'disabled'
        )) NOT VALID;

ALTER TABLE import_sources
    ADD CONSTRAINT fk_import_sources_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE
        NOT VALID;

ALTER TABLE import_standard_rows
    ADD CONSTRAINT fk_import_standard_rows_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE
        NOT VALID;

ALTER TABLE import_preview_rows
    ADD CONSTRAINT fk_import_preview_rows_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE
        NOT VALID;

ALTER TABLE import_decision_groups
    ADD CONSTRAINT fk_import_decision_groups_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE
        NOT VALID;

ALTER TABLE import_history_materializations
    ADD CONSTRAINT fk_import_history_materializations_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE
        NOT VALID;

ALTER TABLE import_confirm_operations
    ADD CONSTRAINT fk_import_confirm_operations_session_user
        FOREIGN KEY (session_id, user_id)
        REFERENCES import_sessions (id, user_id)
        ON DELETE CASCADE
        NOT VALID;

CREATE UNIQUE INDEX uq_import_decision_group_members_preview_role
    ON import_decision_group_members (group_id, preview_row_id, member_role)
    WHERE preview_row_id IS NOT NULL;

CREATE UNIQUE INDEX uq_import_decision_group_members_standard_role
    ON import_decision_group_members (group_id, standard_row_id, member_role)
    WHERE standard_row_id IS NOT NULL;

CREATE UNIQUE INDEX uq_import_decision_group_members_history_role
    ON import_decision_group_members (group_id, history_bill_id, member_role)
    WHERE history_bill_id IS NOT NULL;

ALTER TABLE import_sessions
    VALIDATE CONSTRAINT chk_import_sessions_status,
    VALIDATE CONSTRAINT chk_import_sessions_import_mode;

ALTER TABLE import_preview_rows
    VALIDATE CONSTRAINT chk_import_preview_rows_operation_kind;

ALTER TABLE import_decision_groups
    VALIDATE CONSTRAINT chk_import_decision_groups_group_type,
    VALIDATE CONSTRAINT chk_import_decision_groups_decision_status;

ALTER TABLE import_confirm_operations
    VALIDATE CONSTRAINT chk_import_confirm_operations_operation_kind,
    VALIDATE CONSTRAINT chk_import_confirm_operations_status;

ALTER TABLE import_learning_lifecycle
    VALIDATE CONSTRAINT chk_import_learning_lifecycle_status;

ALTER TABLE import_sources
    VALIDATE CONSTRAINT fk_import_sources_session_user;

ALTER TABLE import_standard_rows
    VALIDATE CONSTRAINT fk_import_standard_rows_session_user;

ALTER TABLE import_preview_rows
    VALIDATE CONSTRAINT fk_import_preview_rows_session_user;

ALTER TABLE import_decision_groups
    VALIDATE CONSTRAINT fk_import_decision_groups_session_user;

ALTER TABLE import_history_materializations
    VALIDATE CONSTRAINT fk_import_history_materializations_session_user;

ALTER TABLE import_confirm_operations
    VALIDATE CONSTRAINT fk_import_confirm_operations_session_user;
