//! Rust import-domain routes.
//!
//! These handlers own the parser-first import runtime, preview pagination,
//! transfer/learning decisions, LLM/OCR entrypoints, and confirm/cancel
//! staging cleanup for the `/api/bills/import...` and related LLM/OCR routes.

// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use bill_analyser_core::{
    account_rules::{
        compile_account_rule_candidates, match_compiled_account_rules, AccountRuleCandidate,
        AccountRuleMatch, AccountRuleMatchContext, CompiledAccountRuleCandidate,
        ACCOUNT_ROLE_DESTINATION, ACCOUNT_ROLE_INVESTMENT, ACCOUNT_ROLE_SOURCE,
        TRANSACTION_SCOPE_EXPENSE, TRANSACTION_SCOPE_INCOME, TRANSACTION_SCOPE_INVESTMENT,
        TRANSACTION_SCOPE_TRANSFER,
    },
    amount_cents_bucket, attach_import_preview_matching_payload,
    attach_import_preview_state_snapshot_to_canonical_row, build_composite_match_features,
    build_import_learning_recommendation_key, build_import_learning_vector_recall_queries,
    build_import_preview_filter_index_item, build_learning_rule_result_summary,
    build_llm_account_rule_induction_prompt, build_llm_category_rule_induction_prompt,
    build_llm_classification_prompt, build_llm_import_preview_recommendation_prompt,
    build_llm_provider_config, build_llm_rule_expression_synthesis_prompt,
    build_receipt_transaction_draft, build_runtime_llm_config_from_saved_config,
    category_rules::{
        select_category_rule_candidate, CategoryRuleCandidate, CategoryRuleCandidateDraft,
    },
    coerce_preview_selected_value, composite_hash_from_features, copy_runtime_llm_config,
    find_import_reconciliation_candidates, llm_available_providers, normalize_history_operation,
    normalize_import_preview_page_query, normalize_provider_auth_config,
    normalize_weaviate_transaction_type_scope, ocr_available_providers_with_disabled,
    parse_llm_json_array_response, parse_payment_screenshot_text, provider_auth_access_token,
    provider_auth_has_refresh_credential, provider_auth_is_expired, provider_auth_refresh_token,
    redact_provider_auth_config, redact_secrets_in_value, render_llm_prompt_template,
    safe_llm_config_payload, score_learning_rule_similarity, validate_llm_vision_base_url,
    AccountLookup, CategoryLookup, DedupBill, DuplicateGroup, ImportLearningRecommendationKeyInput,
    ImportPreviewIndexData, ImportPreviewPageData, ImportSessionSummary, ImportStageConfirmData,
    ImportStageDedupData, ImportStageParseData, LlmProviderConfigContract, OcrConfigContract,
    OcrProviderTextResult, ReceiptDraftAccount, ReceiptDraftCategory, ReceiptDraftCategoryRule,
    ReceiptDraftContext, ReceiptDraftTag, ReconciliationCandidateType, SmartDeduplicationEngine,
    TransferPair, UserId, IMPORT_PREVIEW_SORT_KEYS, LLM_SYSTEM_PROMPT, NETWORK_OCR_PROVIDER_NAME,
    OCR_DISABLED_PROVIDER_NAME, WEAVIATE_RECALL_DEFAULT_LIMIT,
    WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE,
};
use bill_analyser_db::{
    accept_postgres_llm_candidate, activate_postgres_llm_config,
    apply_import_decision_group_command, apply_preview_llm_recommendation,
    apply_preview_patches_and_load_selected_if_current,
    apply_preview_patches_and_update_selection_by_query,
    apply_preview_patches_preserving_selection, claim_import_group_reclassification,
    clear_import_preview_materialization_state, clear_session_data,
    confirm_versioned_import_command_with_receipt_read_source, count_postgres_llm_candidates,
    create_postgres_llm_candidate, create_postgres_llm_config, dedup_bills_from_parser_templates,
    delete_postgres_llm_config, effective_postgres_llm_config_from_saved,
    finish_import_group_reclassification, get_import_decision_groups_by_session,
    get_import_group_reclassification_state, get_import_history_candidate_bills_for_session,
    get_import_preview_categories_by_ids, get_import_preview_category_by_id, get_import_session,
    get_import_standard_rows_by_session, get_llm_memory_events, get_postgres_llm_candidate_by_id,
    get_postgres_llm_config, get_preview_bill_by_id, get_preview_by_ids, get_preview_by_session,
    get_preview_filter_index_by_session, get_unprocessed_templates_for_dedup,
    has_import_learning_feature_vector_sources, has_postgres_llm_account_rule_candidate_duplicate,
    has_postgres_llm_rule_candidate_duplicate, init_import_staging_schema,
    insert_import_decision_groups_batch, insert_import_history_materializations_batch,
    insert_preview_bills_batch, list_postgres_llm_candidates, list_postgres_llm_configs,
    load_import_account_catalog_records, load_import_category_catalog_records,
    load_import_stage2_account_rule_candidates, load_import_stage2_category_rule_records,
    load_import_stage2_context, load_import_stage2_learning_lifecycle_views,
    load_import_stage2_recurring_template_records, load_postgres_ocr_config_setting,
    mark_unprocessed_parser_templates_processed_for_session,
    parser_template_draft_from_standard_bill, patch_preview_selection,
    preview_draft_from_history_duplicate, preview_draft_from_history_transfer,
    preview_drafts_from_dedup_bills, preview_id_snapshot_hash, query_preview_page_by_session,
    reject_postgres_llm_candidate, review_preview_llm_recommendation,
    save_import_annotation_samples, set_import_decision_materialization_failed,
    set_import_decision_materialization_status, stage_import_parser_templates_with_sources,
    store_postgres_ocr_config_setting, update_import_session_status, update_postgres_llm_config,
    update_preview_bill, update_preview_bills_batch, update_preview_recurring_match_decision,
    ConfirmCommand, ConfirmTimeEffect, DbError, ImportAnnotationSampleDraft,
    ImportCategoryCatalogRecord, ImportDecisionGroupCommand, ImportDecisionGroupCommandResult,
    ImportDecisionGroupDraft, ImportDecisionGroupMemberDraft, ImportDecisionPreviewVersion,
    ImportHistoryBillRow, ImportHistoryDuplicatePreviewInput, ImportHistoryMaterializationDraft,
    ImportHistoryRewriteAcknowledgement, ImportHistoryTransferPreviewInput,
    ImportLearningLifecycleView, ImportPreviewCategoryLookup,
    ImportPreviewConditionalSelectionCommand, ImportPreviewDecision, ImportPreviewDecisionResult,
    ImportPreviewDraft, ImportPreviewExpectedState, ImportPreviewLlmDecisionResult,
    ImportPreviewLlmReviewRequest, ImportPreviewLlmSuggestion, ImportPreviewPageRequest,
    ImportPreviewPatch, ImportPreviewPatchField, ImportPreviewPatchValue,
    ImportPreviewQueryFilters, ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate,
    ImportPreviewRow, ImportPreviewSelectionMode, ImportPreviewSelectionTarget, ImportSessionDraft,
    ImportSessionRow, ImportSessionStatusUpdate, ImportSourceDraft, ImportStage2CategoryRuleRecord,
    ImportStage2ContextRows, ImportStage2RecurringTemplateRecord, ImportStandardRowDraft,
    LlmCandidateDraft, LlmConfigDraft, LlmConfigUpdate, PostgresPool, PostgresRepositoryRuntime,
};
use bill_analyser_parsers::{
    parse_dedicated_import_bytes_with_decision, parser_source_label, post_process_raw_bills,
    DedicatedParserDecision, RawBill, StandardBill, MAX_SPREADSHEET_TOTAL_CELL_BYTES,
};
use bytes::{Bytes, BytesMut};
use chrono::{NaiveDate, Utc};
use encoding_rs::{GB18030, GBK};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, QueryBuilder, Row};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    env, fs,
    io::Read,
    path::{Component, Path as FsPath, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration as StdDuration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::import_contract_telemetry::observe_import_version_contract;
use tokio::time::sleep;
use url::Url;

type Connection = PostgresPool;

use crate::{
    auth::resolve_user_id_from_headers,
    config::HttpShellConfig,
    import_config_handlers::{
        delete_import_config_handler, list_import_configs_handler, match_import_config_handler,
        save_import_config_handler, suggest_import_config_handler,
    },
    state::HttpAppState,
    weaviate::{
        recall_import_learning_candidates, WeaviateImportLearningRecallHit,
        WeaviateImportLearningRecallRequest,
    },
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
static IMPORT_SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
static OCR_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
static OCR_RATE_LIMIT_BUCKETS: OnceLock<Mutex<HashMap<i64, VecDeque<Instant>>>> = OnceLock::new();
static LLM_RATE_LIMIT_BUCKETS: OnceLock<Mutex<HashMap<i64, VecDeque<Instant>>>> = OnceLock::new();
const LLM_PROVIDER_RESPONSE_MAX_BYTES: usize = 1_048_576;
const LLM_CANDIDATE_RAW_RESPONSE_MAX_BYTES: usize = 16_384;
const LLM_RULE_INDUCTION_MAX_CANDIDATES_PER_GROUP: usize = 5;
const LLM_RULE_SYNTHESIS_MAX_GROUPS: usize = 8;
const LLM_RULE_SYNTHESIS_MAX_EVIDENCE_PER_GROUP: usize = 4;
pub const IMPORT_SKELETON_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("POST", "/api/bills/import/preview"),
    ("POST", "/api/bills/import/v2/parse"),
    ("POST", "/api/bills/import/v2/parse_generic"),
    ("POST", "/api/bills/import/v2/dedup"),
    ("POST", "/api/bills/import/v2/confirm"),
    ("GET", "/api/bills/import/v2/session/{session_id}"),
    ("DELETE", "/api/bills/import/v2/session/{session_id}"),
    ("GET", "/api/bills/import/v2/preview/{session_id}"),
    ("GET", "/api/bills/import/v2/preview/{session_id}/index"),
    ("PUT", "/api/bills/import/v2/preview/{session_id}/selection"),
    ("PUT", "/api/bills/import/v2/preview/{session_id}/update"),
    ("POST", "/api/bills/import/v2/reclassify/{session_id}"),
    (
        "GET",
        "/api/bills/import/v2/preview-item/{preview_id}/recurring-candidates",
    ),
    (
        "PUT",
        "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
    ),
    (
        "DELETE",
        "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
    ),
    (
        "POST",
        "/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
    ),
    (
        "POST",
        "/api/bills/import/v2/learning/{session_id}/suggestions",
    ),
    (
        "GET",
        "/api/bills/import/v2/learning/{session_id}/suggestions",
    ),
    ("POST", "/api/bills/import/v2/learning/{session_id}/promote"),
    ("GET", "/api/bills/import/configs"),
    ("POST", "/api/bills/import/configs"),
    ("POST", "/api/bills/import/configs/match"),
    ("POST", "/api/bills/import/configs/suggest"),
    ("DELETE", "/api/bills/import/configs/{config_id}"),
    ("POST", "/api/llm/preview-recommend/accept"),
    ("POST", "/api/llm/preview-recommend/reject"),
    ("GET", "/api/llm/memory"),
    ("GET", "/api/llm/config"),
    ("POST", "/api/llm/config"),
    ("GET", "/api/llm/configs"),
    ("POST", "/api/llm/configs"),
    ("PUT", "/api/llm/configs/{config_id}"),
    ("DELETE", "/api/llm/configs/{config_id}"),
    ("POST", "/api/llm/configs/{config_id}/activate"),
    ("POST", "/api/llm/configs/{config_id}/test"),
    ("GET", "/api/llm/candidates"),
    ("GET", "/api/llm/candidates/{candidate_id}"),
    ("POST", "/api/llm/candidates/{candidate_id}/accept"),
    ("POST", "/api/llm/candidates/{candidate_id}/reject"),
    ("GET", "/api/ml/receipt-recognition/config"),
    ("PUT", "/api/ml/receipt-recognition/config"),
];

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route(
            "/api/bills/import/preview",
            post(file_preview_handler::import_file_preview_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/parse",
            post(import_parse_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/parse_generic",
            post(import_parse_generic_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/dedup",
            post(import_dedup_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/confirm",
            post(import_confirm_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/session/:session_id",
            get(import_session_runtime_handler).delete(import_session_cancel_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id",
            get(import_preview_page_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id/index",
            get(import_preview_index_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id/selection",
            put(import_preview_selection_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id/update",
            put(import_preview_update_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/reclassify/:session_id",
            post(import_reclassify_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview-item/:preview_id/recurring-candidates",
            get(preview_recurring_candidates_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview-item/:preview_id/recurring-match",
            put(preview_recurring_match_put_runtime_handler)
                .delete(preview_recurring_match_delete_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/preview-item/:preview_id/transfer-decision",
            post(preview_transfer_decision_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/learning/:session_id/suggestions",
            get(import_learning_suggestions_get_runtime_handler)
                .post(import_learning_suggestions_post_runtime_handler),
        )
        .route(
            "/api/bills/import/v2/learning/:session_id/promote",
            post(import_learning_promote_runtime_handler),
        )
        .route(
            "/api/bills/import/configs",
            get(list_import_configs_handler).post(save_import_config_handler),
        )
        .route(
            "/api/bills/import/configs/match",
            post(match_import_config_handler),
        )
        .route(
            "/api/bills/import/configs/suggest",
            post(suggest_import_config_handler),
        )
        .route(
            "/api/bills/import/configs/:config_id",
            delete(delete_import_config_handler),
        )
        .route(
            "/api/llm/preview-recommend",
            post(llm_preview_recommend_runtime_handler),
        )
        .route(
            "/api/llm/preview-recommend/accept",
            post(llm_preview_recommend_accept_runtime_handler),
        )
        .route(
            "/api/llm/preview-recommend/reject",
            post(llm_preview_recommend_reject_runtime_handler),
        )
        .route("/api/llm/memory", get(llm_memory_runtime_handler))
        .route(
            "/api/llm/config",
            get(llm_config_get_runtime_handler).post(llm_config_post_runtime_handler),
        )
        .route(
            "/api/llm/configs",
            get(llm_configs_list_runtime_handler).post(llm_configs_create_runtime_handler),
        )
        .route(
            "/api/llm/configs/:config_id",
            put(llm_config_update_runtime_handler).delete(llm_config_delete_runtime_handler),
        )
        .route(
            "/api/llm/configs/:config_id/activate",
            post(llm_config_activate_runtime_handler),
        )
        .route(
            "/api/llm/configs/:config_id/test",
            post(llm_config_test_runtime_handler),
        )
        .route(
            "/api/llm/candidates",
            get(llm_candidates_list_runtime_handler),
        )
        .route(
            "/api/llm/candidates/:candidate_id",
            get(llm_candidate_get_runtime_handler),
        )
        .route(
            "/api/llm/candidates/:candidate_id/accept",
            post(llm_candidate_accept_runtime_handler),
        )
        .route(
            "/api/llm/candidates/:candidate_id/reject",
            post(llm_candidate_reject_runtime_handler),
        )
        .route(
            "/api/llm/analyze-transactions",
            post(llm_analyze_transactions_runtime_handler),
        )
        .route(
            "/api/llm/rule-synthesis",
            post(llm_rule_synthesis_runtime_handler),
        )
        .route(
            "/api/ml/receipt-recognition/config",
            get(ocr_config_get_runtime_handler).put(ocr_config_put_runtime_handler),
        )
        .route(
            "/api/ml/receipt-recognition",
            post(ocr_recognition_runtime_handler),
        )
        .route(
            "/api/learning/suggestions",
            get(learning_suggestions_list_runtime_handler),
        )
        .route(
            "/api/learning/suggestions/generate",
            post(learning_suggestions_generate_runtime_handler),
        )
        .route(
            "/api/learning/suggestions/:suggestion_id/accept",
            post(learning_suggestion_accept_runtime_handler),
        )
        .route(
            "/api/learning/suggestions/batch-accept",
            post(learning_suggestions_batch_accept_runtime_handler),
        )
        .route(
            "/api/learning/suggestions/:suggestion_id/reject",
            post(learning_suggestion_reject_runtime_handler),
        )
        .route(
            "/api/learning/rules",
            get(learning_rules_list_runtime_handler),
        )
        .route(
            "/api/learning/rules/:rule_id/toggle",
            put(learning_rule_toggle_runtime_handler),
        )
        .route(
            "/api/learning/rules/:rule_id",
            put(learning_rule_update_runtime_handler).delete(learning_rule_delete_runtime_handler),
        )
}

mod ocr_security;

include!("response_contract.rs");
include!("ai_response_contract.rs");
include!("stage_vector_recall.rs");
include!("stage_json_helpers.rs");
include!("stage_handlers.rs");
include!("stage_account_rule_matchers.rs");
include!("duplicate_materialization.rs");
include!("transfer_materialization.rs");
include!("parse_handlers.rs");
include!("llm_handlers.rs");
include!("ocr_learning_handlers.rs");
include!("response_payload_parse_budget.rs");
include!("response_payload.rs");
include!("response_payload_tests.rs");
include!("ai_response_contract_tests.rs");
include!("response_payload_ledger.rs");
include!("multipart_and_ocr.rs");
include!("parser_mapping.rs");
include!("learning_runtime.rs");
include!("import_file_helpers.rs");
include!("preview_mutation_helpers.rs");
include!("runtime_helpers.rs");
mod file_preview_handler;
use file_preview_handler::{
    decode_preview_text, normalize_preview_encoding, read_bounded_preview_file,
    IMPORT_FILE_PREVIEW_HARD_MAX_BYTES,
};
