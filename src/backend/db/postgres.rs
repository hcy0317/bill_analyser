use std::path::PathBuf;

pub const POSTGRES_MIGRATIONS_RELATIVE_DIR: &str = "postgres/migrations";
pub const POSTGRES_INITIAL_SCHEMA_FILE: &str = "0001_initial_authoritative_schema.sql";

pub type PostgresPool = sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresMigrationDescriptor {
    pub version: i64,
    pub file_name: &'static str,
    pub description: &'static str,
    pub required_tables: &'static [&'static str],
    pub required_indexes: &'static [&'static str],
}

const INITIAL_SCHEMA_TABLES: &[&str] = &[
    "users",
    "accounts",
    "categories",
    "tags",
    "bills",
    "bill_tags",
    "settings",
    "parser_templates",
    "category_rules",
    "account_rules",
    "import_sessions",
    "import_sources",
    "import_standard_rows",
    "import_preview_rows",
    "import_decision_groups",
    "import_decision_group_members",
    "import_history_materializations",
    "import_confirm_operations",
    "matching_pairs",
    "matching_suppressions",
    "preview_matching_feedback",
    "matching_feedback_events",
    "import_learning_samples",
    "import_learning_features",
    "import_learning_suggestions",
    "import_learning_lifecycle",
    "import_learning_feedback_events",
    "import_learning_suppressions",
    "learning_threshold_profiles",
    "business_audit_events",
    "vector_outbox_events",
    "migration_audit_events",
];

const INITIAL_SCHEMA_INDEXES: &[&str] = &[
    "idx_bills_user_time_amount_direction",
    "idx_bills_user_updated",
    "idx_accounts_user_lookup",
    "idx_categories_user_lookup",
    "idx_tags_user_lookup",
    "idx_category_rules_user_enabled_type_priority",
    "idx_account_rules_user_enabled_type_priority",
    "idx_import_preview_rows_session_page_sort_key",
    "idx_import_decision_groups_session_group_type",
    "idx_import_decision_group_members_group",
    "idx_import_history_materializations_session_history_bill",
    "idx_matching_pairs_user_status",
    "idx_matching_suppressions_user_pair",
    "idx_import_learning_lifecycle_user_recommendation_key",
    "idx_vector_outbox_events_status_available_at",
    "idx_business_audit_events_user_entity_created",
];

const TRANSACTION_TEMPLATE_TABLES: &[&str] = &["transaction_templates"];

const TRANSACTION_TEMPLATE_INDEXES: &[&str] = &["idx_transaction_templates_user_type_order"];

const BUDGET_TABLES: &[&str] = &["budgets"];

const BUDGET_INDEXES: &[&str] = &["idx_budgets_user_period_category"];

const BUDGET_HISTORY_TABLES: &[&str] = &["budget_history"];

const BUDGET_HISTORY_INDEXES: &[&str] = &[
    "idx_budget_history_user_budget_period",
    "idx_budget_history_user_filter_period",
];

const RECURRING_SUGGESTION_TABLES: &[&str] = &["recurring_suggestions"];

const RECURRING_SUGGESTION_INDEXES: &[&str] = &["idx_recurring_suggestions_user_status"];

const AUTH_SESSION_TWO_FACTOR_TABLES: &[&str] =
    &["token_sessions", "user_two_factor_recovery_codes"];

const AUTH_SESSION_TWO_FACTOR_INDEXES: &[&str] = &[
    "idx_token_sessions_token_hash",
    "idx_token_sessions_refresh_hash",
    "idx_token_sessions_user_active_created",
    "idx_user_two_factor_recovery_codes_user_unused",
];

const AUTH_EXTERNAL_LINK_TABLES: &[&str] = &["user_external_auths"];

const AUTH_EXTERNAL_LINK_INDEXES: &[&str] = &[
    "idx_user_external_auths_user",
    "idx_user_external_auths_type",
];

const BACKUP_OPS_TABLES: &[&str] = &["backup_records", "backup_jobs", "backup_audit_logs"];

const BACKUP_OPS_INDEXES: &[&str] = &[
    "idx_backup_records_status_created",
    "idx_backup_jobs_type_enabled",
    "idx_backup_jobs_user_type_enabled",
    "idx_backup_audit_logs_type",
    "idx_backup_audit_logs_target",
    "idx_backup_audit_logs_created",
    "idx_backup_audit_logs_status",
];

const LLM_IMPORT_RUNTIME_TABLES: &[&str] = &[
    "llm_configs",
    "llm_candidates",
    "llm_memory_events",
    "import_annotation_samples",
];

const LLM_IMPORT_RUNTIME_INDEXES: &[&str] = &[
    "idx_llm_configs_user_active",
    "idx_llm_candidates_user_status",
    "idx_llm_memory_events_user_created",
    "idx_import_annotation_samples_user_session",
];

const IMPORT_SESSION_KEY_TABLES: &[&str] = &["import_sessions"];

const IMPORT_SESSION_KEY_INDEXES: &[&str] = &["idx_import_sessions_user_session_key"];

const ACCOUNT_RULE_SCOPE_CLEANUP_TABLES: &[&str] = &["account_rules"];

const ACCOUNT_RULE_SCOPE_CLEANUP_INDEXES: &[&str] = &["idx_account_rules_user_enabled_priority"];

const ACCOUNT_INITIAL_BALANCE_BACKFILL_TABLES: &[&str] = &["accounts"];

const ACCOUNT_INITIAL_BALANCE_BACKFILL_INDEXES: &[&str] = &[];

const RECURRING_SUGGESTION_COMMENT_TABLES: &[&str] = &["recurring_suggestions"];

const RECURRING_SUGGESTION_COMMENT_INDEXES: &[&str] = &["idx_recurring_suggestions_user_status"];

const POSTGRES_MIGRATION_MANIFEST: &[PostgresMigrationDescriptor] = &[
    PostgresMigrationDescriptor {
        version: 1,
        file_name: POSTGRES_INITIAL_SCHEMA_FILE,
        description: "authoritative PostgreSQL schema foundation for rules, import preview, feedback, learning, audit, and vector outbox",
        required_tables: INITIAL_SCHEMA_TABLES,
        required_indexes: INITIAL_SCHEMA_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 2,
        file_name: "0002_transaction_templates.sql",
        description: "authoritative PostgreSQL transaction template runtime table",
        required_tables: TRANSACTION_TEMPLATE_TABLES,
        required_indexes: TRANSACTION_TEMPLATE_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 3,
        file_name: "0003_budgets.sql",
        description: "authoritative PostgreSQL budget runtime table",
        required_tables: BUDGET_TABLES,
        required_indexes: BUDGET_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 4,
        file_name: "0004_transaction_template_minor_units.sql",
        description: "store transaction template minor-unit amounts as exact integer values",
        required_tables: TRANSACTION_TEMPLATE_TABLES,
        required_indexes: TRANSACTION_TEMPLATE_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 5,
        file_name: "0005_budget_history.sql",
        description: "authoritative PostgreSQL budget execution history snapshots",
        required_tables: BUDGET_HISTORY_TABLES,
        required_indexes: BUDGET_HISTORY_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 6,
        file_name: "0006_recurring_suggestions.sql",
        description: "authoritative PostgreSQL recurring suggestion runtime table",
        required_tables: RECURRING_SUGGESTION_TABLES,
        required_indexes: RECURRING_SUGGESTION_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 7,
        file_name: "0007_auth_sessions_two_factor.sql",
        description: "authoritative PostgreSQL auth token sessions and two-factor recovery codes",
        required_tables: AUTH_SESSION_TWO_FACTOR_TABLES,
        required_indexes: AUTH_SESSION_TWO_FACTOR_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 8,
        file_name: "0008_auth_external_links.sql",
        description: "authoritative PostgreSQL external auth links for profile auth routes",
        required_tables: AUTH_EXTERNAL_LINK_TABLES,
        required_indexes: AUTH_EXTERNAL_LINK_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 9,
        file_name: "0009_backup_ops.sql",
        description: "authoritative PostgreSQL backup ops records, jobs, and audit metadata",
        required_tables: BACKUP_OPS_TABLES,
        required_indexes: BACKUP_OPS_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 10,
        file_name: "0010_llm_import_runtime.sql",
        description: "authoritative PostgreSQL LLM config, candidate, memory, and import annotation tables",
        required_tables: LLM_IMPORT_RUNTIME_TABLES,
        required_indexes: LLM_IMPORT_RUNTIME_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 11,
        file_name: "0011_import_session_key.sql",
        description: "authoritative import session API key and counters",
        required_tables: IMPORT_SESSION_KEY_TABLES,
        required_indexes: IMPORT_SESSION_KEY_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 12,
        file_name: "0012_drop_account_rule_scope_columns.sql",
        description: "remove deprecated persisted account rule role/type/field scope columns",
        required_tables: ACCOUNT_RULE_SCOPE_CLEANUP_TABLES,
        required_indexes: ACCOUNT_RULE_SCOPE_CLEANUP_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 13,
        file_name: "0013_account_initial_balance_cents_backfill.sql",
        description: "backfill legacy yuan JSON money fields into explicit cents",
        required_tables: ACCOUNT_INITIAL_BALANCE_BACKFILL_TABLES,
        required_indexes: ACCOUNT_INITIAL_BALANCE_BACKFILL_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 14,
        file_name: "0014_recurring_suggestion_amount_cents_comment.sql",
        description: "document recurring suggestion amount_cents as the explicit API cents contract",
        required_tables: RECURRING_SUGGESTION_COMMENT_TABLES,
        required_indexes: RECURRING_SUGGESTION_COMMENT_INDEXES,
    },
];

pub fn postgres_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(POSTGRES_MIGRATIONS_RELATIVE_DIR)
}

pub fn postgres_initial_schema_path() -> PathBuf {
    postgres_migrations_dir().join(POSTGRES_INITIAL_SCHEMA_FILE)
}

pub fn postgres_migration_manifest() -> &'static [PostgresMigrationDescriptor] {
    POSTGRES_MIGRATION_MANIFEST
}

pub async fn run_postgres_migrations(
    pool: &PostgresPool,
) -> Result<(), sqlx::migrate::MigrateError> {
    let migrator = sqlx::migrate::Migrator::new(postgres_migrations_dir()).await?;
    migrator.run(pool).await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::*;

    #[test]
    fn postgres_manifest_points_to_existing_initial_schema() {
        let manifest = postgres_migration_manifest();
        assert_eq!(manifest.len(), 14);
        assert_eq!(manifest[0].version, 1);
        assert_eq!(manifest[0].file_name, POSTGRES_INITIAL_SCHEMA_FILE);
        for (index, descriptor) in manifest.iter().enumerate() {
            assert_eq!(descriptor.version, i64::try_from(index + 1).unwrap());
        }
        assert!(postgres_initial_schema_path().exists());
        for descriptor in manifest.iter().skip(1) {
            assert!(postgres_migrations_dir()
                .join(descriptor.file_name)
                .exists());
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
}
