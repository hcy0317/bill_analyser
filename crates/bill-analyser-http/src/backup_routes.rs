use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use bill_analyser_core::{normalize_backup_job_payload, UserId};
use bill_analyser_db::{
    create_backup_audit_log_best_effort, create_or_update_backup_job, init_backup_ops_schema,
    list_backup_jobs, BackupAuditLogDraft, BackupJobDraft, DbError, SqliteConnectionConfig,
    SqliteDbPath, SqliteRuntime,
};
use serde_json::{json, Map, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, proxy::ProxyState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

type RouteResult<T> = Result<T, Box<Response>>;

struct AuthenticatedBackupRuntime {
    runtime: SqliteRuntime,
    user_id: UserId,
}

pub const BACKUP_OPS_ROUTE_PATTERNS: &[(&str, &str)] =
    &[("GET", "/api/backup/jobs"), ("POST", "/api/backup/jobs")];

pub const BACKUP_OPS_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/backup/"),
    ("POST", "/api/backup/cleanup"),
    ("POST", "/api/backup/create"),
    ("DELETE", "/api/backup/delete/{filename}"),
    ("GET", "/api/backup/download/{filename}"),
    ("POST", "/api/backup/restore/{filename}"),
    ("POST", "/api/backup/restore/verify"),
];

pub fn backup_ops_runtime_router() -> Router<ProxyState> {
    Router::new().route(
        "/api/backup/jobs",
        get(list_backup_jobs_handler).post(save_backup_job_handler),
    )
}

async fn list_backup_jobs_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    match list_backup_jobs_response(&state, &headers) {
        Ok(response) => response,
        Err(response) => *response,
    }
}

async fn save_backup_job_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match save_backup_job_response(&state, &headers, body) {
        Ok(response) => response,
        Err(response) => *response,
    }
}

fn list_backup_jobs_response(state: &ProxyState, headers: &HeaderMap) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let jobs = list_backup_jobs(auth_runtime.runtime.connection(), auth_runtime.user_id)
        .map_err(|_| Box::new(db_error_response()))?;
    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": jobs }),
    ))
}

fn save_backup_job_response(
    state: &ProxyState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = optional_json_body(&body);
    let normalized = match normalize_backup_job_payload(&payload) {
        Ok(job) => job,
        Err(error) => {
            write_backup_job_audit(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                validation_audit_details(&payload),
                0,
                "failed",
                Some(error.error.clone()),
            );
            return Ok(error_response(
                status_or_internal(error.status_code),
                error.message,
            ));
        }
    };
    let response_payload = json!({
        "id": normalized.id,
        "job_type": normalized.job_type,
        "schedule_expr": normalized.schedule_expr,
        "retention_days": normalized.retention_days,
        "retention_count": normalized.retention_count,
        "enabled": normalized.enabled,
        "last_status": normalized.last_status,
    });

    let job_id = create_or_update_backup_job(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        BackupJobDraft::from(normalized.clone()),
    )
    .map_err(|error| {
        write_backup_job_audit(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            validation_audit_details(&payload),
            0,
            "failed",
            Some(error.to_string()),
        );
        Box::new(db_write_error_response(error))
    })?;

    write_backup_job_audit(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        json!({
            "user_id": auth_runtime.user_id.get(),
            "job_id": job_id,
            "job_type": normalized.job_type,
            "retention_days": normalized.retention_days,
            "retention_count": normalized.retention_count,
        }),
        1,
        "success",
        None,
    );

    let mut response_payload = response_payload;
    if let Some(object) = response_payload.as_object_mut() {
        object.insert("id".to_string(), json!(job_id));
    }
    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": response_payload }),
    ))
}

fn authenticated_backup_runtime(
    state: &ProxyState,
    headers: &HeaderMap,
) -> RouteResult<AuthenticatedBackupRuntime> {
    let user_id = user_id_from_headers(headers, &state.config)?;
    let runtime = open_runtime(state)?;
    init_backup_ops_schema(runtime.connection()).map_err(|_| Box::new(db_error_response()))?;
    Ok(AuthenticatedBackupRuntime { runtime, user_id })
}

fn optional_json_body(body: &Bytes) -> Value {
    if body.is_empty() {
        return Value::Object(Map::new());
    }
    serde_json::from_slice(body).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn validation_audit_details(payload: &Value) -> Value {
    json!({
        "job_type": audit_safe_json_field(payload, "job_type"),
        "id": audit_safe_json_field(payload, "id"),
        "retention_days": audit_safe_json_field(payload, "retention_days"),
        "retention_count": audit_safe_json_field(payload, "retention_count"),
    })
}

fn write_backup_job_audit(
    connection: &rusqlite::Connection,
    user_id: UserId,
    headers: &HeaderMap,
    details: Value,
    affected_count: i64,
    status: &str,
    error_message: Option<String>,
) {
    let details = with_audit_actor(details, user_id);
    create_backup_audit_log_best_effort(
        connection,
        BackupAuditLogDraft {
            operation_type: "backup_job_saved".to_string(),
            details,
            affected_count,
            ip_address: client_ip(headers),
            user_agent: header_text(headers, "user-agent"),
            status: status.to_string(),
            error_message,
        },
    );
}

fn with_audit_actor(mut details: Value, user_id: UserId) -> Value {
    let Value::Object(object) = &mut details else {
        return json!({ "user_id": user_id.get(), "details": details });
    };
    object.insert("user_id".to_string(), json!(user_id.get()));
    details
}

fn client_ip(headers: &HeaderMap) -> Option<String> {
    let forwarded_for = header_text(headers, "x-forwarded-for");
    if let Some(value) = forwarded_for {
        let first = value.split(',').next().unwrap_or_default().trim();
        if !first.is_empty() {
            return Some(first.to_string());
        }
    }
    header_text(headers, "x-real-ip")
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn audit_safe_json_field(payload: &Value, key: &str) -> String {
    payload.get(key).map(audit_safe_value).unwrap_or_default()
}

fn audit_safe_value(value: &Value) -> String {
    let raw = value.as_str().map(str::to_string).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    });
    raw.trim().chars().take(128).collect()
}

fn open_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust backup ops DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
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
    .map_err(|_| Box::new(db_error_response()))
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust backup ops route runtime DB error",
    )
}

fn db_write_error_response(error: DbError) -> Response {
    match error {
        DbError::InvalidOperation(message) if message == "backup job not found" => {
            error_response(StatusCode::NOT_FOUND, message)
        }
        _ => db_error_response(),
    }
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({
            "success": false,
            "error": message.to_string(),
        }),
    )
}
