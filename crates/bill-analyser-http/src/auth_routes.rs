use std::net::SocketAddr;

use axum::{
    body::{Body, Bytes},
    extract::{connect_info::ConnectInfo, Path, State},
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{
    auth::{
        infer_token_type_from_user_agent, json_object_or_empty, parse_user_agent_device_name,
        validate_refresh_token_claims, AuthRestError, TokenKind,
    },
    build_user_investment_keyword_settings, UserId,
};
use bill_analyser_db::{
    auth_email_exists, auth_username_exists, cleanup_expired_sessions,
    count_recent_token_password_failures, create_auth_log, create_registered_user_with_defaults,
    create_token_session, get_active_logout_session_by_token_hash, get_active_refresh_session,
    get_auth_token_user, get_auth_user_profile, get_login_user_by_login_name,
    increment_failed_login, invalidate_other_user_sessions, invalidate_session_by_id,
    invalidate_session_by_token_hash, list_application_cloud_settings, list_user_sessions,
    rotate_refresh_token_session, update_user_last_login, ApplicationCloudSettingRow, AuthLogDraft,
    AuthLoginUserRow, AuthUserProfileRow, CreateTokenSessionDraft, DbError, RegisterPresetCategory,
    RegisterPresetSubCategory, RegisterUserDraft, SqliteConnectionConfig, SqliteDbPath,
    SqliteRuntime, TokenSessionRow,
};
use chrono::{Duration as ChronoDuration, Local, NaiveDateTime, TimeZone, Utc};
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
    proxy::{proxy_request, ProxyState},
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const TOKEN_PASSWORD_FAILURE_LIMIT: i64 = 5;
const TOKEN_PASSWORD_FAILURE_WINDOW_MINUTES: i64 = 15;
const FALLBACK_CLIENT_IP: &str = "127.0.0.1";
const AUTH_ALLOWED_CORS_ORIGINS: &[&str] = &["http://localhost:8081", "http://127.0.0.1:8081"];

type RouteResult<T> = Result<T, Box<Response>>;

pub const AUTH_TOKEN_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/tokens"),
    ("DELETE", "/api/tokens"),
    ("DELETE", "/api/tokens/{token_id}"),
    ("POST", "/api/tokens/api"),
    ("POST", "/api/tokens/mcp"),
    ("POST", "/api/tokens/refresh"),
    ("POST", "/api/auth/login"),
    ("POST", "/api/auth/register"),
    ("POST", "/api/auth/logout"),
];

pub const AUTH_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

pub fn auth_token_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route(
            "/api/auth/login",
            post(login_handler).options(login_options_handler),
        )
        .route(
            "/api/auth/register",
            post(register_handler).options(register_options_handler),
        )
        .route("/api/tokens/api", post(generate_api_token_handler))
        .route("/api/tokens/mcp", post(generate_mcp_token_handler))
        .route("/api/tokens/refresh", post(refresh_token_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route(
            "/api/tokens",
            get(list_tokens_handler).delete(revoke_other_tokens_handler),
        )
        .route("/api/tokens/:token_id", delete(revoke_token_handler))
        .layer(middleware::from_fn(auth_cors_middleware))
}

async fn login_options_handler(
    State(state): State<ProxyState>,
    request: Request<Body>,
) -> Response {
    proxy_request(state, request).await
}

async fn register_options_handler(
    State(state): State<ProxyState>,
    request: Request<Body>,
) -> Response {
    proxy_request(state, request).await
}

async fn auth_cors_middleware(request: Request<Body>, next: Next) -> Response {
    let origin = request.headers().get(header::ORIGIN).cloned();
    let mut response = next.run(request).await;
    apply_auth_cors_headers(origin.as_ref(), response.headers_mut());
    response
}

async fn register_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let body = request_body_object(&body);
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if username.is_empty() || email.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Username, email and password are required",
        ));
    }
    if !state.config.auth_enable_user_registration {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Registration disabled",
            "User registration is currently disabled",
        ));
    }
    if let Err(message) = state.config.auth_password_policy.validate(password) {
        return auth_rest_error_response(AuthRestError::new(400, "Invalid password", message));
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let username_exists = match auth_username_exists(runtime.connection(), &username) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if username_exists {
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: None,
                username: &username,
                event_type: "register_failed",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: false,
                error_message: Some("Username already exists".to_string()),
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            409,
            "Username exists",
            "Username already exists",
        ));
    }
    let email_exists = match auth_email_exists(runtime.connection(), &email) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if email_exists {
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: None,
                username: &username,
                event_type: "register_failed",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: false,
                error_message: Some("Email already exists".to_string()),
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            409,
            "Email exists",
            "Email already exists",
        ));
    }

    let password_hash = match bcrypt::hash(password, bcrypt::DEFAULT_COST) {
        Ok(value) => value,
        Err(_) => {
            return auth_rest_error_response(AuthRestError::new(
                500,
                "Internal Server Error",
                "Rust auth register runtime password hashing error",
            ))
        }
    };
    let language = body
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("zh_Hans")
        .to_string();
    let default_currency = body
        .get("defaultCurrency")
        .and_then(Value::as_str)
        .unwrap_or("CNY")
        .to_string();
    let first_day_of_week = body
        .get("firstDayOfWeek")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let nickname = body
        .get("nickname")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let nickname = if nickname.is_empty() {
        username.clone()
    } else {
        nickname
    };
    let created_at = utc_now_text();
    let register_result = match create_registered_user_with_defaults(
        runtime.connection(),
        &RegisterUserDraft {
            username: username.clone(),
            email: email.clone(),
            password_hash,
            nickname,
            language,
            default_currency,
            first_day_of_week,
            email_verified: !state.config.auth_require_email_verification,
            created_at: created_at.clone(),
        },
        &register_preset_categories_from_body(&body),
        &AuthLogDraft {
            user_id: None,
            username: username.clone(),
            event_type: "register_success".to_string(),
            ip_address: ip_address.clone(),
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: None,
            created_at,
        },
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    success_result(
        StatusCode::OK,
        json!({
            "user_id": register_result.user_id,
            "username": username,
            "email": email,
            "needVerifyEmail": state.config.auth_require_email_verification,
            "presetCategoriesSaved": register_result.preset_categories_saved,
            "presetAccountsSaved": register_result.preset_accounts_saved,
            "message": "Registration successful",
        }),
    )
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

async fn login_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let body = request_body_object(&body);
    let login_name = body
        .get("loginName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if login_name.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Username and password are required",
        ));
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));

    let user = match get_login_user_by_login_name(runtime.connection(), &login_name) {
        Ok(Some(value)) => value,
        Ok(None) => {
            if log_auth_event(
                runtime.connection(),
                AuthEvent {
                    user_id: None,
                    username: &login_name,
                    event_type: "login_failed",
                    ip_address: &ip_address,
                    user_agent: &request_user_agent,
                    success: false,
                    error_message: Some("User not found".to_string()),
                    metadata: None,
                },
            )
            .is_err()
            {
                return db_error_response();
            }
            return invalid_login_credentials_response();
        }
        Err(_) => return db_error_response(),
    };

    if login_lock_is_active(&user.locked_until) {
        if log_login_failure(
            runtime.connection(),
            &user,
            &ip_address,
            &request_user_agent,
            "Account locked",
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Account locked",
            "Account is temporarily locked due to multiple failed login attempts",
        ));
    }
    let expired_locked_until = if user.locked_until.trim().is_empty() {
        None
    } else {
        Some(user.locked_until.as_str())
    };

    if !bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        let lockout_until = login_lockout_until_text(state.config.auth_lockout_duration_minutes);
        if increment_failed_login(
            runtime.connection(),
            user.profile.id,
            state.config.auth_max_login_attempts,
            &lockout_until,
            expired_locked_until,
        )
        .is_err()
            || log_login_failure(
                runtime.connection(),
                &user,
                &ip_address,
                &request_user_agent,
                "Invalid password",
            )
            .is_err()
        {
            return db_error_response();
        }
        return invalid_login_credentials_response();
    }

    if !user.is_active {
        if log_login_failure(
            runtime.connection(),
            &user,
            &ip_address,
            &request_user_agent,
            "Account not active",
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Account not active",
            "Your account has been deactivated",
        ));
    }

    if user.two_factor_enabled {
        let pending_token = match issue_action_token(&user, "pending_2fa", 1, &state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: Some(user.profile.id),
                username: &user.profile.username,
                event_type: "login_2fa_pending",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: true,
                error_message: None,
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        return success_result(
            StatusCode::OK,
            json!({
                "token": pending_token,
                "need2FA": true,
            }),
        );
    }

    let cloud_settings =
        match list_application_cloud_settings(runtime.connection(), user.profile.id) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let tokens = match issue_session_tokens(user.profile.id, &user.profile.username, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let now = utc_now_text();
    let session_draft = CreateTokenSessionDraft {
        user_id: user.profile.id,
        token_hash: sha256_hex(&tokens.access_token),
        refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
        expires_at: tokens.expires_at.clone(),
        refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
        user_agent: request_user_agent.clone(),
        ip_address: ip_address.clone(),
        created_at: now.clone(),
    };
    if persist_login_success(
        runtime.connection(),
        &session_draft,
        user.profile.id,
        &user.profile.username,
        &now,
        &ip_address,
        &request_user_agent,
    )
    .is_err()
    {
        return db_error_response();
    }
    let mut user_payload = user_profile_payload(&user.profile);
    if let Value::Object(ref mut object) = user_payload {
        object.insert("id".to_string(), Value::from(user.profile.id.get()));
    }

    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "need2FA": false,
            "user": user_payload,
            "applicationCloudSettings": application_cloud_settings_payload(cloud_settings),
        }),
    )
}

async fn refresh_token_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let body = request_body_object(&body);
    let refresh_token = body
        .get("refreshToken")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if refresh_token.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Refresh token is required",
        ));
    }

    let claims = match validate_refresh_jwt(refresh_token, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let refresh_token_hash = sha256_hex(refresh_token);
    let refresh_session =
        match get_active_refresh_session(runtime.connection(), &refresh_token_hash) {
            Ok(Some(value)) => value,
            Ok(None) => return invalid_refresh_token_response(),
            Err(_) => return db_error_response(),
        };
    if refresh_session.user_id != claims.user_id || !refresh_session.user_is_active {
        return invalid_refresh_token_response();
    }
    if refresh_session_is_expired(&refresh_session.refresh_expires_at) {
        return refresh_token_expired_response();
    }

    let user = match get_auth_user_profile(runtime.connection(), claims.user_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User does not exist",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let tokens = match issue_session_tokens(user.id, &user.username, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let session_id = match rotate_refresh_token_session(
        runtime.connection(),
        refresh_session.id,
        &refresh_token_hash,
        &CreateTokenSessionDraft {
            user_id: user.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent,
            ip_address,
            created_at: now_text(),
        },
    ) {
        Ok(Some(value)) => value,
        Ok(None) => return invalid_refresh_token_response(),
        Err(_) => return db_error_response(),
    };
    if session_id <= 0 {
        return db_error_response();
    }
    let cloud_settings = match list_application_cloud_settings(runtime.connection(), user.id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };

    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "newToken": tokens.access_token,
            "user": user_profile_payload(&user),
            "applicationCloudSettings": application_cloud_settings_payload(cloud_settings),
        }),
    )
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

async fn logout_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let token = match parse_logout_bearer_token(&headers) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let token_hash = sha256_hex(&token);
    let session = match get_active_logout_session_by_token_hash(runtime.connection(), &token_hash) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };

    if let Some(session) = session {
        if invalidate_session_by_token_hash(runtime.connection(), &token_hash).is_err() {
            return db_error_response();
        }
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: Some(session.user_id),
                username: &session.username,
                event_type: "logout",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: true,
                error_message: None,
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
    } else {
        emit_logout_session_not_found_warning(&token_hash);
    }

    json_response(
        StatusCode::OK,
        json!({
            "success": true,
            "result": true,
            "message": "Logged out successfully"
        }),
    )
}

fn parse_logout_bearer_token(headers: &HeaderMap) -> Result<String, AuthRestError> {
    let auth_header = header_value(headers, header::AUTHORIZATION.as_str());
    if auth_header.is_empty() {
        return Err(AuthRestError::unauthorized("Missing authorization header"));
    }

    let mut parts = auth_header.split_whitespace();
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default();
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(AuthRestError::unauthorized("Invalid authorization header"));
    }
    Ok(token.to_string())
}

fn emit_logout_session_not_found_warning(token_hash: &str) {
    eprintln!("{}", logout_session_not_found_warning_payload(token_hash));
}

fn logout_session_not_found_warning_payload(token_hash: &str) -> Value {
    json!({
        "level": "warn",
        "target": "bill_analyser_http::auth_routes",
        "event": "logout_session_not_found",
        "token_hash_prefix": token_hash.chars().take(16).collect::<String>(),
    })
}

fn invalid_login_credentials_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Invalid credentials",
        "Invalid username or password",
    ))
}

fn log_login_failure(
    connection: &rusqlite::Connection,
    user: &AuthLoginUserRow,
    ip_address: &str,
    user_agent: &str,
    error_message: &str,
) -> bill_analyser_db::DbResult<i64> {
    log_auth_event(
        connection,
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type: "login_failed",
            ip_address,
            user_agent,
            success: false,
            error_message: Some(error_message.to_string()),
            metadata: None,
        },
    )
}

fn login_lock_is_active(locked_until: &str) -> bool {
    let value = locked_until.trim();
    if value.is_empty() {
        return false;
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|datetime| Utc::now().naive_utc() < datetime)
        .unwrap_or(false)
}

struct IssuedAccessToken {
    access_token: String,
    expires_at: String,
}

struct IssuedSessionTokens {
    access_token: String,
    refresh_token: String,
    expires_at: String,
    refresh_expires_at: String,
}

fn validate_refresh_jwt(
    token: &str,
    state: &ProxyState,
) -> RouteResult<bill_analyser_core::auth::RefreshTokenClaims> {
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
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or_else(invalid_refresh_token_box)?;
    let encoded_payload = parts.next().ok_or_else(invalid_refresh_token_box)?;
    let encoded_signature = parts.next().ok_or_else(invalid_refresh_token_box)?;
    if parts.next().is_some() {
        return Err(invalid_refresh_token_box());
    }

    let header = decode_jwt_part(encoded_header)?;
    let payload = decode_jwt_part(encoded_payload)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err(invalid_refresh_token_box());
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    verify_hmac_signature(
        secret,
        hmac_algorithm,
        signing_input.as_bytes(),
        encoded_signature,
    )?;
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_refresh_token_box)?;
    if exp <= Utc::now().timestamp() {
        return Err(Box::new(refresh_token_expired_response()));
    }
    validate_refresh_token_claims(&payload)
        .map_err(|error| Box::new(auth_rest_error_response(error)))
}

fn issue_session_tokens(
    user_id: UserId,
    username: &str,
    state: &ProxyState,
) -> RouteResult<IssuedSessionTokens> {
    let now = Local::now();
    let access_expires_at = now + ChronoDuration::days(state.config.auth_jwt_expiration_days);
    let refresh_expires_at =
        now + ChronoDuration::days(state.config.auth_refresh_token_expiration_days);
    let access_payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "iat": now.timestamp(),
        "exp": access_expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });
    let refresh_payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "refresh",
        "iat": now.timestamp(),
        "exp": refresh_expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });

    Ok(IssuedSessionTokens {
        access_token: sign_jwt(&access_payload, state)?,
        refresh_token: sign_jwt(&refresh_payload, state)?,
        expires_at: access_expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
        refresh_expires_at: refresh_expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

fn issue_action_token(
    user: &AuthLoginUserRow,
    token_type: &str,
    expires_in_hours: i64,
    state: &ProxyState,
) -> RouteResult<String> {
    let now = Local::now();
    let expires_at = now + ChronoDuration::hours(expires_in_hours);
    let payload = json!({
        "user_id": user.profile.id.get(),
        "username": user.profile.username.clone(),
        "email": user.profile.email.clone(),
        "type": token_type,
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });
    sign_jwt(&payload, state)
}

fn issue_access_token(
    user_id: bill_analyser_core::UserId,
    username: &str,
    state: &ProxyState,
    token_kind: TokenKind,
    expires_in_seconds: i64,
) -> RouteResult<IssuedAccessToken> {
    let now = Local::now();
    let expires_at = if expires_in_seconds > 0 {
        now + ChronoDuration::seconds(expires_in_seconds)
    } else {
        now + ChronoDuration::days(365 * 100)
    };
    let nonce = random_nonce_hex()?;
    let payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "token_kind": token_kind.as_str(),
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": nonce,
    });
    Ok(IssuedAccessToken {
        access_token: sign_jwt(&payload, state)?,
        expires_at: expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

fn sign_jwt(payload: &Value, state: &ProxyState) -> RouteResult<String> {
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
    let header = json!({ "alg": algorithm, "typ": "JWT" });
    let encoded_header = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header).map_err(|_| Box::new(db_error_response()))?);
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(payload).map_err(|_| Box::new(db_error_response()))?);
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    Ok(format!("{signing_input}.{encoded_signature}"))
}

fn decode_jwt_part(encoded: &str) -> RouteResult<Value> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| invalid_refresh_token_box())?;
    serde_json::from_slice(&decoded).map_err(|_| invalid_refresh_token_box())
}

fn verify_hmac_signature(
    secret: &str,
    algorithm: hmac::Algorithm,
    signing_input: &[u8],
    encoded_signature: &str,
) -> RouteResult<()> {
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| invalid_refresh_token_box())?;
    let key = hmac::Key::new(algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input, &signature).map_err(|_| invalid_refresh_token_box())
}

fn invalid_refresh_token_box() -> Box<Response> {
    Box::new(invalid_refresh_token_response())
}

fn invalid_refresh_token_response() -> Response {
    auth_rest_error_response(AuthRestError::invalid_token(401, "Invalid refresh token"))
}

fn refresh_token_expired_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Token expired",
        "Refresh token has expired",
    ))
}

fn refresh_session_is_expired(expires_at: &str) -> bool {
    let normalized = expires_at.trim();
    if normalized.is_empty() {
        return true;
    }
    NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(normalized, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|datetime| Local::now().naive_local() > datetime)
        .unwrap_or(true)
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

fn register_preset_categories_from_body(body: &Map<String, Value>) -> Vec<RegisterPresetCategory> {
    let Some(categories) = body.get("categories").and_then(Value::as_array) else {
        return Vec::new();
    };
    categories
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if name.is_empty() {
                return None;
            }
            let sub_categories = object
                .get("subCategories")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|sub_item| {
                    let sub_object = sub_item.as_object()?;
                    let name = sub_object
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if name.is_empty() {
                        return None;
                    }
                    Some(RegisterPresetSubCategory {
                        name,
                        icon: sub_object
                            .get("icon")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        color: sub_object
                            .get("color")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect();
            Some(RegisterPresetCategory {
                name,
                type_code: object.get("type").and_then(Value::as_i64).unwrap_or(3),
                icon: object
                    .get("icon")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                color: object
                    .get("color")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                sub_categories,
            })
        })
        .collect()
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
    forwarded_header_ip(headers).unwrap_or_else(|| FALLBACK_CLIENT_IP.to_string())
}

fn login_client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    forwarded_header_ip(headers)
        .or_else(|| peer_addr.map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| FALLBACK_CLIENT_IP.to_string())
}

fn forwarded_header_ip(headers: &HeaderMap) -> Option<String> {
    let forwarded_for = header_value(headers, "x-forwarded-for");
    if !forwarded_for.is_empty() {
        return Some(
            forwarded_for
                .split(',')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
        );
    }
    let real_ip = header_value(headers, "x-real-ip");
    if !real_ip.is_empty() {
        return Some(real_ip);
    }
    None
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
            created_at: utc_now_text(),
        },
    )
}

fn persist_login_success(
    connection: &rusqlite::Connection,
    session_draft: &CreateTokenSessionDraft,
    user_id: UserId,
    username: &str,
    now: &str,
    ip_address: &str,
    user_agent: &str,
) -> bill_analyser_db::DbResult<i64> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let session_id = create_token_session(connection, session_draft)?;
        if !update_user_last_login(connection, user_id, now, ip_address)? {
            return Err(DbError::InvalidOperation(
                "login user row was not updated".to_string(),
            ));
        }
        log_auth_event(
            connection,
            AuthEvent {
                user_id: Some(user_id),
                username,
                event_type: "login_success",
                ip_address,
                user_agent,
                success: true,
                error_message: None,
                metadata: Some(json!({ "session_id": session_id }).to_string()),
            },
        )?;
        Ok(session_id)
    })();

    match result {
        Ok(session_id) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(session_id)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
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

fn user_profile_payload(user: &AuthUserProfileRow) -> Value {
    let nickname = if user.nickname.is_empty() {
        user.username.clone()
    } else {
        user.nickname.clone()
    };
    let mut keyword_source = Map::new();
    if let Some(value) = user.investment_platform_keywords.as_deref() {
        keyword_source.insert(
            "investment_platform_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = user.investment_product_keywords.as_deref() {
        keyword_source.insert(
            "investment_product_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = user.investment_exclude_keywords.as_deref() {
        keyword_source.insert(
            "investment_exclude_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    let investment_settings = build_user_investment_keyword_settings(Some(&keyword_source));
    json!({
        "username": user.username,
        "email": user.email,
        "nickname": nickname,
        "avatar": user.avatar,
        "avatarProvider": "internal",
        "defaultAccountId": optional_id_string(user.default_account_id),
        "transactionEditScope": user.transaction_edit_scope,
        "language": user.language,
        "defaultCurrency": user.default_currency,
        "firstDayOfWeek": user.first_day_of_week,
        "fiscalYearStart": user.fiscal_year_start,
        "calendarDisplayType": user.calendar_display_type,
        "dateDisplayType": user.date_display_type,
        "longDateFormat": user.long_date_format,
        "shortDateFormat": user.short_date_format,
        "longTimeFormat": user.long_time_format,
        "shortTimeFormat": user.short_time_format,
        "fiscalYearFormat": user.fiscal_year_format,
        "currencyDisplayType": user.currency_display_type,
        "numeralSystem": user.numeral_system,
        "decimalSeparator": user.decimal_separator,
        "digitGroupingSymbol": user.digit_grouping_symbol,
        "digitGrouping": user.digit_grouping,
        "coordinateDisplayType": user.coordinate_display_type,
        "expenseAmountColor": user.expense_amount_color,
        "incomeAmountColor": user.income_amount_color,
        "cashAccountId": optional_id_string(user.cash_account_id),
        "cashTransferCategoryId": optional_id_string(user.cash_transfer_category_id),
        "importLearningEnabled": user.import_learning_enabled,
        "investmentPlatformKeywords": investment_settings["platform_keywords"].clone(),
        "investmentProductKeywords": investment_settings["product_keywords"].clone(),
        "investmentExcludeKeywords": investment_settings["exclude_keywords"].clone(),
        "emailVerified": user.email_verified,
    })
}

fn application_cloud_settings_payload(settings: Vec<ApplicationCloudSettingRow>) -> Value {
    Value::Array(
        settings
            .into_iter()
            .map(|setting| {
                json!({
                    "settingKey": setting.setting_key,
                    "settingValue": setting.setting_value,
                })
            })
            .collect(),
    )
}

fn optional_id_string(value: Option<i64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
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

fn utc_now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn login_lockout_until_text(minutes: i64) -> String {
    (Utc::now().naive_utc() + ChronoDuration::minutes(minutes))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn token_failure_window_start_text() -> String {
    (Utc::now().naive_utc() - ChronoDuration::minutes(TOKEN_PASSWORD_FAILURE_WINDOW_MINUTES))
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

fn apply_auth_cors_headers(origin: Option<&HeaderValue>, headers: &mut HeaderMap) {
    let Some(origin) = origin else {
        return;
    };
    let Ok(origin_text) = origin.to_str() else {
        return;
    };
    if !AUTH_ALLOWED_CORS_ORIGINS.contains(&origin_text) {
        return;
    }
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin.clone());
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Content-Type, Authorization"),
    );
    headers.append(header::VARY, HeaderValue::from_static("Origin"));
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
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.0.2.99, 198.51.100.2"),
        );
        assert_eq!(
            client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "198.51.100.20"
        );
        assert_eq!(
            login_client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "192.0.2.99"
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

        let warning = logout_session_not_found_warning_payload("0123456789abcdefdeadbeefcafebabe");
        assert_eq!(warning["event"], "logout_session_not_found");
        assert_eq!(warning["token_hash_prefix"], "0123456789abcdef");
        assert!(!warning.to_string().contains("deadbeef"));
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
