use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bill_analyser_core::{
    matching::{detect_recurring_patterns, serialize_recurring_suggestions},
    UserId,
};
use bill_analyser_db::{
    accept_recurring_suggestion, count_recurring_suggestions,
    detect_and_save_recurring_suggestions, get_bills_linked_to_recurring,
    list_recent_bills_for_recurring_detection, list_recurring_suggestions,
    query_calendar_events_payload, query_net_worth_payload, reject_recurring_suggestion,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, proxy::ProxyState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

type RouteResult<T> = Result<T, Box<Response>>;

pub const MATCHING_RECURRING_CALENDAR_NETWORTH_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/calendar/events"),
    ("GET", "/api/networth/snapshot"),
    ("GET", "/api/recurring/suggestions"),
    ("POST", "/api/recurring/suggestions/detect"),
    ("POST", "/api/recurring/suggestions/{suggestion_id}/accept"),
    ("POST", "/api/recurring/suggestions/{suggestion_id}/reject"),
];

pub const MATCHING_RECURRING_CALENDAR_NETWORTH_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
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
];

pub fn matching_recurring_calendar_networth_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route("/api/calendar/events", get(calendar_events_handler))
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

async fn calendar_events_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventsQuery>,
) -> Response {
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
    let runtime = match open_runtime(&state, "matching calendar") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_calendar_events_payload(runtime.connection(), user_id, start_date, end_date) {
        Ok(data) => success_data(StatusCode::OK, json!(data)),
        Err(error) => db_error_response(error),
    }
}

async fn networth_snapshot_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state, "networth") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_net_worth_payload(runtime.connection(), user_id) {
        Ok(snapshot) => success_data(StatusCode::OK, snapshot),
        Err(error) => db_error_response(error),
    }
}

async fn list_suggestions_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<RecurringSuggestionsQuery>,
) -> Response {
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
    let runtime = match open_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
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

async fn detect_suggestions_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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

async fn accept_suggestion_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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

async fn reject_suggestion_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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

fn open_runtime(state: &ProxyState, label: &str) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Rust {label} DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"),
        ))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
        ))
    })?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: state.config.timeout,
    })
    .map_err(|error| Box::new(db_error_response(error)))
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
