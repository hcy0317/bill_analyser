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

const IMPORT_PREVIEW_FILTER_INDEX_TABLES: &[&str] = &["import_preview_rows"];

const IMPORT_PREVIEW_FILTER_INDEXES: &[&str] = &[
    "idx_import_preview_rows_session_category",
    "idx_import_preview_rows_session_account",
    "idx_import_preview_rows_session_transfer_account",
    "idx_import_preview_rows_session_selected",
    "idx_import_preview_rows_session_type",
];

const IMPORT_STAGING_CLEANUP_INDEX_TABLES: &[&str] = &[
    "import_sessions",
    "import_sources",
    "import_standard_rows",
    "import_preview_rows",
    "import_decision_groups",
    "import_decision_group_members",
    "import_confirm_operations",
    "preview_matching_feedback",
    "import_learning_samples",
    "import_learning_suggestions",
    "import_learning_feedback_events",
];

const IMPORT_STAGING_CLEANUP_INDEXES: &[&str] = &[
    "idx_import_sessions_user_id",
    "idx_import_sources_user_session",
    "idx_import_standard_rows_user_session",
    "idx_import_preview_rows_user_session",
    "idx_import_preview_rows_base_standard_row_id",
    "idx_import_decision_groups_base_preview_row_id",
    "idx_import_decision_group_members_preview_row_id",
    "idx_import_decision_group_members_standard_row_id",
    "idx_import_confirm_operations_session_id",
    "idx_import_confirm_operations_preview_row_id",
    "idx_preview_matching_feedback_session_id",
    "idx_preview_matching_feedback_preview_row_id",
    "idx_preview_matching_feedback_group_id",
    "idx_import_learning_samples_preview_row_id",
    "idx_import_learning_suggestions_session_id",
    "idx_import_learning_suggestions_preview_row_id",
    "idx_import_learning_feedback_events_suggestion_id",
];

const IMPORT_DECISION_OPERATION_TABLES: &[&str] = &["import_confirm_operations"];

const IMPORT_DECISION_OPERATION_INDEXES: &[&str] = &[
    "idx_import_confirm_operations_group_decision_idempotency",
    "idx_import_confirm_operations_history_rewrite_idempotency",
];

const IMPORT_PREVIEW_TIME_PAGE_TABLES: &[&str] = &["import_preview_rows"];
const IMPORT_PREVIEW_TIME_PAGE_INDEXES: &[&str] = &["idx_import_preview_rows_session_occurred_at"];
const IMPORT_PREVIEW_SIGNAL_FLAG_TABLES: &[&str] = &["import_preview_rows"];
const IMPORT_PREVIEW_SIGNAL_FLAG_INDEXES: &[&str] = &[];

const AUTH_SESSION_AUTHORITY_TABLES: &[&str] = &["token_sessions"];
const AUTH_SESSION_AUTHORITY_INDEXES: &[&str] = &["idx_token_sessions_authority_lookup"];
const IMPORT_CONFIG_TABLES: &[&str] = &["import_configs"];
const IMPORT_CONFIG_INDEXES: &[&str] = &[
    "uq_import_configs_user_name_format",
    "uq_import_configs_user_format_default",
    "idx_import_configs_user_format_list",
];
const LLM_ACCOUNT_RULE_CANDIDATE_TABLES: &[&str] = &["llm_candidates"];
const LLM_ACCOUNT_RULE_CANDIDATE_INDEXES: &[&str] = &[];
const IMPORT_PREVIEW_SIGNAL_STATUS_TABLES: &[&str] = &["import_preview_rows"];
const IMPORT_PREVIEW_SIGNAL_STATUS_INDEXES: &[&str] = &[];
const IMPORT_SCHEMA_INVARIANT_TABLES: &[&str] = &[
    "import_sessions",
    "import_sources",
    "import_standard_rows",
    "import_preview_rows",
    "import_decision_groups",
    "import_decision_group_members",
    "import_history_materializations",
    "import_confirm_operations",
    "import_learning_lifecycle",
];
const IMPORT_SCHEMA_INVARIANT_INDEXES: &[&str] = &[
    "uq_import_decision_group_members_preview_role",
    "uq_import_decision_group_members_standard_role",
    "uq_import_decision_group_members_history_role",
];

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
    PostgresMigrationDescriptor {
        version: 15,
        file_name: "0015_import_preview_filter_indexes.sql",
        description: "add import preview filter and facet indexes for server-paged review",
        required_tables: IMPORT_PREVIEW_FILTER_INDEX_TABLES,
        required_indexes: IMPORT_PREVIEW_FILTER_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 16,
        file_name: "0016_import_staging_cleanup_fk_indexes.sql",
        description: "add import staging cleanup and foreign-key helper indexes",
        required_tables: IMPORT_STAGING_CLEANUP_INDEX_TABLES,
        required_indexes: IMPORT_STAGING_CLEANUP_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 17,
        file_name: "0017_import_staging_cleanup_indexes_retained.sql",
        description: "retain import staging cleanup helper indexes for existing version 17 deployments",
        required_tables: IMPORT_STAGING_CLEANUP_INDEX_TABLES,
        required_indexes: IMPORT_STAGING_CLEANUP_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 18,
        file_name: "0018_import_decision_group_operations.sql",
        description: "persist idempotency keys for decision-group and history-rewrite confirm operations",
        required_tables: IMPORT_DECISION_OPERATION_TABLES,
        required_indexes: IMPORT_DECISION_OPERATION_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 19,
        file_name: "0019_import_preview_time_page_index.sql",
        description: "add import preview session time-order index for bounded server pages",
        required_tables: IMPORT_PREVIEW_TIME_PAGE_TABLES,
        required_indexes: IMPORT_PREVIEW_TIME_PAGE_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 20,
        file_name: "0020_import_preview_signal_flags.sql",
        description: "centralize immutable import preview signal flag projection for bounded metadata queries",
        required_tables: IMPORT_PREVIEW_SIGNAL_FLAG_TABLES,
        required_indexes: IMPORT_PREVIEW_SIGNAL_FLAG_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 21,
        file_name: "0021_token_session_authority_lookup.sql",
        description: "index authoritative access-token session lookups",
        required_tables: AUTH_SESSION_AUTHORITY_TABLES,
        required_indexes: AUTH_SESSION_AUTHORITY_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 22,
        file_name: "0022_import_configs.sql",
        description: "persist user-scoped import mapping templates with deterministic defaults",
        required_tables: IMPORT_CONFIG_TABLES,
        required_indexes: IMPORT_CONFIG_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 23,
        file_name: "0023_llm_account_rule_candidates.sql",
        description: "add canonical account targets to LLM rule candidates",
        required_tables: LLM_ACCOUNT_RULE_CANDIDATE_TABLES,
        required_indexes: LLM_ACCOUNT_RULE_CANDIDATE_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 24,
        file_name: "0024_import_preview_signal_status_fail_closed.sql",
        description: "fail closed unknown import preview signal statuses in database projections",
        required_tables: IMPORT_PREVIEW_SIGNAL_STATUS_TABLES,
        required_indexes: IMPORT_PREVIEW_SIGNAL_STATUS_INDEXES,
    },
    PostgresMigrationDescriptor {
        version: 25,
        file_name: "0025_import_schema_invariants.sql",
        description: "enforce import lifecycle values, session ownership, and nullable member uniqueness",
        required_tables: IMPORT_SCHEMA_INVARIANT_TABLES,
        required_indexes: IMPORT_SCHEMA_INVARIANT_INDEXES,
    },
];
