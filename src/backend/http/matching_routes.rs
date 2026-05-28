// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::collections::BTreeSet;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use bill_analyser_core::{
    matching::{
        build_matching_candidate_action_payload, detect_recurring_patterns,
        normalize_reconcile_history_families, parse_manual_pair_request,
        parse_reconciliation_candidates_query, serialize_recurring_suggestions,
    },
    UserId,
};
use bill_analyser_db::{
    accept_postgres_recurring_suggestion, accept_recurring_suggestion,
    apply_matching_candidate_action, count_postgres_recurring_suggestions,
    count_recurring_suggestions, create_manual_matching_pair, create_postgres_manual_matching_pair,
    delete_manual_matching_pair, delete_postgres_manual_matching_pair,
    detect_and_save_postgres_recurring_suggestions, detect_and_save_recurring_suggestions,
    get_bills_linked_to_recurring, get_postgres_bills_linked_to_recurring,
    list_postgres_recent_bills_for_recurring_detection, list_postgres_recurring_suggestions,
    list_recent_bills_for_recurring_detection, list_reconciliation_candidates_payload,
    list_recurring_suggestions, query_calendar_events_payload,
    query_matching_bill_candidates_payload, query_matching_bill_feedback_payload,
    query_matching_pairs_payload, query_matching_session_candidates_payload,
    query_net_worth_payload, query_postgres_calendar_events_payload,
    query_postgres_matching_bill_candidates_payload, query_postgres_matching_bill_feedback_payload,
    query_postgres_matching_pairs_payload, query_postgres_matching_session_candidates_payload,
    query_postgres_net_worth_payload, query_postgres_reconciliation_candidates_payload,
    reject_postgres_recurring_suggestion, reject_recurring_suggestion, ImportPreviewExpectedState,
    ImportPreviewLearningApply, ImportPreviewRecurringCandidate, MatchingRuntimeError,
    PostgresRepositoryRuntime, PreviewMatchingActionRequest, ReconciliationCandidateFilters,
    SqliteRuntime,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

type RouteResult<T> = Result<T, Box<Response>>;

pub const MATCHING_RECURRING_CALENDAR_NETWORTH_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/calendar/events"),
    ("GET", "/api/matching/bills/{bill_id}/candidates"),
    ("GET", "/api/matching/bills/{bill_id}/feedback"),
    ("GET", "/api/matching/candidates"),
    ("POST", "/api/matching/candidates/{*candidate_id}/accept"),
    ("POST", "/api/matching/candidates/{*candidate_id}/clear"),
    ("POST", "/api/matching/candidates/{*candidate_id}/reject"),
    ("GET", "/api/matching/investment-settings"),
    ("PUT", "/api/matching/investment-settings"),
    ("POST", "/api/matching/manual-pair"),
    ("GET", "/api/matching/pairs"),
    ("DELETE", "/api/matching/pairs/{pair_id}"),
    ("POST", "/api/matching/reconcile-history"),
    ("GET", "/api/matching/reconciliation-candidates"),
    ("GET", "/api/matching/sessions/{session_id}/candidates"),
    ("GET", "/api/networth/snapshot"),
    ("GET", "/api/recurring/suggestions"),
    ("POST", "/api/recurring/suggestions/detect"),
    ("POST", "/api/recurring/suggestions/{suggestion_id}/accept"),
    ("POST", "/api/recurring/suggestions/{suggestion_id}/reject"),
];

#[tracing::instrument(level = "debug", skip_all)]
pub fn matching_recurring_calendar_networth_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route("/api/calendar/events", get(calendar_events_handler))
        .route(
            "/api/matching/bills/:bill_id/candidates",
            get(matching_bill_candidates_handler),
        )
        .route(
            "/api/matching/bills/:bill_id/feedback",
            get(matching_bill_feedback_handler),
        )
        .route("/api/matching/candidates", get(matching_candidates_handler))
        .route(
            "/api/matching/candidates/:candidate_id/accept",
            post(accept_matching_candidate_handler),
        )
        .route(
            "/api/matching/candidates/:candidate_id/clear",
            post(clear_matching_candidate_handler),
        )
        .route(
            "/api/matching/candidates/:candidate_id/reject",
            post(reject_matching_candidate_handler),
        )
        .route(
            "/api/matching/investment-settings",
            get(investment_settings_gone_handler).put(investment_settings_gone_handler),
        )
        .route(
            "/api/matching/manual-pair",
            post(create_manual_pair_handler),
        )
        .route("/api/matching/pairs", get(matching_pairs_handler))
        .route(
            "/api/matching/pairs/:pair_id",
            delete(delete_manual_pair_handler),
        )
        .route(
            "/api/matching/reconcile-history",
            post(reconcile_history_handler),
        )
        .route(
            "/api/matching/reconciliation-candidates",
            get(reconciliation_candidates_handler),
        )
        .route(
            "/api/matching/sessions/:session_id/candidates",
            get(matching_session_candidates_handler),
        )
        .route("/api/networth/snapshot", get(networth_snapshot_handler))
        .route("/api/recurring/suggestions", get(list_suggestions_handler))
        .route(
            "/api/recurring/suggestions/detect",
            post(detect_suggestions_handler),
        )
        .route(
            "/api/recurring/suggestions/:suggestion_id/accept",
            post(accept_suggestion_handler),
        )
        .route(
            "/api/recurring/suggestions/:suggestion_id/reject",
            post(reject_suggestion_handler),
        )
}

#[derive(Debug, Default, Deserialize)]
struct CalendarEventsQuery {
    start_date: Option<String>,
    end_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RecurringSuggestionsQuery {
    status: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MatchingCandidatesQuery {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "billId")]
    bill_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ReconciliationCandidatesQuery {
    #[serde(rename = "candidateType")]
    candidate_type: Option<String>,
    status: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "previewId")]
    preview_id: Option<String>,
    #[serde(rename = "billId")]
    bill_id: Option<String>,
    limit: Option<String>,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn calendar_events_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "calendar_events_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let start_date = match required_date(query.start_date.as_deref(), "start_date") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let end_date = match required_date(query.end_date.as_deref(), "end_date") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching calendar") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_calendar_events_payload(
            runtime.pool(),
            user_id,
            start_date,
            end_date,
        )
        .await
        {
            Ok(data) => success_data(StatusCode::OK, json!(data)),
            Err(error) => db_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "matching calendar") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_calendar_events_payload(runtime.connection(), user_id, start_date, end_date) {
        Ok(data) => success_data(StatusCode::OK, json!(data)),
        Err(error) => db_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn networth_snapshot_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "networth_snapshot_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "networth") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_net_worth_payload(runtime.pool(), user_id).await {
            Ok(snapshot) => success_data(StatusCode::OK, snapshot),
            Err(error) => db_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "networth") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_net_worth_payload(runtime.connection(), user_id) {
        Ok(snapshot) => success_data(StatusCode::OK, snapshot),
        Err(error) => db_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn list_suggestions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<RecurringSuggestionsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "list_suggestions_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let limit = match parse_limit(query.limit.as_deref()) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let offset = match parse_offset(query.offset.as_deref()) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let total =
            match count_postgres_recurring_suggestions(runtime.pool(), user_id, status).await {
                Ok(value) => value,
                Err(error) => return db_error_response(error),
            };
        return match list_postgres_recurring_suggestions(
            runtime.pool(),
            user_id,
            status,
            limit,
            offset,
        )
        .await
        {
            Ok(items) => success_data(
                StatusCode::OK,
                json!({
                    "total": total,
                    "items": serialize_recurring_suggestions(&items),
                    "limit": limit,
                    "offset": offset,
                }),
            ),
            Err(error) => db_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let total = match count_recurring_suggestions(runtime.connection(), user_id, status) {
        Ok(value) => value,
        Err(error) => return db_error_response(error),
    };
    match list_recurring_suggestions(runtime.connection(), user_id, status, limit, offset) {
        Ok(items) => success_data(
            StatusCode::OK,
            json!({
                "total": total,
                "items": serialize_recurring_suggestions(&items),
                "limit": limit,
                "offset": offset,
            }),
        ),
        Err(error) => db_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn detect_suggestions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "detect_suggestions_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let bills = match list_postgres_recent_bills_for_recurring_detection(
            runtime.pool(),
            user_id,
            10_000,
        )
        .await
        {
            Ok(value) => value,
            Err(error) => return db_error_response(error),
        };
        let linked_ids = match get_postgres_bills_linked_to_recurring(runtime.pool(), user_id).await
        {
            Ok(value) => value,
            Err(error) => return db_error_response(error),
        };
        let patterns = detect_recurring_patterns(&bills, 3, &linked_ids);
        return match detect_and_save_postgres_recurring_suggestions(
            runtime.pool(),
            user_id,
            &patterns,
        )
        .await
        {
            Ok(summary) => success_data(
                StatusCode::OK,
                json!({
                    "detected": patterns.len(),
                    "created": summary.created,
                    "updated": summary.updated,
                    "skipped": summary.skipped,
                }),
            ),
            Err(error) => db_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bills =
        match list_recent_bills_for_recurring_detection(runtime.connection(), user_id, 10_000) {
            Ok(value) => value,
            Err(error) => return db_error_response(error),
        };
    let linked_ids = match get_bills_linked_to_recurring(runtime.connection(), user_id) {
        Ok(value) => value,
        Err(error) => return db_error_response(error),
    };
    let patterns = detect_recurring_patterns(&bills, 3, &linked_ids);
    match detect_and_save_recurring_suggestions(runtime.connection(), user_id, &patterns) {
        Ok(summary) => success_data(
            StatusCode::OK,
            json!({
                "detected": patterns.len(),
                "created": summary.created,
                "updated": summary.updated,
                "skipped": summary.skipped,
            }),
        ),
        Err(error) => db_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn accept_suggestion_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "accept_suggestion_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match accept_postgres_recurring_suggestion(runtime.pool(), user_id, suggestion_id)
            .await
        {
            Ok(Some(value)) => success_data(StatusCode::OK, value),
            Ok(None) => not_found("Suggestion not found or already processed"),
            Err(error) => db_error_response(error),
        };
    }
    let mut runtime = match open_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match accept_recurring_suggestion(runtime.connection_mut(), user_id, suggestion_id) {
        Ok(Some(value)) => success_data(StatusCode::OK, value),
        Ok(None) => not_found("Suggestion not found or already processed"),
        Err(error) => db_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reject_suggestion_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reject_suggestion_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match reject_postgres_recurring_suggestion(runtime.pool(), user_id, suggestion_id)
            .await
        {
            Ok(true) => success_data(StatusCode::OK, json!({ "status": "rejected" })),
            Ok(false) => not_found("Suggestion not found or already processed"),
            Err(error) => db_error_response(error),
        };
    }
    let mut runtime = match open_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match reject_recurring_suggestion(runtime.connection_mut(), user_id, suggestion_id) {
        Ok(true) => success_data(StatusCode::OK, json!({ "status": "rejected" })),
        Ok(false) => not_found("Suggestion not found or already processed"),
        Err(error) => db_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_session_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_session_candidates_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_matching_session_candidates_payload(
            runtime.pool(),
            user_id,
            &session_id,
        )
        .await
        {
            Ok(Some(data)) => success_data(StatusCode::OK, data),
            Ok(None) => error_response(StatusCode::NOT_FOUND, "Import session not found"),
            Err(error) => matching_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_matching_session_candidates_payload(runtime.connection(), user_id, &session_id) {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Import session not found"),
        Err(error) => matching_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_bill_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_bill_candidates_handler",
        "business operation entered"
    );
    matching_bill_candidates_response(&state, &headers, bill_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<MatchingCandidatesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_candidates_handler",
        "business operation entered"
    );
    let session_id = query
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let raw_bill_id = query
        .bill_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if session_id.is_some() == raw_bill_id.is_some() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Exactly one of sessionId or billId is required",
        );
    }
    if let Some(session_id) = session_id {
        return matching_session_candidates_response(&state, &headers, session_id).await;
    }
    let bill_id = match raw_bill_id.and_then(|value| value.parse::<i64>().ok()) {
        Some(value) if value > 0 => value,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid billId"),
    };
    matching_bill_candidates_response(&state, &headers, bill_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_bill_feedback_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_bill_feedback_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_matching_bill_feedback_payload(runtime.pool(), user_id, bill_id)
            .await
        {
            Ok(Some(data)) => success_data(StatusCode::OK, data),
            Ok(None) => error_response(StatusCode::NOT_FOUND, "Bill not found"),
            Err(error) => matching_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_matching_bill_feedback_payload(runtime.connection(), user_id, bill_id) {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Bill not found"),
        Err(error) => matching_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reconciliation_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<ReconciliationCandidatesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reconciliation_candidates_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let query_map = reconciliation_query_to_map(query);
    let parsed_query = match parse_reconciliation_candidates_query(&query_map) {
        Ok(value) => value,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let filters = reconciliation_filters_from_value(&parsed_query);
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_reconciliation_candidates_payload(runtime.pool(), user_id).await
        {
            Ok(data) => success_data(StatusCode::OK, data),
            Err(error) => matching_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match list_reconciliation_candidates_payload(runtime.connection(), user_id, &filters) {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_pairs_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_pairs_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_matching_pairs_payload(runtime.pool(), user_id).await {
            Ok(data) => success_data(StatusCode::OK, data),
            Err(error) => matching_error_response(error),
        };
    }
    let runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_matching_pairs_payload(runtime.connection(), user_id) {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_manual_pair_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "create_manual_pair_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let request = match parse_manual_pair_request(&payload) {
        Ok(value) => value,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match create_postgres_manual_matching_pair(
            runtime.pool(),
            user_id,
            request.bill_id,
            request.candidate_bill_id,
            &request.pair_type,
        )
        .await
        {
            Ok(data) => success_data(StatusCode::OK, data),
            Err(error) => matching_error_response(error),
        };
    }
    let mut runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match create_manual_matching_pair(
        runtime.connection_mut(),
        user_id,
        request.bill_id,
        request.candidate_bill_id,
        &request.pair_type,
        None,
    ) {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_manual_pair_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(pair_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "delete_manual_pair_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match delete_postgres_manual_matching_pair(runtime.pool(), user_id, pair_id).await {
            Ok(data) => success_data(StatusCode::OK, data),
            Err(error) => matching_error_response(error),
        };
    }
    let mut runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match delete_manual_matching_pair(runtime.connection_mut(), user_id, pair_id) {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn accept_matching_candidate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "accept_matching_candidate_handler",
        "business operation entered"
    );
    matching_candidate_action_response(&state, &headers, candidate_id, "accept", payload)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reject_matching_candidate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reject_matching_candidate_handler",
        "business operation entered"
    );
    matching_candidate_action_response(&state, &headers, candidate_id, "reject", payload)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_matching_candidate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "clear_matching_candidate_handler",
        "business operation entered"
    );
    matching_candidate_action_response(&state, &headers, candidate_id, "clear", payload)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn investment_settings_gone_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "investment_settings_gone_handler",
        "business operation entered"
    );
    match user_id_from_headers(&headers, &state.config) {
        Ok(_) => error_response(
            StatusCode::GONE,
            "Investment recognition settings are managed by category rules",
        ),
        Err(response) => *response,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reconcile_history_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reconcile_history_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return error_response(StatusCode::BAD_REQUEST, "Invalid request");
    };
    let bill_ids = match parse_reconcile_history_bill_ids(object) {
        Ok(value) => value,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let families = normalize_reconcile_history_families(object.get("families"));
    let allowed_families = families.iter().map(String::as_str).collect::<Vec<_>>();
    if state.config.database_backend.uses_postgres() {
        return reconcile_history_postgres_response(&state, user_id, &bill_ids, &allowed_families)
            .await;
    }
    let runtime = match open_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let mut results = Vec::new();
    let mut candidate_count = 0usize;
    let mut linked_pair_keys = BTreeSet::new();
    for bill_id in &bill_ids {
        let payload =
            match query_matching_bill_candidates_payload(runtime.connection(), user_id, *bill_id) {
                Ok(Some(value)) => value,
                Ok(None) => return error_response(StatusCode::NOT_FOUND, "Bill not found"),
                Err(error) => return matching_error_response(error),
            };
        let linked_pair = payload
            .get("linkedPair")
            .filter(|value| value.is_object())
            .filter(|value| pair_type_in_allowed_families(value, &allowed_families))
            .cloned()
            .unwrap_or(Value::Null);
        let candidates = payload
            .get("candidates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|candidate| {
                        candidate_kind_in_allowed_families(candidate, &allowed_families)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        candidate_count += candidates.len();
        if linked_pair.is_object() {
            if let Some(pair_key) = matching_history_pair_key(&linked_pair) {
                linked_pair_keys.insert(pair_key);
            }
        }
        results.push(json!({
            "billId": payload.get("billId").and_then(value_to_i64).unwrap_or(*bill_id),
            "linkedPair": linked_pair,
            "candidates": candidates,
            "reconciliation": payload.get("reconciliation").cloned().unwrap_or(Value::Null),
        }));
    }

    success_data(
        StatusCode::OK,
        json!({
            "summary": {
                "billCount": results.len(),
                "candidateCount": candidate_count,
                "linkedPairCount": linked_pair_keys.len(),
            },
            "results": results,
        }),
    )
}

fn open_runtime(state: &HttpAppState, label: &'static str) -> RouteResult<SqliteRuntime> {
    state
        .open_sqlite_repository_runtime(label)
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message("Rust matching route runtime DB error"),
            ))
        })
}

async fn reconcile_history_postgres_response(
    state: &HttpAppState,
    user_id: UserId,
    bill_ids: &[i64],
    allowed_families: &[&str],
) -> Response {
    let runtime = match open_postgres_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut results = Vec::new();
    let mut candidate_count = 0usize;
    let mut linked_pair_keys = BTreeSet::new();
    for bill_id in bill_ids {
        let payload = match query_postgres_matching_bill_candidates_payload(
            runtime.pool(),
            user_id,
            *bill_id,
        )
        .await
        {
            Ok(Some(value)) => value,
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Bill not found"),
            Err(error) => return matching_error_response(error),
        };
        let linked_pair = payload
            .get("linkedPair")
            .filter(|value| value.is_object())
            .filter(|value| pair_type_in_allowed_families(value, allowed_families))
            .cloned()
            .unwrap_or(Value::Null);
        let candidates = payload
            .get("candidates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|candidate| {
                        candidate_kind_in_allowed_families(candidate, allowed_families)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        candidate_count += candidates.len();
        if linked_pair.is_object() {
            if let Some(pair_key) = matching_history_pair_key(&linked_pair) {
                linked_pair_keys.insert(pair_key);
            }
        }
        results.push(json!({
            "billId": payload.get("billId").and_then(value_to_i64).unwrap_or(*bill_id),
            "linkedPair": linked_pair,
            "candidates": candidates,
            "reconciliation": payload.get("reconciliation").cloned().unwrap_or(Value::Null),
        }));
    }
    success_data(
        StatusCode::OK,
        json!({
            "summary": {
                "billCount": results.len(),
                "candidateCount": candidate_count,
                "linkedPairCount": linked_pair_keys.len(),
            },
            "results": results,
        }),
    )
}

fn open_postgres_runtime(
    state: &HttpAppState,
    label: &'static str,
) -> RouteResult<PostgresRepositoryRuntime> {
    state
        .open_postgres_repository_runtime(label)
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message("Rust matching route PostgreSQL runtime DB error"),
            ))
        })
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(message_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn required_date(value: Option<&str>, name: &str) -> RouteResult<NaiveDate> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(Box::new(message_response(
            StatusCode::BAD_REQUEST,
            "start_date and end_date are required",
        )));
    };
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        Box::new(message_response(
            StatusCode::BAD_REQUEST,
            format!("{name} must use YYYY-MM-DD format"),
        ))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_limit(value: Option<&str>) -> RouteResult<usize> {
    let limit = match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.parse::<usize>().map_err(|error| {
            Box::new(message_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            ))
        })?,
        None => 200,
    };
    Ok(limit.min(1000))
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_offset(value: Option<&str>) -> RouteResult<usize> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.parse::<usize>().map_err(|error| {
            Box::new(message_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            ))
        }),
        None => Ok(0),
    }
}

async fn matching_session_candidates_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    session_id: &str,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_matching_session_candidates_payload(
            runtime.pool(),
            user_id,
            session_id,
        )
        .await
        {
            Ok(Some(data)) => success_data(StatusCode::OK, data),
            Ok(None) => error_response(StatusCode::NOT_FOUND, "Import session not found"),
            Err(error) => matching_error_response(error),
        };
    }
    let runtime = match open_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_matching_session_candidates_payload(runtime.connection(), user_id, session_id) {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Import session not found"),
        Err(error) => matching_error_response(error),
    }
}

async fn matching_bill_candidates_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    bill_id: i64,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(state, "matching") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_matching_bill_candidates_payload(
            runtime.pool(),
            user_id,
            bill_id,
        )
        .await
        {
            Ok(Some(data)) => success_data(StatusCode::OK, data),
            Ok(None) => error_response(StatusCode::NOT_FOUND, "Bill not found"),
            Err(error) => matching_error_response(error),
        };
    }
    let runtime = match open_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_matching_bill_candidates_payload(runtime.connection(), user_id, bill_id) {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Bill not found"),
        Err(error) => matching_error_response(error),
    }
}

fn matching_candidate_action_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    candidate_id: String,
    action: &str,
    payload: Option<Json<Value>>,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return error_response(StatusCode::BAD_REQUEST, "Invalid request");
    };
    let request = match preview_action_request_from_payload(object) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        return error_response(
            StatusCode::CONFLICT,
            "PostgreSQL matching candidate actions require a materialized candidate",
        );
    }
    let mut runtime = match open_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match apply_matching_candidate_action(
        runtime.connection_mut(),
        user_id,
        &candidate_id,
        action,
        &request,
    ) {
        Ok(result) => {
            let mut action_payload = result
                .as_object()
                .map(|object| build_matching_candidate_action_payload(&candidate_id, object))
                .unwrap_or_else(|| json!({ "candidateId": candidate_id, "action": action }));
            if let (Some(target), Some(pair)) = (
                action_payload.as_object_mut(),
                result
                    .get("pair")
                    .filter(|value| value.get("leftBillId").is_some()),
            ) {
                target.insert("pair".to_string(), pair.clone());
            }
            success_data(StatusCode::OK, action_payload)
        }
        Err(error) => matching_error_response(error),
    }
}

fn preview_action_request_from_payload(
    object: &Map<String, Value>,
) -> RouteResult<PreviewMatchingActionRequest> {
    Ok(PreviewMatchingActionRequest {
        expected_state: expected_state_from_payload(object)?,
        response_mode_preview_item: response_mode_is_preview_item(object),
        reviewed_type: first_value(object, &["reviewedType", "reviewed_type", "type"])
            .and_then(value_to_text),
        recurring_id: first_value(object, &["recurringId", "recurring_id"])
            .and_then(value_to_i64)
            .filter(|value| *value > 0),
        recurring_candidate_count: recurring_candidate_count_from_payload(object),
        recurring_candidate: recurring_candidate_from_payload(object),
        learning_apply: learning_apply_from_payload(object),
    })
}

fn expected_state_from_payload(
    object: &Map<String, Value>,
) -> RouteResult<Option<ImportPreviewExpectedState>> {
    let Some(expected_state_value) = first_value(object, &["expectedState", "expected_state"])
    else {
        return Ok(None);
    };
    let Some(expected_state) = expected_state_value.as_object() else {
        return Err(Box::new(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid request",
        )));
    };
    Ok(Some(ImportPreviewExpectedState {
        session_id: first_value(expected_state, &["sessionId", "session_id"])
            .and_then(value_to_text),
        preview_type: first_value(expected_state, &["type", "previewType", "preview_type"])
            .and_then(value_to_text)
            .map(|value| normalize_preview_type_text(&value)),
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
    }))
}

fn learning_apply_from_payload(object: &Map<String, Value>) -> Option<ImportPreviewLearningApply> {
    let apply = ImportPreviewLearningApply {
        preview_type: first_value(object, &["type", "previewType", "preview_type"])
            .and_then(value_to_text)
            .map(|value| normalize_preview_type_text(&value)),
        preview_main_category: first_value(
            object,
            &[
                "mainCategory",
                "previewMainCategory",
                "preview_main_category",
            ],
        )
        .and_then(value_to_text),
        preview_sub_category: first_value(
            object,
            &["subCategory", "previewSubCategory", "preview_sub_category"],
        )
        .and_then(value_to_text),
        preview_source_account_id: optional_id_field_from_object(
            object,
            &[
                "sourceAccountId",
                "previewSourceAccountId",
                "preview_source_account_id",
            ],
        ),
        preview_destination_account_id: optional_id_field_from_object(
            object,
            &[
                "destinationAccountId",
                "previewDestinationAccountId",
                "preview_destination_account_id",
            ],
        ),
        rule_id: first_value(object, &["ruleId", "rule_id"])
            .and_then(value_to_i64)
            .filter(|value| *value > 0),
    };
    if apply.preview_type.is_some()
        || apply.preview_main_category.is_some()
        || apply.preview_sub_category.is_some()
        || apply.preview_source_account_id.is_some()
        || apply.preview_destination_account_id.is_some()
        || apply.rule_id.is_some()
    {
        Some(apply)
    } else {
        None
    }
}

fn recurring_candidate_from_payload(
    object: &Map<String, Value>,
) -> Option<ImportPreviewRecurringCandidate> {
    let recurring_id = first_value(object, &["recurringId", "recurring_id"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)?;
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
    .and_then(Value::as_object);
    Some(ImportPreviewRecurringCandidate {
        id: candidate
            .and_then(|candidate| first_value(candidate, &["id", "recurringId", "recurring_id"]))
            .and_then(value_to_i64)
            .filter(|value| *value > 0)
            .unwrap_or(recurring_id),
        name: candidate
            .and_then(|candidate| {
                first_value(candidate, &["name", "recurringName", "recurring_name"])
            })
            .and_then(value_to_text)
            .unwrap_or_default(),
        match_score: candidate
            .and_then(|candidate| first_value(candidate, &["matchScore", "match_score"]))
            .and_then(value_to_f64)
            .unwrap_or_default(),
        match_reasons: candidate
            .and_then(|candidate| first_value(candidate, &["matchReasons", "match_reasons"]))
            .map(match_reasons_from_value)
            .unwrap_or_default(),
        matched_occurrence_date: candidate
            .and_then(|candidate| {
                first_value(
                    candidate,
                    &[
                        "matchedOccurrenceDate",
                        "matched_occurrence_date",
                        "matchedDate",
                        "matched_date",
                    ],
                )
            })
            .and_then(value_to_text)
            .unwrap_or_default(),
    })
}

fn recurring_candidate_count_from_payload(object: &Map<String, Value>) -> i64 {
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
    .unwrap_or_else(|| {
        i64::from(
            first_value(object, &["recurringId", "recurring_id"])
                .and_then(value_to_i64)
                .is_some_and(|value| value > 0),
        )
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

fn reconciliation_query_to_map(query: ReconciliationCandidatesQuery) -> Map<String, Value> {
    let mut map = Map::new();
    insert_query_string(&mut map, "candidateType", query.candidate_type);
    insert_query_string(&mut map, "status", query.status);
    insert_query_string(&mut map, "sessionId", query.session_id);
    insert_query_string(&mut map, "previewId", query.preview_id);
    insert_query_string(&mut map, "billId", query.bill_id);
    insert_query_string(&mut map, "limit", query.limit);
    map
}

#[tracing::instrument(level = "debug", skip_all)]
fn insert_query_string(map: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        map.insert(key.to_string(), json!(value));
    }
}

fn reconciliation_filters_from_value(value: &Value) -> ReconciliationCandidateFilters {
    let object = value.as_object().cloned().unwrap_or_default();
    ReconciliationCandidateFilters {
        session_id: object.get("session_id").and_then(non_empty_value_string),
        preview_id: object.get("preview_id").and_then(value_to_i64),
        existing_bill_id: object.get("existing_bill_id").and_then(value_to_i64),
        candidate_type: object
            .get("candidate_type")
            .and_then(non_empty_value_string),
        status: object.get("status").and_then(non_empty_value_string),
        limit: object.get("limit").and_then(value_to_i64).unwrap_or(200),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_reconcile_history_bill_ids(object: &Map<String, Value>) -> Result<Vec<i64>, &'static str> {
    let Some(raw_bill_ids) = object.get("billIds") else {
        return Err("billIds is required");
    };
    let Some(raw_bill_ids) = raw_bill_ids.as_array() else {
        return Err("billIds must be a non-empty list");
    };
    if raw_bill_ids.is_empty() {
        return Err("billIds must be a non-empty list");
    }
    let mut bill_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for raw_bill_id in raw_bill_ids {
        let Some(bill_id) = value_to_i64(raw_bill_id).filter(|value| *value > 0) else {
            return Err("Invalid billIds");
        };
        if seen.insert(bill_id) {
            bill_ids.push(bill_id);
        }
    }
    Ok(bill_ids)
}

fn pair_type_in_allowed_families(value: &Value, allowed_families: &[&str]) -> bool {
    let pair_type = value
        .as_object()
        .and_then(|object| first_value(object, &["pairType", "pair_type"]))
        .and_then(value_to_text)
        .unwrap_or_else(|| "transfer".to_string())
        .trim()
        .to_ascii_lowercase();
    allowed_families.contains(&pair_type.as_str())
}

fn candidate_kind_in_allowed_families(value: &Value, allowed_families: &[&str]) -> bool {
    let kind = value
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(value_to_text)
        .unwrap_or_else(|| "transfer".to_string())
        .trim()
        .to_ascii_lowercase();
    allowed_families.contains(&kind.as_str())
}

fn matching_history_pair_key(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let pair_id = first_value(object, &["id"])
        .and_then(value_to_i64)
        .unwrap_or_default();
    if pair_id > 0 {
        return Some(format!("pair:{pair_id}"));
    }
    let left = first_value(object, &["leftBillId", "left_bill_id"]).and_then(value_to_i64)?;
    let right = first_value(object, &["rightBillId", "right_bill_id"]).and_then(value_to_i64)?;
    Some(format!("bills:{}:{}", left.min(right), left.max(right)))
}

fn first_value<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn non_empty_value_string(value: &Value) -> Option<String> {
    value_to_text(value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
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
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_preview_type_text(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "expense" => "支出".to_string(),
        "income" => "收入".to_string(),
        "transfer" => "转账".to_string(),
        "investment" => "投资".to_string(),
        _ => value.to_string(),
    }
}

fn response_mode_is_preview_item(object: &Map<String, Value>) -> bool {
    first_value(object, &["responseMode", "response_mode"]).is_some_and(|value| {
        value
            .as_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("preview-item"))
    })
}

fn match_reasons_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(value_to_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect(),
        value => value_to_text(value)
            .unwrap_or_default()
            .split(['|', ','])
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
    }
}

fn matching_error_response(error: MatchingRuntimeError) -> Response {
    error_response(status_or_internal(error.status_code()), error.message())
}

fn success_data(status: StatusCode, data: Value) -> Response {
    json_response(status, json!({ "success": true, "data": data }))
}

fn not_found(message: impl ToString) -> Response {
    message_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response(error: impl ToString) -> Response {
    message_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn message_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "message": message.to_string() }),
    )
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_action_payload_parses_expected_state_learning_and_recurring_aliases() {
        let payload = json!({
            "responseMode": "preview-item",
            "reviewedType": "transfer",
            "type": "income",
            "mainCategory": "Food",
            "subCategory": "Coffee",
            "sourceAccountId": "10",
            "destinationAccountId": null,
            "ruleId": "8",
            "recurringId": "900",
            "candidateCount": "2",
            "targetCandidate": {
                "id": "900",
                "name": "Monthly Rent",
                "matchScore": "0.91",
                "matchReasons": "same_amount|monthly",
                "matchedDate": "2026-04-01"
            },
            "expectedState": {
                "sessionId": "session-http",
                "previewType": "expense",
                "mainCategory": "Food",
                "subCategory": "Coffee",
                "recurringId": null,
                "sourceAccountId": 10,
                "destinationAccountId": null,
                "matchingFeedback": {"transfer": {"review_status": "pending"}}
            }
        });
        let request = preview_action_request_from_payload(payload.as_object().expect("object"))
            .expect("parsed action request");

        assert!(request.response_mode_preview_item);
        assert_eq!(request.reviewed_type.as_deref(), Some("transfer"));
        assert_eq!(request.recurring_id, Some(900));
        assert_eq!(request.recurring_candidate_count, 2);
        let recurring = request.recurring_candidate.expect("recurring candidate");
        assert_eq!(recurring.id, 900);
        assert_eq!(recurring.name, "Monthly Rent");
        assert_eq!(recurring.match_score, 0.91);
        assert_eq!(recurring.match_reasons, vec!["same_amount", "monthly"]);
        assert_eq!(recurring.matched_occurrence_date, "2026-04-01");

        let expected = request.expected_state.expect("expected state");
        assert_eq!(expected.session_id.as_deref(), Some("session-http"));
        assert_eq!(expected.preview_type.as_deref(), Some("支出"));
        assert_eq!(expected.preview_main_category.as_deref(), Some("Food"));
        assert_eq!(expected.preview_sub_category.as_deref(), Some("Coffee"));
        assert_eq!(expected.preview_recurring_id, Some(None));
        assert_eq!(expected.preview_source_account_id, Some(Some(10)));
        assert_eq!(expected.preview_destination_account_id, Some(None));
        assert_eq!(
            expected.preview_matching_feedback,
            Some(json!({"transfer": {"review_status": "pending"}}))
        );

        let learning = request.learning_apply.expect("learning apply");
        assert_eq!(learning.preview_type.as_deref(), Some("收入"));
        assert_eq!(learning.preview_main_category.as_deref(), Some("Food"));
        assert_eq!(learning.preview_sub_category.as_deref(), Some("Coffee"));
        assert_eq!(learning.preview_source_account_id, Some(Some(10)));
        assert_eq!(learning.preview_destination_account_id, Some(None));
        assert_eq!(learning.rule_id, Some(8));
    }

    #[test]
    fn preview_action_payload_rejects_bad_expected_state_and_handles_empty_sections() {
        let invalid = json!({"expectedState": true});
        let error = preview_action_request_from_payload(invalid.as_object().expect("object"))
            .expect_err("invalid expected state");
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);

        let empty = json!({});
        let request = preview_action_request_from_payload(empty.as_object().expect("object"))
            .expect("empty payload parses");
        assert!(request.expected_state.is_none());
        assert!(request.learning_apply.is_none());
        assert_eq!(request.recurring_candidate_count, 0);
        assert!(request.recurring_candidate.is_none());

        let fallback_recurring = json!({"recurring_id": 12});
        let request =
            preview_action_request_from_payload(fallback_recurring.as_object().expect("object"))
                .expect("fallback recurring parses");
        assert_eq!(request.recurring_id, Some(12));
        assert_eq!(request.recurring_candidate_count, 1);
        assert_eq!(request.recurring_candidate.expect("candidate").id, 12);
    }

    #[test]
    fn matching_route_helper_parsers_cover_filters_and_history_edges() {
        let query = ReconciliationCandidatesQuery {
            candidate_type: Some("Duplicate".to_string()),
            status: Some("Pending".to_string()),
            session_id: Some("session-http".to_string()),
            preview_id: Some("101".to_string()),
            bill_id: Some("401".to_string()),
            limit: Some("900".to_string()),
        };
        let query_value = Value::Object(
            parse_reconciliation_candidates_query(&reconciliation_query_to_map(query))
                .expect("query parses")
                .as_object()
                .expect("query object")
                .clone(),
        );
        let filters = reconciliation_filters_from_value(&query_value);
        assert_eq!(filters.session_id.as_deref(), Some("session-http"));
        assert_eq!(filters.preview_id, Some(101));
        assert_eq!(filters.existing_bill_id, Some(401));
        assert_eq!(filters.candidate_type.as_deref(), Some("duplicate"));
        assert_eq!(filters.status.as_deref(), Some("pending"));
        assert_eq!(filters.limit, 500);

        let empty_filters = reconciliation_filters_from_value(&json!(null));
        assert_eq!(empty_filters.limit, 200);
        assert_eq!(
            parse_reconciliation_candidates_query(&Map::from_iter([(
                "candidateType".to_string(),
                json!("unknown"),
            )]))
            .expect_err("invalid type"),
            "Invalid candidateType"
        );

        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::new()).expect_err("missing bill ids"),
            "billIds is required"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([(
                "billIds".to_string(),
                json!("20"),
            )]))
            .expect_err("not list"),
            "billIds must be a non-empty list"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([
                ("billIds".to_string(), json!([]),)
            ]))
            .expect_err("empty list"),
            "billIds must be a non-empty list"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([(
                "billIds".to_string(),
                json!([20, "20", 0]),
            )]))
            .expect_err("invalid item"),
            "Invalid billIds"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([(
                "billIds".to_string(),
                json!([20, "20", 21]),
            )]))
            .expect("deduped ids"),
            vec![20, 21]
        );
    }

    #[test]
    fn matching_route_value_helpers_cover_aliases_and_fallbacks() {
        assert!(pair_type_in_allowed_families(
            &json!({"pair_type": "investment"}),
            &["investment"]
        ));
        assert!(pair_type_in_allowed_families(&json!({}), &["transfer"]));
        assert!(!candidate_kind_in_allowed_families(
            &json!({"kind": "learning"}),
            &["transfer"]
        ));
        assert!(candidate_kind_in_allowed_families(
            &json!({"kind": "learning"}),
            &["learning"]
        ));

        assert_eq!(
            matching_history_pair_key(&json!({"id": 9})),
            Some("pair:9".to_string())
        );
        assert_eq!(
            matching_history_pair_key(&json!({"left_bill_id": 30, "rightBillId": 20})),
            Some("bills:20:30".to_string())
        );
        assert!(matching_history_pair_key(&json!({"leftBillId": 30})).is_none());

        assert_eq!(value_to_text(&json!(true)).as_deref(), Some("true"));
        assert_eq!(
            value_to_text(&json!({"a": 1})).as_deref(),
            Some("{\"a\":1}")
        );
        assert_eq!(value_to_i64(&json!("42")), Some(42));
        assert_eq!(value_to_i64(&json!(42_u64)), Some(42));
        assert!(value_to_i64(&json!("")).is_none());
        assert!(value_to_i64(&json!(false)).is_none());
        assert_eq!(value_to_f64(&json!("4.2")), Some(4.2));
        assert!(value_to_f64(&json!("")).is_none());
        assert!(value_to_f64(&json!({})).is_none());

        assert_eq!(normalize_preview_type_text("transfer"), "转账");
        assert_eq!(normalize_preview_type_text("investment"), "投资");
        assert_eq!(normalize_preview_type_text("custom"), "custom");
        assert!(response_mode_is_preview_item(
            json!({"response_mode": "preview-item"})
                .as_object()
                .expect("object")
        ));
        assert_eq!(
            match_reasons_from_value(&json!(["same", true, ""])),
            vec!["same", "true"]
        );
        assert_eq!(
            match_reasons_from_value(&json!("same|monthly,amount")),
            vec!["same", "monthly", "amount"]
        );

        assert_eq!(
            optional_id_field_from_object(
                json!({"sourceAccountId": "10"})
                    .as_object()
                    .expect("object"),
                &["sourceAccountId"]
            ),
            Some(Some(10))
        );
        assert_eq!(
            optional_id_field_from_object(
                json!({"sourceAccountId": null})
                    .as_object()
                    .expect("object"),
                &["sourceAccountId"]
            ),
            Some(None)
        );
        assert_eq!(
            optional_id_field_from_object(
                json!({"sourceAccountId": false})
                    .as_object()
                    .expect("object"),
                &["sourceAccountId"]
            ),
            Some(None)
        );

        assert_eq!(
            matching_error_response(MatchingRuntimeError::NotFound("missing".to_string())).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(status_or_internal(1000), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
