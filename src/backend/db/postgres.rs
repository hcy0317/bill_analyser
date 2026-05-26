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
    "account_aliases_legacy",
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

const POSTGRES_MIGRATION_MANIFEST: &[PostgresMigrationDescriptor] =
    &[PostgresMigrationDescriptor {
        version: 1,
        file_name: POSTGRES_INITIAL_SCHEMA_FILE,
        description: "authoritative PostgreSQL schema foundation for rules, import preview, feedback, learning, audit, and vector outbox",
        required_tables: INITIAL_SCHEMA_TABLES,
        required_indexes: INITIAL_SCHEMA_INDEXES,
    }];

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
        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest[0].version, 1);
        assert_eq!(manifest[0].file_name, POSTGRES_INITIAL_SCHEMA_FILE);
        assert!(postgres_initial_schema_path().exists());
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
