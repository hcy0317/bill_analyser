//! Internal SQLite runtime and PostgreSQL migration foundations for the Bill Analyser Rust migration.
//!
//! Rust-owned HTTP domains use this crate for their SQLite repositories while
//! domain repositories now execute through the Rust SQLite runtime.

// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

pub mod app_settings;
pub mod auth;
pub mod auth_postgres;
pub mod auth_registration;
pub mod backup;
pub mod backup_postgres;
pub mod bills;
pub mod budgets;
pub mod connection;
pub mod error;
pub mod import_staging;
pub mod llm;
pub mod matching;
pub mod path;
pub mod postgres;
pub mod postgres_account_recovery;
pub mod postgres_migration;
pub mod recurring;
pub mod runtime;
pub mod schema;
pub mod statistics;
pub mod taxonomy;
pub mod transaction;
pub mod user_data;
pub mod user_scope;
pub mod vector_outbox;

pub use app_settings::{
    get_app_setting, get_app_setting_row, init_app_settings_schema, load_ocr_config_setting,
    normalize_ocr_config_for_storage, set_app_setting, store_ocr_config_setting, AppSettingDraft,
    AppSettingRow, OCR_CONFIG_SETTING_KEY,
};
pub use auth::{
    auth_account_belongs_to_user, auth_category_belongs_to_user, auth_email_exists_for_other_user,
    cleanup_expired_sessions, clear_expired_login_lock, clear_two_factor_recovery_codes,
    consume_two_factor_recovery_code, count_active_two_factor_recovery_codes,
    count_auth_events_since, count_recent_token_password_failures, create_auth_log,
    create_auth_log_under_event_limit, create_token_session, delete_application_cloud_settings,
    delete_user_external_auth, disable_two_factor_and_clear_recovery_codes,
    enable_two_factor_with_recovery_codes_and_session, get_active_logout_session_by_token_hash,
    get_active_refresh_session, get_auth_token_user, get_auth_user_profile,
    get_auth_user_two_factor_enabled, get_login_user_by_email, get_login_user_by_id,
    get_login_user_by_login_name, get_user_external_auth, hash_two_factor_recovery_code,
    increment_failed_login, invalidate_other_user_sessions, invalidate_session_by_id,
    invalidate_session_by_token_hash, list_application_cloud_settings, list_user_external_auths,
    list_user_sessions, replace_two_factor_recovery_codes, rotate_refresh_token_session,
    set_user_email_verified, update_application_cloud_settings, update_auth_user_profile,
    update_auth_user_profile_with_auth_log, update_user_last_login, update_user_password_hash,
    ApplicationCloudSettingDraft, ApplicationCloudSettingRow, AuthLogDraft, AuthLoginUserRow,
    AuthLogoutSessionRow, AuthRefreshSessionRow, AuthTokenUserRow, AuthUserProfileRow,
    AuthUserProfileUpdate, CreateTokenSessionDraft, ExternalAuthRow, LoginFailureUpdate,
    TokenSessionRow,
};
pub use auth_postgres::{
    cleanup_postgres_expired_sessions, consume_postgres_two_factor_recovery_code,
    count_postgres_auth_events_since, count_postgres_recent_token_password_failures,
    create_postgres_auth_log, create_postgres_auth_log_under_event_limit,
    create_postgres_registered_user_with_defaults, create_postgres_token_session,
    delete_postgres_application_cloud_settings, delete_postgres_user_external_auth,
    disable_postgres_two_factor_and_clear_recovery_codes,
    enable_postgres_two_factor_with_recovery_codes_and_session,
    get_postgres_active_refresh_session, get_postgres_active_session_id_by_token_hash,
    get_postgres_auth_token_user, get_postgres_auth_user_profile,
    get_postgres_auth_user_two_factor_enabled, get_postgres_login_user_by_id,
    get_postgres_login_user_by_login_name, get_postgres_operation_password,
    get_postgres_user_external_auth, increment_postgres_failed_login,
    invalidate_other_postgres_user_sessions, invalidate_postgres_session_by_id,
    invalidate_postgres_session_by_token_hash, list_postgres_application_cloud_settings,
    list_postgres_user_external_auths, list_postgres_user_sessions,
    postgres_auth_account_belongs_to_user, postgres_auth_category_belongs_to_user,
    postgres_auth_email_exists, postgres_auth_email_exists_for_other_user,
    postgres_auth_username_exists, replace_postgres_two_factor_recovery_codes,
    rotate_postgres_refresh_token_session, set_postgres_user_email_verified,
    update_postgres_application_cloud_settings, update_postgres_auth_user_profile,
    update_postgres_auth_user_profile_with_auth_log, update_postgres_user_last_login,
    update_postgres_user_password_hash,
};
pub use auth_registration::{
    auth_email_exists, auth_username_exists, create_registered_user_with_defaults,
    RegisterDefaultSeedSummary, RegisterPresetCategory, RegisterPresetSubCategory,
    RegisterUserDraft, RegisterUserResult,
};
pub use backup::{
    create_backup_audit_log_best_effort, create_or_update_backup_job, init_backup_ops_schema,
    list_backup_jobs, list_backup_records, update_backup_record_by_filename, upsert_backup_record,
    BackupAuditLogDraft, BackupJobDraft, BackupJobRow, BackupRecordDraft, BackupRecordRow,
};
pub use backup_postgres::{
    create_or_update_postgres_backup_job, create_postgres_backup_audit_log_best_effort,
    list_postgres_backup_jobs, list_postgres_backup_records,
    update_postgres_backup_record_by_filename, upsert_postgres_backup_record,
};
pub use bills::postgres_reads::{
    batch_create_postgres_bills, batch_delete_postgres_bills, batch_update_postgres_bills,
    create_postgres_bill, delete_postgres_bill, get_first_postgres_account_id,
    get_postgres_bill_by_id, get_postgres_bill_tags, get_postgres_bill_update_snapshot,
    get_postgres_reconciliation_account, list_postgres_reconciliation_categories,
    postgres_category_filters_for_ids, query_postgres_bills, resolve_postgres_category_by_id,
    update_postgres_bill, PostgresReconciliationAccount,
};
pub use bills::{
    batch_create_bills, batch_delete_bills, batch_update_bills, bind_bill_to_recurring,
    calculate_bill_hash_from_fields, calculate_bill_hash_from_record, create_bill, delete_bill,
    get_bill_by_id, get_bill_recurring_candidates, get_bill_tags, get_bill_update_snapshot,
    get_first_account_id, list_bills, query_bills, sync_all_account_balances,
    unbind_bill_from_recurring, update_bill, AccountBalanceDiscrepancy, BatchUpdateBillsResult,
    BillCategoryFilter, BillCreateDraft, BillFilters, BillPage, BillRecord,
    BillRecurringBindResult, BillRecurringCandidates, BillUpdateDraft,
    SyncAllAccountBalancesResult,
};
pub use budgets::postgres_reads::{
    create_postgres_budget, create_postgres_budget_execution_snapshots, delete_postgres_budget,
    export_postgres_budgets, get_postgres_budget_by_id, import_postgres_budgets,
    query_postgres_budget_execution_details, query_postgres_budget_execution_history,
    query_postgres_budget_forecast, query_postgres_budgets_for_listing, update_postgres_budget,
};
pub use budgets::{
    create_budget, create_budget_execution_snapshots, delete_budget, export_budgets,
    get_budget_by_id, import_budgets, query_budget_execution_details,
    query_budget_execution_history, query_budget_forecast, query_budgets_for_listing,
    update_budget, BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastFilters,
    BudgetRecord, BudgetUpdateDraft,
};
pub use connection::{PragmaSnapshot, SqliteConnectionConfig, SqliteRuntime};
pub use error::{DbError, DbResult};
pub use import_staging::{
    apply_preview_learning_decision, apply_preview_llm_recommendation,
    apply_preview_patches_preserving_selection, apply_preview_transfer_decision,
    batch_update_preview_classification, calculate_import_bill_hash,
    clear_import_preview_materialization_state, clear_session_data, clear_user_import_staging_data,
    confirm_preview_to_bills, confirm_preview_to_bills_with_ack, count_preview_by_session,
    create_import_session, create_llm_memory_event, dedup_bill_from_parser_template,
    dedup_bills_from_parser_templates, get_import_annotation_samples,
    get_import_decision_groups_by_session, get_import_history_candidate_bills_for_session,
    get_import_history_materializations_by_session, get_import_learning_lifecycle_view,
    get_import_session, get_import_sources_by_session, get_import_standard_rows_by_session,
    get_llm_memory_events, get_parser_templates_by_session, get_preview_bill_by_id,
    get_preview_by_ids, get_preview_by_session, get_preview_filter_index_by_session,
    get_preview_page_by_session, get_unprocessed_templates_for_dedup, init_import_staging_schema,
    insert_import_decision_groups_batch, insert_import_history_materializations_batch,
    insert_parser_template, insert_parser_templates_batch, insert_preview_bill,
    insert_preview_bills_batch, mark_unprocessed_parser_templates_processed_for_session,
    parser_template_draft_from_standard_bill, parser_template_drafts_from_standard_bills,
    preview_draft_from_dedup_bill, preview_draft_from_history_duplicate,
    preview_draft_from_history_transfer, preview_drafts_from_dedup_bills,
    query_preview_page_by_session, record_import_learning_lifecycle_feedback,
    replace_preview_selection_with_patches, reset_session_preview_selection,
    review_preview_llm_recommendation, save_import_annotation_samples,
    stage_import_parser_templates, stage_import_parser_templates_with_sources,
    update_import_session_status, update_parser_template_status, update_preview_bill,
    update_preview_bills_batch, update_preview_recurring_match_decision, update_preview_selection,
    update_session_preview_selection_by_query, ClearSessionDataResult, ConfirmPreviewResult,
    ImportAnnotationSampleDraft, ImportAnnotationSampleRow, ImportDecisionGroupDraft,
    ImportDecisionGroupMemberDraft, ImportDecisionGroupMemberRow, ImportDecisionGroupRow,
    ImportHistoryBillRow, ImportHistoryDuplicatePreviewInput, ImportHistoryMaterializationDraft,
    ImportHistoryMaterializationRow, ImportHistoryRewriteAcknowledgement,
    ImportHistoryRewriteAcknowledgementOperation, ImportHistoryTransferPreviewInput,
    ImportLearningLifecycleRecordInput, ImportLearningLifecycleView, ImportParseStagingResult,
    ImportParserTemplateDraft, ImportParserTemplateRow, ImportPreviewClassificationUpdate,
    ImportPreviewCounts, ImportPreviewDecision, ImportPreviewDecisionResult, ImportPreviewDraft,
    ImportPreviewExpectedState, ImportPreviewFacetEntry, ImportPreviewFacets,
    ImportPreviewFilterIndexRow, ImportPreviewLearningApply, ImportPreviewLlmApplyRequest,
    ImportPreviewLlmDecisionResult, ImportPreviewLlmReviewRequest, ImportPreviewLlmSuggestion,
    ImportPreviewMetadata, ImportPreviewPageRequest, ImportPreviewPageResult, ImportPreviewPatch,
    ImportPreviewPatchField, ImportPreviewPatchValue, ImportPreviewQueryFilters,
    ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate, ImportPreviewRow,
    ImportSessionDraft, ImportSessionRow, ImportSessionStatusUpdate, ImportSourceDraft,
    ImportSourceRow, ImportStandardRow, ImportStandardRowDraft, LlmMemoryEventDraft,
    LlmMemoryEventRow,
};
pub use llm::{
    accept_llm_candidate, activate_llm_config, count_llm_candidates, create_llm_candidate,
    create_llm_config, default_llm_runtime_config, delete_llm_config,
    effective_llm_config_from_saved, get_active_llm_config, get_llm_candidate_by_id,
    init_llm_runtime_schema, list_llm_candidates, list_llm_configs, reject_llm_candidate,
    update_llm_candidate_status, update_llm_config, LlmCandidateDraft, LlmConfigDraft,
    LlmConfigUpdate,
};
pub use matching::postgres_reads::{
    create_postgres_manual_matching_pair, delete_postgres_manual_matching_pair,
    query_postgres_matching_bill_candidates_payload, query_postgres_matching_bill_feedback_payload,
    query_postgres_matching_pairs_payload, query_postgres_matching_session_candidates_payload,
    query_postgres_reconciliation_candidates_payload,
};
pub use matching::{
    apply_matching_candidate_action, create_manual_matching_pair, delete_manual_matching_pair,
    init_matching_runtime_schema, list_reconciliation_candidates_payload,
    query_matching_bill_candidates_payload, query_matching_bill_feedback_payload,
    query_matching_pairs_payload, query_matching_session_candidates_payload, MatchingRuntimeError,
    PreviewMatchingActionRequest, ReconciliationCandidateFilters,
};
pub use path::SqliteDbPath;
pub use postgres::{
    postgres_initial_schema_path, postgres_migration_manifest, postgres_migrations_dir,
    run_postgres_migrations, PostgresMigrationDescriptor, PostgresPool,
    POSTGRES_INITIAL_SCHEMA_FILE, POSTGRES_MIGRATIONS_RELATIVE_DIR,
};
pub use postgres_account_recovery::{
    apply_postgres_account_recovery, build_postgres_account_recovery_bundle_from_connection,
    build_postgres_account_recovery_dry_run,
    build_postgres_account_recovery_dry_run_from_connection,
    inspect_postgres_account_recovery_source,
    inspect_postgres_account_recovery_source_from_connection,
    resolve_postgres_account_recovery_target, AccountRecoveryApplyOptions,
    AccountRecoveryApplyReport, AccountRecoveryDryRunReport, AccountRecoveryExpectedCounts,
    AccountRecoveryIdPlan, AccountRecoverySourceCounts, AccountRecoverySourceReport,
    AccountRecoveryTargetCounts, AccountRecoveryTargetRef, AccountRecoveryTargetTriplet,
    ACCOUNT_RECOVERY_DEFAULT_SOURCE_USER_ID, ACCOUNT_RECOVERY_ID_BLOCK_SIZE,
};
pub use postgres_migration::{
    export_sqlite_to_postgres_bundle, export_sqlite_to_postgres_bundle_from_connection,
    import_postgres_bundle_to_postgres, import_postgres_bundle_to_postgres_with_name,
    import_postgres_bundle_to_sink, load_sqlite_to_postgres_bundle_json,
    sqlite_to_postgres_dry_run, sqlite_to_postgres_dry_run_from_connection,
    write_sqlite_to_postgres_json, MigrationTableStatus, PostgresImportSink, PostgresTargetRow,
    SqliteToPostgresDryRunReport, SqliteToPostgresExportBundle, SqliteToPostgresImportCheckReport,
    SqliteToPostgresTableExport, SqliteToPostgresTableReport,
};
pub use recurring::{
    accept_postgres_recurring_suggestion, accept_recurring_suggestion,
    bind_postgres_bill_to_recurring, count_postgres_recurring_suggestions,
    count_recurring_suggestions, detect_and_save_postgres_recurring_suggestions,
    detect_and_save_recurring_suggestions, get_bills_linked_to_recurring,
    get_postgres_bill_recurring_candidates, get_postgres_bills_linked_to_recurring,
    init_recurring_runtime_schema, list_postgres_recent_bills_for_recurring_detection,
    list_postgres_recurring_suggestions, list_recent_bills_for_recurring_detection,
    list_recurring_suggestions, reject_postgres_recurring_suggestion, reject_recurring_suggestion,
    unbind_postgres_bill_from_recurring, RecurringSuggestionSaveSummary,
};
pub use runtime::{
    DatabaseRuntimeBackend, DatabaseRuntimeConfig, DatabaseRuntimeProvider,
    PostgresRepositoryRuntime, SqliteRuntimeOpenMode,
};
pub use schema::{
    init_auth_security_schema, init_foundational_schema, migrate_bills_hash_unique_constraint,
    migrate_categories_unique_constraint, migrate_core_user_scope_constraints,
    migrate_user_exchange_rates_unique_constraint, migrate_user_id_field, schema_inventory,
    SchemaDryRun, SchemaDryRunReport, SchemaResponsibility,
};
pub use statistics::{
    delete_postgres_user_custom_exchange_rate, delete_user_custom_exchange_rate,
    find_postgres_statistics_all_date_range, find_statistics_all_date_range,
    get_postgres_statistics_user_default_currency, get_statistics_user_default_currency,
    list_postgres_user_custom_exchange_rates, list_user_custom_exchange_rates,
    query_asset_trends_payload, query_calendar_events_payload, query_category_pie_payload,
    query_category_statistics_payload, query_category_trends_payload,
    query_insight_anomaly_summary_payload, query_net_worth_payload,
    query_postgres_asset_trends_payload, query_postgres_calendar_events_payload,
    query_postgres_category_pie_payload, query_postgres_category_statistics_payload,
    query_postgres_category_trends_payload, query_postgres_insight_anomaly_summary_payload,
    query_postgres_net_worth_payload, query_postgres_statistics_analyzer_category_payload,
    query_postgres_statistics_analyzer_comparison_payload,
    query_postgres_statistics_analyzer_report_payload,
    query_postgres_statistics_analyzer_trends_payload, query_postgres_top_merchants_payload,
    query_postgres_transaction_amount_period, query_statistics_analyzer_category_payload,
    query_statistics_analyzer_comparison_payload, query_statistics_analyzer_report_payload,
    query_statistics_analyzer_trends_payload, query_top_merchants_payload,
    query_transaction_amount_period, upsert_postgres_user_custom_exchange_rate,
    upsert_user_custom_exchange_rate, StatisticsAllDateRange, StatisticsBillFilters,
    UserCustomExchangeRateUpsert,
};
pub use taxonomy::account_rules::{AccountRuleRecord, AccountRulesRepository};
pub use taxonomy::category_rules::{CategoryRuleRecord, CategoryRulesRepository};
pub use taxonomy::postgres_reads::{
    create_postgres_account, create_postgres_category, delete_postgres_account,
    delete_postgres_categories_by_main_category, delete_postgres_category,
    get_postgres_account_by_id, get_postgres_category_by_id, get_postgres_category_by_name,
    get_postgres_sub_accounts, list_postgres_category_rules, update_postgres_account,
    update_postgres_account_display_orders, update_postgres_category,
    update_postgres_category_display_order, update_postgres_main_category_name,
};
pub use transaction::run_transaction;
pub use user_data::{
    clear_postgres_user_data, clear_postgres_user_transactions, clear_user_data,
    clear_user_transactions, create_postgres_user_data_audit_event,
    get_postgres_user_data_statistics, get_user_data_statistics,
    list_postgres_user_data_categories, list_user_data_categories, load_postgres_user_data_export,
    load_user_data_export, PostgresUserDataAuditEvent, UserDataClearAllResult,
    UserDataExportBundle, UserDataExportCategory,
};
pub use user_scope::UserScope;
pub use vector_outbox::{
    claim_pending_vector_outbox_events, enqueue_vector_outbox_event,
    load_import_learning_feature_vector_sources, mark_vector_outbox_event_failed,
    mark_vector_outbox_event_succeeded, ImportLearningFeatureVectorSource, VectorOutboxEvent,
    VectorOutboxEventDraft, VECTOR_OUTBOX_STATUS_COMPLETED, VECTOR_OUTBOX_STATUS_FAILED,
    VECTOR_OUTBOX_STATUS_PENDING, VECTOR_OUTBOX_STATUS_PROCESSING,
};
