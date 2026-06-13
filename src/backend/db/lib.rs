//! PostgreSQL-only repository foundations for the Bill Analyser Rust runtime.
//!
//! HTTP business routes use PostgreSQL as the sole authority and Weaviate for
//! vector recall.

// 中文导读：PostgreSQL repository 层，负责 schema 管理、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。

pub mod app_settings;
pub mod auth;
pub mod auth_postgres;
pub mod auth_registration;
pub mod auth_registration_defaults;
pub mod backup;
pub mod backup_postgres;
pub mod bills;
pub mod budgets;
pub mod error;
pub mod import_staging;
pub mod llm;
pub mod matching;
pub mod postgres;
pub mod recurring;
pub mod runtime;
pub mod statistics;
pub mod taxonomy;
pub mod user_data;
pub mod user_scope;
pub mod vector_outbox;

pub use app_settings::{
    get_postgres_app_setting, get_postgres_app_setting_row, load_postgres_ocr_config_setting,
    normalize_ocr_config_for_storage, set_postgres_app_setting, store_postgres_ocr_config_setting,
    AppSettingDraft, AppSettingRow, OCR_CONFIG_SETTING_KEY,
};
pub use auth::{
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
    RegisterDefaultSeedPackage, RegisterDefaultSeedSummary, RegisterPresetCategory,
    RegisterPresetSubCategory, RegisterUserDraft, RegisterUserResult,
};
pub use backup::{
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
    get_postgres_bill_by_id, get_postgres_bill_tags, get_postgres_reconciliation_account,
    list_postgres_reconciliation_categories, postgres_category_filters_for_ids,
    query_postgres_bills, resolve_postgres_category_by_id, update_postgres_bill,
    PostgresReconciliationAccount,
};
pub use bills::{
    calculate_bill_hash_from_fields, calculate_bill_hash_from_record, AccountBalanceDiscrepancy,
    BatchUpdateBillsResult, BillCategoryFilter, BillCreateDraft, BillFilters, BillPage, BillRecord,
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
    BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastFilters, BudgetRecord,
    BudgetUpdateDraft,
};
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
    get_import_preview_category_by_id, get_import_session, get_import_sources_by_session,
    get_import_standard_rows_by_session, get_llm_memory_events, get_parser_templates_by_session,
    get_preview_bill_by_id, get_preview_by_ids, get_preview_by_session,
    get_preview_filter_index_by_session, get_preview_page_by_session,
    get_unprocessed_templates_for_dedup, init_import_staging_schema,
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
    ImportParserTemplateDraft, ImportParserTemplateRow, ImportPreviewCategoryLookup,
    ImportPreviewClassificationUpdate, ImportPreviewCounts, ImportPreviewDecision,
    ImportPreviewDecisionResult, ImportPreviewDraft, ImportPreviewExpectedState,
    ImportPreviewFacetEntry, ImportPreviewFacets, ImportPreviewFilterIndexRow,
    ImportPreviewLearningApply, ImportPreviewLlmApplyRequest, ImportPreviewLlmDecisionResult,
    ImportPreviewLlmReviewRequest, ImportPreviewLlmSuggestion, ImportPreviewMetadata,
    ImportPreviewPageRequest, ImportPreviewPageResult, ImportPreviewPatch, ImportPreviewPatchField,
    ImportPreviewPatchValue, ImportPreviewQueryFilters, ImportPreviewRecurringCandidate,
    ImportPreviewRecurringMatchUpdate, ImportPreviewRow, ImportPreviewSelectionMode,
    ImportPreviewSelectionTarget, ImportSessionDraft, ImportSessionRow, ImportSessionStatusUpdate,
    ImportSourceDraft, ImportSourceRow, ImportStandardRow, ImportStandardRowDraft,
    LlmMemoryEventDraft, LlmMemoryEventRow,
};
pub use llm::{
    accept_postgres_llm_candidate, activate_postgres_llm_config, count_postgres_llm_candidates,
    create_postgres_llm_candidate, create_postgres_llm_config, default_llm_runtime_config,
    delete_postgres_llm_config, effective_postgres_llm_config_from_saved,
    get_postgres_active_llm_config, get_postgres_llm_candidate_by_id, list_postgres_llm_candidates,
    list_postgres_llm_configs, reject_postgres_llm_candidate, update_postgres_llm_candidate_status,
    update_postgres_llm_config, LlmCandidateDraft, LlmConfigDraft, LlmConfigUpdate,
};
pub use matching::postgres_reads::{
    create_postgres_manual_matching_pair, delete_postgres_manual_matching_pair,
    query_postgres_matching_bill_candidates_payload, query_postgres_matching_bill_feedback_payload,
    query_postgres_matching_pairs_payload, query_postgres_matching_session_candidates_payload,
    query_postgres_reconciliation_candidates_payload,
};
pub use matching::{
    MatchingRuntimeError, PreviewMatchingActionRequest, ReconciliationCandidateFilters,
};
pub use postgres::{
    postgres_initial_schema_path, postgres_migration_manifest, postgres_migrations_dir,
    run_postgres_migrations, PostgresMigrationDescriptor, PostgresPool,
    POSTGRES_INITIAL_SCHEMA_FILE, POSTGRES_MIGRATIONS_RELATIVE_DIR,
};
pub use recurring::{
    accept_postgres_recurring_suggestion, bind_postgres_bill_to_recurring,
    count_postgres_recurring_suggestions, detect_and_save_postgres_recurring_suggestions,
    get_postgres_bill_recurring_candidates, get_postgres_bills_linked_to_recurring,
    list_postgres_recent_bills_for_recurring_detection, list_postgres_recurring_suggestions,
    reject_postgres_recurring_suggestion, unbind_postgres_bill_from_recurring,
    RecurringSuggestionSaveSummary,
};
pub use runtime::{DatabaseRuntimeConfig, DatabaseRuntimeProvider, PostgresRepositoryRuntime};
pub use statistics::{
    delete_postgres_user_custom_exchange_rate, find_postgres_statistics_all_date_range,
    get_postgres_statistics_user_default_currency, list_postgres_user_custom_exchange_rates,
    query_postgres_asset_trends_payload, query_postgres_calendar_events_payload,
    query_postgres_category_pie_payload, query_postgres_category_statistics_payload,
    query_postgres_category_trends_payload, query_postgres_insight_anomaly_summary_payload,
    query_postgres_net_worth_payload, query_postgres_statistics_analyzer_category_payload,
    query_postgres_statistics_analyzer_comparison_payload,
    query_postgres_statistics_analyzer_report_payload,
    query_postgres_statistics_analyzer_trends_payload, query_postgres_top_merchants_payload,
    query_postgres_transaction_amount_period, upsert_postgres_user_custom_exchange_rate,
    StatisticsAllDateRange, StatisticsBillFilters, UserCustomExchangeRateUpsert,
};
pub use taxonomy::account_rules::AccountRuleRecord;
pub use taxonomy::category_rules::CategoryRuleRecord;
pub use taxonomy::postgres_reads::{
    create_postgres_account, create_postgres_category, create_postgres_category_rule,
    delete_postgres_account, delete_postgres_categories_by_main_category, delete_postgres_category,
    get_postgres_account_by_id, get_postgres_category_by_id, get_postgres_category_by_name,
    get_postgres_sub_accounts, list_postgres_category_rules, update_postgres_account,
    update_postgres_account_display_orders, update_postgres_category,
    update_postgres_category_display_order, update_postgres_main_category_name,
};
pub use user_data::{
    clear_postgres_user_data, clear_postgres_user_transactions,
    create_postgres_user_data_audit_event, get_postgres_user_data_statistics,
    list_postgres_user_data_categories, load_postgres_user_data_export, PostgresUserDataAuditEvent,
    UserDataClearAllResult, UserDataExportBundle, UserDataExportCategory,
};
pub use user_scope::UserScope;
pub use vector_outbox::{
    claim_pending_vector_outbox_events, enqueue_vector_outbox_event,
    load_import_learning_feature_vector_sources, mark_vector_outbox_event_failed,
    mark_vector_outbox_event_succeeded, ImportLearningFeatureVectorSource, VectorOutboxEvent,
    VectorOutboxEventDraft, VECTOR_OUTBOX_STATUS_COMPLETED, VECTOR_OUTBOX_STATUS_FAILED,
    VECTOR_OUTBOX_STATUS_PENDING, VECTOR_OUTBOX_STATUS_PROCESSING,
};
