// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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
        detect_recurring_patterns, normalize_reconcile_history_families, parse_manual_pair_request,
        parse_reconciliation_candidates_query, serialize_recurring_suggestions,
    },
    UserId,
};
use bill_analyser_db::{
    accept_postgres_recurring_suggestion, count_postgres_recurring_suggestions,
    create_postgres_manual_matching_pair, delete_postgres_manual_matching_pair,
    detect_and_save_postgres_recurring_suggestions, get_postgres_bills_linked_to_recurring,
    list_postgres_recent_bills_for_recurring_detection, list_postgres_recurring_suggestions,
    query_postgres_calendar_events_payload, query_postgres_matching_bill_candidates_payload,
    query_postgres_matching_bill_feedback_payload, query_postgres_matching_pairs_payload,
    query_postgres_matching_session_candidates_payload, query_postgres_net_worth_payload,
    query_postgres_reconciliation_candidates_payload, reject_postgres_recurring_suggestion,
    ImportPreviewExpectedState, ImportPreviewLearningApply, ImportPreviewRecurringCandidate,
    MatchingRuntimeError, PostgresRepositoryRuntime, PreviewMatchingActionRequest,
    ReconciliationCandidateFilters,
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

include!("matching_routes/types.rs");
include!("matching_routes/calendar_networth.rs");
include!("matching_routes/recurring_suggestions.rs");
include!("matching_routes/matching_candidates.rs");
include!("matching_routes/manual_pairs.rs");
include!("matching_routes/reconcile_history.rs");
include!("matching_routes/runtime_helpers.rs");
include!("matching_routes/payloads.rs");
include!("matching_routes/reconciliation_filters.rs");
include!("matching_routes/value_helpers.rs");
include!("matching_routes/responses.rs");
#[cfg(test)]
include!("matching_routes/tests.rs");
