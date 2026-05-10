use std::net::SocketAddr;

use axum::{
    body::{Body, Bytes},
    extract::{connect_info::ConnectInfo, Path, State},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
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
    build_user_investment_keyword_settings, serialize_keyword_list, user_data_statistics_response,
    UserId,
};
use bill_analyser_db::{
    auth_account_belongs_to_user, auth_category_belongs_to_user, auth_email_exists,
    auth_email_exists_for_other_user, auth_username_exists, cleanup_expired_sessions,
    count_recent_token_password_failures, create_auth_log, create_auth_log_under_event_limit,
    create_registered_user_with_defaults, create_token_session, delete_application_cloud_settings,
    delete_user_external_auth, get_active_logout_session_by_token_hash, get_active_refresh_session,
    get_auth_token_user, get_auth_user_profile, get_auth_user_two_factor_enabled,
    get_login_user_by_email, get_login_user_by_login_name,
    get_user_data_statistics as get_db_user_data_statistics, get_user_external_auth,
    increment_failed_login, init_auth_security_schema, invalidate_other_user_sessions,
    invalidate_session_by_id, invalidate_session_by_token_hash, list_application_cloud_settings,
    list_user_external_auths, list_user_sessions, rotate_refresh_token_session,
    set_user_email_verified, update_application_cloud_settings, update_auth_user_profile,
    update_auth_user_profile_with_auth_log, update_user_last_login, update_user_password_hash,
    ApplicationCloudSettingDraft, ApplicationCloudSettingRow, AuthLogDraft, AuthLoginUserRow,
    AuthUserProfileRow, AuthUserProfileUpdate, CreateTokenSessionDraft, DbError, ExternalAuthRow,
    RegisterPresetCategory, RegisterPresetSubCategory, RegisterUserDraft, SqliteConnectionConfig,
    SqliteDbPath, SqliteRuntime, TokenSessionRow,
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
const PROFILE_VERIFICATION_RESEND_LIMIT: i64 = 3;
const PROFILE_VERIFICATION_RESEND_WINDOW_MINUTES: i64 = 5;
const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;
const FALLBACK_CLIENT_IP: &str = "127.0.0.1";
const AUTH_ALLOWED_CORS_ORIGINS: &[&str] = &["http://localhost:8081", "http://127.0.0.1:8081"];
const BILL_ANALYSER_APP_VERSION: &str = "1.0.0";

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
    ("POST", "/api/auth/email/verify"),
    ("POST", "/api/auth/email/resend-verification"),
    ("POST", "/api/auth/password/forgot"),
    ("POST", "/api/auth/password/reset"),
    ("POST", "/api/auth/oauth2/authorize"),
    ("GET", "/api/profile"),
    ("PUT", "/api/profile"),
    ("POST", "/api/profile/avatar"),
    ("DELETE", "/api/profile/avatar"),
    ("POST", "/api/profile/email/resend-verification"),
    ("GET", "/api/profile/cloud-settings"),
    ("PUT", "/api/profile/cloud-settings"),
    ("DELETE", "/api/profile/cloud-settings"),
    ("GET", "/api/profile/external-auths"),
    ("POST", "/api/profile/external-auths/unlink"),
    ("GET", "/api/system/version"),
    ("GET", "/api/data/statistics"),
    ("GET", "/api/2fa/status"),
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
        .route(
            "/api/auth/oauth2/authorize",
            post(authorize_oauth2_callback_handler).options(auth_options_handler),
        )
        .route(
            "/api/auth/email/verify",
            post(verify_email_handler).options(auth_options_handler),
        )
        .route(
            "/api/auth/email/resend-verification",
            post(resend_public_verification_email_handler).options(auth_options_handler),
        )
        .route(
            "/api/auth/password/forgot",
            post(forgot_password_handler).options(auth_options_handler),
        )
        .route(
            "/api/auth/password/reset",
            post(reset_password_handler).options(auth_options_handler),
        )
        .route("/api/tokens/api", post(generate_api_token_handler))
        .route("/api/tokens/mcp", post(generate_mcp_token_handler))
        .route("/api/tokens/refresh", post(refresh_token_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route(
            "/api/profile",
            get(get_profile_handler)
                .put(update_profile_handler)
                .options(auth_options_handler),
        )
        .route(
            "/api/profile/avatar",
            post(update_profile_avatar_handler)
                .delete(remove_profile_avatar_handler)
                .options(auth_options_handler),
        )
        .route(
            "/api/profile/email/resend-verification",
            post(resend_profile_verification_email_handler).options(auth_options_handler),
        )
        .route(
            "/api/profile/cloud-settings",
            get(get_profile_cloud_settings_handler)
                .put(update_profile_cloud_settings_handler)
                .delete(delete_profile_cloud_settings_handler)
                .options(auth_options_handler),
        )
        .route(
            "/api/profile/external-auths",
            get(list_profile_external_auths_handler).options(auth_options_handler),
        )
        .route(
            "/api/profile/external-auths/unlink",
            post(unlink_profile_external_auth_handler).options(auth_options_handler),
        )
        .route(
            "/api/system/version",
            get(system_version_handler).options(auth_options_handler),
        )
        .route(
            "/api/data/statistics",
            get(get_user_data_statistics_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/status",
            get(get_two_factor_status_handler).options(auth_options_handler),
        )
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

async fn auth_options_handler() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

async fn auth_cors_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let origin = request.headers().get(header::ORIGIN).cloned();
    let requested_headers = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .cloned();
    let mut response = next.run(request).await;
    apply_auth_cors_headers(origin.as_ref(), response.headers_mut());
    if method == Method::OPTIONS {
        apply_auth_preflight_headers(requested_headers.as_ref(), response.headers_mut());
    }
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

async fn get_profile_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_auth_user_profile(runtime.connection(), auth.user_id) {
        Ok(Some(user)) => success_result(StatusCode::OK, user_profile_payload(&user)),
        Ok(None) => {
            auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
        }
        Err(_) => db_error_response(),
    }
}

async fn update_profile_handler(
    State(state): State<ProxyState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    if body.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Request body is required",
        ));
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let current_user = match get_auth_user_profile(runtime.connection(), auth.user_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let updates =
        match validated_profile_updates(runtime.connection(), auth.user_id, &current_user, &body) {
            Ok(value) => value,
            Err(error) => return auth_rest_error_response(error),
        };
    let email_changed = profile_email_changed(&updates, &current_user.email);
    let updated_at = utc_now_text();
    let update_result = if email_changed.is_some() {
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
        update_auth_user_profile_with_auth_log(
            runtime.connection(),
            auth.user_id,
            &updates,
            &updated_at,
            &AuthLogDraft {
                user_id: Some(auth.user_id),
                username: current_user.username.clone(),
                event_type: "profile_email_changed".to_string(),
                ip_address,
                user_agent: request_user_agent,
                success: true,
                error_message: None,
                metadata: Some(
                    json!({
                        "email_changed": true,
                        "email_verified_reset": true
                    })
                    .to_string(),
                ),
                created_at: updated_at.clone(),
            },
        )
    } else {
        update_auth_user_profile(runtime.connection(), auth.user_id, &updates, &updated_at)
    };
    if !updates.is_empty() && !matches!(update_result, Ok(true)) {
        return auth_rest_error_response(AuthRestError::new(
            500,
            "Update failed",
            "Failed to update user profile",
        ));
    }

    match get_auth_user_profile(runtime.connection(), auth.user_id) {
        Ok(Some(user)) => success_result(
            StatusCode::OK,
            json!({ "user": user_profile_payload(&user) }),
        ),
        Ok(None) => auth_rest_error_response(AuthRestError::new(
            404,
            "User not found after update",
            "User not found after update",
        )),
        Err(_) => db_error_response(),
    }
}

async fn update_profile_avatar_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let avatar = match avatar_data_url_from_multipart(&headers, &body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    update_profile_avatar_value(&state, auth.user_id, avatar).await
}

async fn remove_profile_avatar_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    update_profile_avatar_value(&state, auth.user_id, String::new()).await
}

async fn update_profile_avatar_value(
    state: &ProxyState,
    user_id: UserId,
    avatar: String,
) -> Response {
    let runtime = match open_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let updates = [AuthUserProfileUpdate::Avatar(avatar)];
    if !matches!(
        update_auth_user_profile(runtime.connection(), user_id, &updates, &utc_now_text()),
        Ok(true)
    ) {
        return auth_rest_error_response(AuthRestError::new(
            500,
            "Update failed",
            "Failed to update avatar",
        ));
    }
    match get_auth_user_profile(runtime.connection(), user_id) {
        Ok(Some(user)) => success_result(StatusCode::OK, user_profile_payload(&user)),
        Ok(None) => {
            auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
        }
        Err(_) => db_error_response(),
    }
}

async fn resend_profile_verification_email_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_user_profile(runtime.connection(), auth.user_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    if user.email.trim().is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email is required to resend verification email",
        ));
    }
    let since = (Utc::now() - ChronoDuration::minutes(PROFILE_VERIFICATION_RESEND_WINDOW_MINUTES))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let created_at = utc_now_text();
    match create_auth_log_under_event_limit(
        runtime.connection(),
        auth.user_id,
        "verification_email_resend_requested",
        &since,
        PROFILE_VERIFICATION_RESEND_LIMIT,
        &AuthLogDraft {
            user_id: Some(user.id),
            username: user.username.clone(),
            event_type: "verification_email_resend_requested".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "email_present": true,
                    "email_verified": user.email_verified,
                    "require_email_verification": state.config.auth_require_email_verification,
                    "delivery": "not_configured_mock_success"
                })
                .to_string(),
            ),
            created_at,
        },
    ) {
        Ok(true) => {}
        Ok(false) => {
            return auth_rest_error_response(AuthRestError::new(
                429,
                "Too Many Requests",
                "Too many verification email resend requests",
            ));
        }
        Err(_) => return db_error_response(),
    }
    success_result(StatusCode::OK, Value::Bool(true))
}

async fn get_profile_cloud_settings_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match list_application_cloud_settings(runtime.connection(), auth.user_id) {
        Ok(settings) if settings.is_empty() => success_result(StatusCode::OK, Value::Bool(false)),
        Ok(settings) => {
            success_result(StatusCode::OK, application_cloud_settings_payload(settings))
        }
        Err(_) => db_error_response(),
    }
}

async fn update_profile_cloud_settings_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let empty_settings = Vec::new();
    let settings = match body.get("settings") {
        Some(Value::Array(values)) => values,
        Some(_) => {
            return auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "settings must be an array",
            ));
        }
        None => &empty_settings,
    };
    let mut drafts = Vec::with_capacity(settings.len());
    for setting in settings {
        match validate_application_cloud_setting(setting) {
            Ok(value) => drafts.push(value),
            Err(error) => return auth_rest_error_response(error),
        }
    }
    let full_update = body
        .get("fullUpdate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match update_application_cloud_settings(
        runtime.connection(),
        auth.user_id,
        &drafts,
        full_update,
        &utc_now_text(),
    ) {
        Ok(_) => success_result(StatusCode::OK, Value::Bool(true)),
        Err(_) => db_error_response(),
    }
}

async fn delete_profile_cloud_settings_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match delete_application_cloud_settings(runtime.connection(), auth.user_id) {
        Ok(_) => success_result(StatusCode::OK, Value::Bool(true)),
        Err(_) => db_error_response(),
    }
}

async fn list_profile_external_auths_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let rows = match list_user_external_auths(runtime.connection(), auth.user_id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let mut result: Vec<ExternalAuthInfo> =
        rows.into_iter().map(ExternalAuthInfo::linked).collect();

    let oauth2_provider = state.config.auth_oauth2_provider.trim();
    if state.config.auth_enable_oauth2
        && !oauth2_provider.is_empty()
        && !result
            .iter()
            .any(|item| item.external_auth_type == oauth2_provider)
    {
        result.push(ExternalAuthInfo {
            external_auth_category: "oauth2".to_string(),
            external_auth_type: oauth2_provider.to_string(),
            linked: false,
            external_username: String::new(),
            created_at: 0,
        });
    }

    result.sort_by(|left, right| {
        right
            .linked
            .cmp(&left.linked)
            .then_with(|| left.external_auth_type.cmp(&right.external_auth_type))
            .then_with(|| right.created_at.cmp(&left.created_at))
    });

    success_result(StatusCode::OK, external_auths_payload(result))
}

async fn authorize_oauth2_callback_handler(State(state): State<ProxyState>) -> Response {
    if !state.config.auth_enable_oauth2 {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "OAuth2 disabled",
            "OAuth2 login is currently disabled",
        ));
    }

    auth_rest_error_response(AuthRestError::new(
        501,
        "Not Implemented",
        "OAuth2 callback authorization is not implemented in this workspace build",
    ))
}

async fn verify_email_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let peer_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let body = request_body_object(&body);
    let token = body
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if token.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Verification token is required",
        ));
    }
    let request_new_token = body
        .get("requestNewToken")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let payload = match validate_action_jwt(
        token,
        &state,
        "verify_email",
        "Verification token is invalid or expired",
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = match action_user_id(&payload, "Verification token is invalid") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_user_profile(runtime.connection(), user_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    if payload.get("email").and_then(Value::as_str) != Some(user.email.trim()) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Invalid token",
            "Verification token does not match email",
        ));
    }
    if set_user_email_verified(runtime.connection(), user_id, true, &utc_now_text()).is_err() {
        return db_error_response();
    }
    let user = match get_auth_user_profile(runtime.connection(), user_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let mut new_token = Value::Null;
    if request_new_token {
        let tokens = match issue_session_tokens(user.id, &user.username, &state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let created_at = now_text();
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, peer_addr);
        if create_token_session(
            runtime.connection(),
            &CreateTokenSessionDraft {
                user_id: user.id,
                token_hash: sha256_hex(&tokens.access_token),
                refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
                expires_at: tokens.expires_at,
                refresh_expires_at: Some(tokens.refresh_expires_at),
                user_agent: request_user_agent,
                ip_address,
                created_at,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        new_token = Value::String(tokens.access_token);
    }
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, peer_addr);
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: "email_verified",
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

    success_result(
        StatusCode::OK,
        json!({
            "newToken": new_token,
            "user": user_profile_payload(&user),
            "notificationContent": "",
        }),
    )
}

async fn resend_public_verification_email_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let body = request_body_object(&body);
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
    if email.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email and password are required",
        ));
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_email(runtime.connection(), &email) {
        Ok(Some(value)) if bcrypt::verify(password, &value.password_hash).unwrap_or(false) => value,
        Ok(_) => {
            return auth_rest_error_response(AuthRestError::new(
                401,
                "Invalid credentials",
                "Invalid email or password",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let verification_token = match issue_action_token(&user, "verify_email", 24, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type: "verification_email_resend_requested",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "email": email,
                    "delivery": "not_configured_mock_success",
                    "verification_token": verification_token,
                })
                .to_string(),
            ),
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

async fn forgot_password_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let body = request_body_object(&body);
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if email.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email is required",
        ));
    }
    if !state.config.auth_enable_user_forget_password {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Forget password disabled",
            "Forget password is currently disabled",
        ));
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_email(runtime.connection(), &email) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if let Some(user) = user {
        let reset_token = match issue_action_token(&user, "reset_password", 24, &state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: Some(user.profile.id),
                username: &user.profile.username,
                event_type: "password_reset_requested",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: true,
                error_message: None,
                metadata: Some(
                    json!({
                        "email": email,
                        "delivery": "not_configured_mock_success",
                        "reset_token": reset_token,
                    })
                    .to_string(),
                ),
            },
        )
        .is_err()
        {
            return db_error_response();
        }
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

async fn reset_password_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let body = request_body_object(&body);
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
    let token = body
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if email.is_empty() || password.is_empty() || token.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email, password and token are required",
        ));
    }
    if !state.config.auth_enable_user_forget_password {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Forget password disabled",
            "Forget password is currently disabled",
        ));
    }
    if let Err(message) = state.config.auth_password_policy.validate(password) {
        return auth_rest_error_response(AuthRestError::new(400, "Invalid password", message));
    }
    let payload = match validate_action_jwt(
        token,
        &state,
        "reset_password",
        "Reset password token is invalid or expired",
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if payload.get("email").and_then(Value::as_str) != Some(email.as_str()) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Invalid token",
            "Reset password token does not match email",
        ));
    }
    let user_id = match action_user_id(&payload, "Invalid token") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_user_profile(runtime.connection(), user_id) {
        Ok(Some(value)) if value.email.trim() == email => value,
        Ok(_) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let password_hash = match bcrypt::hash(password, bcrypt::DEFAULT_COST) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if update_user_password_hash(
        runtime.connection(),
        user.id,
        &password_hash,
        &utc_now_text(),
    )
    .is_err()
    {
        return db_error_response();
    }
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: "password_reset_completed",
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

    success_result(StatusCode::OK, Value::Bool(true))
}

async fn unlink_profile_external_auth_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let external_auth_type = body
        .get("externalAuthType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if external_auth_type.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "externalAuthType is required",
        ));
    }
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if password.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "password is required",
        ));
    }

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
    if !bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Invalid password",
        ));
    }

    let existing =
        match get_user_external_auth(runtime.connection(), auth.user_id, &external_auth_type) {
            Ok(Some(value)) => value,
            Ok(None) => {
                return auth_rest_error_response(AuthRestError::new(
                    404,
                    "Not Found",
                    "Third-party login is not linked",
                ));
            }
            Err(_) => return db_error_response(),
        };
    let success =
        match delete_user_external_auth(runtime.connection(), auth.user_id, &external_auth_type) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: "external_auth_unlinked",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success,
            error_message: None,
            metadata: Some(
                json!({
                    "external_auth_type": external_auth_type,
                    "external_auth_category": existing.external_auth_category,
                })
                .to_string(),
            ),
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(StatusCode::OK, Value::Bool(success))
}

async fn system_version_handler() -> Response {
    success_result(
        StatusCode::OK,
        json!({
            "version": BILL_ANALYSER_APP_VERSION,
            "commitHash": "",
            "buildTime": ""
        }),
    )
}

async fn get_user_data_statistics_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_db_user_data_statistics(runtime.connection(), auth.user_id) {
        Ok(statistics) => json_response(StatusCode::OK, user_data_statistics_response(&statistics)),
        Err(_) => db_error_response(),
    }
}

async fn get_two_factor_status_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_auth_user_two_factor_enabled(runtime.connection(), auth.user_id) {
        Ok(Some(enabled)) => success_result(
            StatusCode::OK,
            json!({
                "enable": enabled,
                "isEnabled": enabled
            }),
        ),
        Ok(None) => {
            auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
        }
        Err(_) => db_error_response(),
    }
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

fn validate_action_jwt(
    token: &str,
    state: &ProxyState,
    expected_type: &str,
    invalid_message: &'static str,
) -> RouteResult<Value> {
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
    let encoded_header = parts
        .next()
        .ok_or_else(|| invalid_action_token_box(invalid_message))?;
    let encoded_payload = parts
        .next()
        .ok_or_else(|| invalid_action_token_box(invalid_message))?;
    let encoded_signature = parts
        .next()
        .ok_or_else(|| invalid_action_token_box(invalid_message))?;
    if parts.next().is_some() {
        return Err(invalid_action_token_box(invalid_message));
    }

    let header = decode_action_jwt_part(encoded_header, invalid_message)?;
    let payload = decode_action_jwt_part(encoded_payload, invalid_message)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err(invalid_action_token_box(invalid_message));
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| invalid_action_token_box(invalid_message))?;
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input.as_bytes(), &signature)
        .map_err(|_| invalid_action_token_box(invalid_message))?;
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid_action_token_box(invalid_message))?;
    if exp <= Utc::now().timestamp() {
        return Err(invalid_action_token_box(invalid_message));
    }
    if payload.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err(invalid_action_token_box(invalid_message));
    }
    Ok(payload)
}

fn decode_action_jwt_part(encoded: &str, invalid_message: &'static str) -> RouteResult<Value> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| invalid_action_token_box(invalid_message))?;
    serde_json::from_slice(&decoded).map_err(|_| invalid_action_token_box(invalid_message))
}

fn action_user_id(payload: &Value, invalid_message: &'static str) -> RouteResult<UserId> {
    let raw_user_id = payload
        .get("user_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_action_token_box(invalid_message))?;
    UserId::new(raw_user_id).map_err(|_| invalid_action_token_box(invalid_message))
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

fn invalid_action_token_box(message: &'static str) -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::new(
        400,
        "Invalid token",
        message,
    )))
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

fn validated_profile_updates(
    connection: &rusqlite::Connection,
    user_id: UserId,
    current_user: &AuthUserProfileRow,
    body: &Map<String, Value>,
) -> Result<Vec<AuthUserProfileUpdate>, AuthRestError> {
    if body.contains_key("avatar") {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar must be updated via /api/profile/avatar",
        ));
    }
    let updates = profile_updates_from_body(body)?;
    validate_profile_email_update(connection, user_id, current_user, &updates)?;
    validate_profile_reference_ids(connection, user_id, &updates)?;
    Ok(updates)
}

fn validate_profile_email_update(
    connection: &rusqlite::Connection,
    user_id: UserId,
    current_user: &AuthUserProfileRow,
    updates: &[AuthUserProfileUpdate],
) -> Result<(), AuthRestError> {
    let Some(new_email) = updates.iter().find_map(|update| match update {
        AuthUserProfileUpdate::Email(value) => Some(value),
        _ => None,
    }) else {
        return Ok(());
    };
    if !valid_email_address(new_email) {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "Invalid email address",
        ));
    }
    if new_email == &current_user.email {
        return Ok(());
    }
    match auth_email_exists_for_other_user(connection, user_id, new_email) {
        Ok(false) => Ok(()),
        Ok(true) => Err(AuthRestError::new(
            409,
            "Email exists",
            "Email already exists",
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

fn validate_profile_reference_ids(
    connection: &rusqlite::Connection,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
) -> Result<(), AuthRestError> {
    for update in updates {
        match update {
            AuthUserProfileUpdate::DefaultAccountId(Some(account_id)) => {
                validate_profile_account_id(connection, user_id, *account_id, "defaultAccountId")?;
            }
            AuthUserProfileUpdate::CashAccountId(Some(account_id)) => {
                validate_profile_account_id(connection, user_id, *account_id, "cashAccountId")?;
            }
            AuthUserProfileUpdate::CashTransferCategoryId(Some(category_id)) => {
                validate_profile_category_id(
                    connection,
                    user_id,
                    *category_id,
                    "cashTransferCategoryId",
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_profile_account_id(
    connection: &rusqlite::Connection,
    user_id: UserId,
    account_id: i64,
    field_name: &str,
) -> Result<(), AuthRestError> {
    match auth_account_belongs_to_user(connection, user_id, account_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("{field_name} is invalid"),
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

fn validate_profile_category_id(
    connection: &rusqlite::Connection,
    user_id: UserId,
    category_id: i64,
    field_name: &str,
) -> Result<(), AuthRestError> {
    match auth_category_belongs_to_user(connection, user_id, category_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("{field_name} is invalid"),
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

fn valid_email_address(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) || value.matches('@').count() != 1
    {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.contains("..")
        && domain
            .split('.')
            .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
}

fn profile_email_changed(updates: &[AuthUserProfileUpdate], current_email: &str) -> Option<String> {
    updates.iter().find_map(|update| match update {
        AuthUserProfileUpdate::Email(value) if value != current_email => Some(value.clone()),
        _ => None,
    })
}

fn profile_updates_from_body(
    body: &Map<String, Value>,
) -> Result<Vec<AuthUserProfileUpdate>, AuthRestError> {
    let mut updates = Vec::new();
    if let Some(value) = body.get("nickname") {
        updates.push(AuthUserProfileUpdate::Nickname(profile_string(
            value, "nickname",
        )?));
    }
    if let Some(value) = body.get("email") {
        updates.push(AuthUserProfileUpdate::Email(
            profile_string(value, "email")?.trim().to_string(),
        ));
    }
    if let Some(value) = body.get("language") {
        updates.push(AuthUserProfileUpdate::Language(profile_string(
            value, "language",
        )?));
    }
    if let Some(value) = body.get("defaultCurrency") {
        updates.push(AuthUserProfileUpdate::DefaultCurrency(profile_string(
            value,
            "defaultCurrency",
        )?));
    }
    if let Some(value) = body.get("firstDayOfWeek") {
        updates.push(AuthUserProfileUpdate::FirstDayOfWeek(profile_i64(
            value,
            "firstDayOfWeek",
        )?));
    }
    if let Some(value) = body.get("defaultAccountId") {
        updates.push(AuthUserProfileUpdate::DefaultAccountId(
            optional_profile_id(value, "defaultAccountId")?,
        ));
    }
    if let Some(value) = body.get("transactionEditScope") {
        updates.push(AuthUserProfileUpdate::TransactionEditScope(profile_i64(
            value,
            "transactionEditScope",
        )?));
    }
    if let Some(value) = body.get("fiscalYearStart") {
        updates.push(AuthUserProfileUpdate::FiscalYearStart(profile_i64(
            value,
            "fiscalYearStart",
        )?));
    }
    if let Some(value) = body.get("calendarDisplayType") {
        updates.push(AuthUserProfileUpdate::CalendarDisplayType(profile_i64(
            value,
            "calendarDisplayType",
        )?));
    }
    if let Some(value) = body.get("dateDisplayType") {
        updates.push(AuthUserProfileUpdate::DateDisplayType(profile_i64(
            value,
            "dateDisplayType",
        )?));
    }
    if let Some(value) = body.get("longDateFormat") {
        updates.push(AuthUserProfileUpdate::LongDateFormat(profile_i64(
            value,
            "longDateFormat",
        )?));
    }
    if let Some(value) = body.get("shortDateFormat") {
        updates.push(AuthUserProfileUpdate::ShortDateFormat(profile_i64(
            value,
            "shortDateFormat",
        )?));
    }
    if let Some(value) = body.get("longTimeFormat") {
        updates.push(AuthUserProfileUpdate::LongTimeFormat(profile_i64(
            value,
            "longTimeFormat",
        )?));
    }
    if let Some(value) = body.get("shortTimeFormat") {
        updates.push(AuthUserProfileUpdate::ShortTimeFormat(profile_i64(
            value,
            "shortTimeFormat",
        )?));
    }
    if let Some(value) = body.get("fiscalYearFormat") {
        updates.push(AuthUserProfileUpdate::FiscalYearFormat(profile_i64(
            value,
            "fiscalYearFormat",
        )?));
    }
    if let Some(value) = body.get("currencyDisplayType") {
        updates.push(AuthUserProfileUpdate::CurrencyDisplayType(profile_i64(
            value,
            "currencyDisplayType",
        )?));
    }
    if let Some(value) = body.get("numeralSystem") {
        updates.push(AuthUserProfileUpdate::NumeralSystem(profile_i64(
            value,
            "numeralSystem",
        )?));
    }
    if let Some(value) = body.get("decimalSeparator") {
        updates.push(AuthUserProfileUpdate::DecimalSeparator(profile_i64(
            value,
            "decimalSeparator",
        )?));
    }
    if let Some(value) = body.get("digitGroupingSymbol") {
        updates.push(AuthUserProfileUpdate::DigitGroupingSymbol(profile_i64(
            value,
            "digitGroupingSymbol",
        )?));
    }
    if let Some(value) = body.get("digitGrouping") {
        updates.push(AuthUserProfileUpdate::DigitGrouping(profile_i64(
            value,
            "digitGrouping",
        )?));
    }
    if let Some(value) = body.get("coordinateDisplayType") {
        updates.push(AuthUserProfileUpdate::CoordinateDisplayType(profile_i64(
            value,
            "coordinateDisplayType",
        )?));
    }
    if let Some(value) = body.get("expenseAmountColor") {
        updates.push(AuthUserProfileUpdate::ExpenseAmountColor(profile_i64(
            value,
            "expenseAmountColor",
        )?));
    }
    if let Some(value) = body.get("incomeAmountColor") {
        updates.push(AuthUserProfileUpdate::IncomeAmountColor(profile_i64(
            value,
            "incomeAmountColor",
        )?));
    }
    if let Some(value) = body.get("cashAccountId") {
        updates.push(AuthUserProfileUpdate::CashAccountId(optional_profile_id(
            value,
            "cashAccountId",
        )?));
    }
    if let Some(value) = body.get("cashTransferCategoryId") {
        updates.push(AuthUserProfileUpdate::CashTransferCategoryId(
            optional_profile_id(value, "cashTransferCategoryId")?,
        ));
    }
    if let Some(value) = body.get("importLearningEnabled") {
        updates.push(AuthUserProfileUpdate::ImportLearningEnabled(profile_bool(
            value,
            "importLearningEnabled",
        )?));
    }
    if let Some(value) = body.get("investmentPlatformKeywords") {
        updates.push(AuthUserProfileUpdate::InvestmentPlatformKeywords(
            serialize_keyword_list(Some(value)),
        ));
    }
    if let Some(value) = body.get("investmentProductKeywords") {
        updates.push(AuthUserProfileUpdate::InvestmentProductKeywords(
            serialize_keyword_list(Some(value)),
        ));
    }
    if let Some(value) = body.get("investmentExcludeKeywords") {
        updates.push(AuthUserProfileUpdate::InvestmentExcludeKeywords(
            serialize_keyword_list(Some(value)),
        ));
    }
    Ok(updates)
}

fn profile_string(value: &Value, field_name: &str) -> Result<String, AuthRestError> {
    value
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| AuthRestError::new(400, "Bad Request", format!("{field_name} is invalid")))
}

fn profile_i64(value: &Value, field_name: &str) -> Result<i64, AuthRestError> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse().ok())
        })
        .ok_or_else(|| AuthRestError::new(400, "Bad Request", format!("{field_name} is invalid")))
}

fn profile_bool(value: &Value, field_name: &str) -> Result<bool, AuthRestError> {
    if let Some(value) = value.as_bool() {
        return Ok(value);
    }
    if let Some(value) = value.as_i64() {
        return match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("{field_name} is invalid"),
            )),
        };
    }
    if let Some(value) = value.as_str() {
        return match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("{field_name} is invalid"),
            )),
        };
    }
    Err(AuthRestError::new(
        400,
        "Bad Request",
        format!("{field_name} is invalid"),
    ))
}

fn optional_profile_id(value: &Value, field_name: &str) -> Result<Option<i64>, AuthRestError> {
    if value.is_null() {
        return Ok(None);
    }
    if value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    let parsed = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            AuthRestError::new(400, "Bad Request", format!("{field_name} is invalid"))
        })?;
    Ok(Some(parsed))
}

fn avatar_data_url_from_multipart(headers: &HeaderMap, body: &[u8]) -> RouteResult<String> {
    let content_type = header_value(headers, header::CONTENT_TYPE.as_str());
    let boundary = multipart_boundary(&content_type).ok_or_else(|| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar file is required",
        )))
    })?;
    let parts = multipart_parts(body, boundary.as_bytes());
    for part in parts {
        let Some((raw_headers, raw_body)) = split_multipart_part(part) else {
            continue;
        };
        let header_text = String::from_utf8_lossy(raw_headers);
        if !header_text.contains("name=\"avatar\"") {
            continue;
        }
        let payload = trim_trailing_newline(raw_body);
        if payload.is_empty() {
            return Err(Box::new(auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Avatar file is empty",
            ))));
        }
        return avatar_data_url_from_payload(payload, multipart_part_content_type(&header_text));
    }
    Err(Box::new(auth_rest_error_response(AuthRestError::new(
        400,
        "Bad Request",
        "Avatar file is required",
    ))))
}

fn avatar_data_url_from_payload(
    payload: &[u8],
    declared_mime_type: Option<String>,
) -> RouteResult<String> {
    if payload.len() > MAX_AVATAR_BYTES {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar file is too large",
        ))));
    }
    let Some(detected_mime_type) = detect_avatar_mime_type(payload) else {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Unsupported avatar file type",
        ))));
    };
    if let Some(declared_mime_type) = declared_mime_type {
        if !declared_mime_type.eq_ignore_ascii_case(detected_mime_type) {
            return Err(Box::new(auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Avatar MIME type does not match file content",
            ))));
        }
    }
    Ok(format!(
        "data:{detected_mime_type};base64,{}",
        general_purpose::STANDARD.encode(payload)
    ))
}

fn detect_avatar_mime_type(payload: &[u8]) -> Option<&'static str> {
    if payload.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if payload.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if payload.starts_with(b"GIF87a") || payload.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if payload.len() >= 12 && payload.starts_with(b"RIFF") && &payload[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn multipart_boundary(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|segment| {
        let segment = segment.trim();
        let value = segment.strip_prefix("boundary=")?;
        Some(value.trim_matches('"').to_string())
    })
}

fn multipart_parts<'a>(body: &'a [u8], boundary: &[u8]) -> Vec<&'a [u8]> {
    let delimiter = [b"--".as_slice(), boundary].concat();
    let mut parts = Vec::new();
    let mut search_start = 0;
    while let Some(boundary_start) = find_bytes(&body[search_start..], &delimiter) {
        let part_start = search_start + boundary_start + delimiter.len();
        if body.get(part_start..part_start + 2) == Some(b"--") {
            break;
        }
        let part_start = if body.get(part_start..part_start + 2) == Some(b"\r\n") {
            part_start + 2
        } else if body.get(part_start..part_start + 1) == Some(b"\n") {
            part_start + 1
        } else {
            part_start
        };
        let Some(next_boundary) = find_bytes(&body[part_start..], &delimiter) else {
            break;
        };
        let part_end = part_start + next_boundary;
        parts.push(&body[part_start..part_end]);
        search_start = part_end;
    }
    parts
}

fn split_multipart_part(part: &[u8]) -> Option<(&[u8], &[u8])> {
    if let Some(index) = find_bytes(part, b"\r\n\r\n") {
        return Some((&part[..index], &part[index + 4..]));
    }
    find_bytes(part, b"\n\n").map(|index| (&part[..index], &part[index + 2..]))
}

fn trim_trailing_newline(mut value: &[u8]) -> &[u8] {
    if value.ends_with(b"\r\n") {
        value = &value[..value.len().saturating_sub(2)];
    } else if value.ends_with(b"\n") {
        value = &value[..value.len().saturating_sub(1)];
    }
    value
}

fn multipart_part_content_type(header_text: &str) -> Option<String> {
    header_text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-type") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn validate_application_cloud_setting(
    setting: &Value,
) -> Result<ApplicationCloudSettingDraft, AuthRestError> {
    let setting_key = setting
        .get("settingKey")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let Some(setting_value) = setting.get("settingValue").and_then(Value::as_str) else {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("Invalid setting value for {setting_key}"),
        ));
    };
    if setting_key.is_empty() {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "settingKey is required",
        ));
    }
    match application_cloud_setting_type(&setting_key) {
        Some("string") => {}
        Some("number") => {
            if setting_value.parse::<f64>().is_err() {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid number value for {setting_key}"),
                ));
            }
        }
        Some("boolean") => {
            if !matches!(setting_value, "true" | "false") {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid boolean value for {setting_key}"),
                ));
            }
        }
        Some("string_boolean_map") => {
            let parsed = serde_json::from_str::<Value>(setting_value).map_err(|_| {
                AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid JSON value for {setting_key}"),
                )
            })?;
            let Some(object) = parsed.as_object() else {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid map value for {setting_key}"),
                ));
            };
            if object
                .iter()
                .any(|(map_key, map_value)| map_key.is_empty() || !map_value.is_boolean())
            {
                return Err(AuthRestError::new(
                    400,
                    "Bad Request",
                    format!("Invalid map value for {setting_key}"),
                ));
            }
        }
        Some(_) => {
            return Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("Unsupported setting type for {setting_key}"),
            ));
        }
        None => {
            return Err(AuthRestError::new(
                400,
                "Bad Request",
                format!("Unsupported setting key: {setting_key}"),
            ));
        }
    }
    Ok(ApplicationCloudSettingDraft {
        setting_key,
        setting_value: setting_value.to_string(),
    })
}

fn application_cloud_setting_type(setting_key: &str) -> Option<&'static str> {
    match setting_key {
        "showAccountBalance"
        | "showAmountInHomePage"
        | "showTotalAmountInTransactionListPage"
        | "showTagInTransactionListPage"
        | "autoGetCurrentGeoLocation"
        | "alwaysShowTransactionPicturesInMobileTransactionEditPage" => Some("boolean"),
        "timezoneUsedForStatisticsInHomePage"
        | "itemsCountInTransactionListPage"
        | "currencySortByInExchangeRatesPage"
        | "statistics.defaultChartDataType"
        | "statistics.defaultTimezoneType"
        | "statistics.defaultSortingType"
        | "statistics.defaultCategoricalChartType"
        | "statistics.defaultCategoricalChartDataRangeType"
        | "statistics.defaultTrendChartType"
        | "statistics.defaultTrendChartDataRangeType"
        | "statistics.defaultAssetTrendsChartType"
        | "statistics.defaultAssetTrendsChartDataRangeType" => Some("number"),
        "overviewAccountFilterInHomePage"
        | "overviewTransactionCategoryFilterInHomePage"
        | "totalAmountExcludeAccountIds"
        | "statistics.defaultAccountFilter"
        | "statistics.defaultTransactionCategoryFilter" => Some("string_boolean_map"),
        "autoSaveTransactionDraft" => Some("string"),
        _ => None,
    }
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
    let runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: false,
        busy_timeout: state.config.timeout,
    })
    .map_err(|_| Box::new(db_error_response()))?;
    init_auth_security_schema(runtime.connection()).map_err(|_| Box::new(db_error_response()))?;
    Ok(runtime)
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

struct ExternalAuthInfo {
    external_auth_category: String,
    external_auth_type: String,
    linked: bool,
    external_username: String,
    created_at: i64,
}

impl ExternalAuthInfo {
    fn linked(row: ExternalAuthRow) -> Self {
        Self {
            external_auth_category: row.external_auth_category,
            external_auth_type: row.external_auth_type,
            linked: true,
            external_username: row.external_username,
            created_at: datetime_to_unix_millis(&row.created_at),
        }
    }
}

fn external_auths_payload(auths: Vec<ExternalAuthInfo>) -> Value {
    Value::Array(
        auths
            .into_iter()
            .map(|auth| {
                json!({
                    "externalAuthCategory": auth.external_auth_category,
                    "externalAuthType": auth.external_auth_type,
                    "linked": auth.linked,
                    "externalUsername": auth.external_username,
                    "createdAt": auth.created_at,
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

fn apply_auth_preflight_headers(requested_headers: Option<&HeaderValue>, headers: &mut HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        requested_headers
            .cloned()
            .unwrap_or_else(|| HeaderValue::from_static("authorization, content-type")),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    headers.append(
        header::VARY,
        HeaderValue::from_static("Access-Control-Request-Headers"),
    );
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

    #[test]
    fn profile_helper_edges_cover_updates_multipart_and_cloud_validation() {
        let body = json!({
            "nickname": "Alice",
            "email": "alice@example.test",
            "avatar": true,
            "language": "en",
            "defaultCurrency": "USD",
            "firstDayOfWeek": 2,
            "defaultAccountId": "",
            "transactionEditScope": "3",
            "fiscalYearStart": 4,
            "calendarDisplayType": 5,
            "dateDisplayType": "6",
            "longDateFormat": 7,
            "shortDateFormat": 8,
            "longTimeFormat": 9,
            "shortTimeFormat": 10,
            "fiscalYearFormat": 11,
            "currencyDisplayType": 12,
            "numeralSystem": 13,
            "decimalSeparator": 14,
            "digitGroupingSymbol": 15,
            "digitGrouping": 16,
            "coordinateDisplayType": 17,
            "expenseAmountColor": 18,
            "incomeAmountColor": 19,
            "cashAccountId": null,
            "cashTransferCategoryId": "200",
            "importLearningEnabled": "0",
            "investmentPlatformKeywords": ["蚂蚁财富", "雪球"],
            "investmentProductKeywords": "基金",
            "investmentExcludeKeywords": ["还款"]
        });
        let updates = profile_updates_from_body(body.as_object().expect("object"))
            .expect("profile updates parse");
        assert_eq!(updates.len(), 29);
        assert!(updates.contains(&AuthUserProfileUpdate::Nickname("Alice".to_string())));
        assert!(!updates
            .iter()
            .any(|update| matches!(update, AuthUserProfileUpdate::Avatar(_))));
        assert!(updates.contains(&AuthUserProfileUpdate::DefaultAccountId(None)));
        assert!(updates.contains(&AuthUserProfileUpdate::TransactionEditScope(3)));
        assert!(updates.contains(&AuthUserProfileUpdate::FiscalYearStart(4)));
        assert!(updates.contains(&AuthUserProfileUpdate::CashTransferCategoryId(Some(200))));
        assert!(updates.contains(&AuthUserProfileUpdate::ImportLearningEnabled(false)));
        assert_eq!(
            profile_string(&json!("en"), "language").expect("profile string"),
            "en"
        );
        assert!(profile_string(&Value::Null, "language").is_err());
        assert!(valid_email_address("alice@example.test"));
        assert!(!valid_email_address("not-an-email"));
        assert!(!valid_email_address("a@b@c.com"));
        assert!(!valid_email_address("alice @example.test"));
        assert!(!valid_email_address("alice@example .test"));
        assert!(!valid_email_address("alice@-example.test"));
        assert!(!valid_email_address("alice@example-.test"));
        assert_eq!(
            profile_email_changed(
                &[AuthUserProfileUpdate::Email("new@example.test".to_string())],
                "old@example.test",
            )
            .as_deref(),
            Some("new@example.test")
        );
        assert!(profile_email_changed(
            &[AuthUserProfileUpdate::Email(
                "same@example.test".to_string()
            )],
            "same@example.test",
        )
        .is_none());

        assert!(profile_i64(&Value::Bool(true), "firstDayOfWeek").is_err());
        assert_eq!(
            profile_i64(&json!("42"), "firstDayOfWeek").expect("numeric string"),
            42
        );
        assert!(profile_i64(
            &Value::Number(serde_json::Number::from(u64::MAX)),
            "firstDayOfWeek"
        )
        .is_err());
        assert!(profile_i64(&json!({"unexpected": true}), "firstDayOfWeek").is_err());
        assert!(profile_bool(&Value::Bool(true), "importLearningEnabled").expect("bool"));
        assert!(profile_bool(&json!(1), "importLearningEnabled").expect("one"));
        assert!(profile_bool(&json!("yes"), "importLearningEnabled").is_err());
        assert!(!profile_bool(&json!("false"), "importLearningEnabled").expect("false string"));
        assert!(!profile_bool(&json!("0"), "importLearningEnabled").expect("zero string"));
        assert!(profile_bool(&Value::Null, "importLearningEnabled").is_err());
        assert_eq!(
            optional_profile_id(&Value::Null, "defaultAccountId").expect("null clears"),
            None
        );
        assert_eq!(
            optional_profile_id(&json!(""), "defaultAccountId").expect("empty clears"),
            None
        );
        assert!(optional_profile_id(&json!("-1"), "defaultAccountId").is_err());
        assert!(optional_profile_id(&json!("0"), "defaultAccountId").is_err());
        assert!(optional_profile_id(&json!("abc"), "defaultAccountId").is_err());
        assert_eq!(
            optional_profile_id(&json!("7"), "defaultAccountId").expect("positive id"),
            Some(7)
        );

        let mut headers = HeaderMap::new();
        assert!(avatar_data_url_from_multipart(&headers, b"").is_err());
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=\"quoted\""),
        );
        assert_eq!(
            multipart_boundary(
                headers
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
            )
            .as_deref(),
            Some("quoted")
        );

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=plain"),
        );
        let no_avatar =
            b"--plain\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nvalue\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, no_avatar).is_err());
        let empty_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\n\r\n\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, empty_avatar).is_err());
        let text_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, text_avatar).is_err());
        let png_payload = b"\x89PNG\r\n\x1A\navatar";
        let png_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n\x1A\navatar\r\n--plain--\r\n";
        assert_eq!(
            avatar_data_url_from_multipart(&headers, png_avatar).expect("png avatar"),
            format!(
                "data:image/png;base64,{}",
                general_purpose::STANDARD.encode(png_payload)
            )
        );
        let mismatched_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\nContent-Type: image/jpeg\r\n\r\n\x89PNG\r\n\x1A\navatar\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, mismatched_avatar).is_err());
        assert!(avatar_data_url_from_payload(
            &vec![0_u8; MAX_AVATAR_BYTES + 1],
            Some("image/png".to_string())
        )
        .is_err());

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=lf"),
        );
        let lf_avatar =
            b"--lf\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.webp\"\n\nRIFFxxxxWEBPdata\n--lf--\n";
        assert_eq!(
            avatar_data_url_from_multipart(&headers, lf_avatar).expect("lf avatar"),
            "data:image/webp;base64,UklGRnh4eHhXRUJQZGF0YQ=="
        );
        assert_eq!(find_bytes(b"abc", b""), None);
        assert_eq!(find_bytes(b"abc", b"abcd"), None);

        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "autoSaveTransactionDraft", "settingValue": "draft"})
            )
            .expect("string setting")
            .setting_value,
            "draft"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "itemsCountInTransactionListPage", "settingValue": "25"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "itemsCountInTransactionListPage", "settingValue": "NaN?"})
            )
            .expect_err("invalid number")
            .message,
            "Invalid number value for itemsCountInTransactionListPage"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "showAmountInHomePage", "settingValue": "true"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "showAmountInHomePage", "settingValue": "1"})
            )
            .expect_err("invalid boolean")
            .message,
            "Invalid boolean value for showAmountInHomePage"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":true}"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{"})
            )
            .expect_err("invalid json")
            .message,
            "Invalid JSON value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "[]"})
            )
            .expect_err("invalid map")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"\":false}"})
            )
            .expect_err("empty map key")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":\"yes\"}"})
            )
            .expect_err("non boolean map value")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(&json!({"settingKey": "", "settingValue": "x"}))
                .expect_err("empty key")
                .message,
            "settingKey is required"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "autoSaveTransactionDraft", "settingValue": true})
            )
            .expect_err("non string value")
            .message,
            "Invalid setting value for autoSaveTransactionDraft"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "unsupported", "settingValue": "x"})
            )
            .expect_err("unsupported key")
            .message,
            "Unsupported setting key: unsupported"
        );
        assert_eq!(application_cloud_setting_type("unknown"), None);
    }
}
