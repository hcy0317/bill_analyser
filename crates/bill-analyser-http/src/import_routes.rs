//! Opt-in import-domain route skeletons.
//!
//! These handlers intentionally do not execute import business logic or write
//! the database. They let the Rust HTTP shell prove route precedence and
//! endpoint coverage before later slices replace Python-owned implementations.

use axum::{
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use bill_analyser_core::{
    build_composite_match_features, build_import_preview_filter_index_item,
    build_llm_candidate_list_response, build_llm_candidate_reject_response,
    build_llm_config_get_response, build_ocr_config_success_response,
    build_unknown_ocr_provider_response, can_delete_python_import_paths,
    coerce_preview_selected_value, composite_hash_from_features, copy_runtime_llm_config,
    import_preview_index_success, import_preview_page_success,
    import_session_cancel_missing_response, import_session_cancel_success_response,
    import_session_not_found_response, import_session_success, import_stage_confirm_success,
    import_stage_dedup_success, import_stage_parse_success, import_v2_data_response,
    import_v2_error_response, preview_state_conflict_response, safe_llm_config_payload,
    AiRouteResponse, ImportDeletionEvidence, ImportPreviewIndexData, ImportPreviewPageData,
    ImportSessionSummary, ImportStageConfirmData, ImportStageDedupData, ImportStageParseData,
    ImportV2RouteResponse, SmartDeduplicationEngine, UserId,
};
use bill_analyser_db::{
    accept_llm_candidate, activate_llm_config, apply_preview_transfer_decision,
    calculate_import_bill_hash, clear_session_data, confirm_preview_to_bills, count_llm_candidates,
    create_llm_config, dedup_bills_from_parser_templates, delete_llm_config,
    effective_llm_config_from_saved, get_app_setting, get_import_annotation_samples,
    get_import_session, get_llm_memory_events, get_preview_bill_by_id, get_preview_by_session,
    get_preview_page_by_session, get_unprocessed_templates_for_dedup, init_app_settings_schema,
    init_import_staging_schema, init_llm_runtime_schema, insert_preview_bills_batch,
    list_llm_candidates, list_llm_configs, load_ocr_config_setting,
    parser_template_drafts_from_standard_bills, preview_drafts_from_dedup_bills,
    reject_llm_candidate, replace_preview_selection_with_patches, reset_session_preview_selection,
    review_preview_llm_recommendation, save_import_annotation_samples, set_app_setting,
    stage_import_parser_templates, store_ocr_config_setting, update_import_session_status,
    update_llm_config, update_parser_template_status, update_preview_bill,
    update_preview_bills_batch, update_preview_recurring_match_decision, update_preview_selection,
    AppSettingDraft, ImportAnnotationSampleDraft, ImportPreviewDecision,
    ImportPreviewDecisionResult, ImportPreviewExpectedState, ImportPreviewLlmDecisionResult,
    ImportPreviewLlmReviewRequest, ImportPreviewLlmSuggestion, ImportPreviewPatch,
    ImportPreviewPatchField, ImportPreviewPatchValue, ImportPreviewRecurringCandidate,
    ImportPreviewRecurringMatchUpdate, ImportPreviewRow, ImportSessionDraft,
    ImportSessionStatusUpdate, LlmConfigDraft, LlmConfigUpdate, SqliteConnectionConfig,
    SqliteDbPath, SqliteRuntime,
};
use bill_analyser_parsers::{
    parse_dedicated_import_bytes, post_process_raw_bills, RawBill, StandardBill,
};
use bytes::Bytes;
use chrono::Utc;
use encoding_rs::GBK;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Component, Path as FsPath, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, proxy::ProxyState};

const NOT_YET_OWNED_ERROR: &str =
    "Rust import route skeleton is not business-owned; Python proxy remains authoritative";
const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
static IMPORT_SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub const IMPORT_SKELETON_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("POST", "/api/bills/import/v2/parse"),
    ("POST", "/api/bills/import/v2/parse_generic"),
    ("POST", "/api/bills/import/v2/dedup"),
    ("POST", "/api/bills/import/v2/confirm"),
    ("GET", "/api/bills/import/v2/session/{session_id}"),
    ("DELETE", "/api/bills/import/v2/session/{session_id}"),
    ("GET", "/api/bills/import/v2/preview/{session_id}"),
    ("GET", "/api/bills/import/v2/preview/{session_id}/index"),
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

pub fn import_skeleton_router() -> Router<ProxyState> {
    Router::new()
        .route("/api/bills/import/v2/parse", post(not_yet_owned_handler))
        .route(
            "/api/bills/import/v2/parse_generic",
            post(not_yet_owned_handler),
        )
        .route("/api/bills/import/v2/dedup", post(not_yet_owned_handler))
        .route("/api/bills/import/v2/confirm", post(not_yet_owned_handler))
        .route(
            "/api/bills/import/v2/session/:session_id",
            get(session_not_found_handler).delete(session_cancel_missing_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id",
            get(session_not_found_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id/index",
            get(session_not_found_handler),
        )
        .route(
            "/api/bills/import/v2/preview/:session_id/update",
            put(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/v2/reclassify/:session_id",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/v2/preview-item/:preview_id/recurring-candidates",
            get(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/v2/preview-item/:preview_id/recurring-match",
            put(not_yet_owned_handler).delete(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/v2/preview-item/:preview_id/transfer-decision",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/v2/learning/:session_id/suggestions",
            get(not_yet_owned_handler).post(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/v2/learning/:session_id/promote",
            post(not_yet_owned_handler),
        )
        .route("/api/bills/import/preview", post(not_yet_owned_handler))
        .route("/api/bills/import/confirm", post(not_yet_owned_handler))
        .route("/api/bills/import/batch", post(not_yet_owned_handler))
        .route("/api/bills/parse_import", post(not_yet_owned_handler))
        .route("/api/bills/import/upload", post(not_yet_owned_handler))
        .route("/api/bills/import/parsers", get(not_yet_owned_handler))
        .route("/api/bills/import/reclassify", post(not_yet_owned_handler))
        .route(
            "/api/bills/import/configs",
            get(not_yet_owned_handler).post(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/configs/match",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/configs/suggest",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/configs/:config_id",
            delete(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/learning-rules",
            get(not_yet_owned_handler),
        )
        .route(
            "/api/bills/import/learning-rules/:rule_id",
            put(not_yet_owned_handler).delete(not_yet_owned_handler),
        )
        .route(
            "/api/llm/preview-recommend/accept",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/llm/preview-recommend/reject",
            post(not_yet_owned_handler),
        )
        .route("/api/llm/memory", get(not_yet_owned_handler))
        .route(
            "/api/llm/config",
            get(not_yet_owned_handler).post(not_yet_owned_handler),
        )
        .route(
            "/api/llm/configs",
            get(not_yet_owned_handler).post(not_yet_owned_handler),
        )
        .route(
            "/api/llm/configs/:config_id",
            put(not_yet_owned_handler).delete(not_yet_owned_handler),
        )
        .route(
            "/api/llm/configs/:config_id/activate",
            post(not_yet_owned_handler),
        )
        .route("/api/llm/candidates", get(not_yet_owned_handler))
        .route(
            "/api/llm/candidates/:candidate_id",
            get(not_yet_owned_handler),
        )
        .route(
            "/api/llm/candidates/:candidate_id/accept",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/llm/candidates/:candidate_id/reject",
            post(not_yet_owned_handler),
        )
        .route(
            "/api/ml/receipt-recognition/config",
            get(not_yet_owned_handler).put(not_yet_owned_handler),
        )
}

pub fn import_runtime_router() -> Router<ProxyState> {
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
            "/api/ml/receipt-recognition/config",
            get(ocr_config_get_runtime_handler).put(ocr_config_put_runtime_handler),
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

pub async fn not_yet_owned_handler(method: Method, OriginalUri(uri): OriginalUri) -> Response {
    route_response(not_yet_owned_response(method.as_str(), uri.path()))
}

pub async fn session_not_found_handler() -> Response {
    route_response(import_session_not_found_response())
}

pub async fn session_cancel_missing_handler() -> Response {
    route_response(import_session_cancel_missing_response())
}

pub async fn import_parse_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = content_type_from_headers(&headers);
    let content_type_lower = content_type.to_ascii_lowercase();
    if content_type_lower.contains("multipart/form-data") {
        return import_parse_multipart_runtime_response(&state, &headers, &content_type, &body);
    }
    if content_type_lower.contains("application/json") || content_type.is_empty() {
        let payload = match serde_json::from_slice::<Value>(&body) {
            Ok(payload) => payload,
            Err(_) => return route_response(import_v2_error_response(400, "Invalid JSON request")),
        };
        return import_parse_json_runtime_response(&state, &headers, &payload, false);
    }
    route_response(import_v2_error_response(
        415,
        "Unsupported import parse content type",
    ))
}

pub async fn import_parse_generic_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    import_parse_json_runtime_response(&state, &headers, &payload, true)
}

pub async fn legacy_import_preview_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let request = match import_preview_request_from_body(&headers, &body) {
        Ok(request) => request,
        Err(response) => return route_response(response),
    };
    let preview = if let Some(temp_path) = request.temp_path {
        let temp_path = match validate_import_temp_path(&temp_path, user_id, None) {
            Ok(path) => path,
            Err(response) => return route_response(response),
        };
        match preview_rows_from_temp_path(&temp_path, request.delimiter.as_deref()) {
            Ok(preview) => preview,
            Err(response) => return route_response(response),
        }
    } else if let Some(uploaded_file) = request.uploaded_file {
        match preview_rows_from_file_bytes(&uploaded_file, request.delimiter.as_deref()) {
            Ok(preview) => preview,
            Err(response) => return route_response(response),
        }
    } else {
        return route_response(import_v2_error_response(400, "Missing temp_path or file"));
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": preview,
        }),
    })
}

pub async fn legacy_import_parsers_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return route_response(response);
    }
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": legacy_import_parsers(),
        }),
    })
}

pub async fn legacy_parse_import_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = content_type_from_headers(&headers);
    if !content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        return route_response(import_v2_error_response(
            415,
            "Unsupported parse_import content type",
        ));
    }
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let form = match parse_multipart_form_data(&content_type, &body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let Some(file_part) = form.file_parts().first().copied() else {
        return route_response(import_v2_error_response(400, "No file provided"));
    };
    let filename = file_part
        .filename
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("import-file.csv");
    let request = parse_legacy_import_file(&form, filename, &file_part.body, user_id);
    let parsed = match request {
        Ok(parsed) => parsed,
        Err(response) => return route_response(response),
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": {
                "items": parsed.items,
                "totalCount": parsed.total_count,
                "parserType": parsed.parser_type,
                "detectedParserType": parsed.detected_parser_type,
            }
        }),
    })
}

pub async fn legacy_import_upload_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = content_type_from_headers(&headers);
    if !content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        return route_response(import_v2_error_response(
            415,
            "Unsupported import upload content type",
        ));
    }
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let form = match parse_multipart_form_data(&content_type, &body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let Some(file_part) = form.file_parts().first().copied() else {
        return route_response(import_v2_error_response(400, "No file provided"));
    };
    let filename = file_part
        .filename
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("import-file.csv");
    let parsed = match parse_legacy_import_file(&form, filename, &file_part.body, user_id) {
        Ok(parsed) => parsed,
        Err(response) => return route_response(response),
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": {
                "preview": parsed.items,
                "total": parsed.total_count,
                "valid": parsed.total_count,
                "invalid": 0,
                "inserted": 0,
                "duplicates": 0,
                "dedup_stats": Value::Null,
                "parser_type": parsed.parser_type,
                "errors": [],
            }
        }),
    })
}

pub async fn legacy_import_reclassify_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return route_response(response);
    }
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(transactions) = object.get("transactions").and_then(Value::as_array) else {
        return route_response(import_v2_error_response(400, "缺少transactions字段"));
    };
    let result = transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| legacy_reclassify_passthrough(index, transaction))
        .collect::<Vec<_>>();
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": result}),
    })
}

pub async fn import_configs_list_runtime_handler(
    State(state): State<ProxyState>,
    Query(query): Query<ImportConfigQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    if let Some(file_format) = query
        .file_format()
        .map(|value| normalize_config_text(value))
    {
        configs.retain(|config| {
            config_text(config, &["fileFormat", "file_format"])
                .map(|value| normalize_config_text(&value) == file_format)
                .unwrap_or(false)
        });
    }
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    configs.truncate(limit);
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": configs}),
    })
}

pub async fn import_configs_save_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    if config_text_from_object(object, &["name"]).is_none() {
        return route_response(import_v2_error_response(400, "name is required"));
    }
    if config_text_from_object(object, &["fileFormat", "file_format"]).is_none() {
        return route_response(import_v2_error_response(400, "fileFormat is required"));
    }
    let Some(field_mappings) = first_value(object, &["fieldMappings", "field_mappings"]) else {
        return route_response(import_v2_error_response(400, "fieldMappings is required"));
    };
    if !field_mappings
        .as_object()
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return route_response(import_v2_error_response(400, "fieldMappings is required"));
    }

    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let requested_id = first_value(object, &["id"])
        .and_then(value_to_i64)
        .filter(|id| *id > 0);
    let config_id = requested_id.unwrap_or_else(|| {
        configs
            .iter()
            .filter_map(config_id_value)
            .max()
            .unwrap_or(0)
            + 1
    });
    let saved = build_import_config_from_payload(object, config_id);
    if bool_value(&saved, "isDefault").unwrap_or(false) {
        for config in &mut configs {
            if let Some(object) = config.as_object_mut() {
                object.insert("isDefault".to_string(), json!(false));
            }
        }
    }
    if let Some(existing) = configs
        .iter_mut()
        .find(|config| config_id_value(config) == Some(config_id))
    {
        *existing = saved;
    } else {
        configs.push(saved);
    }
    configs.sort_by_key(|config| config_id_value(config).unwrap_or(i64::MAX));
    if let Err(response) = store_import_configs(runtime.connection(), user_id, &configs) {
        return route_response(response);
    }
    route_response(ImportV2RouteResponse {
        status_code: 201,
        body: json!({"success": true, "result": {"id": config_id}}),
    })
}

pub async fn import_configs_match_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(file_format) = config_text_from_object(object, &["fileFormat", "file_format"]) else {
        return route_response(import_v2_error_response(400, "fileFormat is required"));
    };
    let Some(headers) = string_array_field_from_object(object, &["headers"]) else {
        return route_response(import_v2_error_response(400, "headers is required"));
    };
    if headers.is_empty() {
        return route_response(import_v2_error_response(400, "headers is required"));
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let result = best_import_config_match(&configs, &file_format, &headers);
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": result}),
    })
}

pub async fn import_configs_suggest_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(file_format) = config_text_from_object(object, &["fileFormat", "file_format"]) else {
        return route_response(import_v2_error_response(400, "fileFormat is required"));
    };
    let Some(headers) = string_array_field_from_object(object, &["headers"]) else {
        return route_response(import_v2_error_response(400, "headers is required"));
    };
    if headers.is_empty() {
        return route_response(import_v2_error_response(400, "headers is required"));
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let matched = best_import_config_match(&configs, &file_format, &headers);
    let suggestion = build_import_config_suggestion(&file_format, &headers, matched.as_ref());
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": suggestion}),
    })
}

pub async fn import_configs_delete_runtime_handler(
    State(state): State<ProxyState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let before = configs.len();
    configs.retain(|config| config_id_value(config) != Some(config_id));
    if before == configs.len() {
        return route_response(import_v2_error_response(404, "Config not found"));
    }
    if let Err(response) = store_import_configs(runtime.connection(), user_id, &configs) {
        return route_response(response);
    }
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": true}),
    })
}

pub async fn legacy_import_confirm_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }

    if let Some(session_id) =
        first_value(object, &["session_id", "sessionId"]).and_then(value_to_text)
    {
        match get_import_session(runtime.connection(), &session_id, user_id) {
            Ok(Some(_)) => {}
            Ok(None) => return route_response(import_session_not_found_response()),
            Err(error) => return route_response(db_error_response(error)),
        }
        let result = match confirm_preview_to_bills(runtime.connection_mut(), &session_id, user_id)
        {
            Ok(result) => result,
            Err(error) => return route_response(db_error_response(error)),
        };
        return route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({
                "success": true,
                "result": {
                    "total": result.confirmed_count + result.skipped_count + result.duplicate_count,
                    "inserted": result.confirmed_count,
                    "duplicates": result.duplicate_count,
                    "skipped": result.skipped_count,
                    "errors": result.errors,
                }
            }),
        });
    }

    let Some(bills) = first_value(object, &["bills", "transactions"]).and_then(Value::as_array)
    else {
        return route_response(import_v2_error_response(400, "bills is required"));
    };
    if bills.is_empty() {
        return route_response(import_v2_error_response(400, "bills is required"));
    }
    if let Err(response) = init_legacy_bills_runtime_schema(&runtime) {
        return route_response(response);
    }
    let result = match insert_legacy_confirmed_bills(runtime.connection_mut(), user_id, bills) {
        Ok(result) => result,
        Err(response) => return route_response(response),
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": result["inserted"].as_i64().unwrap_or_default() >= 0, "result": result}),
    })
}

pub async fn legacy_import_batch_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(file_path) = first_text_from_object(object, &["file_path", "filePath"]) else {
        return route_response(import_v2_error_response(400, "file_path is required"));
    };
    let path = match validate_import_temp_path(&file_path, user_id, None) {
        Ok(path) => path,
        Err(response) => return route_response(response),
    };
    let text = match read_import_temp_text(&path) {
        Ok(text) => text,
        Err(response) => return route_response(response),
    };
    let parsed = parse_standard_bills_from_csv_text(&text, "auto", false);
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": {
                "success": true,
                "total": parsed.bills.len(),
                "inserted": 0,
                "duplicates": 0,
                "preview": parsed
                    .bills
                    .iter()
                    .map(|bill| legacy_import_item_from_standard_bill(bill, "auto"))
                    .collect::<Vec<_>>(),
                "runtime": "rust-import-db-runtime-partial",
            }
        }),
    })
}

pub async fn import_dedup_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match required_session_id_from_payload(object) {
        Ok(session_id) => session_id,
        Err(response) => return route_response(response),
    };
    let include_preview =
        bool_field_from_object(object, &["include_preview", "includePreview"]).unwrap_or(true);
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }

    let templates =
        match get_unprocessed_templates_for_dedup(runtime.connection(), &session_id, user_id) {
            Ok(templates) => templates,
            Err(error) => return route_response(db_error_response(error)),
        };
    let dedup_input = dedup_bills_from_parser_templates(&templates);
    let dedup_result = SmartDeduplicationEngine.process(dedup_input);
    let preview_drafts = preview_drafts_from_dedup_bills(&dedup_result.kept_bills);
    let inserted_preview = match insert_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_drafts,
    ) {
        Ok(inserted) => inserted,
        Err(error) => return route_response(db_error_response(error)),
    };
    let template_ids = templates
        .iter()
        .map(|template| template.id)
        .collect::<Vec<_>>();
    if let Err(error) =
        update_parser_template_status(runtime.connection_mut(), &template_ids, true, None, user_id)
    {
        return route_response(db_error_response(error));
    }
    if let Err(error) = update_import_session_status(
        runtime.connection(),
        &ImportSessionStatusUpdate {
            session_id: session_id.clone(),
            user_id,
            status: "preview".to_string(),
            total_parsed: Some(usize_to_i64(templates.len())),
            total_preview: Some(usize_to_i64(inserted_preview)),
            total_confirmed: None,
        },
    ) {
        return route_response(db_error_response(error));
    }
    let preview = if include_preview {
        match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
            Ok(rows) => rows.into_iter().map(preview_row_to_value).collect(),
            Err(error) => return route_response(db_error_response(error)),
        }
    } else {
        Vec::new()
    };
    route_response(import_stage_dedup_success(ImportStageDedupData {
        session_id,
        preview,
        preview_included: include_preview,
        total: dedup_result.original_count,
        after_dedup: inserted_preview,
        dedup_stats: json!({
            "removed": dedup_result.removed_count,
            "duplicates": dedup_result.duplicate_groups.len(),
            "transfer_pairs": dedup_result.transfer_pairs.len(),
            "split_merge": dedup_result.split_groups.len(),
        }),
        match_stats: json!({
            "runtime": "rust",
            "database_candidates": 0,
            "provider_bypassed": true,
        }),
    }))
}

pub async fn import_confirm_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match required_session_id_from_payload(object) {
        Ok(session_id) => session_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }

    if first_value(object, &["preview_updates", "previewUpdates"]).is_some() {
        let update_items = match preview_update_items_from_payload(&payload) {
            Ok(items) => items,
            Err(response) => return route_response(response),
        };
        let mut patches = Vec::with_capacity(update_items.len());
        for item in update_items {
            let preview_id = match preview_id_from_payload(item) {
                Ok(preview_id) => preview_id,
                Err(response) => return route_response(response),
            };
            match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
                Ok(Some(preview)) if preview.session_id == session_id => {}
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(404, "Preview bill not found"));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
            patches.push(build_preview_patch_from_payload(preview_id, item));
        }
        if let Err(error) = replace_preview_selection_with_patches(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &patches,
        ) {
            return route_response(db_error_response(error));
        }
    } else if let Some(selected_ids) =
        id_list_field_from_object(object, &["selected_ids", "selectedIds"])
    {
        for preview_id in &selected_ids {
            match get_preview_bill_by_id(runtime.connection(), *preview_id, user_id) {
                Ok(Some(preview)) if preview.session_id == session_id => {}
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(404, "Preview bill not found"));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
        }
        if let Err(error) =
            reset_session_preview_selection(runtime.connection(), &session_id, user_id)
        {
            return route_response(db_error_response(error));
        }
        if let Err(error) =
            update_preview_selection(runtime.connection_mut(), &selected_ids, true, user_id)
        {
            return route_response(db_error_response(error));
        }
    }

    let result = match confirm_preview_to_bills(runtime.connection_mut(), &session_id, user_id) {
        Ok(result) => result,
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_stage_confirm_success(ImportStageConfirmData {
        imported_count: result.confirmed_count,
        skipped_count: result.skipped_count + result.duplicate_count,
        errors: result.errors,
    }))
}

pub async fn import_session_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(session)) => route_response(import_session_success(ImportSessionSummary {
            session_id: session.session_id,
            status: session.status,
            created_at: session.created_at,
            parsed_count: non_negative_usize(session.total_parsed),
            preview_count: non_negative_usize(session.total_preview),
            file_paths: json!([]),
        })),
        Ok(None) => route_response(import_session_not_found_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_session_cancel_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => match clear_session_data(runtime.connection_mut(), &session_id, user_id) {
            Ok(_) => route_response(import_session_cancel_success_response()),
            Err(error) => route_response(db_error_response(error)),
        },
        Ok(None) => route_response(import_session_cancel_missing_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_preview_page_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    Query(query): Query<PreviewPageQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .or(query.page_size_camel)
        .unwrap_or(100)
        .clamp(1, 500);
    let selected_only = query
        .selected_only
        .or(query.selected_only_camel)
        .unwrap_or(false);
    match get_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        page as i64,
        page_size as i64,
        selected_only,
    ) {
        Ok((preview, total)) => {
            let preview = preview
                .into_iter()
                .map(|row| serde_json::to_value(row).unwrap_or_else(|_| json!({})))
                .collect();
            route_response(import_preview_page_success(ImportPreviewPageData {
                preview,
                total: non_negative_usize(total),
                page,
                page_size,
            }))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_preview_index_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let rows = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
        Ok(rows) => rows,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categories_by_id = BTreeMap::new();
    let accounts_by_id = BTreeMap::new();
    let items = rows
        .into_iter()
        .filter_map(|row| preview_row_to_value(row).as_object().cloned())
        .map(|row| build_import_preview_filter_index_item(&row, &categories_by_id, &accounts_by_id))
        .collect::<Vec<_>>();
    route_response(import_preview_index_success(ImportPreviewIndexData {
        total: items.len(),
        items,
    }))
}

pub async fn import_preview_update_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let preview_id = match preview_id_from_payload(object) {
        Ok(preview_id) => preview_id,
        Err(response) => return route_response(response),
    };
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) if preview.session_id == session_id => preview,
        Ok(Some(_)) | Ok(None) => {
            return route_response(import_v2_error_response(404, "Preview bill not found"));
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    let patch = build_preview_patch_from_payload(preview.id, object);
    let updated = match update_preview_bill(runtime.connection(), &session_id, user_id, &patch) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    if response_mode_is_preview_item(object) {
        let preview_item = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => preview_row_to_value(preview),
            Ok(Some(_)) | Ok(None) => Value::Null,
            Err(error) => return route_response(db_error_response(error)),
        };
        route_response(import_v2_data_response(json!({
            "updated": updated,
            "previewItem": preview_item,
        })))
    } else {
        route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": updated}),
        })
    }
}

pub async fn import_reclassify_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let update_items = match preview_update_items_from_payload(&payload) {
        Ok(update_items) => update_items,
        Err(response) => return route_response(response),
    };
    let mut patches = Vec::with_capacity(update_items.len());
    for item in update_items {
        let preview_id = match preview_id_from_payload(item) {
            Ok(preview_id) => preview_id,
            Err(response) => return route_response(response),
        };
        match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => {}
            Ok(Some(_)) | Ok(None) => {
                return route_response(import_v2_error_response(404, "Preview bill not found"));
            }
            Err(error) => return route_response(db_error_response(error)),
        }
        patches.push(build_preview_patch_from_payload(preview_id, item));
    }
    let updated = match update_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        patches.as_slice(),
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    let preview = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
        Ok(preview) => preview,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categorized = preview.iter().filter(preview_row_is_categorized).count();
    let account_matched = preview.iter().filter(preview_row_has_account).count();
    let preview: Vec<Value> = preview.into_iter().map(preview_row_to_value).collect();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "total": preview.len(),
        "categorized": categorized,
        "account_matched": account_matched,
        "session_samples_saved": 0,
        "annotation_applied": 0,
        "updated": updated,
        "preview": preview,
    })))
}

pub async fn preview_recurring_candidates_runtime_handler(
    State(state): State<ProxyState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_v2_data_response(json!({
        "previewId": preview.id,
        "sessionId": preview.session_id,
        "linkedRecurringId": preview.preview_recurring_id,
        "linkedRecurringName": preview.preview_recurring_name,
        "candidates": [],
        "candidate_count": 0,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn preview_recurring_match_put_runtime_handler(
    State(state): State<ProxyState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let recurring_id = match first_value(object, &["recurringId", "recurring_id"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => return route_response(import_v2_error_response(400, "Missing recurringId")),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let target_candidate = recurring_candidate_from_payload(object, recurring_id);
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: Some(recurring_id),
        candidate_count: recurring_candidate_count_from_payload(object, target_candidate.as_ref()),
        target_candidate,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": recurring_id}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn preview_recurring_match_delete_runtime_handler(
    State(state): State<ProxyState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: None,
        candidate_count: 0,
        target_candidate: None,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": Value::Null}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn preview_transfer_decision_runtime_handler(
    State(state): State<ProxyState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let decision = match decision_from_payload(object) {
        Ok(decision) => decision,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let reviewed_type = first_value(object, &["reviewedType", "reviewed_type"])
        .and_then(value_to_text)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "转账".to_string());
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        decision,
        &reviewed_type,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"decision": decision_name(decision)}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_learning_suggestions_get_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    import_learning_suggestions_response(state, session_id, headers, None).await
}

pub async fn import_learning_suggestions_post_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    import_learning_suggestions_response(state, session_id, headers, Some(payload)).await
}

async fn import_learning_suggestions_response(
    state: ProxyState,
    session_id: String,
    headers: HeaderMap,
    payload: Option<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates = match payload.as_ref() {
        Some(payload) => {
            match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, payload) {
                Ok(updated) => updated,
                Err(response) => return route_response(response),
            }
        }
        None => 0,
    };
    let preview_ids = payload
        .as_ref()
        .map(preview_ids_from_payload)
        .unwrap_or_default();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "suggestions": [],
        "count": 0,
        "preview_ids": preview_ids,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn import_learning_promote_runtime_handler(
    State(state): State<ProxyState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates =
        match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, &payload) {
            Ok(updated) => updated,
            Err(response) => return route_response(response),
        };
    let annotation_samples = annotation_samples_from_payload(&payload);
    let saved_samples = if annotation_samples.is_empty() {
        0
    } else {
        match save_import_annotation_samples(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &annotation_samples,
        ) {
            Ok(saved) => saved,
            Err(error) => return route_response(db_error_response(error)),
        }
    };
    let mut preview_ids = preview_ids_from_payload(&payload);
    preview_ids.extend(annotation_samples.iter().map(|sample| sample.preview_id));
    preview_ids.sort_unstable();
    preview_ids.dedup();
    let promoted = match promote_import_learning_rules(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_ids,
    ) {
        Ok(result) => result,
        Err(response) => return route_response(response),
    };
    route_response(import_v2_data_response(json!({
        "success": true,
        "session_id": session_id,
        "selected_samples": preview_ids.len(),
        "saved_samples": saved_samples,
        "rules_total": promoted.rules_total,
        "created": promoted.created,
        "updated": promoted.updated,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn import_learning_rules_list_runtime_handler(
    State(state): State<ProxyState>,
    Query(query): Query<ImportLearningRulesQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size().clamp(1, 500);
    let enabled_only = query.enabled_only();
    let total = match count_import_learning_rules(runtime.connection(), user_id, enabled_only) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    let rules = match load_import_learning_rules(
        runtime.connection(),
        user_id,
        enabled_only,
        page_size,
        offset,
    ) {
        Ok(rules) => rules,
        Err(response) => return route_response(response),
    };
    let total_pages = if total <= 0 {
        0
    } else {
        (total as usize).div_ceil(page_size) as i64
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": rules,
            "totalCount": total,
            "page": page,
            "pageSize": page_size,
            "totalPages": total_pages,
        }),
    })
}

pub async fn import_learning_rule_update_runtime_handler(
    State(state): State<ProxyState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    if let Err(response) =
        update_import_learning_rule(runtime.connection(), rule_id, user_id, object)
    {
        return route_response(response);
    }
    match get_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(Some(rule)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": rule}),
        }),
        Ok(None) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

pub async fn import_learning_rule_delete_runtime_handler(
    State(state): State<ProxyState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match delete_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": true}),
        }),
        Ok(false) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

pub async fn llm_preview_recommend_accept_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    llm_preview_recommend_review_response(
        state,
        headers,
        payload,
        ImportPreviewDecision::Accept,
        "accept",
    )
    .await
}

pub async fn llm_preview_recommend_reject_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    llm_preview_recommend_review_response(
        state,
        headers,
        payload,
        ImportPreviewDecision::Reject,
        "reject",
    )
    .await
}

async fn llm_preview_recommend_review_response(
    state: ProxyState,
    headers: HeaderMap,
    payload: Value,
    decision: ImportPreviewDecision,
    decision_text: &str,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match first_value(object, &["session_id", "sessionId"])
        .and_then(value_to_text)
        .filter(|value| !value.trim().is_empty())
    {
        Some(value) => value,
        None => {
            return route_response(import_v2_error_response(
                400,
                "session_id and preview_id are required",
            ));
        }
    };
    let preview_id = match first_value(object, &["preview_id", "previewId"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => {
            return route_response(import_v2_error_response(
                400,
                "session_id and preview_id are required",
            ));
        }
    };
    let suggestion = first_value(object, &["suggestion"]).and_then(llm_suggestion_from_value);
    let user_correction =
        first_value(object, &["user_correction", "userCorrection"]).and_then(Value::as_object);
    let user_correction_category = user_correction.and_then(|object| {
        first_value(
            object,
            &[
                "category",
                "categoryName",
                "mainCategory",
                "main_category",
                "suggested_main_category",
            ],
        )
        .and_then(value_to_text)
    });
    let user_correction_account = user_correction.and_then(|object| {
        first_value(
            object,
            &[
                "account",
                "accountName",
                "sourceAccount",
                "source_account",
                "suggested_source_account",
            ],
        )
        .and_then(value_to_text)
    });
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match review_preview_llm_recommendation(
        runtime.connection_mut(),
        ImportPreviewLlmReviewRequest {
            session_id: &session_id,
            preview_id,
            user_id,
            decision,
            suggestion: suggestion.as_ref(),
            user_correction_category: user_correction_category.as_deref(),
            user_correction_account: user_correction_account.as_deref(),
        },
    ) {
        Ok(result) => route_response(llm_decision_result_response(
            result,
            &session_id,
            preview_id,
            decision_text,
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_memory_runtime_handler(
    State(state): State<ProxyState>,
    Query(query): Query<LlmMemoryQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);
    match get_llm_memory_events(
        runtime.connection(),
        user_id,
        query.session_id.as_deref(),
        query.event_type.as_deref(),
        limit,
        offset,
    ) {
        Ok(events) => {
            let total = events.len();
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true, "data": events, "total": total}),
            })
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_config_get_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let config = match state
        .get_llm_runtime_config(user_id_value)
        .map(Ok)
        .unwrap_or_else(|| effective_llm_config_from_saved(runtime.connection(), user_id_value))
    {
        Ok(config) => config,
        Err(error) => return route_response(db_error_response(error)),
    };
    ai_route_response(build_llm_config_get_response(&config))
}

pub async fn llm_config_post_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let payload = match json_body_or_empty(&body) {
        Ok(payload) => payload,
        Err(response) => return route_response(response),
    };
    let Some(object) = payload.as_object().filter(|object| !object.is_empty()) else {
        return route_response(ImportV2RouteResponse {
            status_code: 400,
            body: json!({"success": false, "error": "No data provided"}),
        });
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let base_config = match state
        .get_llm_runtime_config(user_id_value)
        .map(Ok)
        .unwrap_or_else(|| effective_llm_config_from_saved(runtime.connection(), user_id_value))
    {
        Ok(config) => config,
        Err(error) => return route_response(db_error_response(error)),
    };
    let updated = update_runtime_llm_config_payload(&base_config, object);
    state.set_llm_runtime_config(user_id_value, updated.clone());
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": llm_runtime_config_response_data(&updated, false)}),
    })
}

pub async fn llm_configs_list_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match list_llm_configs(runtime.connection(), user_id_value) {
        Ok(configs) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({
                "success": true,
                "data": configs.iter().map(safe_llm_config_payload).collect::<Vec<_>>(),
            }),
        }),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_configs_create_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let payload = match json_body_or_empty(&body) {
        Ok(payload) => payload,
        Err(response) => return route_response(response),
    };
    let object = payload.as_object().cloned().unwrap_or_default();
    let name = text_from_map(&object, "name").trim().to_string();
    if name.is_empty() {
        return route_response(ImportV2RouteResponse {
            status_code: 400,
            body: json!({"success": false, "error": "name is required"}),
        });
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let draft = LlmConfigDraft {
        name,
        provider: text_from_map_or(&object, "provider", "openai"),
        model: text_from_map_or(&object, "model", ""),
        api_key: text_from_map_or(&object, "api_key", ""),
        base_url: text_from_map_or(&object, "base_url", ""),
        advanced_settings: object
            .get("advanced_settings")
            .cloned()
            .unwrap_or_else(|| json!({})),
        is_active: object
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    match create_llm_config(runtime.connection(), user_id_value, &draft) {
        Ok(config) => {
            if config.get("is_active").and_then(Value::as_i64).unwrap_or(0) != 0 {
                state.clear_llm_runtime_config(user_id_value);
            }
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true, "data": safe_llm_config_payload(&config)}),
            })
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_config_update_runtime_handler(
    State(state): State<ProxyState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let payload = match json_body_or_empty(&body) {
        Ok(payload) => payload,
        Err(response) => return route_response(response),
    };
    let object = payload.as_object().cloned().unwrap_or_default();
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_llm_config(
        runtime.connection(),
        config_id,
        user_id_value,
        &llm_config_update_from_map(&object),
    ) {
        Ok(Some(config)) => {
            if config.get("is_active").and_then(Value::as_i64).unwrap_or(0) != 0 {
                state.clear_llm_runtime_config(user_id_value);
            }
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true, "data": safe_llm_config_payload(&config)}),
            })
        }
        Ok(None) => route_response(llm_not_found_response("config_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_config_delete_runtime_handler(
    State(state): State<ProxyState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match delete_llm_config(runtime.connection(), config_id, user_id_value) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true}),
        }),
        Ok(false) => route_response(llm_not_found_response("config_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_config_activate_runtime_handler(
    State(state): State<ProxyState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match activate_llm_config(runtime.connection(), config_id, user_id_value) {
        Ok(true) => {
            state.clear_llm_runtime_config(user_id_value);
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true}),
            })
        }
        Ok(false) => route_response(llm_not_found_response("config_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_candidates_list_runtime_handler(
    State(state): State<ProxyState>,
    Query(query): Query<LlmCandidatesQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    let candidates = match list_llm_candidates(
        runtime.connection(),
        user_id_value,
        query.status.as_deref(),
        query.r#type.as_deref(),
        limit,
        offset,
    ) {
        Ok(candidates) => candidates,
        Err(error) => return route_response(db_error_response(error)),
    };
    match count_llm_candidates(
        runtime.connection(),
        user_id_value,
        query.status.as_deref(),
        query.r#type.as_deref(),
    ) {
        Ok(total) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: build_llm_candidate_list_response(candidates, total),
        }),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_candidate_get_runtime_handler(
    State(state): State<ProxyState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match bill_analyser_db::get_llm_candidate_by_id(
        runtime.connection(),
        candidate_id,
        user_id_value,
    ) {
        Ok(Some(candidate)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "data": candidate}),
        }),
        Ok(None) => route_response(llm_not_found_response(&format!(
            "Candidate {candidate_id} not found"
        ))),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_candidate_accept_runtime_handler(
    State(state): State<ProxyState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match accept_llm_candidate(runtime.connection(), candidate_id, user_id_value) {
        Ok(Some(result)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "data": result}),
        }),
        Ok(None) => route_response(llm_not_found_response(&format!(
            "Candidate {candidate_id} not found"
        ))),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_candidate_reject_runtime_handler(
    State(state): State<ProxyState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match reject_llm_candidate(runtime.connection(), candidate_id, user_id_value) {
        Ok(Some(rejected)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: build_llm_candidate_reject_response(rejected),
        }),
        Ok(None) => route_response(llm_not_found_response(&format!(
            "Candidate {candidate_id} not found"
        ))),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn ocr_config_get_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return route_response(response);
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_ocr_runtime_schema(&runtime) {
        return route_response(response);
    }
    match load_ocr_config_setting(runtime.connection()) {
        Ok(config) => ai_route_response(build_ocr_config_success_response(&config)),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn ocr_config_put_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return route_response(response);
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_ocr_runtime_schema(&runtime) {
        return route_response(response);
    }
    match store_ocr_config_setting(runtime.connection(), Some(&payload)) {
        Ok(config) => ai_route_response(build_ocr_config_success_response(&config)),
        Err(bill_analyser_db::DbError::InvalidOperation(message))
            if message == "unknown OCR provider" =>
        {
            ai_route_response(build_unknown_ocr_provider_response())
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn learning_suggestions_list_runtime_handler(
    State(state): State<ProxyState>,
    Query(query): Query<LearningCenterListQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0);
    let status = query
        .status
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let total = match count_learning_suggestions(runtime.connection(), user_id, status) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let items =
        match load_learning_suggestions(runtime.connection(), user_id, status, limit, offset) {
            Ok(items) => items,
            Err(response) => return route_response(response),
        };
    route_response(learning_data_response(json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

pub async fn learning_suggestions_generate_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match mine_learning_suggestions(runtime.connection_mut(), user_id) {
        Ok(stats) => route_response(learning_data_response(stats)),
        Err(response) => route_response(response),
    }
}

pub async fn learning_suggestion_accept_runtime_handler(
    State(state): State<ProxyState>,
    Path(suggestion_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match accept_learning_suggestion(runtime.connection_mut(), suggestion_id, user_id) {
        Ok(LearningSuggestionDecision::Accepted(result)) => {
            route_response(learning_data_response(result))
        }
        Ok(LearningSuggestionDecision::Conflict(result)) => route_response(ImportV2RouteResponse {
            status_code: 409,
            body: json!({"success": false, "error": result["error"], "data": result}),
        }),
        Ok(LearningSuggestionDecision::NotFound) => {
            route_response(learning_error_response(404, "suggestion_not_found"))
        }
        Err(response) => route_response(response),
    }
}

pub async fn learning_suggestions_batch_accept_runtime_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(raw_ids) = first_value(object, &["suggestionIds", "suggestion_ids"]) else {
        return route_response(learning_error_response(
            400,
            "suggestionIds must be a non-empty array",
        ));
    };
    let Some(raw_ids) = raw_ids.as_array() else {
        return route_response(learning_error_response(
            400,
            "suggestionIds must be a non-empty array",
        ));
    };
    if raw_ids.is_empty() {
        return route_response(learning_error_response(
            400,
            "suggestionIds must be a non-empty array",
        ));
    }
    if raw_ids.len() > 100 {
        return route_response(learning_error_response(
            400,
            "batch size must not exceed 100",
        ));
    }
    let mut suggestion_ids = Vec::new();
    for item in raw_ids {
        let Some(id) = value_to_i64(item).filter(|value| *value > 0) else {
            return route_response(learning_error_response(400, "Invalid suggestionIds"));
        };
        if !suggestion_ids.contains(&id) {
            suggestion_ids.push(id);
        }
    }

    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut accepted = Vec::new();
    let mut failed = Vec::new();
    for suggestion_id in suggestion_ids {
        match accept_learning_suggestion(runtime.connection_mut(), suggestion_id, user_id) {
            Ok(LearningSuggestionDecision::Accepted(result)) => {
                accepted.push(json!({"id": suggestion_id, "ruleId": result["rule_id"]}));
            }
            Ok(LearningSuggestionDecision::Conflict(result)) => {
                failed.push(json!({"id": suggestion_id, "error": result["error"]}));
            }
            Ok(LearningSuggestionDecision::NotFound) => {
                failed.push(json!({"id": suggestion_id, "error": "suggestion_not_found"}));
            }
            Err(response) => return route_response(response),
        }
    }
    route_response(learning_data_response(json!({
        "accepted": accepted,
        "failed": failed,
        "acceptedCount": accepted.len(),
        "failedCount": failed.len(),
    })))
}

pub async fn learning_suggestion_reject_runtime_handler(
    State(state): State<ProxyState>,
    Path(suggestion_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match reject_learning_suggestion(runtime.connection_mut(), suggestion_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true}),
        }),
        Ok(false) => route_response(learning_error_response(
            404,
            "suggestion_not_found_or_not_pending",
        )),
        Err(response) => route_response(response),
    }
}

pub async fn learning_rules_list_runtime_handler(
    State(state): State<ProxyState>,
    Query(query): Query<LearningCenterListQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0);
    let enabled_only = query.enabled_only();
    let total = match count_import_learning_rules(runtime.connection(), user_id, enabled_only) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let items = match load_import_learning_rules(
        runtime.connection(),
        user_id,
        enabled_only,
        limit,
        offset,
    ) {
        Ok(items) => items
            .into_iter()
            .map(|item| learning_rule_camel_to_snake(&item))
            .collect::<Vec<_>>(),
        Err(response) => return route_response(response),
    };
    route_response(learning_data_response(json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

pub async fn learning_rule_toggle_runtime_handler(
    State(state): State<ProxyState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let enabled = first_value(object, &["enabled"])
        .and_then(value_to_bool)
        .unwrap_or(true);
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match set_import_learning_rule_enabled(runtime.connection_mut(), rule_id, user_id, enabled) {
        Ok(true) => route_response(learning_data_response(json!({
            "ruleId": rule_id,
            "enabled": enabled,
        }))),
        Ok(false) => route_response(learning_error_response(404, "rule_not_found")),
        Err(response) => route_response(response),
    }
}

pub async fn learning_rule_update_runtime_handler(
    State(state): State<ProxyState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let has_edit = first_value(object, &["matchValue", "match_value"]).is_some()
        || first_value(object, &["learnedType", "learned_type"]).is_some()
        || first_value(object, &["learnedCategoryId", "learned_category_id"]).is_some()
        || first_value(object, &["enabled"]).is_some();
    if !has_edit {
        return route_response(learning_error_response(400, "no_fields_to_update"));
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    if let Err(response) =
        update_import_learning_rule(runtime.connection(), rule_id, user_id, object)
    {
        if response.status_code == 404 {
            return route_response(learning_error_response(404, "rule_not_found"));
        }
        return route_response(response);
    }
    match get_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(Some(rule)) => {
            route_response(learning_data_response(learning_rule_camel_to_snake(&rule)))
        }
        Ok(None) => route_response(learning_error_response(404, "rule_not_found")),
        Err(response) => route_response(response),
    }
}

pub async fn learning_rule_delete_runtime_handler(
    State(state): State<ProxyState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match delete_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true}),
        }),
        Ok(false) => route_response(learning_error_response(404, "rule_not_found")),
        Err(response) => route_response(response),
    }
}

pub fn not_yet_owned_response(method: &str, path: &str) -> ImportV2RouteResponse {
    let deletion_allowed = can_delete_python_import_paths(ImportDeletionEvidence::default());
    let mut response =
        import_v2_error_response(StatusCode::NOT_IMPLEMENTED.as_u16(), NOT_YET_OWNED_ERROR);
    response.body["data"] = json!({
        "method": method,
        "path": path,
        "route_owner": "python_proxied",
        "business_migration": "import_route_skeleton",
        "db_write_allowed": false,
        "python_import_deletion_allowed": deletion_allowed,
    });
    response
}

fn route_response(response: ImportV2RouteResponse) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(response.body)).into_response()
}

fn ai_route_response(response: AiRouteResponse) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(response.body)).into_response()
}

fn payload_object(payload: &Value) -> Result<&Map<String, Value>, ImportV2RouteResponse> {
    payload
        .as_object()
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))
}

#[derive(Debug)]
struct ImportParseRuntimeInput {
    session_id: String,
    parser_id: String,
    standard_bills: Vec<StandardBill>,
    file_count: i64,
    files: Vec<Value>,
    unmatched_files: Vec<Value>,
    require_existing_session: bool,
}

fn import_parse_json_runtime_response(
    state: &ProxyState,
    headers: &HeaderMap,
    payload: &Value,
    require_existing_session: bool,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = if require_existing_session {
        match required_session_id_from_payload(object) {
            Ok(session_id) => session_id,
            Err(response) => return route_response(response),
        }
    } else {
        optional_session_id_from_payload(object).unwrap_or_else(generate_import_session_id)
    };
    let parser_id = parser_id_from_payload(object);
    let mut files = Vec::new();
    let standard_bills = if let Some(temp_path) =
        first_text_from_object(object, &["temp_path", "tempPath"])
    {
        let standard_bills =
            match standard_bills_from_temp_path_payload(object, &temp_path, user_id, &session_id) {
                Ok(standard_bills) => standard_bills,
                Err(response) => return route_response(response),
            };
        files.push(json!({
            "filename": FsPath::new(&temp_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("import-file"),
            "temp_path": temp_path,
            "parser_id": parser_id,
            "parsed_count": standard_bills.len(),
            "source": "temp_path",
        }));
        standard_bills
    } else {
        match standard_bills_from_payload(object) {
            Ok(standard_bills) => standard_bills,
            Err(response) => return route_response(response),
        }
    };
    if standard_bills.is_empty() {
        return route_response(import_v2_error_response(400, "No valid bills to parse"));
    }
    let file_count = first_value(object, &["file_count", "fileCount"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
        .unwrap_or_else(|| usize_to_i64(files.len().max(1)));
    persist_import_parse_runtime_response(
        state,
        user_id,
        ImportParseRuntimeInput {
            session_id,
            parser_id,
            standard_bills,
            file_count,
            files,
            unmatched_files: Vec::new(),
            require_existing_session,
        },
    )
}

fn import_parse_multipart_runtime_response(
    state: &ProxyState,
    headers: &HeaderMap,
    content_type: &str,
    body: &[u8],
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let form = match parse_multipart_form_data(content_type, body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let session_id = form
        .text_value(&["session_id", "sessionId"])
        .unwrap_or_else(generate_import_session_id);
    let requested_parser = form
        .text_value(&["parser_id", "parserId", "parser_type", "parserType"])
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let file_parts = form.file_parts();
    if file_parts.is_empty() {
        return route_response(import_v2_error_response(400, "Missing import files"));
    }
    let file_count = file_parts.len();

    let mut standard_bills = Vec::new();
    let mut files = Vec::new();
    let mut unmatched_files = Vec::new();
    let mut first_detected_parser_id: Option<String> = None;

    for part in file_parts {
        let original_name = part
            .filename
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "import-file.csv".to_string());
        let parsed = parse_dedicated_import_bytes(&original_name, &part.body, &requested_parser);
        if let Some(parsed) = parsed.filter(|parsed| !parsed.bills.is_empty()) {
            first_detected_parser_id.get_or_insert_with(|| parsed.parser_id.clone());
            let parsed_count = parsed.bills.len();
            files.push(json!({
                "filename": original_name,
                "parser_id": parsed.parser_id,
                "parsed_count": parsed_count,
                "delimiter": delimiter_to_response(parsed.delimiter),
            }));
            standard_bills.extend(parsed.bills);
        } else {
            let unmatched_parser_id = if requested_parser == "auto" {
                "rust-import"
            } else {
                requested_parser.as_str()
            };
            let temp_path = match save_unmatched_import_file(
                user_id,
                &session_id,
                &original_name,
                &part.body,
            ) {
                Ok(path) => path,
                Err(response) => return route_response(response),
            };
            unmatched_files.push(json!({
                "original_name": original_name,
                "originalName": original_name,
                "filename": original_name,
                "temp_path": temp_path,
                "tempPath": temp_path,
                "parser_id": unmatched_parser_id,
                "reason": "No dedicated Rust parser matched the uploaded file",
            }));
        }
    }

    if standard_bills.is_empty() && unmatched_files.is_empty() {
        return route_response(import_v2_error_response(400, "No import files parsed"));
    }

    let parser_id = first_detected_parser_id.unwrap_or_else(|| {
        if requested_parser == "auto" {
            "rust-import".to_string()
        } else {
            requested_parser
        }
    });
    persist_import_parse_runtime_response(
        state,
        user_id,
        ImportParseRuntimeInput {
            session_id,
            parser_id,
            standard_bills,
            file_count: usize_to_i64(file_count),
            files,
            unmatched_files,
            require_existing_session: false,
        },
    )
}

fn persist_import_parse_runtime_response(
    state: &ProxyState,
    user_id: UserId,
    input: ImportParseRuntimeInput,
) -> Response {
    let mut runtime = match open_runtime(state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let drafts =
        parser_template_drafts_from_standard_bills(&input.standard_bills, &input.parser_id);
    let staging_result = match stage_import_parser_templates(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: input.session_id.clone(),
            user_id,
            file_count: input.file_count.max(1),
        },
        &drafts,
        input.require_existing_session,
    ) {
        Ok(result) => result,
        Err(error) => return route_response(db_error_response(error)),
    };
    if !staging_result.session_found {
        return route_response(import_session_not_found_response());
    }
    route_response(import_stage_parse_success(ImportStageParseData {
        session_id: input.session_id,
        parsed_count: staging_result.inserted_count,
        files: input.files,
        unmatched_files: input.unmatched_files,
        errors: Vec::new(),
    }))
}

fn required_session_id_from_payload(
    object: &Map<String, Value>,
) -> Result<String, ImportV2RouteResponse> {
    optional_session_id_from_payload(object)
        .ok_or_else(|| import_v2_error_response(400, "Missing session_id"))
}

fn optional_session_id_from_payload(object: &Map<String, Value>) -> Option<String> {
    first_value(object, &["session_id", "sessionId"])
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parser_id_from_payload(object: &Map<String, Value>) -> String {
    first_value(
        object,
        &[
            "parser_id",
            "parserId",
            "parser_type",
            "parserType",
            "source",
        ],
    )
    .and_then(value_to_text)
    .map(|value| value.trim().to_ascii_lowercase())
    .filter(|value| !value.is_empty() && value != "auto")
    .unwrap_or_else(|| "rust-import".to_string())
}

fn standard_bills_from_payload(
    object: &Map<String, Value>,
) -> Result<Vec<StandardBill>, ImportV2RouteResponse> {
    let bills = first_value(
        object,
        &[
            "bills",
            "standard_bills",
            "standardBills",
            "raw_bills",
            "rawBills",
            "rows",
            "transactions",
            "items",
        ],
    )
    .and_then(Value::as_array)
    .ok_or_else(|| {
        import_v2_error_response(
            400,
            "Rust import parse currently requires inline normalized bills",
        )
    })?;
    let parsed = bills
        .iter()
        .map(standard_bill_from_payload_value)
        .filter(|bill| !bill.date.trim().is_empty())
        .collect::<Vec<_>>();
    if parsed.is_empty() && !bills.is_empty() {
        return Err(import_v2_error_response(400, "No valid bills to parse"));
    }
    Ok(parsed)
}

fn standard_bill_from_payload_value(value: &Value) -> StandardBill {
    let mut normalized = value.clone();
    if let Some(object) = normalized.as_object_mut() {
        copy_alias_if_missing(
            object,
            "date",
            &[
                "trade_time",
                "time",
                "交易时间",
                "交易日期",
                "记账日期",
                "日期",
            ],
        );
        copy_alias_if_missing(
            object,
            "type",
            &[
                "transaction_type",
                "trade_type",
                "收支类型",
                "收/支",
                "类型",
            ],
        );
        copy_alias_if_missing(
            object,
            "amount",
            &[
                "source_amount",
                "sourceAmount",
                "金额",
                "金额(元)",
                "交易金额",
            ],
        );
        copy_alias_if_missing(
            object,
            "description",
            &[
                "remark",
                "memo",
                "summary",
                "商品",
                "备注",
                "说明",
                "交易说明",
            ],
        );
        copy_alias_if_missing(
            object,
            "counterparty",
            &["opponent", "merchant", "shop", "对方", "交易对方", "商户"],
        );
        copy_alias_if_missing(
            object,
            "payment_method",
            &[
                "paymentMethod",
                "channel",
                "account",
                "账户",
                "支付方式",
                "收/付款方式",
            ],
        );
        copy_alias_if_missing(object, "original_category", &["交易分类", "分类", "类别"]);
        copy_alias_if_missing(object, "transaction_id", &["交易单号", "订单号"]);
        copy_alias_if_missing(object, "merchant_id", &["商家订单号", "商户单号"]);
        copy_alias_if_missing(object, "status", &["交易状态", "当前状态", "状态"]);
    }
    StandardBill::from_json_value(&normalized)
}

fn copy_alias_if_missing(object: &mut Map<String, Value>, target: &str, aliases: &[&str]) {
    if object
        .get(target)
        .and_then(value_to_text)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return;
    }
    if let Some(value) = aliases.iter().find_map(|alias| object.get(*alias).cloned()) {
        object.insert(target.to_string(), value);
    }
}

#[derive(Debug, Clone)]
struct MultipartPart {
    name: String,
    filename: Option<String>,
    body: Vec<u8>,
}

#[derive(Debug, Default)]
struct MultipartForm {
    parts: Vec<MultipartPart>,
}

impl MultipartForm {
    fn text_value(&self, keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            self.parts
                .iter()
                .find(|part| part.name == *key && part.filename.is_none())
                .map(|part| decode_import_text(&part.body).trim().to_string())
                .filter(|value| !value.is_empty())
        })
    }

    fn file_parts(&self) -> Vec<&MultipartPart> {
        self.parts
            .iter()
            .filter(|part| part.filename.is_some() || part.name == "file" || part.name == "files")
            .filter(|part| !part.body.is_empty())
            .collect()
    }
}

#[derive(Debug, Default)]
struct ParsedCsvBills {
    bills: Vec<StandardBill>,
}

#[derive(Debug, Default)]
struct ImportPreviewRequest {
    temp_path: Option<String>,
    uploaded_file: Option<Vec<u8>>,
    delimiter: Option<String>,
}

#[derive(Debug)]
struct LegacyImportParseResult {
    items: Vec<Value>,
    total_count: usize,
    parser_type: String,
    detected_parser_type: String,
}

fn content_type_from_headers(headers: &HeaderMap) -> String {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn first_text_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    first_value(object, keys)
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn json_body_or_empty(body: &[u8]) -> Result<Value, ImportV2RouteResponse> {
    if body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice::<Value>(body)
        .map_err(|_| import_v2_error_response(400, "Invalid JSON request"))
}

fn text_from_map(object: &Map<String, Value>, key: &str) -> String {
    object.get(key).and_then(value_to_text).unwrap_or_default()
}

fn text_from_map_or(object: &Map<String, Value>, key: &str, default: &str) -> String {
    let text = text_from_map(object, key);
    if text.trim().is_empty() {
        default.to_string()
    } else {
        text
    }
}

fn update_runtime_llm_config_payload(base_config: &Value, object: &Map<String, Value>) -> Value {
    let mut config = copy_runtime_llm_config(base_config)
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(enabled) = object.get("enabled").and_then(Value::as_bool) {
        config.insert("enabled".to_string(), json!(enabled));
    }
    if let Some(provider) = object.get("provider").and_then(Value::as_str) {
        config.insert("provider".to_string(), json!(provider));
    }
    if let Some(provider_config) = object.get("provider_config").and_then(Value::as_object) {
        config.insert(
            "provider_config".to_string(),
            Value::Object(provider_config.clone()),
        );
    }
    if let Some(advanced_settings) = object.get("advanced_settings") {
        config.insert("advanced_settings".to_string(), advanced_settings.clone());
    }
    copy_runtime_llm_config(&Value::Object(config))
}

fn llm_runtime_config_response_data(config: &Value, include_available_providers: bool) -> Value {
    let copied = copy_runtime_llm_config(config);
    let object = copied.as_object();
    let provider_config = object
        .and_then(|item| item.get("provider_config"))
        .and_then(Value::as_object);
    let mut response = Map::new();
    response.insert(
        "enabled".to_string(),
        json!(object
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false)),
    );
    response.insert(
        "provider".to_string(),
        json!(object
            .and_then(|item| item.get("provider"))
            .and_then(Value::as_str)
            .unwrap_or("openai")),
    );
    response.insert(
        "model".to_string(),
        json!(provider_config
            .and_then(|item| item.get("model"))
            .and_then(Value::as_str)
            .unwrap_or_default()),
    );
    response.insert(
        "advanced_settings".to_string(),
        copied
            .get("advanced_settings")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    if include_available_providers {
        response.insert(
            "available_providers".to_string(),
            build_llm_config_get_response(config).body["data"]["available_providers"].clone(),
        );
    }
    Value::Object(response)
}

fn llm_config_update_from_map(object: &Map<String, Value>) -> LlmConfigUpdate {
    let api_key = object
        .get("api_key")
        .and_then(value_to_text)
        .and_then(|value| {
            if value == "********" {
                None
            } else {
                Some(value)
            }
        });
    LlmConfigUpdate {
        name: object.get("name").and_then(value_to_text),
        provider: object.get("provider").and_then(value_to_text),
        model: object.get("model").and_then(value_to_text),
        api_key,
        base_url: object.get("base_url").and_then(value_to_text),
        advanced_settings: object.get("advanced_settings").cloned(),
        is_active: object.get("is_active").and_then(Value::as_bool),
    }
}

fn llm_not_found_response(message: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 404,
        body: json!({"success": false, "error": message}),
    }
}

fn parse_multipart_form_data(
    content_type: &str,
    body: &[u8],
) -> Result<MultipartForm, ImportV2RouteResponse> {
    let boundary = multipart_boundary_from_content_type(content_type)
        .ok_or_else(|| import_v2_error_response(400, "Missing multipart boundary"))?;
    let marker = format!("--{boundary}");
    let mut form = MultipartForm::default();
    for raw_part in split_bytes(body, marker.as_bytes()) {
        let raw_part = trim_part_boundary(raw_part);
        if raw_part.is_empty() || raw_part == b"--" {
            continue;
        }
        let Some((headers, part_body)) =
            split_once_bytes(raw_part, b"\r\n\r\n").or_else(|| split_once_bytes(raw_part, b"\n\n"))
        else {
            continue;
        };
        let headers_text = String::from_utf8_lossy(headers);
        let Some(disposition) = headers_text.lines().find(|line| {
            line.to_ascii_lowercase()
                .starts_with("content-disposition:")
        }) else {
            continue;
        };
        let Some(name) = disposition_param(disposition, "name") else {
            continue;
        };
        let filename = disposition_param(disposition, "filename")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        form.parts.push(MultipartPart {
            name,
            filename,
            body: strip_trailing_newline(part_body).to_vec(),
        });
    }
    Ok(form)
}

fn multipart_boundary_from_content_type(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let part = part.trim();
        let value = part.strip_prefix("boundary=")?;
        Some(value.trim_matches('"').to_string())
    })
}

fn disposition_param(disposition: &str, param: &str) -> Option<String> {
    let prefix = format!("{param}=");
    disposition.split(';').find_map(|part| {
        let part = part.trim();
        let raw_value = part.strip_prefix(&prefix)?;
        Some(raw_value.trim_matches('"').to_string())
    })
}

fn split_bytes<'a>(body: &'a [u8], marker: &[u8]) -> Vec<&'a [u8]> {
    if marker.is_empty() {
        return vec![body];
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while let Some(offset) = find_bytes(&body[start..], marker) {
        parts.push(&body[start..start + offset]);
        start += offset + marker.len();
    }
    parts.push(&body[start..]);
    parts
}

fn split_once_bytes<'a>(body: &'a [u8], marker: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let offset = find_bytes(body, marker)?;
    Some((&body[..offset], &body[offset + marker.len()..]))
}

fn find_bytes(body: &[u8], marker: &[u8]) -> Option<usize> {
    if marker.is_empty() || marker.len() > body.len() {
        return None;
    }
    body.windows(marker.len())
        .position(|window| window == marker)
}

fn trim_part_boundary(mut part: &[u8]) -> &[u8] {
    while part.starts_with(b"\r\n") {
        part = &part[2..];
    }
    while part.starts_with(b"\n") {
        part = &part[1..];
    }
    part
}

fn strip_trailing_newline(mut body: &[u8]) -> &[u8] {
    if body.ends_with(b"\r\n") {
        body = &body[..body.len().saturating_sub(2)];
    } else if body.ends_with(b"\n") {
        body = &body[..body.len().saturating_sub(1)];
    }
    body
}

fn decode_import_text(bytes: &[u8]) -> String {
    let decoded = if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_string()
    } else {
        let (text, _, _) = GBK.decode(bytes);
        text.into_owned()
    };
    decoded.trim_start_matches('\u{feff}').to_string()
}

fn resolve_import_file_parser_id(requested_parser: &str, filename: &str, content: &str) -> String {
    let requested_parser = requested_parser.trim().to_ascii_lowercase();
    if !requested_parser.is_empty() && requested_parser != "auto" {
        return requested_parser;
    }
    let probe = format!("{} {}", filename.to_ascii_lowercase(), content);
    for (keyword, parser_id) in [
        ("微信", "wechat"),
        ("wechat", "wechat"),
        ("支付宝", "alipay"),
        ("alipay", "alipay"),
        ("农业银行", "abc"),
        ("abc", "abc"),
        ("工商银行", "icbc"),
        ("icbc", "icbc"),
        ("建设银行", "ccb"),
        ("ccb", "ccb"),
        ("招商银行", "cmb"),
        ("cmb", "cmb"),
        ("民生银行", "cmbc"),
        ("cmbc", "cmbc"),
    ] {
        if probe.contains(keyword) {
            return parser_id.to_string();
        }
    }
    "rust-import".to_string()
}

fn parse_standard_bills_from_csv_text(
    text: &str,
    parser_id: &str,
    require_known_headers: bool,
) -> ParsedCsvBills {
    let Some((start_line, delimiter)) = detect_csv_table_start(text, require_known_headers) else {
        return ParsedCsvBills::default();
    };
    let csv_text = text.lines().skip(start_line).collect::<Vec<_>>().join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(csv_text.as_bytes());
    let headers = match reader.headers() {
        Ok(headers) => headers.clone(),
        Err(_) => return ParsedCsvBills::default(),
    };
    if require_known_headers && !looks_like_import_headers(headers.iter()) {
        return ParsedCsvBills::default();
    }
    let mut raw_bills = Vec::new();
    for record in reader.records().filter_map(Result::ok) {
        let mut raw_bill = RawBill::default();
        for (index, header) in headers.iter().enumerate() {
            let Some(value) = record.get(index) else {
                continue;
            };
            apply_csv_header_to_raw_bill(&mut raw_bill, header, value);
        }
        if !raw_bill.date.trim().is_empty()
            || !raw_bill.trade_time.trim().is_empty()
            || !raw_bill.amount.trim().is_empty()
        {
            raw_bills.push(raw_bill);
        }
    }
    ParsedCsvBills {
        bills: post_process_raw_bills(parser_id, &raw_bills),
    }
}

fn detect_csv_table_start(text: &str, require_known_headers: bool) -> Option<(usize, char)> {
    for (index, line) in text.lines().enumerate() {
        let Some(delimiter) = detect_delimiter_for_line(line, None) else {
            continue;
        };
        let headers = split_preview_line(line, delimiter);
        if require_known_headers {
            if looks_like_import_headers(headers.iter().map(String::as_str)) {
                return Some((index, delimiter));
            }
        } else if headers.len() > 1 {
            return Some((index, delimiter));
        }
    }
    None
}

fn looks_like_import_headers<'a>(headers: impl Iterator<Item = &'a str>) -> bool {
    let headers = headers.map(normalized_header_key).collect::<Vec<_>>();
    let has_date = headers.iter().any(|header| {
        header.contains("交易时间")
            || header.contains("交易日期")
            || header.contains("记账日期")
            || header == "date"
            || header == "time"
            || header.contains("trade_time")
    });
    let has_amount = headers.iter().any(|header| {
        header.contains("金额") || header == "amount" || header.contains("source_amount")
    });
    has_date && has_amount
}

fn apply_csv_header_to_raw_bill(raw_bill: &mut RawBill, header: &str, value: &str) {
    let header = normalized_header_key(header);
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if header.contains("交易时间")
        || header.contains("交易日期")
        || header.contains("记账日期")
        || header == "date"
        || header == "time"
        || header.contains("trade_time")
    {
        raw_bill.trade_time = value.to_string();
    } else if header.contains("收支类型")
        || header.contains("收/支")
        || header.contains("交易类型")
        || header == "type"
        || header.contains("trade_type")
        || header.contains("transaction_type")
    {
        raw_bill.transaction_type = value.to_string();
    } else if header.contains("金额") || header == "amount" || header.contains("source_amount") {
        raw_bill.amount = value.to_string();
    } else if header.contains("商品") || header.contains("商品说明") || header == "goods" {
        raw_bill.goods = value.to_string();
    } else if header.contains("备注") || header.contains("说明") || header == "remark" {
        raw_bill.remark = value.to_string();
    } else if header.contains("摘要") || header == "summary" || header == "abstract" {
        raw_bill.summary = value.to_string();
    } else if header.contains("交易对方") || header.contains("对方") || header == "counterparty"
    {
        raw_bill.counterparty = value.to_string();
    } else if header.contains("商户") || header == "merchant" {
        raw_bill.merchant = value.to_string();
    } else if header.contains("店铺") || header == "shop" {
        raw_bill.shop = value.to_string();
    } else if header.contains("支付方式")
        || header.contains("收/付款方式")
        || header == "payment_method"
        || header == "account"
    {
        raw_bill.payment_method = value.to_string();
    } else if header.contains("交易分类") || header == "category" {
        raw_bill.original_category = value.to_string();
    } else if header.contains("交易单号") || header == "transaction_id" || header == "order_id"
    {
        raw_bill.transaction_id = value.to_string();
    } else if header.contains("商家订单号") || header == "merchant_id" {
        raw_bill.merchant_id = value.to_string();
    } else if header.contains("状态") || header == "status" {
        raw_bill.status = value.to_string();
    } else if raw_bill.description.is_empty() {
        raw_bill.description = value.to_string();
    }
}

fn normalized_header_key(header: &str) -> String {
    header
        .trim_matches(|ch: char| ch == '\u{feff}' || ch == '"' || ch == '\'')
        .trim()
        .replace([' ', '\t', '\r', '\n'], "")
        .to_ascii_lowercase()
}

fn standard_bills_from_temp_path_payload(
    object: &Map<String, Value>,
    temp_path: &str,
    user_id: UserId,
    session_id: &str,
) -> Result<Vec<StandardBill>, ImportV2RouteResponse> {
    let path = validate_import_temp_path(temp_path, user_id, Some(session_id))?;
    let text = read_import_temp_text(&path)?;
    standard_bills_from_column_mapped_text(object, &text)
}

fn standard_bills_from_column_mapped_text(
    object: &Map<String, Value>,
    text: &str,
) -> Result<Vec<StandardBill>, ImportV2RouteResponse> {
    let delimiter = first_text_from_object(object, &["delimiter"]);
    let mut rows = csv_rows_from_text(text, delimiter.as_deref())?;
    let column_mapping = column_mapping_from_payload(object)?;
    if !column_mapping.contains_key(&1) || !column_mapping.contains_key(&8) {
        return Err(import_v2_error_response(
            400,
            "Column mapping requires transaction time and amount columns",
        ));
    }
    let type_mapping = transaction_type_mapping_from_payload(object);
    let has_header =
        bool_field_from_object(object, &["has_header_line", "hasHeaderLine"]).unwrap_or(false);
    if has_header {
        if let Some(header_index) = rows
            .iter()
            .position(|row| looks_like_import_headers(row.iter().map(String::as_str)))
        {
            rows = rows.split_off(header_index);
        }
    }
    let mut bills = Vec::new();
    for row in rows.iter().skip(has_header as usize) {
        let Some((raw_bill, main_category, sub_category)) =
            raw_bill_from_column_mapped_row(row, &column_mapping, &type_mapping)
        else {
            continue;
        };
        let mut parsed = post_process_raw_bills("generic", &[raw_bill]);
        for bill in &mut parsed {
            bill.main_category = main_category.clone();
            bill.sub_category = sub_category.clone();
        }
        bills.extend(parsed);
    }
    Ok(bills)
}

fn column_mapping_from_payload(
    object: &Map<String, Value>,
) -> Result<HashMap<i64, usize>, ImportV2RouteResponse> {
    let Some(mapping) = first_value(
        object,
        &["column_mapping", "columnMapping", "dataColumnMapping"],
    ) else {
        return Err(import_v2_error_response(400, "Missing column_mapping"));
    };
    let mapping_value = value_or_json_object(mapping)?;
    let mut result = HashMap::new();
    for (key, value) in &mapping_value {
        let Some(column_type) = key.parse::<i64>().ok().filter(|value| *value > 0) else {
            continue;
        };
        let Some(column_index) = value_to_i64(value)
            .filter(|value| *value >= 0)
            .and_then(|value| usize::try_from(value).ok())
        else {
            continue;
        };
        result.insert(column_type, column_index);
    }
    if result.is_empty() {
        return Err(import_v2_error_response(400, "Invalid column_mapping"));
    }
    Ok(result)
}

fn transaction_type_mapping_from_payload(object: &Map<String, Value>) -> HashMap<String, String> {
    let Some(mapping) = first_value(
        object,
        &["transaction_type_mapping", "transactionTypeMapping"],
    ) else {
        return HashMap::new();
    };
    let Ok(mapping_value) = value_or_json_object(mapping) else {
        return HashMap::new();
    };
    mapping_value
        .iter()
        .filter_map(|(source, target)| {
            let mapped = value_to_i64(target).and_then(transaction_type_label_from_i64)?;
            Some((source.trim().to_string(), mapped.to_string()))
        })
        .collect()
}

fn value_or_json_object(value: &Value) -> Result<Map<String, Value>, ImportV2RouteResponse> {
    if let Some(object) = value.as_object() {
        return Ok(object.clone());
    }
    if let Some(text) = value.as_str() {
        let parsed = serde_json::from_str::<Value>(text)
            .map_err(|_| import_v2_error_response(400, "Invalid request"))?;
        if let Some(object) = parsed.as_object() {
            return Ok(object.clone());
        }
    }
    Err(import_v2_error_response(400, "Invalid request"))
}

fn raw_bill_from_column_mapped_row(
    row: &[String],
    column_mapping: &HashMap<i64, usize>,
    type_mapping: &HashMap<String, String>,
) -> Option<(RawBill, String, String)> {
    let mut raw_bill = RawBill {
        trade_time: mapped_column_text(row, column_mapping, 1),
        transaction_type: mapped_column_text(row, column_mapping, 3),
        amount: mapped_column_text(row, column_mapping, 8),
        payment_method: mapped_column_text(row, column_mapping, 6),
        description: mapped_column_text(row, column_mapping, 14),
        original_category: mapped_column_text(row, column_mapping, 4),
        ..RawBill::default()
    };
    if let Some(mapped_type) = type_mapping.get(raw_bill.transaction_type.trim()) {
        raw_bill.transaction_type = mapped_type.clone();
    }
    if raw_bill.description.trim().is_empty() {
        raw_bill.description = first_non_empty_text([
            mapped_column_text(row, column_mapping, 4),
            mapped_column_text(row, column_mapping, 5),
            raw_bill.payment_method.clone(),
        ]);
    }
    if raw_bill.trade_time.trim().is_empty() || raw_bill.amount.trim().is_empty() {
        return None;
    }
    let main_category = mapped_column_text(row, column_mapping, 4);
    let sub_category = mapped_column_text(row, column_mapping, 5);
    Some((raw_bill, main_category, sub_category))
}

fn mapped_column_text(
    row: &[String],
    column_mapping: &HashMap<i64, usize>,
    column_type: i64,
) -> String {
    column_mapping
        .get(&column_type)
        .and_then(|index| row.get(*index))
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn transaction_type_label_from_i64(value: i64) -> Option<&'static str> {
    match value {
        2 => Some("收入"),
        3 => Some("支出"),
        4 => Some("转账"),
        5 => Some("投资"),
        _ => None,
    }
}

fn first_non_empty_text(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn normalize_config_text(value: &str) -> String {
    value
        .trim()
        .replace([' ', '\t', '\r', '\n'], "")
        .to_ascii_lowercase()
}

fn config_text(value: &Value, keys: &[&str]) -> Option<String> {
    value
        .as_object()
        .and_then(|object| config_text_from_object(object, keys))
}

fn config_text_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    first_value(object, keys)
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn config_id_value(config: &Value) -> Option<i64> {
    config.as_object().and_then(|object| {
        first_value(object, &["id", "configId", "config_id"])
            .and_then(value_to_i64)
            .filter(|value| *value > 0)
    })
}

fn bool_value(config: &Value, key: &str) -> Option<bool> {
    config
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(value_to_bool)
}

fn value_to_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn string_array_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Option<Vec<String>> {
    first_value(object, keys).and_then(|value| match value {
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(value_to_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>(),
        ),
        Value::String(text) => Some(
            text.split([',', '|'])
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>(),
        ),
        _ => None,
    })
}

fn config_string_array(config: &Value, keys: &[&str]) -> Option<Vec<String>> {
    config
        .as_object()
        .and_then(|object| string_array_field_from_object(object, keys))
}

fn header_signature_from_headers(headers: &[String]) -> String {
    headers
        .iter()
        .map(|header| header.trim())
        .filter(|header| !header.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}

fn normalize_learning_match_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

fn legacy_bill_type(object: &Map<String, Value>) -> String {
    first_value(object, &["type", "transaction_type", "transactionType"])
        .and_then(value_to_preview_type_text)
        .unwrap_or_else(|| "支出".to_string())
}

fn legacy_bill_amount(object: &Map<String, Value>) -> Option<f64> {
    first_value(
        object,
        &[
            "sourceAmount",
            "source_amount",
            "destinationAmount",
            "destination_amount",
        ],
    )
    .and_then(value_to_f64)
    .map(|value| value / 100.0)
    .or_else(|| first_value(object, &["amount"]).and_then(value_to_f64))
}

fn legacy_confirm_amount_for_type(bill_type: &str, amount: f64) -> f64 {
    let amount = amount.abs();
    if matches!(bill_type.trim(), "支出" | "expense") {
        -amount
    } else {
        amount
    }
}

fn legacy_bill_date(object: &Map<String, Value>) -> String {
    if let Some(date) = first_text_from_object(
        object,
        &["timeText", "date", "transaction_time", "tradeTime"],
    ) {
        return date.split_whitespace().next().unwrap_or(&date).to_string();
    }
    if let Some(timestamp) = first_value(object, &["time"]).and_then(value_to_i64) {
        if let Some(datetime) = chrono::DateTime::from_timestamp(timestamp, 0) {
            return datetime.date_naive().format("%Y-%m-%d").to_string();
        }
    }
    Utc::now().date_naive().format("%Y-%m-%d").to_string()
}

fn user_id_i64_value(user_id: UserId) -> Result<i64, ImportV2RouteResponse> {
    i64::try_from(user_id.get())
        .map_err(|_| import_v2_error_response(400, "user id exceeds sqlite integer range"))
}

fn import_preview_request_from_body(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<ImportPreviewRequest, ImportV2RouteResponse> {
    let content_type = content_type_from_headers(headers);
    let content_type_lower = content_type.to_ascii_lowercase();
    if content_type_lower.contains("multipart/form-data") {
        let form = parse_multipart_form_data(&content_type, body)?;
        return Ok(ImportPreviewRequest {
            temp_path: form.text_value(&["temp_path", "tempPath"]),
            uploaded_file: form.file_parts().first().map(|part| part.body.clone()),
            delimiter: form.text_value(&["delimiter"]),
        });
    }
    if content_type_lower.contains("application/json") {
        let payload = serde_json::from_slice::<Value>(body)
            .map_err(|_| import_v2_error_response(400, "Invalid JSON request"))?;
        let object = payload_object(&payload)?;
        return Ok(ImportPreviewRequest {
            temp_path: first_text_from_object(object, &["temp_path", "tempPath"]),
            uploaded_file: None,
            delimiter: first_text_from_object(object, &["delimiter"]),
        });
    }
    let form = parse_urlencoded_form(body);
    Ok(ImportPreviewRequest {
        temp_path: form
            .get("temp_path")
            .or_else(|| form.get("tempPath"))
            .cloned(),
        uploaded_file: None,
        delimiter: form.get("delimiter").cloned(),
    })
}

fn parse_legacy_import_file(
    form: &MultipartForm,
    filename: &str,
    body: &[u8],
    _user_id: UserId,
) -> Result<LegacyImportParseResult, ImportV2RouteResponse> {
    let object = multipart_text_object(form);
    let requested_file_type = form
        .text_value(&[
            "fileType",
            "file_type",
            "parser_type",
            "parserType",
            "parser_id",
            "parserId",
        ])
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let force_generic = matches!(
        requested_file_type.as_str(),
        "generic" | "csv" | "xlsx" | "xls" | "txt"
    );
    let use_column_mapping = first_value(&object, &["columnMapping", "column_mapping"])
        .is_some_and(mapping_value_is_present);
    let dedicated_parsed = if force_generic || use_column_mapping {
        None
    } else {
        parse_dedicated_import_bytes(filename, body, &requested_file_type)
    };
    let text = decode_import_text(body);
    let detected_parser_type = dedicated_parsed
        .as_ref()
        .map(|parsed| parsed.parser_id.clone())
        .unwrap_or_else(|| {
            resolve_import_file_parser_id("auto", filename, &text).replace("rust-import", "")
        });
    let (parser_type, bills) = if force_generic || use_column_mapping {
        let bills = if use_column_mapping {
            standard_bills_from_column_mapped_text(&object, &text)?
        } else {
            parse_standard_bills_from_csv_text(&text, "generic", true).bills
        };
        ("generic".to_string(), bills)
    } else if let Some(parsed) = dedicated_parsed {
        (parsed.parser_id, parsed.bills)
    } else {
        let parser_id = if requested_file_type == "auto" {
            if detected_parser_type.is_empty() {
                "rust-import".to_string()
            } else {
                detected_parser_type.clone()
            }
        } else {
            requested_file_type
        };
        let parsed = parse_standard_bills_from_csv_text(&text, &parser_id, true);
        (parser_id, parsed.bills)
    };
    if bills.is_empty() {
        return Err(import_v2_error_response(400, "No valid bills to parse"));
    }
    let items = bills
        .iter()
        .map(|bill| legacy_import_item_from_standard_bill(bill, &parser_type))
        .collect::<Vec<_>>();
    Ok(LegacyImportParseResult {
        total_count: items.len(),
        items,
        parser_type,
        detected_parser_type,
    })
}

fn multipart_text_object(form: &MultipartForm) -> Map<String, Value> {
    let mut object = Map::new();
    for part in &form.parts {
        if part.filename.is_some() {
            continue;
        }
        let text = decode_import_text(&part.body).trim().to_string();
        if text.is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
        object.insert(part.name.clone(), value);
    }
    object
}

fn mapping_value_is_present(value: &Value) -> bool {
    match value {
        Value::Object(object) => !object.is_empty(),
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| value.as_object().map(|object| !object.is_empty()))
            .unwrap_or_else(|| !text.trim().is_empty()),
        _ => false,
    }
}

fn legacy_import_item_from_standard_bill(bill: &StandardBill, parser_id: &str) -> Value {
    let frontend_type = frontend_type_from_bill_type(&bill.transaction_type);
    let source_amount = bill.amount.to_cents().saturating_abs();
    let amount = bill
        .amount
        .to_yuan_string()
        .parse::<f64>()
        .unwrap_or_default()
        .abs();
    let original_category = original_category_name(&bill.main_category, &bill.sub_category)
        .unwrap_or_else(|| bill.original_category.clone());
    json!({
        "type": frontend_type,
        "categoryId": "",
        "originalCategoryName": original_category,
        "time": legacy_import_time_seconds(&bill.date),
        "utcOffset": 0,
        "sourceAccountId": positive_id_text(&bill.source_account_id),
        "originalSourceAccountName": bill.payment_method,
        "originalSourceAccountCurrency": "CNY",
        "destinationAccountId": "",
        "originalDestinationAccountName": "",
        "originalDestinationAccountCurrency": "CNY",
        "sourceAmount": source_amount,
        "destinationAmount": 0,
        "tagIds": [],
        "originalTagNames": [],
        "comment": legacy_comment_text(&bill.description),
        "counterparty": bill.counterparty,
        "paymentMethod": bill.payment_method,
        "timeText": bill.date,
        "categoryName": bill.main_category,
        "subCategoryName": bill.sub_category,
        "accountName": "",
        "amount": amount,
        "description": legacy_comment_text(&bill.description),
        "parserSource": parser_id,
        "parserTags": bill.parser_tags,
        "isManuallyAnnotated": false,
    })
}

fn legacy_comment_text(description: &str) -> String {
    description
        .split('|')
        .next()
        .unwrap_or(description)
        .trim()
        .to_string()
}

fn legacy_reclassify_passthrough(index: usize, transaction: &Value) -> Value {
    let object = transaction.as_object();
    json!({
        "index": index,
        "mainCategory": object
            .and_then(|item| first_value(item, &["mainCategory", "main_category", "categoryName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "subCategory": object
            .and_then(|item| first_value(item, &["subCategory", "sub_category", "subCategoryName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "categoryId": object
            .and_then(|item| first_value(item, &["categoryId", "category_id"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "categoryName": object
            .and_then(|item| first_value(item, &["categoryName", "originalCategoryName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "sourceAccountId": object
            .and_then(|item| first_value(item, &["sourceAccountId", "source_account_id"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "sourceAccountName": object
            .and_then(|item| first_value(item, &["accountName", "sourceAccountName", "originalSourceAccountName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "destinationAccountId": object
            .and_then(|item| first_value(item, &["destinationAccountId", "destination_account_id"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "destinationAccountName": object
            .and_then(|item| first_value(item, &["destinationAccountName", "originalDestinationAccountName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })
}

fn frontend_type_from_bill_type(raw_type: &str) -> i64 {
    match raw_type.trim() {
        "余额调整" => 1,
        "收入" | "退款" => 2,
        "转账" => 4,
        "投资" => 5,
        _ => 3,
    }
}

fn original_category_name(main_category: &str, sub_category: &str) -> Option<String> {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if main_category.is_empty() && sub_category.is_empty() {
        None
    } else if sub_category.is_empty() {
        Some(main_category.to_string())
    } else if main_category.is_empty() {
        Some(sub_category.to_string())
    } else {
        Some(format!("{main_category}-{sub_category}"))
    }
}

fn positive_id_text(raw_value: &str) -> String {
    raw_value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn legacy_import_time_seconds(raw_value: &str) -> i64 {
    let text = raw_value.trim();
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(text, format) {
            return datetime.and_utc().timestamp();
        }
    }
    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(text, format) {
            if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                return datetime.and_utc().timestamp();
            }
        }
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn legacy_import_parsers() -> Value {
    json!([
        {
            "id": "auto",
            "name": "自动识别",
            "description": "自动检测文件类型并选择合适的解析器",
            "supported_formats": ["csv"],
        },
        {
            "id": "wechat",
            "name": "微信支付",
            "description": "解析微信支付账单CSV文件",
            "supported_formats": ["csv"],
        },
        {
            "id": "alipay",
            "name": "支付宝",
            "description": "解析支付宝交易明细CSV文件",
            "supported_formats": ["csv"],
        },
        {
            "id": "icbc",
            "name": "工商银行",
            "description": "解析工商银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
        {
            "id": "cmbc",
            "name": "民生银行",
            "description": "解析民生银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
        {
            "id": "abc",
            "name": "农业银行",
            "description": "解析农业银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
        {
            "id": "ccb",
            "name": "建设银行",
            "description": "解析建设银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
    ])
}

fn init_import_config_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_app_settings_schema(runtime.connection()).map_err(db_error_response)
}

fn import_config_setting_key(user_id: UserId) -> String {
    format!("import_configs_user_{}", user_id.get())
}

fn load_import_configs(
    connection: &Connection,
    user_id: UserId,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let stored = get_app_setting(connection, &import_config_setting_key(user_id))
        .map_err(db_error_response)?;
    let Some(stored) = stored else {
        return Ok(Vec::new());
    };
    let parsed = serde_json::from_str::<Value>(&stored).unwrap_or_else(|_| Value::Array(vec![]));
    let configs = parsed
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(configs)
}

fn store_import_configs(
    connection: &Connection,
    user_id: UserId,
    configs: &[Value],
) -> Result<(), ImportV2RouteResponse> {
    set_app_setting(
        connection,
        &AppSettingDraft {
            key: import_config_setting_key(user_id),
            value: Value::Array(configs.to_vec()).to_string(),
            value_type: "json".to_string(),
            description: Some("User import column mapping templates".to_string()),
            is_encrypted: false,
        },
    )
    .map(|_| ())
    .map_err(db_error_response)
}

fn build_import_config_from_payload(object: &Map<String, Value>, config_id: i64) -> Value {
    let now = now_text();
    let field_mappings = first_value(object, &["fieldMappings", "field_mappings"])
        .cloned()
        .unwrap_or_else(|| json!({}));
    let file_format = config_text_from_object(object, &["fileFormat", "file_format"])
        .unwrap_or_else(|| "csv".to_string());
    let sample_headers =
        string_array_field_from_object(object, &["sampleHeaders", "sample_headers", "headers"])
            .unwrap_or_default();
    let header_signature =
        config_text_from_object(object, &["headerSignature", "header_signature"])
            .unwrap_or_else(|| header_signature_from_headers(&sample_headers));
    json!({
        "id": config_id,
        "name": config_text_from_object(object, &["name"]).unwrap_or_default(),
        "fileFormat": file_format,
        "description": config_text_from_object(object, &["description"]),
        "descriptionSummary": config_text_from_object(object, &["descriptionSummary", "description_summary"]),
        "fieldMappings": field_mappings,
        "columnMapping": first_value(object, &["columnMapping", "column_mapping"]).cloned().unwrap_or_else(|| first_value(object, &["fieldMappings", "field_mappings"]).cloned().unwrap_or_else(|| json!({}))),
        "transactionTypeMapping": first_value(object, &["transactionTypeMapping", "transaction_type_mapping"]).cloned().unwrap_or_else(|| json!({
            "收入": 2,
            "支出": 3,
            "转账": 4,
            "投资": 5,
        })),
        "dateFormat": config_text_from_object(object, &["dateFormat", "date_format"]).unwrap_or_else(|| "%Y-%m-%d %H:%M:%S".to_string()),
        "encoding": config_text_from_object(object, &["encoding"]).unwrap_or_else(|| "utf-8".to_string()),
        "delimiter": config_text_from_object(object, &["delimiter"]).unwrap_or_else(|| ",".to_string()),
        "skipRows": first_value(object, &["skipRows", "skip_rows"]).and_then(value_to_i64).unwrap_or(0),
        "hasHeader": first_value(object, &["hasHeader", "has_header"]).and_then(value_to_bool).unwrap_or(true),
        "customRules": first_value(object, &["customRules", "custom_rules"]).cloned().unwrap_or_else(|| json!([])),
        "sampleHeaders": sample_headers,
        "headerSignature": header_signature,
        "isDefault": first_value(object, &["isDefault", "is_default"]).and_then(value_to_bool).unwrap_or(false),
        "defaultRecommendation": first_value(object, &["defaultRecommendation", "default_recommendation"]).and_then(value_to_bool).unwrap_or(false),
        "useCount": first_value(object, &["useCount", "use_count"]).and_then(value_to_i64).unwrap_or(0),
        "lastUsedAt": config_text_from_object(object, &["lastUsedAt", "last_used_at"]),
        "createdAt": config_text_from_object(object, &["createdAt", "created_at"]).unwrap_or_else(|| now.clone()),
        "updatedAt": now,
    })
}

fn best_import_config_match(
    configs: &[Value],
    file_format: &str,
    headers: &[String],
) -> Option<Value> {
    let requested_format = normalize_config_text(file_format);
    let normalized_headers = headers
        .iter()
        .map(|header| normalize_config_text(header))
        .filter(|header| !header.is_empty())
        .collect::<Vec<_>>();
    if normalized_headers.is_empty() {
        return None;
    }
    let mut best: Option<(Value, usize, f64)> = None;
    for config in configs {
        let config_format = config_text(config, &["fileFormat", "file_format"])
            .map(|value| normalize_config_text(&value))
            .unwrap_or_default();
        if config_format != requested_format {
            continue;
        }
        let sample_headers = config_string_array(config, &["sampleHeaders", "sample_headers"])
            .or_else(|| {
                config_text(config, &["headerSignature", "header_signature"]).map(|signature| {
                    signature
                        .split('|')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        let sample_headers = sample_headers
            .iter()
            .map(|header| normalize_config_text(header))
            .collect::<Vec<_>>();
        let matched = normalized_headers
            .iter()
            .filter(|header| sample_headers.contains(header))
            .count();
        let score = matched as f64 / normalized_headers.len() as f64;
        if matched == 0 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, best_matched, best_score)| {
                matched > *best_matched || (matched == *best_matched && score > *best_score)
            })
            .unwrap_or(true)
        {
            best = Some((config.clone(), matched, score));
        }
    }
    best.map(|(mut config, matched, score)| {
        if let Some(object) = config.as_object_mut() {
            object.insert("matchScore".to_string(), json!(score));
            object.insert("matchedHeaderCount".to_string(), json!(matched));
            object.insert(
                "matchReason".to_string(),
                json!(format!("Matched {matched} import headers")),
            );
            object.insert("defaultRecommendation".to_string(), json!(score >= 0.5));
        }
        config
    })
}

fn build_import_config_suggestion(
    file_format: &str,
    headers: &[String],
    matched: Option<&Value>,
) -> Value {
    let mut field_mappings = Map::new();
    for (index, header) in headers.iter().enumerate() {
        if let Some(column_type) = import_config_column_type_for_header(header) {
            field_mappings.insert(column_type.to_string(), json!(index));
        }
    }
    let confidence = if field_mappings.contains_key("1") && field_mappings.contains_key("8") {
        0.8
    } else {
        0.45
    };
    let column_mapping = field_mappings.clone();
    json!({
        "fileFormat": file_format,
        "fieldMappings": field_mappings,
        "columnMapping": column_mapping,
        "transactionTypeMapping": {
            "收入": 2,
            "支出": 3,
            "转账": 4,
            "投资": 5,
        },
        "hasHeader": true,
        "encoding": "utf-8",
        "delimiter": ",",
        "confidence": confidence,
        "matchedConfig": matched.cloned(),
        "defaultRecommendation": matched.is_some(),
    })
}

fn import_config_column_type_for_header(header: &str) -> Option<i64> {
    let header = normalize_config_text(header);
    if header.contains("交易时间")
        || header.contains("交易日期")
        || header.contains("记账日期")
        || header == "date"
        || header == "time"
        || header.contains("trade")
    {
        Some(1)
    } else if header.contains("收支类型") || header.contains("交易类型") || header.contains("type")
    {
        Some(3)
    } else if header.contains("分类") || header.contains("category") {
        Some(4)
    } else if header.contains("账户")
        || header.contains("支付方式")
        || header.contains("付款方式")
        || header.contains("account")
        || header.contains("payment")
    {
        Some(6)
    } else if header.contains("金额") || header.contains("amount") {
        Some(8)
    } else if header.contains("备注")
        || header.contains("说明")
        || header.contains("摘要")
        || header.contains("商品")
        || header.contains("商户")
        || header.contains("对方")
        || header.contains("remark")
        || header.contains("description")
        || header.contains("merchant")
        || header.contains("counterparty")
    {
        Some(14)
    } else {
        None
    }
}

fn init_import_learning_runtime_schema(
    runtime: &SqliteRuntime,
) -> Result<(), ImportV2RouteResponse> {
    init_import_runtime_schema(runtime)?;
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_learning_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                learned_type TEXT,
                learned_category_id INTEGER,
                learned_source_account_id INTEGER,
                learned_destination_account_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                source_session_id TEXT,
                source_preview_id INTEGER,
                parser_id TEXT,
                composite_match_hash TEXT,
                match_features_json TEXT,
                applied_count INTEGER NOT NULL DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_rules_user_enabled
                ON import_learning_rules(user_id, enabled);
            CREATE TABLE IF NOT EXISTS import_learning_rule_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                rule_id INTEGER,
                session_id TEXT,
                preview_id INTEGER,
                action TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            ",
        )
        .map_err(db_error_response)
}

fn init_global_learning_runtime_schema(
    runtime: &SqliteRuntime,
) -> Result<(), ImportV2RouteResponse> {
    init_import_learning_runtime_schema(runtime)?;
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_learning_corpus_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                session_id TEXT NOT NULL,
                preview_id INTEGER NOT NULL,
                parser_id TEXT,
                counterparty TEXT,
                description TEXT,
                payment_method TEXT,
                composite_match_hash TEXT,
                match_features_json TEXT,
                annotated_type TEXT,
                annotated_category_id INTEGER,
                annotated_source_account_id INTEGER,
                annotated_destination_account_id INTEGER,
                source_snapshot_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, session_id, preview_id)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_user
                ON import_learning_corpus_samples(user_id, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_hash
                ON import_learning_corpus_samples(user_id, composite_match_hash);
            CREATE TABLE IF NOT EXISTS import_learning_feedback_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                event_type TEXT NOT NULL,
                rule_id INTEGER,
                suggestion_id INTEGER,
                session_id TEXT,
                preview_id INTEGER,
                bill_id INTEGER,
                candidate_id TEXT,
                payload_json TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_user
                ON import_learning_feedback_events(user_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_type
                ON import_learning_feedback_events(user_id, event_type);
            CREATE TABLE IF NOT EXISTS import_learning_concept_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                concept_key TEXT NOT NULL,
                concept_type TEXT NOT NULL,
                sample_count INTEGER NOT NULL DEFAULT 0,
                accepted_count INTEGER NOT NULL DEFAULT 0,
                rejected_count INTEGER NOT NULL DEFAULT 0,
                auto_applied_count INTEGER NOT NULL DEFAULT 0,
                rollback_count INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, concept_key, concept_type)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_concept_stats_user
                ON import_learning_concept_stats(user_id, concept_type);
            CREATE TABLE IF NOT EXISTS import_learning_suggestions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                composite_match_hash TEXT,
                match_features_json TEXT,
                suggested_type TEXT,
                suggested_category_id INTEGER,
                suggested_source_account_id INTEGER,
                suggested_destination_account_id INTEGER,
                sample_count INTEGER NOT NULL DEFAULT 1,
                source_session_ids_json TEXT,
                source_preview_ids_json TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                existing_rule_id INTEGER,
                summary TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            );
            CREATE INDEX IF NOT EXISTS idx_learning_suggestions_user_status
                ON import_learning_suggestions(user_id, status);
            ",
        )
        .map_err(db_error_response)?;
    for (table_name, column_name, definition) in [
        ("import_learning_rules", "parser_id", "TEXT"),
        ("import_learning_rules", "composite_match_hash", "TEXT"),
        ("import_learning_rules", "match_features_json", "TEXT"),
        ("import_learning_rule_logs", "match_type", "TEXT"),
        ("import_learning_rule_logs", "match_value", "TEXT"),
        (
            "import_learning_rule_logs",
            "normalized_match_value",
            "TEXT",
        ),
        ("import_learning_rule_logs", "payload_json", "TEXT"),
    ] {
        ensure_table_column(runtime.connection(), table_name, column_name, definition)?;
    }
    Ok(())
}

fn ensure_table_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<(), ImportV2RouteResponse> {
    if table_has_column(connection, table_name, column_name).map_err(db_error_response)? {
        return Ok(());
    }
    connection
        .execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"),
            [],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn annotation_samples_from_payload(payload: &Value) -> Vec<ImportAnnotationSampleDraft> {
    let Ok(items) = preview_update_items_from_payload(payload) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let preview_id = preview_id_from_payload(item).ok()?;
            Some(ImportAnnotationSampleDraft {
                preview_id,
                annotated_type: first_value(
                    item,
                    &[
                        "annotated_type",
                        "annotatedType",
                        "preview_type",
                        "previewType",
                        "type",
                    ],
                )
                .and_then(value_to_preview_type_text),
                annotated_category_id: first_value(
                    item,
                    &[
                        "annotated_category_id",
                        "annotatedCategoryId",
                        "category_id",
                        "categoryId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
                annotated_source_account_id: first_value(
                    item,
                    &[
                        "annotated_source_account_id",
                        "annotatedSourceAccountId",
                        "preview_source_account_id",
                        "previewSourceAccountId",
                        "sourceAccountId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
                annotated_destination_account_id: first_value(
                    item,
                    &[
                        "annotated_destination_account_id",
                        "annotatedDestinationAccountId",
                        "preview_destination_account_id",
                        "previewDestinationAccountId",
                        "destinationAccountId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
            })
        })
        .collect()
}

#[derive(Debug, Default)]
struct ImportLearningPromotionResult {
    rules_total: i64,
    created: i64,
    updated: i64,
}

fn promote_import_learning_rules(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    preview_ids: &[i64],
) -> Result<ImportLearningPromotionResult, ImportV2RouteResponse> {
    let user_id_i64 = user_id_i64_value(user_id)?;
    let selected_ids = preview_ids
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    if selected_ids.is_empty() {
        let rules_total = count_import_learning_rules(connection, user_id, None)?;
        return Ok(ImportLearningPromotionResult {
            rules_total,
            ..ImportLearningPromotionResult::default()
        });
    }
    let samples = get_import_annotation_samples(connection, session_id, user_id)
        .map_err(db_error_response)?
        .into_iter()
        .map(|sample| (sample.preview_id, sample))
        .collect::<BTreeMap<_, _>>();
    let mut result = ImportLearningPromotionResult::default();
    for preview_id in selected_ids {
        let Some(preview) =
            get_preview_bill_by_id(connection, preview_id, user_id).map_err(db_error_response)?
        else {
            continue;
        };
        if preview.session_id != session_id {
            continue;
        }
        let Some((features, composite_hash)) = composite_match_features_for_preview(&preview)
        else {
            continue;
        };
        let sample = samples.get(&preview_id);
        let learned_type = sample
            .and_then(|sample| sample.annotated_type.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| preview.preview_type.clone());
        let learned_category_id = sample.and_then(|sample| sample.annotated_category_id);
        let learned_source_account_id = sample
            .and_then(|sample| sample.annotated_source_account_id)
            .or(preview.preview_source_account_id);
        let learned_destination_account_id = sample
            .and_then(|sample| sample.annotated_destination_account_id)
            .or(preview.preview_destination_account_id);
        let now = now_text();
        let existing: Option<i64> = connection
            .query_row(
                "
                SELECT id FROM import_learning_rules
                WHERE user_id = ?1 AND match_type = 'composite' AND normalized_match_value = ?2
                ",
                params![user_id_i64, composite_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error_response)?;
        connection
            .execute(
                "
                INSERT INTO import_learning_rules (
                    user_id, match_type, match_value, normalized_match_value,
                    learned_type, learned_category_id, learned_source_account_id,
                    learned_destination_account_id, enabled, source_session_id,
                    source_preview_id, parser_id, composite_match_hash,
                    match_features_json, created_at, updated_at
                ) VALUES (?1, 'composite', ?2, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?2, ?10, ?11, ?11)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    learned_type = excluded.learned_type,
                    learned_category_id = excluded.learned_category_id,
                    learned_source_account_id = excluded.learned_source_account_id,
                    learned_destination_account_id = excluded.learned_destination_account_id,
                    source_session_id = excluded.source_session_id,
                    source_preview_id = excluded.source_preview_id,
                    parser_id = excluded.parser_id,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    updated_at = excluded.updated_at
                ",
                params![
                    user_id_i64,
                    composite_hash,
                    learned_type,
                    learned_category_id,
                    learned_source_account_id,
                    learned_destination_account_id,
                    session_id,
                    preview_id,
                    preview.preview_parser_id,
                    Value::Object(features).to_string(),
                    now,
                ],
            )
            .map_err(db_error_response)?;
        let rule_id = match existing {
            Some(rule_id) => {
                result.updated += 1;
                rule_id
            }
            None => {
                result.created += 1;
                connection.last_insert_rowid()
            }
        };
        connection
            .execute(
                "
                INSERT INTO import_learning_rule_logs (
                    user_id, rule_id, session_id, preview_id, action, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    user_id_i64,
                    rule_id,
                    session_id,
                    preview_id,
                    if existing.is_some() {
                        "updated"
                    } else {
                        "created"
                    },
                    now,
                ],
            )
            .map_err(db_error_response)?;
    }
    result.rules_total = count_import_learning_rules(connection, user_id, None)?;
    Ok(result)
}

fn composite_match_features_for_preview(
    preview: &ImportPreviewRow,
) -> Option<(Map<String, Value>, String)> {
    let mut values = BTreeMap::new();
    for (key, raw_value) in [
        ("counterparty", preview.preview_counterparty.as_str()),
        ("description", preview.preview_description.as_str()),
        ("parser_id", preview.preview_parser_id.as_str()),
        ("payment_method", preview.preview_payment_method.as_str()),
    ] {
        let normalized = normalize_learning_match_value(raw_value);
        if !normalized.is_empty() {
            values.insert(key, normalized);
        }
    }
    if values.len() < 2 {
        return None;
    }
    let mut features = Map::new();
    let hash = values
        .iter()
        .map(|(key, value)| {
            features.insert((*key).to_string(), json!(value));
            let alias = match *key {
                "counterparty" => "c",
                "description" => "d",
                "payment_method" => "m",
                "parser_id" => "p",
                _ => key,
            };
            format!("{alias}={value}")
        })
        .collect::<Vec<_>>()
        .join("|");
    Some((features, hash))
}

fn count_import_learning_rules(
    connection: &Connection,
    user_id: UserId,
    enabled_only: Option<bool>,
) -> Result<i64, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    let sql = if enabled_only == Some(true) {
        "SELECT COUNT(*) FROM import_learning_rules WHERE user_id = ?1 AND enabled = 1"
    } else {
        "SELECT COUNT(*) FROM import_learning_rules WHERE user_id = ?1"
    };
    connection
        .query_row(sql, params![user_id], |row| row.get(0))
        .map_err(db_error_response)
}

fn load_import_learning_rules(
    connection: &Connection,
    user_id: UserId,
    enabled_only: Option<bool>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    let sql = if enabled_only == Some(true) {
        "
        SELECT * FROM import_learning_rules
        WHERE user_id = ?1 AND enabled = 1
        ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3
        "
    } else {
        "
        SELECT * FROM import_learning_rules
        WHERE user_id = ?1
        ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3
        "
    };
    let mut statement = connection.prepare(sql).map_err(db_error_response)?;
    let rows = statement
        .query_map(
            params![user_id, usize_to_i64(limit), usize_to_i64(offset)],
            learning_rule_row_to_value,
        )
        .map_err(db_error_response)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)
}

fn get_import_learning_rule(
    connection: &Connection,
    rule_id: i64,
    user_id: UserId,
) -> Result<Option<Value>, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    connection
        .query_row(
            "SELECT * FROM import_learning_rules WHERE id = ?1 AND user_id = ?2",
            params![rule_id, user_id],
            learning_rule_row_to_value,
        )
        .optional()
        .map_err(db_error_response)
}

fn update_import_learning_rule(
    connection: &Connection,
    rule_id: i64,
    user_id: UserId,
    object: &Map<String, Value>,
) -> Result<(), ImportV2RouteResponse> {
    let Some(existing) = get_import_learning_rule(connection, rule_id, user_id)? else {
        return Err(import_v2_error_response(404, "Rule not found"));
    };
    let has_edit = first_value(object, &["matchValue", "match_value"]).is_some()
        || first_value(object, &["learnedType", "learned_type"]).is_some()
        || first_value(object, &["learnedCategoryId", "learned_category_id"]).is_some()
        || first_value(object, &["enabled"]).is_some();
    if !has_edit {
        return Err(import_v2_error_response(
            400,
            "No editable rule fields provided",
        ));
    }
    let match_value = first_text_from_object(object, &["matchValue", "match_value"])
        .unwrap_or_else(|| {
            existing["matchValue"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        });
    let normalized_match_value = normalize_learning_match_value(&match_value);
    let learned_type = if first_value(object, &["learnedType", "learned_type"]).is_some() {
        first_text_from_object(object, &["learnedType", "learned_type"])
    } else {
        existing["learnedType"].as_str().map(str::to_string)
    };
    let learned_category_id =
        if first_value(object, &["learnedCategoryId", "learned_category_id"]).is_some() {
            first_value(object, &["learnedCategoryId", "learned_category_id"])
                .and_then(value_to_i64)
                .filter(|value| *value > 0)
        } else {
            existing["learnedCategoryId"].as_i64()
        };
    let enabled = if first_value(object, &["enabled"]).is_some() {
        first_value(object, &["enabled"])
            .and_then(value_to_bool)
            .unwrap_or(true)
    } else {
        existing["enabled"].as_bool().unwrap_or(true)
    };
    let user_id_i64 = user_id_i64_value(user_id)?;
    let changed = connection
        .execute(
            "
            UPDATE import_learning_rules
            SET match_value = ?1,
                normalized_match_value = ?2,
                learned_type = ?3,
                learned_category_id = ?4,
                enabled = ?5,
                updated_at = ?6
            WHERE id = ?7 AND user_id = ?8
            ",
            params![
                match_value,
                normalized_match_value,
                learned_type,
                learned_category_id,
                enabled,
                now_text(),
                rule_id,
                user_id_i64,
            ],
        )
        .map_err(db_error_response)?;
    if changed == 0 {
        Err(import_v2_error_response(404, "Rule not found"))
    } else {
        Ok(())
    }
}

fn delete_import_learning_rule(
    connection: &Connection,
    rule_id: i64,
    user_id: UserId,
) -> Result<bool, ImportV2RouteResponse> {
    let changed = connection
        .execute(
            "DELETE FROM import_learning_rules WHERE id = ?1 AND user_id = ?2",
            params![rule_id, user_id_i64_value(user_id)?],
        )
        .map_err(db_error_response)?;
    Ok(changed > 0)
}

fn learning_rule_row_to_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let match_features: Option<String> = row.get("match_features_json")?;
    let match_features = match_features
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .unwrap_or_else(|| json!({}));
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "userId": row.get::<_, i64>("user_id")?,
        "matchType": row.get::<_, String>("match_type")?,
        "matchValue": row.get::<_, String>("match_value")?,
        "normalizedMatchValue": row.get::<_, String>("normalized_match_value")?,
        "learnedType": row.get::<_, Option<String>>("learned_type")?,
        "learnedCategoryId": row.get::<_, Option<i64>>("learned_category_id")?,
        "learnedSourceAccountId": row.get::<_, Option<i64>>("learned_source_account_id")?,
        "learnedDestinationAccountId": row.get::<_, Option<i64>>("learned_destination_account_id")?,
        "enabled": row.get::<_, bool>("enabled")?,
        "sourceSessionId": row.get::<_, Option<String>>("source_session_id")?,
        "sourcePreviewId": row.get::<_, Option<i64>>("source_preview_id")?,
        "parserId": row.get::<_, Option<String>>("parser_id")?,
        "compositeMatchHash": row.get::<_, Option<String>>("composite_match_hash")?,
        "matchFeatures": match_features,
        "appliedCount": row.get::<_, i64>("applied_count")?,
        "lastAppliedAt": row.get::<_, Option<String>>("last_applied_at")?,
        "createdAt": row.get::<_, String>("created_at")?,
        "updatedAt": row.get::<_, String>("updated_at")?,
    }))
}

fn learning_data_response(data: Value) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

fn learning_error_response(status_code: u16, error: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

fn learning_rule_camel_to_snake(rule: &Value) -> Value {
    json!({
        "id": rule["id"],
        "user_id": rule["userId"],
        "match_type": rule["matchType"],
        "match_value": rule["matchValue"],
        "normalized_match_value": rule["normalizedMatchValue"],
        "learned_type": rule["learnedType"],
        "learned_category_id": rule["learnedCategoryId"],
        "learned_source_account_id": rule["learnedSourceAccountId"],
        "learned_destination_account_id": rule["learnedDestinationAccountId"],
        "enabled": rule["enabled"],
        "source_session_id": rule["sourceSessionId"],
        "source_preview_id": rule["sourcePreviewId"],
        "parser_id": rule["parserId"],
        "composite_match_hash": rule["compositeMatchHash"],
        "match_features_json": rule["matchFeatures"].to_string(),
        "applied_count": rule["appliedCount"],
        "last_applied_at": rule["lastAppliedAt"],
        "created_at": rule["createdAt"],
        "updated_at": rule["updatedAt"],
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LearningSuggestionSignature {
    suggested_type: Option<String>,
    suggested_category_id: Option<i64>,
    suggested_source_account_id: Option<i64>,
    suggested_destination_account_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct LearningSuggestionCandidate {
    match_features: BTreeMap<String, String>,
    signature: LearningSuggestionSignature,
    sample_count: i64,
    source_session_ids: BTreeSet<String>,
    source_preview_ids: BTreeSet<i64>,
    existing_rule_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct LearningSuggestionRow {
    id: i64,
    user_id: i64,
    match_type: String,
    match_value: String,
    normalized_match_value: String,
    composite_match_hash: Option<String>,
    match_features_json: Option<String>,
    suggested_type: Option<String>,
    suggested_category_id: Option<i64>,
    suggested_source_account_id: Option<i64>,
    suggested_destination_account_id: Option<i64>,
    sample_count: i64,
    source_session_ids_json: Option<String>,
    source_preview_ids_json: Option<String>,
    status: String,
    existing_rule_id: Option<i64>,
    summary: Option<String>,
    created_at: String,
    updated_at: String,
}

enum LearningSuggestionDecision {
    Accepted(Value),
    Conflict(Value),
    NotFound,
}

fn mine_learning_suggestions(
    connection: &mut Connection,
    user_id: UserId,
) -> Result<Value, ImportV2RouteResponse> {
    let user_id_i64 = user_id_i64_value(user_id)?;
    let annotations = load_learning_corpus_annotations(connection, user_id_i64)?;
    if annotations.is_empty() {
        return Ok(empty_learning_suggestion_mining_result());
    }
    let existing_rule_hashes = existing_composite_rule_hashes(connection, user_id_i64)?;
    let (candidates, conflicted_hashes) =
        collect_learning_suggestion_candidates(annotations, &existing_rule_hashes);

    let now = now_text();
    let mut created = 0;
    let mut updated = 0;
    let mut skipped_existing = 0;
    for (composite_hash, candidate) in candidates {
        if candidate.existing_rule_id.is_some() {
            skipped_existing += 1;
            continue;
        }
        let existed =
            learning_suggestion_id_by_key(connection, user_id_i64, "composite", &composite_hash)?
                .is_some();
        let session_ids = candidate.source_session_ids.into_iter().collect::<Vec<_>>();
        let preview_ids = candidate.source_preview_ids.into_iter().collect::<Vec<_>>();
        let match_features_json =
            serde_json::to_string(&candidate.match_features).map_err(|error| {
                import_v2_error_response(
                    500,
                    &format!("Unable to serialize learning features: {error}"),
                )
            })?;
        let session_ids_json = serde_json::to_string(&session_ids).map_err(|error| {
            import_v2_error_response(
                500,
                &format!("Unable to serialize source sessions: {error}"),
            )
        })?;
        let preview_ids_json = serde_json::to_string(&preview_ids).map_err(|error| {
            import_v2_error_response(
                500,
                &format!("Unable to serialize source previews: {error}"),
            )
        })?;
        connection
            .execute(
                "
                INSERT INTO import_learning_suggestions (
                    user_id, match_type, match_value, normalized_match_value,
                    composite_match_hash, match_features_json, suggested_type,
                    suggested_category_id, suggested_source_account_id,
                    suggested_destination_account_id, sample_count,
                    source_session_ids_json, source_preview_ids_json, status,
                    existing_rule_id, summary, created_at, updated_at
                ) VALUES (?1, 'composite', ?2, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    'pending', NULL, ?11, ?12, ?12)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    suggested_type = excluded.suggested_type,
                    suggested_category_id = excluded.suggested_category_id,
                    suggested_source_account_id = excluded.suggested_source_account_id,
                    suggested_destination_account_id = excluded.suggested_destination_account_id,
                    sample_count = excluded.sample_count,
                    source_session_ids_json = excluded.source_session_ids_json,
                    source_preview_ids_json = excluded.source_preview_ids_json,
                    summary = excluded.summary,
                    updated_at = excluded.updated_at
                ",
                params![
                    user_id_i64,
                    composite_hash,
                    match_features_json,
                    candidate.signature.suggested_type,
                    candidate.signature.suggested_category_id,
                    candidate.signature.suggested_source_account_id,
                    candidate.signature.suggested_destination_account_id,
                    candidate.sample_count,
                    session_ids_json,
                    preview_ids_json,
                    summarize_learning_suggestion(&candidate.match_features),
                    now,
                ],
            )
            .map_err(db_error_response)?;
        if existed {
            updated += 1;
        } else {
            created += 1;
        }
    }

    Ok(json!({
        "total_annotations": annotations_total(connection, user_id_i64)?,
        "mined": created + updated + skipped_existing,
        "created": created,
        "updated": updated,
        "skipped_conflict": conflicted_hashes.len(),
        "skipped_existing": skipped_existing,
    }))
}

fn empty_learning_suggestion_mining_result() -> Value {
    json!({
        "total_annotations": 0,
        "mined": 0,
        "created": 0,
        "updated": 0,
        "skipped_conflict": 0,
        "skipped_existing": 0,
    })
}

fn annotations_total(connection: &Connection, user_id: i64) -> Result<i64, ImportV2RouteResponse> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM import_learning_corpus_samples WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .map_err(db_error_response)
}

fn load_learning_corpus_annotations(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<LearningSuggestionCandidateInput>, ImportV2RouteResponse> {
    let mut statement = connection
        .prepare(
            "
            SELECT *
            FROM import_learning_corpus_samples
            WHERE user_id = ?1
            ORDER BY session_id, updated_at ASC, id ASC
            ",
        )
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], learning_corpus_row_to_input)
        .map_err(db_error_response)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)
}

#[derive(Debug, Clone)]
struct LearningSuggestionCandidateInput {
    session_id: String,
    preview_id: i64,
    parser_id: String,
    counterparty: String,
    description: String,
    payment_method: String,
    composite_match_hash: String,
    match_features_json: String,
    signature: LearningSuggestionSignature,
}

fn learning_corpus_row_to_input(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<LearningSuggestionCandidateInput> {
    Ok(LearningSuggestionCandidateInput {
        session_id: row.get::<_, String>("session_id")?,
        preview_id: row.get::<_, i64>("preview_id")?,
        parser_id: row
            .get::<_, Option<String>>("parser_id")?
            .unwrap_or_default(),
        counterparty: row
            .get::<_, Option<String>>("counterparty")?
            .unwrap_or_default(),
        description: row
            .get::<_, Option<String>>("description")?
            .unwrap_or_default(),
        payment_method: row
            .get::<_, Option<String>>("payment_method")?
            .unwrap_or_default(),
        composite_match_hash: row
            .get::<_, Option<String>>("composite_match_hash")?
            .unwrap_or_default(),
        match_features_json: row
            .get::<_, Option<String>>("match_features_json")?
            .unwrap_or_default(),
        signature: LearningSuggestionSignature {
            suggested_type: row.get::<_, Option<String>>("annotated_type")?,
            suggested_category_id: row.get::<_, Option<i64>>("annotated_category_id")?,
            suggested_source_account_id: row
                .get::<_, Option<i64>>("annotated_source_account_id")?,
            suggested_destination_account_id: row
                .get::<_, Option<i64>>("annotated_destination_account_id")?,
        },
    })
}

fn existing_composite_rule_hashes(
    connection: &Connection,
    user_id: i64,
) -> Result<BTreeMap<String, i64>, ImportV2RouteResponse> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, normalized_match_value
            FROM import_learning_rules
            WHERE user_id = ?1 AND match_type = 'composite'
            ",
        )
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?))
        })
        .map_err(db_error_response)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(db_error_response)
}

fn collect_learning_suggestion_candidates(
    annotations: Vec<LearningSuggestionCandidateInput>,
    existing_rule_hashes: &BTreeMap<String, i64>,
) -> (
    BTreeMap<String, LearningSuggestionCandidate>,
    BTreeSet<String>,
) {
    let mut candidates: BTreeMap<String, LearningSuggestionCandidate> = BTreeMap::new();
    let mut conflicted_hashes = BTreeSet::new();
    for annotation in annotations {
        let Some((composite_hash, match_features)) = resolve_suggestion_match(&annotation) else {
            continue;
        };
        if conflicted_hashes.contains(&composite_hash) {
            continue;
        }
        if let Some(existing) = candidates.get_mut(&composite_hash) {
            if existing.signature != annotation.signature {
                conflicted_hashes.insert(composite_hash.clone());
                candidates.remove(&composite_hash);
                continue;
            }
            existing.sample_count += 1;
            existing.source_session_ids.insert(annotation.session_id);
            existing.source_preview_ids.insert(annotation.preview_id);
            continue;
        }
        candidates.insert(
            composite_hash.clone(),
            LearningSuggestionCandidate {
                match_features,
                signature: annotation.signature,
                sample_count: 1,
                source_session_ids: BTreeSet::from([annotation.session_id]),
                source_preview_ids: BTreeSet::from([annotation.preview_id]),
                existing_rule_id: existing_rule_hashes.get(&composite_hash).copied(),
            },
        );
    }
    (candidates, conflicted_hashes)
}

fn resolve_suggestion_match(
    annotation: &LearningSuggestionCandidateInput,
) -> Option<(String, BTreeMap<String, String>)> {
    let parsed_features =
        serde_json::from_str::<BTreeMap<String, String>>(annotation.match_features_json.trim())
            .ok()
            .map(|features| {
                features
                    .into_iter()
                    .map(|(key, value)| (key, normalize_learning_match_value(&value)))
                    .filter(|(_, value)| !value.is_empty())
                    .collect::<BTreeMap<_, _>>()
            })
            .filter(|features| features.len() >= 2);
    let match_features = parsed_features.or_else(|| {
        build_composite_match_features(
            &annotation.parser_id,
            &annotation.counterparty,
            &annotation.description,
            &annotation.payment_method,
        )
    })?;
    let composite_hash = if annotation.composite_match_hash.trim().is_empty() {
        composite_hash_from_features(&match_features)
    } else {
        annotation.composite_match_hash.trim().to_string()
    };
    (!composite_hash.is_empty()).then_some((composite_hash, match_features))
}

fn summarize_learning_suggestion(match_features: &BTreeMap<String, String>) -> String {
    let parts = [
        ("counterparty", "交易方"),
        ("description", "描述"),
        ("payment_method", "支付方式"),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        match_features
            .get(key)
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("{label}: {value}"))
    })
    .collect::<Vec<_>>();
    if parts.is_empty() {
        composite_hash_from_features(match_features)
    } else {
        parts.join(" | ")
    }
}

fn learning_suggestion_id_by_key(
    connection: &Connection,
    user_id: i64,
    match_type: &str,
    normalized_match_value: &str,
) -> Result<Option<i64>, ImportV2RouteResponse> {
    connection
        .query_row(
            "
            SELECT id FROM import_learning_suggestions
            WHERE user_id = ?1 AND match_type = ?2 AND normalized_match_value = ?3
            LIMIT 1
            ",
            params![user_id, match_type, normalized_match_value],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error_response)
}

fn count_learning_suggestions(
    connection: &Connection,
    user_id: UserId,
    status: Option<&str>,
) -> Result<i64, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    if let Some(status) = status {
        connection
            .query_row(
                "SELECT COUNT(*) FROM import_learning_suggestions WHERE user_id = ?1 AND status = ?2",
                params![user_id, status],
                |row| row.get(0),
            )
            .map_err(db_error_response)
    } else {
        connection
            .query_row(
                "SELECT COUNT(*) FROM import_learning_suggestions WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .map_err(db_error_response)
    }
}

fn load_learning_suggestions(
    connection: &Connection,
    user_id: UserId,
    status: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    if let Some(status) = status {
        let mut statement = connection
            .prepare(
                "
            SELECT * FROM import_learning_suggestions
            WHERE user_id = ?1 AND status = ?2
            ORDER BY sample_count DESC, updated_at DESC, id DESC
            LIMIT ?3 OFFSET ?4
            ",
            )
            .map_err(db_error_response)?;
        let rows = statement
            .query_map(
                params![user_id, status, usize_to_i64(limit), usize_to_i64(offset)],
                learning_suggestion_row_to_value,
            )
            .map_err(db_error_response)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_error_response)
    } else {
        let mut statement = connection
            .prepare(
                "
            SELECT * FROM import_learning_suggestions
            WHERE user_id = ?1
            ORDER BY sample_count DESC, updated_at DESC, id DESC
            LIMIT ?2 OFFSET ?3
            ",
            )
            .map_err(db_error_response)?;
        let rows = statement
            .query_map(
                params![user_id, usize_to_i64(limit), usize_to_i64(offset)],
                learning_suggestion_row_to_value,
            )
            .map_err(db_error_response)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_error_response)
    }
}

fn learning_suggestion_row_to_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let row = learning_suggestion_row(row)?;
    Ok(json!({
        "id": row.id,
        "user_id": row.user_id,
        "match_type": row.match_type,
        "match_value": row.match_value,
        "normalized_match_value": row.normalized_match_value,
        "composite_match_hash": row.composite_match_hash,
        "match_features_json": row.match_features_json.unwrap_or_default(),
        "suggested_type": row.suggested_type,
        "suggested_category_id": row.suggested_category_id,
        "suggested_source_account_id": row.suggested_source_account_id,
        "suggested_destination_account_id": row.suggested_destination_account_id,
        "sample_count": row.sample_count,
        "source_session_ids_json": row.source_session_ids_json.unwrap_or_default(),
        "source_preview_ids_json": row.source_preview_ids_json.unwrap_or_default(),
        "status": row.status,
        "existing_rule_id": row.existing_rule_id,
        "summary": row.summary.unwrap_or_default(),
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    }))
}

fn learning_suggestion_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LearningSuggestionRow> {
    Ok(LearningSuggestionRow {
        id: row.get("id")?,
        user_id: row.get("user_id")?,
        match_type: row.get("match_type")?,
        match_value: row.get("match_value")?,
        normalized_match_value: row.get("normalized_match_value")?,
        composite_match_hash: row.get("composite_match_hash")?,
        match_features_json: row.get("match_features_json")?,
        suggested_type: row.get("suggested_type")?,
        suggested_category_id: row.get("suggested_category_id")?,
        suggested_source_account_id: row.get("suggested_source_account_id")?,
        suggested_destination_account_id: row.get("suggested_destination_account_id")?,
        sample_count: row.get("sample_count")?,
        source_session_ids_json: row.get("source_session_ids_json")?,
        source_preview_ids_json: row.get("source_preview_ids_json")?,
        status: row.get("status")?,
        existing_rule_id: row.get("existing_rule_id")?,
        summary: row.get("summary")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn get_learning_suggestion(
    connection: &Connection,
    suggestion_id: i64,
    user_id: UserId,
) -> Result<Option<LearningSuggestionRow>, ImportV2RouteResponse> {
    connection
        .query_row(
            "
            SELECT * FROM import_learning_suggestions
            WHERE id = ?1 AND user_id = ?2
            LIMIT 1
            ",
            params![suggestion_id, user_id_i64_value(user_id)?],
            learning_suggestion_row,
        )
        .optional()
        .map_err(db_error_response)
}

fn accept_learning_suggestion(
    connection: &mut Connection,
    suggestion_id: i64,
    user_id: UserId,
) -> Result<LearningSuggestionDecision, ImportV2RouteResponse> {
    let Some(suggestion) = get_learning_suggestion(connection, suggestion_id, user_id)? else {
        return Ok(LearningSuggestionDecision::NotFound);
    };
    if suggestion.status != "pending" {
        return Ok(LearningSuggestionDecision::Conflict(json!({
            "error": "suggestion_not_pending",
            "current_status": suggestion.status,
        })));
    }
    let user_id_i64 = user_id_i64_value(user_id)?;
    let now = now_text();
    let match_features = serde_json::from_str::<BTreeMap<String, String>>(
        suggestion.match_features_json.as_deref().unwrap_or("{}"),
    )
    .unwrap_or_default();
    connection
        .execute(
            "
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id, learned_source_account_id,
                learned_destination_account_id, enabled, parser_id,
                composite_match_hash, match_features_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?12)
            ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                learned_type = excluded.learned_type,
                learned_category_id = excluded.learned_category_id,
                learned_source_account_id = excluded.learned_source_account_id,
                learned_destination_account_id = excluded.learned_destination_account_id,
                enabled = 1,
                parser_id = excluded.parser_id,
                composite_match_hash = excluded.composite_match_hash,
                match_features_json = excluded.match_features_json,
                updated_at = excluded.updated_at
            ",
            params![
                user_id_i64,
                &suggestion.match_type,
                &suggestion.match_value,
                &suggestion.normalized_match_value,
                suggestion.suggested_type.as_deref(),
                suggestion.suggested_category_id,
                suggestion.suggested_source_account_id,
                suggestion.suggested_destination_account_id,
                match_features.get("parser_id").cloned().unwrap_or_default(),
                suggestion.composite_match_hash.as_deref(),
                suggestion.match_features_json.as_deref(),
                now,
            ],
        )
        .map_err(db_error_response)?;
    let rule_id = connection
        .query_row(
            "
            SELECT id FROM import_learning_rules
            WHERE user_id = ?1 AND match_type = ?2 AND normalized_match_value = ?3
            LIMIT 1
            ",
            params![
                user_id_i64,
                &suggestion.match_type,
                &suggestion.normalized_match_value
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(db_error_response)?;
    connection
        .execute(
            "
            UPDATE import_learning_suggestions
            SET status = 'accepted', existing_rule_id = ?1, updated_at = ?2
            WHERE id = ?3 AND user_id = ?4
            ",
            params![rule_id, now, suggestion_id, user_id_i64],
        )
        .map_err(db_error_response)?;
    record_learning_rule_log(
        connection,
        rule_id,
        user_id_i64,
        "created_from_suggestion",
        Some(&suggestion.match_type),
        Some(&suggestion.match_value),
        Some(&suggestion.normalized_match_value),
        None,
        None,
        Some(json!({"suggestion_id": suggestion_id})),
    )?;
    record_learning_feedback_event(
        connection,
        user_id_i64,
        "suggestion_accept",
        Some(rule_id),
        Some(suggestion_id),
        Some(json!({"status": "accepted"})),
    )?;
    Ok(LearningSuggestionDecision::Accepted(json!({
        "suggestion_id": suggestion_id,
        "rule_id": rule_id,
        "status": "accepted",
    })))
}

fn reject_learning_suggestion(
    connection: &mut Connection,
    suggestion_id: i64,
    user_id: UserId,
) -> Result<bool, ImportV2RouteResponse> {
    let Some(suggestion) = get_learning_suggestion(connection, suggestion_id, user_id)? else {
        return Ok(false);
    };
    if suggestion.status != "pending" {
        return Ok(false);
    }
    let user_id_i64 = user_id_i64_value(user_id)?;
    let now = now_text();
    connection
        .execute(
            "
            UPDATE import_learning_suggestions
            SET status = 'rejected', updated_at = ?1
            WHERE id = ?2 AND user_id = ?3
            ",
            params![now, suggestion_id, user_id_i64],
        )
        .map_err(db_error_response)?;
    record_learning_feedback_event(
        connection,
        user_id_i64,
        "suggestion_reject",
        None,
        Some(suggestion_id),
        Some(json!({"status": "rejected"})),
    )?;
    Ok(true)
}

fn set_import_learning_rule_enabled(
    connection: &mut Connection,
    rule_id: i64,
    user_id: UserId,
    enabled: bool,
) -> Result<bool, ImportV2RouteResponse> {
    let Some(existing) = get_import_learning_rule(connection, rule_id, user_id)? else {
        return Ok(false);
    };
    let user_id_i64 = user_id_i64_value(user_id)?;
    connection
        .execute(
            "
            UPDATE import_learning_rules
            SET enabled = ?1, updated_at = ?2
            WHERE id = ?3 AND user_id = ?4
            ",
            params![enabled, now_text(), rule_id, user_id_i64],
        )
        .map_err(db_error_response)?;
    record_learning_rule_log(
        connection,
        rule_id,
        user_id_i64,
        if enabled { "enabled" } else { "disabled" },
        existing["matchType"].as_str(),
        existing["matchValue"].as_str(),
        existing["normalizedMatchValue"].as_str(),
        existing["sourceSessionId"].as_str(),
        existing["sourcePreviewId"].as_i64(),
        Some(json!({"enabled": enabled})),
    )?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn record_learning_rule_log(
    connection: &Connection,
    rule_id: i64,
    user_id: i64,
    action: &str,
    match_type: Option<&str>,
    match_value: Option<&str>,
    normalized_match_value: Option<&str>,
    session_id: Option<&str>,
    preview_id: Option<i64>,
    payload: Option<Value>,
) -> Result<(), ImportV2RouteResponse> {
    let payload_json = payload.map(|value| value.to_string());
    connection
        .execute(
            "
            INSERT INTO import_learning_rule_logs (
                user_id, rule_id, action, match_type, match_value,
                normalized_match_value, session_id, preview_id, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                user_id,
                rule_id,
                action,
                match_type,
                match_value,
                normalized_match_value,
                session_id,
                preview_id,
                payload_json,
                now_text(),
            ],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

fn record_learning_feedback_event(
    connection: &Connection,
    user_id: i64,
    event_type: &str,
    rule_id: Option<i64>,
    suggestion_id: Option<i64>,
    payload: Option<Value>,
) -> Result<(), ImportV2RouteResponse> {
    let payload_json = payload.map(|value| value.to_string());
    connection
        .execute(
            "
            INSERT INTO import_learning_feedback_events (
                user_id, event_type, rule_id, suggestion_id, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                user_id,
                event_type,
                rule_id,
                suggestion_id,
                payload_json,
                now_text(),
            ],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

fn init_legacy_bills_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                date TEXT NOT NULL,
                type TEXT NOT NULL,
                amount REAL NOT NULL,
                counterparty TEXT NOT NULL,
                description TEXT NOT NULL,
                payment_method TEXT DEFAULT '',
                main_category TEXT,
                sub_category TEXT,
                batch_id TEXT,
                hash TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                source_account_id INTEGER DEFAULT 0,
                destination_account_id INTEGER DEFAULT 0,
                destination_amount REAL DEFAULT 0,
                created_from_template INTEGER,
                created_from_recurring INTEGER,
                import_history_id INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash);
            ",
        )
        .map_err(db_error_response)
}

fn insert_legacy_confirmed_bills(
    connection: &mut Connection,
    user_id: UserId,
    bills: &[Value],
) -> Result<Value, ImportV2RouteResponse> {
    let user_id_i64 = user_id_i64_value(user_id)?;
    let batch_id = format!("legacy-import-{}", legacy_import_time_seconds(""));
    let now = now_text();
    let tx = connection.transaction().map_err(db_error_response)?;
    let mut inserted = 0_i64;
    let mut duplicates = 0_i64;
    let mut skipped = 0_i64;
    let mut errors = Vec::new();
    for (index, bill) in bills.iter().enumerate() {
        let Some(object) = bill.as_object() else {
            skipped += 1;
            errors.push(json!({"index": index, "error": "Invalid bill"}));
            continue;
        };
        let Some(amount) = legacy_bill_amount(object) else {
            skipped += 1;
            errors.push(json!({"index": index, "error": "Invalid amount"}));
            continue;
        };
        let bill_type = legacy_bill_type(object);
        let amount = legacy_confirm_amount_for_type(&bill_type, amount);
        let date = legacy_bill_date(object);
        let counterparty = first_text_from_object(object, &["counterparty"]).unwrap_or_else(|| {
            first_text_from_object(object, &["originalSourceAccountName", "accountName"])
                .unwrap_or_default()
        });
        let description = first_text_from_object(object, &["description", "comment", "remark"])
            .unwrap_or_else(|| counterparty.clone());
        let payment_method = first_text_from_object(
            object,
            &[
                "paymentMethod",
                "payment_method",
                "originalSourceAccountName",
                "accountName",
            ],
        )
        .unwrap_or_default();
        let main_category = first_text_from_object(
            object,
            &[
                "mainCategory",
                "main_category",
                "categoryName",
                "originalCategoryName",
            ],
        );
        let sub_category =
            first_text_from_object(object, &["subCategory", "sub_category", "subCategoryName"]);
        let source_account_id = first_value(object, &["sourceAccountId", "source_account_id"])
            .and_then(value_to_i64)
            .unwrap_or_default();
        let destination_account_id =
            first_value(object, &["destinationAccountId", "destination_account_id"])
                .and_then(value_to_i64)
                .unwrap_or_default();
        let destination_amount = first_value(object, &["destinationAmount", "destination_amount"])
            .and_then(value_to_f64)
            .map(|value| value / 100.0)
            .unwrap_or_default();
        let hash =
            calculate_import_bill_hash(&date, &bill_type, amount, &counterparty, &description);
        let changed = tx
            .execute(
                "
                INSERT OR IGNORE INTO bills (
                    user_id, date, type, amount, counterparty, description,
                    payment_method, main_category, sub_category,
                    source_account_id, destination_account_id, destination_amount,
                    batch_id, hash, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
                ",
                params![
                    user_id_i64,
                    date,
                    bill_type,
                    amount,
                    counterparty,
                    description,
                    payment_method,
                    main_category,
                    sub_category,
                    source_account_id,
                    destination_account_id,
                    destination_amount,
                    batch_id,
                    hash,
                    now,
                ],
            )
            .map_err(db_error_response)?;
        if changed > 0 {
            inserted += 1;
        } else {
            duplicates += 1;
        }
    }
    tx.commit().map_err(db_error_response)?;
    Ok(json!({
        "total": bills.len(),
        "inserted": inserted,
        "duplicates": duplicates,
        "skipped": skipped,
        "errors": errors,
        "batch_id": batch_id,
        "runtime": "rust-import-db-runtime-partial",
    }))
}

fn preview_rows_from_temp_path(
    temp_path: &FsPath,
    delimiter: Option<&str>,
) -> Result<Value, ImportV2RouteResponse> {
    let text = read_import_temp_text(temp_path)?;
    preview_rows_from_text(&text, delimiter)
}

fn preview_rows_from_file_bytes(
    body: &[u8],
    delimiter: Option<&str>,
) -> Result<Value, ImportV2RouteResponse> {
    let text = decode_import_text(body);
    preview_rows_from_text(&text, delimiter)
}

fn preview_rows_from_text(
    text: &str,
    delimiter: Option<&str>,
) -> Result<Value, ImportV2RouteResponse> {
    let detected_delimiter = delimiter_from_hint(delimiter)
        .or_else(|| detect_csv_table_start(text, false).map(|(_, delimiter)| delimiter))
        .unwrap_or(',');
    let rows = csv_rows_from_text(text, delimiter)?;
    let headers = rows.first().cloned().unwrap_or_default();
    let total_rows = rows.len();
    Ok(json!({
        "headers": headers,
        "sampleData": rows.iter().take(300).cloned().collect::<Vec<_>>(),
        "previewRows": rows.iter().skip(1).take(50).cloned().collect::<Vec<_>>(),
        "totalRows": total_rows,
        "rowCount": total_rows,
        "encoding": "utf-8-or-gbk",
        "delimiter": delimiter_to_response(Some(detected_delimiter)),
    }))
}

fn csv_rows_from_text(
    text: &str,
    delimiter_hint: Option<&str>,
) -> Result<Vec<Vec<String>>, ImportV2RouteResponse> {
    let delimiter = delimiter_from_hint(delimiter_hint)
        .or_else(|| detect_csv_table_start(text, false).map(|(_, delimiter)| delimiter))
        .unwrap_or(',');
    let start_line = detect_csv_table_start(text, false)
        .map(|(line, _)| line)
        .unwrap_or(0);
    let csv_text = text.lines().skip(start_line).collect::<Vec<_>>().join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(csv_text.as_bytes());
    let rows = reader
        .records()
        .map(|record| {
            record
                .map(|record| {
                    record
                        .iter()
                        .map(|value| value.trim().to_string())
                        .collect()
                })
                .map_err(|error| {
                    import_v2_error_response(400, &format!("Invalid CSV file: {error}"))
                })
        })
        .collect::<Result<Vec<Vec<String>>, ImportV2RouteResponse>>()?;
    Ok(rows)
}

fn split_preview_line(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|value| value.trim().trim_matches('"').to_string())
        .collect()
}

fn detect_delimiter_for_line(line: &str, hint: Option<&str>) -> Option<char> {
    if let Some(delimiter) = delimiter_from_hint(hint) {
        return Some(delimiter);
    }
    [',', '\t', ';']
        .into_iter()
        .map(|delimiter| (delimiter, line.matches(delimiter).count()))
        .max_by_key(|(_, count)| *count)
        .and_then(|(delimiter, count)| if count > 0 { Some(delimiter) } else { None })
}

fn delimiter_from_hint(delimiter: Option<&str>) -> Option<char> {
    let delimiter = delimiter?.trim();
    if delimiter.is_empty() {
        return None;
    }
    match delimiter {
        "\\t" | "tab" | "TAB" => Some('\t'),
        _ => delimiter.chars().next(),
    }
}

fn delimiter_to_response(delimiter: Option<char>) -> String {
    match delimiter.unwrap_or(',') {
        '\t' => "\t".to_string(),
        value => value.to_string(),
    }
}

fn parse_urlencoded_form(body: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(body);
    text.split('&')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (
                percent_decode_form_component(key),
                percent_decode_form_component(value),
            )
        })
        .collect()
}

fn percent_decode_form_component(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                    output.push(value);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            value => {
                output.push(value);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).to_string()
}

fn save_unmatched_import_file(
    user_id: UserId,
    session_id: &str,
    original_name: &str,
    body: &[u8],
) -> Result<String, ImportV2RouteResponse> {
    let user_component = import_temp_user_component(user_id);
    let session_component = sanitize_path_component(session_id);
    let dir = temp_import_root()
        .join(&user_component)
        .join(&session_component);
    fs::create_dir_all(&dir).map_err(|error| {
        import_v2_error_response(
            500,
            &format!("Unable to prepare temp import directory: {error}"),
        )
    })?;
    let file_name = format!(
        "{}-{}",
        generate_import_session_id(),
        sanitize_path_component(original_name)
    );
    let path = dir.join(file_name);
    fs::write(&path, body).map_err(|error| {
        import_v2_error_response(500, &format!("Unable to persist temp import file: {error}"))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| import_v2_error_response(500, "Unable to resolve temp import filename"))?;
    Ok(format!("{user_component}/{session_component}/{file_name}"))
}

fn validate_import_temp_path(
    raw_path: &str,
    user_id: UserId,
    expected_session_id: Option<&str>,
) -> Result<PathBuf, ImportV2RouteResponse> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() || FsPath::new(raw_path).is_absolute() {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    if FsPath::new(raw_path).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    let root = temp_import_root();
    fs::create_dir_all(&root).map_err(|error| {
        import_v2_error_response(
            500,
            &format!("Unable to prepare temp import directory: {error}"),
        )
    })?;
    let root = root.canonicalize().map_err(|error| {
        import_v2_error_response(500, &format!("Unable to resolve temp import root: {error}"))
    })?;
    let mut base = root.join(import_temp_user_component(user_id));
    if let Some(session_id) = expected_session_id {
        base = base.join(sanitize_path_component(session_id));
    }
    let base = base
        .canonicalize()
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    let path = root
        .join(raw_path)
        .canonicalize()
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    if !path.starts_with(&base) {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    Ok(path)
}

fn read_import_temp_text(path: &FsPath) -> Result<String, ImportV2RouteResponse> {
    let bytes = fs::read(path).map_err(|error| {
        import_v2_error_response(500, &format!("Unable to read temp import file: {error}"))
    })?;
    Ok(decode_import_text(&bytes))
}

fn temp_import_root() -> PathBuf {
    std::env::temp_dir().join("bill-analyser-rust-import")
}

fn import_temp_user_component(user_id: UserId) -> String {
    format!("user-{}", user_id.get())
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('.').trim_matches('_');
    if sanitized.is_empty() {
        "import-file".to_string()
    } else {
        sanitized.to_string()
    }
}

fn bool_field_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    first_value(object, keys).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    })
}

fn id_list_field_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<Vec<i64>> {
    first_value(object, keys).and_then(|value| {
        let ids = value
            .as_array()?
            .iter()
            .filter_map(value_to_i64)
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();
        Some(ids)
    })
}

fn generate_import_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = IMPORT_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("rust-import-{nanos}-{counter}")
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn preview_update_items_from_payload(
    payload: &Value,
) -> Result<Vec<&Map<String, Value>>, ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let Some(updates) = first_value(object, &["preview_updates", "previewUpdates"]) else {
        return Ok(Vec::new());
    };
    let updates = updates
        .as_array()
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))?;
    let mut items = Vec::with_capacity(updates.len());
    for item in updates {
        items.push(payload_object(item)?);
    }
    Ok(items)
}

fn preview_id_from_payload(object: &Map<String, Value>) -> Result<i64, ImportV2RouteResponse> {
    first_value(object, &["id", "preview_id", "previewId"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| import_v2_error_response(400, "Missing bill id"))
}

fn build_preview_patch_from_payload(
    preview_id: i64,
    object: &Map<String, Value>,
) -> ImportPreviewPatch {
    let mut changes = Vec::new();
    push_text_change(
        &mut changes,
        object,
        &["date", "preview_date", "previewDate"],
        ImportPreviewPatchField::Date,
    );
    push_preview_type_change(
        &mut changes,
        object,
        &["type", "preview_type", "previewType"],
    );
    push_real_change(
        &mut changes,
        object,
        &["amount", "preview_amount", "previewAmount"],
        ImportPreviewPatchField::Amount,
    );
    push_real_change(
        &mut changes,
        object,
        &[
            "destinationAmount",
            "preview_destination_amount",
            "previewDestinationAmount",
        ],
        ImportPreviewPatchField::DestinationAmount,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "mainCategory",
            "preview_main_category",
            "previewMainCategory",
        ],
        ImportPreviewPatchField::MainCategory,
    );
    push_text_change(
        &mut changes,
        object,
        &["subCategory", "preview_sub_category", "previewSubCategory"],
        ImportPreviewPatchField::SubCategory,
    );
    push_nullable_i64_change(
        &mut changes,
        object,
        &[
            "sourceAccountId",
            "preview_source_account_id",
            "previewSourceAccountId",
        ],
        ImportPreviewPatchField::SourceAccountId,
    );
    push_nullable_i64_change(
        &mut changes,
        object,
        &[
            "destinationAccountId",
            "preview_destination_account_id",
            "previewDestinationAccountId",
        ],
        ImportPreviewPatchField::DestinationAccountId,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "counterparty",
            "preview_counterparty",
            "previewCounterparty",
        ],
        ImportPreviewPatchField::Counterparty,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "paymentMethod",
            "preview_payment_method",
            "previewPaymentMethod",
        ],
        ImportPreviewPatchField::PaymentMethod,
    );
    push_text_change(
        &mut changes,
        object,
        &["description", "preview_description", "previewDescription"],
        ImportPreviewPatchField::Description,
    );
    push_nullable_i64_change(
        &mut changes,
        object,
        &["recurringId", "preview_recurring_id", "previewRecurringId"],
        ImportPreviewPatchField::RecurringId,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "recurringName",
            "preview_recurring_name",
            "previewRecurringName",
        ],
        ImportPreviewPatchField::RecurringName,
    );
    push_i64_change(
        &mut changes,
        object,
        &[
            "recurringCandidateCount",
            "preview_recurring_candidate_count",
            "previewRecurringCandidateCount",
        ],
        ImportPreviewPatchField::RecurringCandidateCount,
    );
    push_real_change(
        &mut changes,
        object,
        &[
            "recurringMatchScore",
            "preview_recurring_match_score",
            "previewRecurringMatchScore",
        ],
        ImportPreviewPatchField::RecurringMatchScore,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "recurringMatchReasons",
            "preview_recurring_match_reasons",
            "previewRecurringMatchReasons",
        ],
        ImportPreviewPatchField::RecurringMatchReasons,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "recurringMatchedDate",
            "preview_recurring_matched_date",
            "previewRecurringMatchedDate",
        ],
        ImportPreviewPatchField::RecurringMatchedDate,
    );
    if let Some(value) = first_value(object, &["isSelected", "selected", "preview_selected"]) {
        changes.push((
            ImportPreviewPatchField::Selected,
            ImportPreviewPatchValue::Bool(coerce_preview_selected_value(Some(value), true)),
        ));
    }
    if let Some(value) = first_value(
        object,
        &[
            "matchingFeedback",
            "preview_matching_feedback",
            "previewMatchingFeedback",
        ],
    ) {
        changes.push((
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Json(value.clone()),
        ));
    }

    let mut patch = ImportPreviewPatch::new(preview_id).with_changes(changes);
    if first_value(
        object,
        &["clear_transfer_decision", "clearTransferDecision"],
    )
    .is_some_and(|value| coerce_preview_selected_value(Some(value), false))
    {
        patch = patch.with_transfer_decision_cleared();
    }
    patch
}

fn apply_preview_updates_from_payload(
    runtime: &mut SqliteRuntime,
    session_id: &str,
    user_id: UserId,
    payload: &Value,
) -> Result<usize, ImportV2RouteResponse> {
    let update_items = preview_update_items_from_payload(payload)?;
    if update_items.is_empty() {
        return Ok(0);
    }

    let mut patches = Vec::with_capacity(update_items.len());
    for item in update_items {
        let preview_id = preview_id_from_payload(item)?;
        match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => {}
            Ok(Some(_)) | Ok(None) => {
                return Err(import_v2_error_response(404, "Preview bill not found"));
            }
            Err(error) => return Err(db_error_response(error)),
        }
        patches.push(build_preview_patch_from_payload(preview_id, item));
    }
    update_preview_bills_batch(runtime.connection_mut(), session_id, user_id, &patches)
        .map_err(db_error_response)
}

fn preview_ids_from_payload(payload: &Value) -> Vec<i64> {
    let Ok(object) = payload_object(payload) else {
        return Vec::new();
    };
    let mut preview_ids = Vec::new();
    if let Some(value) = first_value(object, &["preview_ids", "previewIds"]) {
        match value {
            Value::Array(values) => {
                preview_ids.extend(values.iter().filter_map(value_to_i64).filter(|id| *id > 0));
            }
            other => {
                if let Some(preview_id) = value_to_i64(other).filter(|id| *id > 0) {
                    preview_ids.push(preview_id);
                }
            }
        }
    }
    if preview_ids.is_empty() {
        if let Ok(items) = preview_update_items_from_payload(payload) {
            preview_ids.extend(
                items
                    .iter()
                    .filter_map(|item| preview_id_from_payload(item).ok()),
            );
        }
    }
    preview_ids.sort_unstable();
    preview_ids.dedup();
    preview_ids
}

fn expected_state_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewExpectedState, ImportV2RouteResponse> {
    let expected_state = first_value(object, &["expectedState", "expected_state"])
        .and_then(Value::as_object)
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))?;
    Ok(ImportPreviewExpectedState {
        session_id: first_value(expected_state, &["sessionId", "session_id"])
            .and_then(value_to_text),
        preview_type: first_value(expected_state, &["type", "previewType", "preview_type"])
            .and_then(value_to_text),
        preview_main_category: first_value(
            expected_state,
            &[
                "mainCategory",
                "previewMainCategory",
                "preview_main_category",
            ],
        )
        .and_then(value_to_text),
        preview_sub_category: first_value(
            expected_state,
            &["subCategory", "previewSubCategory", "preview_sub_category"],
        )
        .and_then(value_to_text),
        preview_recurring_id: optional_id_field_from_object(
            expected_state,
            &[
                "recurringTemplateId",
                "recurringId",
                "previewRecurringId",
                "preview_recurring_id",
            ],
        ),
        preview_source_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "sourceAccountId",
                "previewSourceAccountId",
                "preview_source_account_id",
            ],
        ),
        preview_destination_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "destinationAccountId",
                "previewDestinationAccountId",
                "preview_destination_account_id",
            ],
        ),
        preview_matching_feedback: first_value(
            expected_state,
            &[
                "matchingFeedback",
                "previewMatchingFeedback",
                "preview_matching_feedback",
            ],
        )
        .cloned(),
    })
}

fn optional_id_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Option<Option<i64>> {
    first_value(object, keys).map(|value| match value_to_i64(value) {
        Some(value) if value > 0 => Some(value),
        _ => None,
    })
}

fn decision_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewDecision, ImportV2RouteResponse> {
    let decision = first_value(object, &["decision"])
        .and_then(value_to_text)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match decision.as_str() {
        "accept" | "accepted" => Ok(ImportPreviewDecision::Accept),
        "reject" | "rejected" => Ok(ImportPreviewDecision::Reject),
        "clear" | "cleared" => Ok(ImportPreviewDecision::Clear),
        _ => Err(import_v2_error_response(400, "Invalid decision")),
    }
}

fn decision_name(decision: ImportPreviewDecision) -> &'static str {
    match decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    }
}

fn recurring_candidate_from_payload(
    object: &Map<String, Value>,
    recurring_id: i64,
) -> Option<ImportPreviewRecurringCandidate> {
    let candidate = first_value(
        object,
        &[
            "candidate",
            "targetCandidate",
            "target_candidate",
            "recurringCandidate",
            "recurring_candidate",
        ],
    )
    .and_then(Value::as_object)?;
    Some(ImportPreviewRecurringCandidate {
        id: first_value(candidate, &["id", "recurringId", "recurring_id"])
            .and_then(value_to_i64)
            .unwrap_or(recurring_id),
        name: first_value(candidate, &["name", "recurringName", "recurring_name"])
            .and_then(value_to_text)
            .unwrap_or_default(),
        match_score: first_value(candidate, &["matchScore", "match_score"])
            .and_then(value_to_f64)
            .unwrap_or_default(),
        match_reasons: match_reasons_from_value(first_value(
            candidate,
            &["matchReasons", "match_reasons"],
        )),
        matched_occurrence_date: first_value(
            candidate,
            &["matchedOccurrenceDate", "matched_occurrence_date"],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
    })
}

fn recurring_candidate_count_from_payload(
    object: &Map<String, Value>,
    target_candidate: Option<&ImportPreviewRecurringCandidate>,
) -> i64 {
    first_value(
        object,
        &[
            "candidateCount",
            "candidate_count",
            "recurringCandidateCount",
            "previewRecurringCandidateCount",
            "preview_recurring_candidate_count",
        ],
    )
    .and_then(value_to_i64)
    .unwrap_or_else(|| i64::from(target_candidate.is_some()))
}

fn match_reasons_from_value(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(value_to_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect(),
        Some(value) => value_to_text(value)
            .unwrap_or_default()
            .split(['|', ','])
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    }
}

fn preview_decision_result_response(
    result: ImportPreviewDecisionResult,
    extra: Value,
) -> ImportV2RouteResponse {
    if result.state_conflict {
        return preview_state_conflict_response();
    }
    if result.invalid_recurring_id {
        return import_v2_error_response(400, "Invalid recurringId");
    }
    let Some(preview) = result.preview else {
        return import_v2_error_response(404, "Preview bill not found");
    };
    let preview_id = preview.id;
    let session_id = preview.session_id.clone();
    let preview_item = preview_row_to_value(preview);
    let mut data = json!({
        "previewId": preview_id,
        "sessionId": session_id,
        "previewItem": preview_item.clone(),
        "preview": [preview_item],
    });
    if let (Some(data), Some(extra)) = (data.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            data.insert(key.clone(), value.clone());
        }
    }
    import_v2_data_response(data)
}

fn llm_suggestion_from_value(value: &Value) -> Option<ImportPreviewLlmSuggestion> {
    let object = value.as_object()?;
    let suggestion = ImportPreviewLlmSuggestion {
        suggested_main_category: first_value(
            object,
            &[
                "suggested_main_category",
                "suggestedMainCategory",
                "mainCategory",
                "main_category",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_sub_category: first_value(
            object,
            &[
                "suggested_sub_category",
                "suggestedSubCategory",
                "subCategory",
                "sub_category",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_source_account: first_value(
            object,
            &[
                "suggested_source_account",
                "suggestedSourceAccount",
                "sourceAccount",
                "source_account",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_destination_account: first_value(
            object,
            &[
                "suggested_destination_account",
                "suggestedDestinationAccount",
                "destinationAccount",
                "destination_account",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        resolved_source_account_id: first_value(
            object,
            &[
                "resolved_source_account_id",
                "resolvedSourceAccountId",
                "sourceAccountId",
                "source_account_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        resolved_destination_account_id: first_value(
            object,
            &[
                "resolved_destination_account_id",
                "resolvedDestinationAccountId",
                "destinationAccountId",
                "destination_account_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        confidence: first_value(object, &["confidence"])
            .and_then(value_to_f64)
            .unwrap_or_default(),
        reason: first_value(object, &["reason"])
            .and_then(value_to_text)
            .unwrap_or_default(),
    };
    Some(suggestion)
}

fn llm_decision_result_response(
    result: ImportPreviewLlmDecisionResult,
    session_id: &str,
    preview_id: i64,
    decision: &str,
) -> ImportV2RouteResponse {
    let Some(preview) = result.preview else {
        return import_v2_error_response(404, "Preview recommendation is no longer available");
    };
    let llm_payload = preview
        .preview_matching_feedback
        .get("llm")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let preview = preview_row_to_value(preview);
    import_v2_data_response(json!({
        "session_id": session_id,
        "preview_id": preview_id,
        "preview": preview,
        "matching": {"llm": llm_payload},
        "event_id": result.event_id,
        "applied_fields": result.applied_fields,
        "decision": decision,
        "restored": result.restored,
    }))
}

fn first_value<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn push_text_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_text) {
        changes.push((field, ImportPreviewPatchValue::Text(value)));
    }
}

fn push_preview_type_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_preview_type_text) {
        changes.push((
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(value),
        ));
    }
}

fn push_real_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_f64) {
        changes.push((field, ImportPreviewPatchValue::Real(value)));
    }
}

fn push_i64_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_i64) {
        changes.push((field, ImportPreviewPatchValue::Integer(value)));
    }
}

fn push_nullable_i64_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys) {
        let patch_value = match value_to_i64(value) {
            Some(value) if value > 0 => ImportPreviewPatchValue::Integer(value),
            _ => ImportPreviewPatchValue::Null,
        };
        changes.push((field, patch_value));
    }
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn value_to_preview_type_text(value: &Value) -> Option<String> {
    if let Some(label) = value_to_i64(value).and_then(transaction_type_label_from_i64) {
        return Some(label.to_string());
    }
    let text = value_to_text(value)?;
    normalize_transaction_type_text(&text)
}

fn normalize_transaction_type_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = match trimmed.to_ascii_lowercase().as_str() {
        "expense" => "支出",
        "income" => "收入",
        "transfer" => "转账",
        "investment" => "投资",
        _ => trimmed,
    };
    Some(normalized.to_string())
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Bool(value) => Some(i64::from(*value)),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<f64>().ok()
            }
        }
        Value::Bool(value) => Some(f64::from(u8::from(*value))),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn response_mode_is_preview_item(object: &Map<String, Value>) -> bool {
    first_value(object, &["responseMode", "response_mode"]).is_some_and(|value| {
        value
            .as_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("preview-item"))
    })
}

fn preview_row_to_value(row: ImportPreviewRow) -> Value {
    serde_json::to_value(row).unwrap_or_else(|_| json!({}))
}

fn preview_row_is_categorized(row: &&ImportPreviewRow) -> bool {
    !row.preview_main_category.trim().is_empty() || !row.preview_sub_category.trim().is_empty()
}

fn preview_row_has_account(row: &&ImportPreviewRow) -> bool {
    row.preview_source_account_id.is_some() || row.preview_destination_account_id.is_some()
}

fn open_runtime(state: &ProxyState) -> Result<SqliteRuntime, ImportV2RouteResponse> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        import_v2_error_response(
            503,
            "Rust import DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        )
    })?;
    let db_path = SqliteDbPath::application_file(db_path)
        .map_err(|error| import_v2_error_response(503, &error.to_string()))?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: state.config.timeout,
    })
    .map_err(db_error_response)
}

fn init_import_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_import_staging_schema(runtime.connection()).map_err(db_error_response)
}

fn init_ocr_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_app_settings_schema(runtime.connection()).map_err(db_error_response)
}

fn init_llm_config_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_llm_runtime_schema(runtime.connection()).map_err(db_error_response)
}

fn user_id_from_headers(
    headers: &HeaderMap,
    config: &HttpShellConfig,
) -> Result<UserId, ImportV2RouteResponse> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| import_v2_error_response(error.status, &error.message))
}

fn db_error_response(_error: impl std::fmt::Display) -> ImportV2RouteResponse {
    import_v2_error_response(500, "Rust import route runtime DB error")
}

fn non_negative_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

#[derive(Debug, Default, Deserialize)]
pub struct ImportConfigQuery {
    file_format: Option<String>,
    #[serde(rename = "fileFormat")]
    file_format_camel: Option<String>,
    limit: Option<usize>,
}

impl ImportConfigQuery {
    fn file_format(&self) -> Option<&String> {
        self.file_format
            .as_ref()
            .or(self.file_format_camel.as_ref())
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ImportLearningRulesQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    limit: Option<usize>,
    enabled_only: Option<bool>,
    #[serde(rename = "enabledOnly")]
    enabled_only_camel: Option<bool>,
}

impl ImportLearningRulesQuery {
    fn page_size(&self) -> usize {
        self.page_size
            .or(self.page_size_camel)
            .or(self.limit)
            .unwrap_or(100)
    }

    fn enabled_only(&self) -> Option<bool> {
        self.enabled_only.or(self.enabled_only_camel)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct LearningCenterListQuery {
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    enabled_only: Option<bool>,
    #[serde(rename = "enabledOnly")]
    enabled_only_camel: Option<bool>,
}

impl LearningCenterListQuery {
    fn enabled_only(&self) -> Option<bool> {
        self.enabled_only.or(self.enabled_only_camel)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct PreviewPageQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    selected_only: Option<bool>,
    #[serde(rename = "selectedOnly")]
    selected_only_camel: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LlmMemoryQuery {
    session_id: Option<String>,
    event_type: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LlmCandidatesQuery {
    status: Option<String>,
    r#type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}
