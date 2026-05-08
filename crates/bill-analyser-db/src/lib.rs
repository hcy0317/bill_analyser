//! Internal SQLite runtime foundations for the Bill Analyser Rust migration.
//!
//! This crate is scaffolding only. Python/Flask keeps the business runtime
//! shell, and no business database write path is Rust-primary here.

pub mod app_settings;
pub mod auth;
pub mod bills;
pub mod budgets;
pub mod connection;
pub mod error;
pub mod import_staging;
pub mod path;
pub mod schema;
pub mod statistics;
pub mod taxonomy;
pub mod transaction;
pub mod user_scope;

pub use app_settings::{
    get_app_setting, get_app_setting_row, init_app_settings_schema, load_ocr_config_setting,
    normalize_ocr_config_for_storage, set_app_setting, store_ocr_config_setting, AppSettingDraft,
    AppSettingRow, OCR_CONFIG_SETTING_KEY,
};
pub use auth::{
    cleanup_expired_sessions, count_recent_token_password_failures, create_auth_log,
    create_token_session, get_auth_token_user, invalidate_other_user_sessions,
    invalidate_session_by_id, list_user_sessions, AuthLogDraft, AuthTokenUserRow,
    CreateTokenSessionDraft, TokenSessionRow,
};
pub use bills::{
    batch_create_bills, batch_delete_bills, batch_update_bills, calculate_bill_hash_from_fields,
    calculate_bill_hash_from_record, create_bill, delete_bill, get_bill_by_id, get_bill_tags,
    get_bill_update_snapshot, get_first_account_id, query_bills, update_bill,
    BatchUpdateBillsResult, BillCategoryFilter, BillCreateDraft, BillFilters, BillPage, BillRecord,
    BillUpdateDraft,
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
pub use path::SqliteDbPath;
pub use schema::{
    init_foundational_schema, migrate_bills_hash_unique_constraint,
    migrate_categories_unique_constraint, migrate_core_user_scope_constraints,
    migrate_user_exchange_rates_unique_constraint, migrate_user_id_field, schema_inventory,
    SchemaDryRun, SchemaDryRunReport, SchemaResponsibility,
};
pub use statistics::{
    find_statistics_all_date_range, query_asset_trends_payload, query_category_pie_payload,
    query_category_statistics_payload, query_category_trends_payload, query_net_worth_payload,
    query_top_merchants_payload, query_transaction_amount_period, StatisticsAllDateRange,
    StatisticsBillFilters,
};
pub use transaction::run_transaction;
pub use user_scope::UserScope;
