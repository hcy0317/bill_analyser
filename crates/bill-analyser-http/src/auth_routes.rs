use std::net::SocketAddr;

use axum::{
    body::Bytes,
    extract::{connect_info::ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::auth::{
    infer_token_type_from_user_agent, json_object_or_empty, parse_user_agent_device_name,
    AuthRestError, TokenKind,
};
use bill_analyser_db::{
    cleanup_expired_sessions, count_recent_token_password_failures, create_auth_log,
    create_token_session, get_auth_token_user, invalidate_other_user_sessions,
    invalidate_session_by_id, list_user_sessions, AuthLogDraft, CreateTokenSessionDraft,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime, TokenSessionRow,
};
use chrono::{Duration as ChronoDuration, Local, NaiveDateTime, TimeZone};
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    auth::{
        jwt_hmac_algorithm, normalize_jwt_algorithm, resolve_authenticated_user_from_headers,
        AuthenticatedUser, RustRouteAuthError,
    },
    proxy::{ownership_aware_proxy_handler, ProxyState},
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const TOKEN_PASSWORD_FAILURE_LIMIT: i64 = 5;
const TOKEN_PASSWORD_FAILURE_WINDOW_MINUTES: i64 = 15;
const FALLBACK_CLIENT_IP: &str = "127.0.0.1";

type RouteResult<T> = Result<T, Box<Response>>;

pub const AUTH_TOKEN_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/tokens"),
    ("DELETE", "/api/tokens"),
    ("DELETE", "/api/tokens/{token_id}"),
    ("POST", "/api/tokens/api"),
    ("POST", "/api/tokens/mcp"),
];

pub const AUTH_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("POST", "/api/auth/login"),
    ("POST", "/api/auth/logout"),
    ("POST", "/api/auth/register"),
    ("POST", "/api/tokens/refresh"),
];

pub fn auth_token_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route("/api/tokens/api", post(generate_api_token_handler))
        .route("/api/tokens/mcp", post(generate_mcp_token_handler))
        .route("/api/tokens/refresh", any(ownership_aware_proxy_handler))
        .route(
            "/api/tokens",
            get(list_tokens_handler).delete(revoke_other_tokens_handler),
        )
        .route("/api/tokens/:token_id", delete(revoke_token_handler))
}

async fn generate_api_token_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    generate_personal_token(
        TokenKind::Api,
        state,
        headers,
        connect_info.map(|ConnectInfo(addr)| addr),
        body,
    )
    .await
}

async fn generate_mcp_token_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    generate_personal_token(
        TokenKind::Mcp,
        state,
        headers,
        connect_info.map(|ConnectInfo(addr)| addr),
        body,
    )
    .await
}

async fn list_tokens_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if cleanup_expired_sessions(runtime.connection(), &now_text()).is_err() {
        return db_error_response();
    }

    match list_user_sessions(runtime.connection(), auth.user_id) {
        Ok(sessions) => success_result(
            StatusCode::OK,
            Value::Array(
                sessions
                    .into_iter()
                    .map(|session| session_payload(session, auth.session_id))
                    .collect(),
            ),
        ),
        Err(_) => db_error_response(),
    }
}

async fn generate_personal_token(
    token_kind: TokenKind,
    state: ProxyState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if auth.session_id.is_none() {
        return auth_error_response(RustRouteAuthError {
            status: 401,
            message: "Current bearer session is required".to_string(),
        });
    }
    let body = request_body_object(&body);
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Current password is required",
        ));
    }
    let expires_in_seconds = match parse_expires_in_seconds(&body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_token_user(runtime.connection(), auth.user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let token_user_agent = token_kind.user_agent(&request_user_agent);
    let ip_address = client_ip(&headers, peer_addr);
    let response_origin = match request_origin(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let failure_count = match count_recent_token_password_failures(
        runtime.connection(),
        user.id,
        &token_failure_window_start_text(),
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if failure_count >= TOKEN_PASSWORD_FAILURE_LIMIT {
        return auth_rest_error_response(AuthRestError::new(
            429,
            "Too Many Requests",
            "Too many failed token password attempts, please try again later",
        ));
    }

    if !bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: Some(user.id),
                username: &user.username,
                event_type: &format!("{}_token_generate_failed", token_kind.as_str()),
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: false,
                error_message: Some("Invalid password".to_string()),
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            401,
            "Invalid credentials",
            "Current password is incorrect",
        ));
    }

    let issued_token = match issue_access_token(
        user.id,
        &user.username,
        &state,
        token_kind,
        expires_in_seconds,
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let token_hash = sha256_hex(&issued_token.access_token);
    let created_at = now_text();
    let session_id = match create_token_session(
        runtime.connection(),
        &CreateTokenSessionDraft {
            user_id: user.id,
            token_hash,
            refresh_token_hash: None,
            expires_at: issued_token.expires_at.clone(),
            refresh_expires_at: None,
            user_agent: token_user_agent.clone(),
            ip_address: ip_address.clone(),
            created_at: created_at.clone(),
        },
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: &format!("{}_token_generate_success", token_kind.as_str()),
            ip_address: &ip_address,
            user_agent: &token_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "session_id": session_id }).to_string()),
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    let mut result = Map::new();
    result.insert(
        "token".to_string(),
        Value::String(issued_token.access_token),
    );
    match token_kind {
        TokenKind::Api => {
            result.insert(
                "apiBaseUrl".to_string(),
                Value::String(format!("{response_origin}/api")),
            );
        }
        TokenKind::Mcp => {
            result.insert(
                "mcpUrl".to_string(),
                Value::String(format!("{response_origin}/mcp")),
            );
        }
        TokenKind::Session => {}
    }
    success_result(StatusCode::OK, Value::Object(result))
}

async fn revoke_other_tokens_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(current_session_id) = auth.session_id else {
        return auth_error_response(RustRouteAuthError {
            status: 401,
            message: "Current bearer session is required".to_string(),
        });
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match invalidate_other_user_sessions(runtime.connection(), auth.user_id, current_session_id) {
        Ok(revoked_count) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "result": true,
                "revokedCount": revoked_count
            }),
        ),
        Err(_) => db_error_response(),
    }
}

async fn revoke_token_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(token_id): Path<String>,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let token_id = match token_id.trim().parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return auth_rest_error_response(AuthRestError::invalid_request(
                "tokenId must be a valid integer",
            ));
        }
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match invalidate_session_by_id(runtime.connection(), token_id, auth.user_id) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => {
            auth_rest_error_response(AuthRestError::new(404, "Not Found", "Token not found"))
        }
        Err(_) => db_error_response(),
    }
}

struct IssuedAccessToken {
    access_token: String,
    expires_at: String,
}

fn issue_access_token(
    user_id: bill_analyser_core::UserId,
    username: &str,
    state: &ProxyState,
    token_kind: TokenKind,
    expires_in_seconds: i64,
) -> RouteResult<IssuedAccessToken> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let hmac_algorithm = jwt_hmac_algorithm(&algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let now = Local::now();
    let expires_at = if expires_in_seconds > 0 {
        now + ChronoDuration::seconds(expires_in_seconds)
    } else {
        now + ChronoDuration::days(365 * 100)
    };
    let nonce = random_nonce_hex()?;
    let header = json!({ "alg": algorithm, "typ": "JWT" });
    let payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "token_kind": token_kind.as_str(),
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": nonce,
    });
    let encoded_header = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header).map_err(|_| Box::new(db_error_response()))?);
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).map_err(|_| Box::new(db_error_response()))?);
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    Ok(IssuedAccessToken {
        access_token: format!("{signing_input}.{encoded_signature}"),
        expires_at: expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

fn random_nonce_hex() -> RouteResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 16];
    rng.fill(&mut bytes).map_err(|_| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime random generation failed",
        )))
    })?;
    Ok(bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn authenticated_user(headers: &HeaderMap, state: &ProxyState) -> RouteResult<AuthenticatedUser> {
    resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| Box::new(auth_error_response(error)))
}

fn request_body_object(body: &[u8]) -> Map<String, Value> {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    json_object_or_empty(parsed.as_ref())
}

fn parse_expires_in_seconds(body: &Map<String, Value>) -> RouteResult<i64> {
    let Some(value) = body.get("expiresInSeconds") else {
        return Ok(0);
    };
    if value.is_null() {
        return Ok(0);
    }
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(value)
            } else if let Some(value) = number.as_u64() {
                i64::try_from(value).map_err(|_| invalid_expires_response())
            } else {
                number
                    .as_f64()
                    .map(|value| value as i64)
                    .ok_or_else(invalid_expires_response)
            }
        }
        Value::String(value) => value.parse::<i64>().map_err(|_| invalid_expires_response()),
        Value::Bool(value) => Ok(if *value { 1 } else { 0 }),
        _ => Err(invalid_expires_response()),
    }
}

fn invalid_expires_response() -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::invalid_request(
        "expiresInSeconds must be a valid integer",
    )))
}

fn open_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        )))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.to_string(),
        )))
    })?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: false,
        busy_timeout: state.config.timeout,
    })
    .map_err(|_| Box::new(db_error_response()))
}

fn client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    if let Some(addr) = peer_addr {
        return addr.ip().to_string();
    }
    let forwarded_for = header_value(headers, "x-forwarded-for");
    if !forwarded_for.is_empty() {
        return forwarded_for
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
    }
    let real_ip = header_value(headers, "x-real-ip");
    if !real_ip.is_empty() {
        return real_ip;
    }
    FALLBACK_CLIENT_IP.to_string()
}

fn request_origin(state: &ProxyState) -> RouteResult<String> {
    if let Some(public_base_url) = state.config.public_base_url.as_deref() {
        return Ok(public_base_url.to_string());
    }
    Err(Box::new(auth_rest_error_response(AuthRestError::new(
        503,
        "Service Unavailable",
        "Rust auth token runtime requires BILL_ANALYSER_PUBLIC_BASE_URL",
    ))))
}

fn header_value(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

struct AuthEvent<'a> {
    user_id: Option<bill_analyser_core::UserId>,
    username: &'a str,
    event_type: &'a str,
    ip_address: &'a str,
    user_agent: &'a str,
    success: bool,
    error_message: Option<String>,
    metadata: Option<String>,
}

fn log_auth_event(
    connection: &rusqlite::Connection,
    event: AuthEvent<'_>,
) -> bill_analyser_db::DbResult<i64> {
    create_auth_log(
        connection,
        &AuthLogDraft {
            user_id: event.user_id,
            username: event.username.to_string(),
            event_type: event.event_type.to_string(),
            ip_address: event.ip_address.to_string(),
            user_agent: event.user_agent.to_string(),
            success: event.success,
            error_message: event.error_message,
            metadata: event.metadata,
            created_at: now_text(),
        },
    )
}

fn session_payload(session: TokenSessionRow, current_session_id: Option<i64>) -> Value {
    let is_current = current_session_id == Some(session.id);
    let last_activity_at = session.last_activity_at;
    let created_at = session.created_at;
    let last_seen_source = if last_activity_at.is_empty() {
        created_at.as_str()
    } else {
        last_activity_at.as_str()
    };

    json!({
        "tokenId": session.id.to_string(),
        "tokenType": infer_token_type_from_user_agent(&session.user_agent),
        "userAgent": session.user_agent,
        "deviceName": parse_user_agent_device_name(&session.user_agent),
        "ipAddress": session.ip_address,
        "createdAt": created_at,
        "expiresAt": session.expires_at,
        "lastActivityAt": last_activity_at,
        "lastSeen": datetime_to_unix_millis(last_seen_source),
        "isCurrent": is_current,
        "isCurrentToken": is_current
    })
}

fn datetime_to_unix_millis(value: &str) -> i64 {
    let value = value.trim();
    if value.is_empty() {
        return 0;
    }
    let parsed = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"));
    parsed
        .ok()
        .and_then(|datetime| Local.from_local_datetime(&datetime).single())
        .map(|datetime| datetime.timestamp_millis())
        .unwrap_or(0)
}

fn now_text() -> String {
    Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn token_failure_window_start_text() -> String {
    (Local::now().naive_local() - ChronoDuration::minutes(TOKEN_PASSWORD_FAILURE_WINDOW_MINUTES))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn db_error_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        500,
        "Internal Server Error",
        "Rust auth token runtime DB error",
    ))
}

fn auth_error_response(error: RustRouteAuthError) -> Response {
    let error_label = match error.status {
        401 => "Unauthorized",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        _ => "Authentication Error",
    };
    auth_rest_error_response(AuthRestError::new(error.status, error_label, error.message))
}

fn auth_rest_error_response(error: AuthRestError) -> Response {
    json_response(
        status_or_internal(error.status),
        json!({
            "success": false,
            "error": error.error,
            "message": error.message
        }),
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
    use std::time::Duration;

    use axum::http::HeaderValue;
    use bill_analyser_core::UserId;

    use super::*;
    use crate::config::HttpShellConfig;

    fn test_state(config: HttpShellConfig) -> ProxyState {
        ProxyState::new(config).expect("proxy state")
    }

    #[test]
    fn helper_edges_cover_origin_ip_and_expires_parsing() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.10"));
        assert_eq!(client_ip(&headers, None), "203.0.113.10");
        assert_eq!(
            client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "198.51.100.20"
        );
        assert_eq!(client_ip(&HeaderMap::new(), None), FALLBACK_CLIENT_IP);

        assert!(request_origin(&test_state(HttpShellConfig::default())).is_err());
        assert_eq!(
            request_origin(&test_state(
                HttpShellConfig::default().with_public_base_url("https://public.test/")
            ))
            .ok()
            .as_deref(),
            Some("https://public.test")
        );

        let mut body = Map::new();
        assert_eq!(parse_expires_in_seconds(&body).expect("missing expires"), 0);
        body.insert("expiresInSeconds".to_string(), Value::Null);
        assert_eq!(parse_expires_in_seconds(&body).expect("null expires"), 0);
        body.insert("expiresInSeconds".to_string(), Value::Bool(true));
        assert_eq!(parse_expires_in_seconds(&body).expect("bool expires"), 1);
        body.insert("expiresInSeconds".to_string(), json!(12.8));
        assert_eq!(parse_expires_in_seconds(&body).expect("float expires"), 12);
        body.insert(
            "expiresInSeconds".to_string(),
            Value::String("30".to_string()),
        );
        assert_eq!(parse_expires_in_seconds(&body).expect("string expires"), 30);
    }

    #[test]
    fn issue_token_and_auth_error_edges_are_pinned() {
        let user_id = UserId::new(7).expect("user id");
        assert!(issue_access_token(
            user_id,
            "alice",
            &test_state(HttpShellConfig::default()),
            TokenKind::Api,
            60,
        )
        .is_err());
        assert!(issue_access_token(
            user_id,
            "alice",
            &test_state(
                HttpShellConfig::default()
                    .with_auth_jwt_secret("secret")
                    .with_auth_jwt_algorithm("none")
            ),
            TokenKind::Api,
            60,
        )
        .is_err());

        let issued = issue_access_token(
            user_id,
            "alice",
            &test_state(
                HttpShellConfig::new("http://127.0.0.1:5001", Duration::from_millis(100), 1024)
                    .expect("config")
                    .with_auth_jwt_secret("secret")
                    .with_auth_jwt_algorithm("HS512"),
            ),
            TokenKind::Session,
            0,
        )
        .expect("long-lived token");
        assert!(!issued.access_token.is_empty());
        assert!(issued.expires_at.contains('T'));

        assert_eq!(
            auth_error_response(RustRouteAuthError {
                status: 500,
                message: "internal".to_string(),
            })
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            auth_error_response(RustRouteAuthError {
                status: 418,
                message: "teapot".to_string(),
            })
            .status(),
            StatusCode::IM_A_TEAPOT
        );
    }
}
