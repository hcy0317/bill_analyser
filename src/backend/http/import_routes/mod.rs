//! Rust import-domain routes.
//!
//! These handlers own the parser-first import runtime, preview pagination,
//! transfer/learning decisions, LLM/OCR entrypoints, and confirm/cancel
//! staging cleanup for the `/api/bills/import...` and related LLM/OCR routes.

// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{
    attach_import_preview_matching_payload, build_composite_match_features,
    build_import_preview_filter_index_item, build_learning_rule_result_summary,
    build_llm_candidate_list_response, build_llm_candidate_reject_response,
    build_llm_classification_prompt, build_llm_config_get_response,
    build_llm_contract_error_response, build_llm_import_preview_recommendation_prompt,
    build_llm_provider_config, build_llm_rule_expression_synthesis_prompt,
    build_llm_rule_induction_prompt, build_ocr_config_success_response, build_ocr_error_response,
    build_ocr_recognition_success_response_with_context, build_unknown_ocr_provider_response,
    category_rules::match_rule_expression, coerce_preview_selected_value,
    composite_hash_from_features, copy_runtime_llm_config, import_preview_index_success,
    import_preview_page_success, import_session_cancel_missing_response,
    import_session_cancel_success_response, import_session_not_found_response,
    import_session_success, import_stage_confirm_success, import_stage_dedup_success,
    import_stage_parse_success, import_v2_data_response, import_v2_error_response,
    normalize_import_preview_page_query, normalize_provider_auth_config,
    parse_llm_json_array_response, preview_state_conflict_response, provider_auth_access_token,
    provider_auth_has_refresh_credential, provider_auth_is_expired, provider_auth_refresh_token,
    render_llm_prompt_template, safe_llm_config_payload, score_learning_rule_similarity,
    AiRouteResponse, ImportPreviewIndexData, ImportPreviewPageData, ImportSessionSummary,
    ImportStageConfirmData, ImportStageDedupData, ImportStageParseData, ImportV2RouteResponse,
    LlmProviderConfigContract, OcrConfigContract, OcrProviderTextLine, OcrProviderTextResult,
    ReceiptDraftAccount, ReceiptDraftCategory, ReceiptDraftCategoryRule, ReceiptDraftContext,
    ReceiptDraftTag, SmartDeduplicationEngine, UserId, IMPORT_PREVIEW_SORT_KEYS, LLM_SYSTEM_PROMPT,
    NETWORK_OCR_PROVIDER_NAME, OCR_DISABLED_PROVIDER_NAME,
};
use bill_analyser_db::{
    accept_llm_candidate, activate_llm_config, apply_preview_llm_recommendation,
    apply_preview_patches_preserving_selection, apply_preview_transfer_decision,
    calculate_import_bill_hash, clear_session_data, confirm_preview_to_bills, count_llm_candidates,
    create_llm_candidate, create_llm_config, dedup_bills_from_parser_templates, delete_llm_config,
    effective_llm_config_from_saved, get_app_setting, get_import_annotation_samples,
    get_import_session, get_llm_memory_events, get_preview_bill_by_id, get_preview_by_session,
    get_preview_filter_index_by_session, get_unprocessed_templates_for_dedup,
    init_app_settings_schema, init_import_staging_schema, init_llm_runtime_schema,
    insert_preview_bills_batch, list_llm_candidates, list_llm_configs, load_ocr_config_setting,
    mark_unprocessed_parser_templates_processed_for_session,
    parser_template_draft_from_standard_bill, preview_drafts_from_dedup_bills,
    query_preview_page_by_session, reject_llm_candidate, replace_preview_selection_with_patches,
    reset_session_preview_selection, review_preview_llm_recommendation,
    save_import_annotation_samples, set_app_setting, stage_import_parser_templates,
    store_ocr_config_setting, update_import_session_status, update_llm_config, update_preview_bill,
    update_preview_bills_batch, update_preview_recurring_match_decision, update_preview_selection,
    update_session_preview_selection_by_query, AppSettingDraft, ImportAnnotationSampleDraft,
    ImportPreviewDecision, ImportPreviewDecisionResult, ImportPreviewDraft,
    ImportPreviewExpectedState, ImportPreviewLlmDecisionResult, ImportPreviewLlmReviewRequest,
    ImportPreviewLlmSuggestion, ImportPreviewPageRequest, ImportPreviewPatch,
    ImportPreviewPatchField, ImportPreviewPatchValue, ImportPreviewQueryFilters,
    ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate, ImportPreviewRow,
    ImportSessionDraft, ImportSessionStatusUpdate, LlmCandidateDraft, LlmConfigDraft,
    LlmConfigUpdate, SqliteRuntime,
};
use bill_analyser_parsers::{
    parse_dedicated_import_bytes, post_process_raw_bills, RawBill, StandardBill,
};
use bytes::{Bytes, BytesMut};
use chrono::{NaiveDate, Utc};
use encoding_rs::GBK;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    env, fs, io,
    io::Write,
    path::{Component, Path as FsPath, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration as StdDuration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::time::sleep;
use url::Url;

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

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
const IMPORT_LEARNING_MODEL_KEY: &str = "import-learning-dual-head";

pub const IMPORT_SKELETON_ROUTE_PATTERNS: &[(&str, &str)] = &[
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
    ("POST", "/api/bills/import/preview"),
    ("POST", "/api/bills/import/confirm"),
    ("POST", "/api/bills/import/batch"),
    ("POST", "/api/bills/parse_import"),
    ("POST", "/api/bills/import/upload"),
    ("GET", "/api/bills/import/parsers"),
    ("POST", "/api/bills/import/reclassify"),
    ("GET", "/api/bills/import/configs"),
    ("POST", "/api/bills/import/configs"),
    ("POST", "/api/bills/import/configs/match"),
    ("POST", "/api/bills/import/configs/suggest"),
    ("DELETE", "/api/bills/import/configs/{config_id}"),
    ("GET", "/api/bills/import/learning-rules"),
    ("PUT", "/api/bills/import/learning-rules/{rule_id}"),
    ("DELETE", "/api/bills/import/learning-rules/{rule_id}"),
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
            "/api/bills/import/preview",
            post(legacy_import_preview_runtime_handler),
        )
        .route(
            "/api/bills/import/confirm",
            post(legacy_import_confirm_runtime_handler),
        )
        .route(
            "/api/bills/import/batch",
            post(legacy_import_batch_runtime_handler),
        )
        .route(
            "/api/bills/parse_import",
            post(legacy_parse_import_runtime_handler),
        )
        .route(
            "/api/bills/import/upload",
            post(legacy_import_upload_runtime_handler),
        )
        .route(
            "/api/bills/import/parsers",
            get(legacy_import_parsers_runtime_handler),
        )
        .route(
            "/api/bills/import/reclassify",
            post(legacy_import_reclassify_runtime_handler),
        )
        .route(
            "/api/bills/import/configs",
            get(import_configs_list_runtime_handler).post(import_configs_save_runtime_handler),
        )
        .route(
            "/api/bills/import/configs/match",
            post(import_configs_match_runtime_handler),
        )
        .route(
            "/api/bills/import/configs/suggest",
            post(import_configs_suggest_runtime_handler),
        )
        .route(
            "/api/bills/import/configs/:config_id",
            delete(import_configs_delete_runtime_handler),
        )
        .route(
            "/api/bills/import/learning-rules",
            get(import_learning_rules_list_runtime_handler),
        )
        .route(
            "/api/bills/import/learning-rules/:rule_id",
            put(import_learning_rule_update_runtime_handler)
                .delete(import_learning_rule_delete_runtime_handler),
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

include!("legacy_handlers.rs");
include!("stage_handlers.rs");
include!("llm_handlers.rs");
include!("ocr_learning_handlers.rs");
include!("response_payload.rs");
include!("multipart_and_ocr.rs");
include!("parser_mapping.rs");
include!("legacy_import_helpers.rs");
include!("learning_runtime.rs");
include!("legacy_storage.rs");
include!("preview_mutation_helpers.rs");
include!("runtime_helpers.rs");
