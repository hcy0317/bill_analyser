//! Internal SQLite runtime foundations for the Bill Analyser Rust migration.
//!
//! Rust-owned HTTP domains use this crate for their SQLite repositories while
//! unmigrated domains continue through the Python/Flask database facade.

pub mod app_settings;
pub mod auth;
pub mod auth_registration;
pub mod bills;
pub mod budgets;
pub mod connection;
pub mod error;
pub mod import_staging;
pub mod llm;
pub mod path;
pub mod schema;
pub mod statistics;
pub mod taxonomy;
pub mod transaction;
pub mod user_data;
pub mod user_scope;

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
pub use auth_registration::{
    auth_email_exists, auth_username_exists, create_registered_user_with_defaults,
    RegisterDefaultSeedSummary, RegisterPresetCategory, RegisterPresetSubCategory,
    RegisterUserDraft, RegisterUserResult,
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
    apply_preview_transfer_decision, batch_update_preview_classification,
    calculate_import_bill_hash, clear_session_data, confirm_preview_to_bills,
    count_preview_by_session, create_import_session, create_llm_memory_event,
    dedup_bill_from_parser_template, dedup_bills_from_parser_templates,
    get_import_annotation_samples, get_import_session, get_llm_memory_events,
    get_parser_templates_by_session, get_preview_bill_by_id, get_preview_by_ids,
    get_preview_by_session, get_preview_page_by_session, get_unprocessed_templates_for_dedup,
    init_import_staging_schema, insert_parser_template, insert_parser_templates_batch,
    insert_preview_bill, insert_preview_bills_batch, parser_template_draft_from_standard_bill,
    parser_template_drafts_from_standard_bills, preview_draft_from_dedup_bill,
    preview_drafts_from_dedup_bills, replace_preview_selection_with_patches,
    reset_session_preview_selection, review_preview_llm_recommendation,
    save_import_annotation_samples, stage_import_parser_templates, update_import_session_status,
    update_parser_template_status, update_preview_bill, update_preview_bills_batch,
    update_preview_recurring_match_decision, update_preview_selection, ClearSessionDataResult,
    ConfirmPreviewResult, ImportAnnotationSampleDraft, ImportAnnotationSampleRow,
    ImportParseStagingResult, ImportParserTemplateDraft, ImportParserTemplateRow,
    ImportPreviewClassificationUpdate, ImportPreviewDecision, ImportPreviewDecisionResult,
    ImportPreviewDraft, ImportPreviewExpectedState, ImportPreviewLearningApply,
    ImportPreviewLlmApplyRequest, ImportPreviewLlmDecisionResult, ImportPreviewLlmReviewRequest,
    ImportPreviewLlmSuggestion, ImportPreviewPatch, ImportPreviewPatchField,
    ImportPreviewPatchValue, ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate,
    ImportPreviewRow, ImportSessionDraft, ImportSessionRow, ImportSessionStatusUpdate,
    LlmMemoryEventDraft, LlmMemoryEventRow,
};
pub use llm::{
    accept_llm_candidate, activate_llm_config, count_llm_candidates, create_llm_candidate,
    create_llm_config, default_llm_runtime_config, delete_llm_config,
    effective_llm_config_from_saved, get_active_llm_config, get_llm_candidate_by_id,
    init_llm_runtime_schema, list_llm_candidates, list_llm_configs, reject_llm_candidate,
    update_llm_candidate_status, update_llm_config, LlmCandidateDraft, LlmConfigDraft,
    LlmConfigUpdate,
};
pub use path::SqliteDbPath;
pub use schema::{
    init_auth_security_schema, init_foundational_schema, migrate_bills_hash_unique_constraint,
    migrate_categories_unique_constraint, migrate_core_user_scope_constraints,
    migrate_user_exchange_rates_unique_constraint, migrate_user_id_field, schema_inventory,
    SchemaDryRun, SchemaDryRunReport, SchemaResponsibility,
};
pub use statistics::{
    delete_user_custom_exchange_rate, find_statistics_all_date_range,
    get_statistics_user_default_currency, list_user_custom_exchange_rates,
    query_asset_trends_payload, query_category_pie_payload, query_category_statistics_payload,
    query_category_trends_payload, query_insight_anomaly_summary_payload, query_net_worth_payload,
    query_statistics_analyzer_category_payload, query_statistics_analyzer_comparison_payload,
    query_statistics_analyzer_report_payload, query_statistics_analyzer_trends_payload,
    query_top_merchants_payload, query_transaction_amount_period, upsert_user_custom_exchange_rate,
    StatisticsAllDateRange, StatisticsBillFilters, UserCustomExchangeRateUpsert,
};
pub use transaction::run_transaction;
pub use user_data::{
    clear_user_data, clear_user_transactions, get_user_data_statistics, list_user_data_categories,
    load_user_data_export, UserDataClearAllResult, UserDataExportBundle, UserDataExportCategory,
};
pub use user_scope::UserScope;
