use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/embedded_postgres_migrations.rs"));

pub const POSTGRES_MIGRATIONS_RELATIVE_DIR: &str = "postgres/migrations";
pub const POSTGRES_INITIAL_SCHEMA_FILE: &str = "0001_initial_authoritative_schema.sql";

pub type PostgresPool = sqlx::PgPool;

static POSTGRES_MIGRATION_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresMigrationDescriptor {
    pub version: i64,
    pub file_name: &'static str,
    pub description: &'static str,
    pub required_tables: &'static [&'static str],
    pub required_indexes: &'static [&'static str],
}

include!("postgres/migration_manifest.rs");

pub fn postgres_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(POSTGRES_MIGRATIONS_RELATIVE_DIR)
}

pub fn postgres_initial_schema_path() -> PathBuf {
    postgres_migrations_dir().join(POSTGRES_INITIAL_SCHEMA_FILE)
}

pub fn postgres_migration_manifest() -> &'static [PostgresMigrationDescriptor] {
    POSTGRES_MIGRATION_MANIFEST
}

pub fn embedded_postgres_migration_versions() -> Vec<i64> {
    embedded_postgres_migrator()
        .iter()
        .map(|migration| migration.version)
        .collect()
}

pub async fn run_postgres_migrations(
    pool: &PostgresPool,
) -> Result<(), sqlx::migrate::MigrateError> {
    // Concurrent index creation waits for every active virtual transaction.
    // Serialize in-process migration callers before SQLx takes its database
    // advisory lock so parallel tests cannot deadlock the waiting connection
    // against the concurrent index builder. SQLx still owns cross-process locking.
    // SQLx 0.8.6 does not unlock on an apply error, so consume and close the
    // checked-out connection before returning that error.
    let _migration_guard = POSTGRES_MIGRATION_GATE.lock().await;
    let migrator = embedded_postgres_migrator();
    let mut connection = pool.acquire().await?;
    connection.close_on_drop();
    let result = migrator.run_direct(&mut *connection).await;
    if result.is_err() {
        let _ = connection.close().await;
    }
    result
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::*;

    #[test]
    fn postgres_manifest_points_to_existing_initial_schema() {
        let manifest = postgres_migration_manifest();
        assert_eq!(manifest.len(), 41);
        assert_eq!(manifest[0].version, 1);
        assert_eq!(manifest[0].file_name, POSTGRES_INITIAL_SCHEMA_FILE);
        for (index, descriptor) in manifest.iter().enumerate() {
            assert_eq!(descriptor.version, i64::try_from(index + 1).unwrap());
            assert!(
                descriptor
                    .file_name
                    .starts_with(&format!("{:04}_", descriptor.version)),
                "migration file name must preserve manifest order: {}",
                descriptor.file_name
            );
        }
        assert!(postgres_initial_schema_path().exists());
        for descriptor in manifest.iter().skip(1) {
            assert!(postgres_migrations_dir()
                .join(descriptor.file_name)
                .exists());
        }

        let mut migration_files = fs::read_dir(postgres_migrations_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|file_name| file_name.ends_with(".sql"))
            .collect::<Vec<_>>();
        migration_files.sort();
        assert_eq!(
            migration_files,
            manifest
                .iter()
                .map(|descriptor| descriptor.file_name.to_string())
                .collect::<Vec<_>>(),
            "migration directory and manifest must contain the same ordered SQL files"
        );
    }

    #[test]
    fn postgres_migration_files_preserve_deployed_line_endings() {
        const CRLF_VERSIONS: &[i64] = &[1, 2, 3, 4, 10, 11, 12, 13, 19, 20, 24];
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let attributes = fs::read_to_string(repo_root.join(".gitattributes")).unwrap();

        for descriptor in postgres_migration_manifest() {
            let bytes = fs::read(postgres_migrations_dir().join(descriptor.file_name)).unwrap();
            let expected_crlf = CRLF_VERSIONS.contains(&descriptor.version);
            let has_crlf = bytes.windows(2).any(|pair| pair == b"\r\n");
            let has_bare_cr = bytes
                .iter()
                .enumerate()
                .any(|(index, byte)| *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'));

            assert!(!has_bare_cr, "{} contains a bare CR", descriptor.file_name);
            assert_eq!(has_crlf, expected_crlf, "{}", descriptor.file_name);
            if descriptor.version >= 30 {
                assert!(
                    attributes.contains(&format!(
                        "src/backend/db/postgres/migrations/{} text eol=lf",
                        descriptor.file_name
                    )),
                    "{} must pin its immutable SQLx checksum to LF",
                    descriptor.file_name
                );
            }
        }
    }

    #[test]
    fn initial_schema_contains_required_tables_and_indexes() {
        let schema = fs::read_to_string(postgres_initial_schema_path()).unwrap();

        for table in INITIAL_SCHEMA_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in INITIAL_SCHEMA_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }

        assert!(schema.contains("amount stored in cents"));
        assert!(schema.contains("updated_at TIMESTAMPTZ NOT NULL DEFAULT now()"));
        assert!(schema.contains("version BIGINT NOT NULL DEFAULT 1"));
    }

    #[test]
    fn account_rule_scope_cleanup_removes_deprecated_columns_and_rebuilds_index() {
        let schema = fs::read_to_string(postgres_initial_schema_path()).unwrap();
        let account_rules_section = schema
            .split("CREATE TABLE IF NOT EXISTS account_rules")
            .nth(1)
            .and_then(|section| {
                section
                    .split("CREATE TABLE IF NOT EXISTS import_sessions")
                    .next()
            })
            .expect("account_rules section");

        assert!(account_rules_section.contains("account_role_scope"));
        assert!(account_rules_section.contains("transaction_type_scope"));
        assert!(account_rules_section.contains("field_scope"));
        assert!(schema.contains("idx_account_rules_user_enabled_type_priority"));
        assert!(!schema.contains("idx_account_rules_user_enabled_priority"));

        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0012_drop_account_rule_scope_columns.sql"),
        )
        .unwrap();
        assert!(
            migration.contains("DROP INDEX IF EXISTS idx_account_rules_user_enabled_type_priority")
        );
        assert!(migration.contains("DROP COLUMN IF EXISTS account_role_scope"));
        assert!(migration.contains("DROP COLUMN IF EXISTS transaction_type_scope"));
        assert!(migration.contains("DROP COLUMN IF EXISTS field_scope"));
        assert!(migration.contains("idx_account_rules_user_enabled_priority"));
    }

    #[test]
    fn money_json_backfill_migration_converts_legacy_yuan_metadata() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0013_account_initial_balance_cents_backfill.sql"),
        )
        .unwrap();

        assert!(migration.contains("initial_balance_cents"));
        assert!(migration.contains("ROUND((metadata->>'initial_balance')::numeric * 100)::bigint"));
        assert!(migration.contains("NOT (metadata ? 'initial_balance_cents')"));
        assert!(migration.contains("destination_amount_cents"));
        assert!(migration.contains("standard_payload->>'destination_amount'"));
        assert!(migration.contains("standard_payload->>'destinationAmount'"));
        assert!(migration.contains("preview_destination_amount_cents"));
        assert!(migration.contains("preview_payload->>'destination_amount_cents'"));
        assert!(migration.contains("preview_payload->>'destinationAmountCents'"));
        assert!(migration.contains("preview_payload->>'preview_destination_amount'"));
    }

    #[test]
    fn transaction_template_migration_contains_required_tables_and_indexes() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0002_transaction_templates.sql"))
                .unwrap();

        for table in TRANSACTION_TEMPLATE_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in TRANSACTION_TEMPLATE_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("minor units"));
    }

    #[test]
    fn budget_migration_contains_required_tables_indexes_and_amount_comment() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0003_budgets.sql")).unwrap();

        for table in BUDGET_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in BUDGET_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("budget amount stored in cents"));
    }

    #[test]
    fn transaction_template_minor_units_migration_converts_amounts_to_bigint() {
        let schema = fs::read_to_string(
            postgres_migrations_dir().join("0004_transaction_template_minor_units.sql"),
        )
        .unwrap();

        assert!(schema.contains("TYPE BIGINT"));
        assert!(schema.contains("ROUND(source_amount_minor_units)::BIGINT"));
        assert!(schema.contains("exact minor units"));
    }

    #[test]
    fn budget_history_migration_contains_required_tables_indexes_and_amount_comments() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0005_budget_history.sql")).unwrap();

        for table in BUDGET_HISTORY_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in BUDGET_HISTORY_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("stored in cents"));
        assert!(schema
            .contains("UNIQUE (user_id, budget_id, period_start, period_end, filter_summary)"));
    }

    #[test]
    fn recurring_suggestion_migration_contains_required_tables_indexes_and_amount_comments() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0006_recurring_suggestions.sql"))
                .unwrap();

        for table in RECURRING_SUGGESTION_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in RECURRING_SUGGESTION_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("amount stored in cents"));
    }

    #[test]
    fn recurring_suggestion_amount_comment_change_is_forward_migration_only() {
        let original_migration =
            fs::read_to_string(postgres_migrations_dir().join("0006_recurring_suggestions.sql"))
                .unwrap();
        let comment_migration = fs::read_to_string(
            postgres_migrations_dir().join("0014_recurring_suggestion_amount_cents_comment.sql"),
        )
        .unwrap();

        assert!(original_migration.contains(
            "amount stored in cents; API responses project this value back to frontend yuan amount"
        ));
        assert!(comment_migration
            .contains("amount stored in cents; API responses expose this value as amountCents"));
    }

    #[test]
    fn import_preview_filter_migration_contains_required_indexes() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0015_import_preview_filter_indexes.sql"),
        )
        .unwrap();

        for index in IMPORT_PREVIEW_FILTER_INDEXES {
            assert!(migration.contains(index), "missing index {index}");
        }
    }

    #[test]
    fn import_decision_operation_migration_contains_idempotency_column_and_indexes() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0018_import_decision_group_operations.sql"),
        )
        .unwrap();

        assert!(migration.contains("ADD COLUMN IF NOT EXISTS operation_id TEXT"));
        for index in IMPORT_DECISION_OPERATION_INDEXES {
            assert!(migration.contains(index), "missing index {index}");
        }
        assert!(migration.contains("operation_kind = 'decision_group'"));
        assert!(migration.contains("operation_kind = 'history_rewrite'"));
    }

    #[test]
    fn import_preview_time_page_migration_contains_required_index() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0019_import_preview_time_page_index.sql"),
        )
        .unwrap();

        for index in IMPORT_PREVIEW_TIME_PAGE_INDEXES {
            assert!(migration.contains(index), "missing index {index}");
        }
        assert!(migration.contains("session_id, user_id, occurred_at, id"));
    }

    #[test]
    fn bills_foreign_key_index_migrations_are_non_transactional_and_complete() {
        let expected_indexes = [
            "idx_import_preview_rows_history_bill_id",
            "idx_import_decision_group_members_history_bill_id",
            "idx_import_history_materializations_history_bill_id",
            "idx_import_confirm_operations_history_bill_id",
            "idx_import_confirm_operations_created_bill_id",
            "idx_import_confirm_operations_deleted_bill_id",
            "idx_matching_pairs_left_bill_id",
            "idx_matching_pairs_right_bill_id",
            "idx_matching_suppressions_left_bill_id",
            "idx_matching_suppressions_right_bill_id",
            "idx_import_learning_samples_bill_id",
        ];
        let migrator = embedded_postgres_migrator();
        let index_migrations = &postgres_migration_manifest()[29..40];
        assert_eq!(index_migrations.len(), expected_indexes.len());

        for (descriptor, expected_index) in index_migrations.iter().zip(expected_indexes) {
            let migration =
                fs::read_to_string(postgres_migrations_dir().join(descriptor.file_name)).unwrap();
            assert!(migration.starts_with("-- no-transaction"));
            assert_eq!(migration.matches("CREATE INDEX CONCURRENTLY").count(), 1);
            assert!(
                !migration.contains("IF NOT EXISTS"),
                "{} must fail closed on a same-name invalid index",
                descriptor.file_name
            );
            assert!(
                migration.contains(expected_index),
                "missing index {expected_index}"
            );
            assert_eq!(descriptor.required_indexes, &[expected_index]);

            let embedded = migrator
                .iter()
                .find(|candidate| candidate.version == descriptor.version)
                .expect("embedded bills foreign-key migration");
            assert!(
                embedded.no_tx,
                "{} must disable transactions",
                descriptor.file_name
            );
        }
    }

    #[test]
    fn import_preview_signal_flag_migration_contains_projection_functions() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0020_import_preview_signal_flags.sql"),
        )
        .unwrap();

        assert!(migration.contains("FUNCTION import_preview_signal_flags"));
        assert!(migration.contains("FUNCTION import_preview_meaningful_feedback"));
        assert!(migration.contains("IMMUTABLE"));
        assert!(migration.contains("PARALLEL SAFE"));
    }

    #[test]
    fn import_preview_signal_status_migration_fails_closed_unknown_states() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0024_import_preview_signal_status_fail_closed.sql"),
        )
        .unwrap();

        assert!(migration.contains("CREATE OR REPLACE FUNCTION import_preview_signal_flags"));
        assert!(migration.contains("canonical_statuses TEXT[]"));
        assert!(migration.contains("reconciliation_status = ANY(canonical_statuses)"));
        assert!(migration.contains("transfer_status = ANY(canonical_statuses)"));
        assert!(migration.contains("PARALLEL SAFE"));
    }

    #[test]
    fn import_parser_signal_evidence_migration_requires_parser_identity() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0029_import_parser_signal_evidence.sql"),
        )
        .unwrap();

        assert!(migration.contains("CREATE OR REPLACE FUNCTION import_preview_parser_evidence"));
        assert!(migration.contains("parser_section->>'parser_id'"));
        assert!(migration.contains("parser_section->'parser_tags'"));
        assert!(migration.contains("parser_source := import_preview_parser_evidence(payload)"));
        assert!(!migration.contains("feedback ? 'parser'"));
    }

    #[test]
    fn import_schema_invariants_migration_declares_scoped_ownership_and_nullable_uniqueness() {
        let migration =
            fs::read_to_string(postgres_migrations_dir().join("0025_import_schema_invariants.sql"))
                .unwrap();

        for constraint in [
            "uq_import_sessions_id_user",
            "chk_import_sessions_status",
            "chk_import_sessions_import_mode",
            "chk_import_preview_rows_operation_kind",
            "chk_import_decision_groups_group_type",
            "chk_import_decision_groups_decision_status",
            "chk_import_confirm_operations_operation_kind",
            "chk_import_confirm_operations_status",
            "chk_import_learning_lifecycle_status",
            "fk_import_sources_session_user",
            "fk_import_standard_rows_session_user",
            "fk_import_preview_rows_session_user",
            "fk_import_decision_groups_session_user",
            "fk_import_history_materializations_session_user",
            "fk_import_confirm_operations_session_user",
        ] {
            assert!(
                migration.contains(constraint),
                "missing import invariant {constraint}"
            );
        }

        for index in [
            "uq_import_decision_group_members_preview_role",
            "uq_import_decision_group_members_standard_role",
            "uq_import_decision_group_members_history_role",
        ] {
            assert!(migration.contains(index), "missing partial index {index}");
        }
        assert!(migration.contains("NOT VALID"));
        assert!(migration.contains("VALIDATE CONSTRAINT"));
        for lifecycle_status in ["'pending'", "'accepted'", "'disabled'"] {
            assert!(
                migration.contains(lifecycle_status),
                "missing compatible learning lifecycle status {lifecycle_status}"
            );
        }
    }

    #[tokio::test]
    async fn recurring_suggestion_migration_checksum_matches_applied_contract() {
        let migrator = sqlx::migrate::Migrator::new(postgres_migrations_dir())
            .await
            .expect("migrations load");
        let migration = migrator
            .iter()
            .find(|migration| migration.version == 6)
            .expect("recurring suggestions migration exists");
        let checksum = migration
            .checksum
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        assert_eq!(
            checksum,
            "25d94b1891c87b2f4e0179392418b0408bd1474f96bd083427872611985f485791c0082bbd321e5151c17ac74f93b1f8"
        );
    }

    #[test]
    fn auth_session_two_factor_migration_contains_required_tables_and_indexes() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0007_auth_sessions_two_factor.sql"))
                .unwrap();

        for table in AUTH_SESSION_TWO_FACTOR_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in AUTH_SESSION_TWO_FACTOR_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("refresh_token_hash"));
        assert!(schema.contains("code_hash"));
    }

    #[test]
    fn auth_external_link_migration_contains_required_tables_and_indexes() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0008_auth_external_links.sql"))
                .unwrap();

        for table in AUTH_EXTERNAL_LINK_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in AUTH_EXTERNAL_LINK_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("UNIQUE (user_id, external_auth_type)"));
    }

    #[test]
    fn backup_ops_migration_contains_required_tables_and_indexes() {
        let schema =
            fs::read_to_string(postgres_migrations_dir().join("0009_backup_ops.sql")).unwrap();

        for table in BACKUP_OPS_TABLES {
            assert!(
                schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "missing table {table}"
            );
        }
        for index in BACKUP_OPS_INDEXES {
            assert!(schema.contains(index), "missing index {index}");
        }
        assert!(schema.contains("UNIQUE (user_id, job_type)"));
        assert!(schema.contains("metadata JSONB"));
    }

    #[tokio::test]
    async fn run_postgres_migrations_loads_files_before_pool_failure() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://bill:secret@127.0.0.1:1/bill_analyser")
            .unwrap();

        let result = run_postgres_migrations(&pool).await;

        assert!(result.is_err());
    }

    #[test]
    fn import_signal_column_expand_migration_is_additive_and_unindexed() {
        let migration = fs::read_to_string(
            postgres_migrations_dir().join("0026_import_signal_projection_columns.sql"),
        )
        .unwrap();

        for column in [
            "signal_parser BOOLEAN",
            "signal_platform_duplicate BOOLEAN",
            "signal_transfer BOOLEAN",
            "signal_history BOOLEAN",
            "signal_learning BOOLEAN",
            "signal_llm BOOLEAN",
        ] {
            assert!(
                migration.contains(column),
                "missing nullable column {column}"
            );
        }
        assert!(migration.contains("signal_projection_version SMALLINT NOT NULL DEFAULT 0"));
        assert!(migration.contains("chk_import_preview_rows_signal_projection_version"));
        assert!(migration.contains("signal_projection_version IN (0, 1)"));
        assert!(!migration.contains("CREATE INDEX"));
        assert!(!migration.contains("UPDATE import_preview_rows"));
        assert!(!migration.contains("import_preview_signal_flags("));
    }

    #[test]
    fn import_confirm_receipt_expand_migration_is_typed_immutable_and_runtime_neutral() {
        let migration =
            fs::read_to_string(postgres_migrations_dir().join("0027_import_confirm_receipts.sql"))
                .unwrap();

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS import_confirm_receipts"));
        for contract in [
            "CONSTRAINT pk_import_confirm_receipts PRIMARY KEY (session_id)",
            "CONSTRAINT fk_import_confirm_receipts_session_user",
            "FOREIGN KEY (session_id, user_id)",
            "REFERENCES import_sessions (id, user_id)",
            "command_fingerprint ~ '^[0-9a-f]{64}$'",
            "request_session_version > 0",
            "jsonb_typeof(success_envelope) = 'object'",
            "CREATE TRIGGER trg_import_confirm_receipts_immutable",
        ] {
            assert!(
                migration.contains(contract),
                "missing receipt contract {contract}"
            );
        }
        assert!(!migration.contains("updated_at"));
        assert!(!migration.contains("INSERT INTO import_confirm_receipts"));
        assert!(!migration.contains("UPDATE import_sessions"));
    }
}
