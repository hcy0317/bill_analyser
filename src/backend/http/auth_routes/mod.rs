// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
};

use axum::{
    body::{Body, Bytes},
    extract::{connect_info::ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{
    adapters::transaction::serialize_optional_export_cell,
    auth::{
        infer_token_type_from_user_agent, json_object_or_empty, parse_user_agent_device_name,
        validate_refresh_token_claims, AuthRestError, TokenKind,
    },
    build_user_investment_keyword_settings, parse_comma_separated_ints,
    parse_export_timestamp_millis, serialize_keyword_list, user_data_statistics_response,
    UserDataClearKind, UserId,
};
use bill_analyser_db::{
    auth_account_belongs_to_user, auth_category_belongs_to_user, auth_email_exists,
    auth_email_exists_for_other_user, auth_username_exists, cleanup_expired_sessions,
    cleanup_postgres_expired_sessions, clear_postgres_user_data, clear_postgres_user_transactions,
    clear_user_data, clear_user_transactions, consume_postgres_two_factor_recovery_code,
    consume_two_factor_recovery_code, count_auth_events_since, count_postgres_auth_events_since,
    count_postgres_recent_token_password_failures, count_recent_token_password_failures,
    create_auth_log, create_auth_log_under_event_limit, create_postgres_auth_log,
    create_postgres_auth_log_under_event_limit, create_postgres_registered_user_with_defaults,
    create_postgres_token_session, create_postgres_user_data_audit_event,
    create_registered_user_with_defaults, create_token_session, delete_application_cloud_settings,
    delete_postgres_application_cloud_settings, delete_postgres_user_external_auth,
    delete_user_external_auth, disable_postgres_two_factor_and_clear_recovery_codes,
    disable_two_factor_and_clear_recovery_codes,
    enable_postgres_two_factor_with_recovery_codes_and_session,
    enable_two_factor_with_recovery_codes_and_session, get_active_logout_session_by_token_hash,
    get_active_refresh_session, get_app_setting, get_auth_token_user, get_auth_user_profile,
    get_auth_user_two_factor_enabled, get_login_user_by_email, get_login_user_by_id,
    get_login_user_by_login_name, get_postgres_active_refresh_session,
    get_postgres_active_session_id_by_token_hash, get_postgres_auth_token_user,
    get_postgres_auth_user_profile, get_postgres_auth_user_two_factor_enabled,
    get_postgres_login_user_by_id, get_postgres_login_user_by_login_name,
    get_postgres_operation_password, get_postgres_user_data_statistics,
    get_postgres_user_external_auth, get_user_data_statistics as get_db_user_data_statistics,
    get_user_external_auth, increment_failed_login, increment_postgres_failed_login,
    init_app_settings_schema, init_auth_security_schema, invalidate_other_postgres_user_sessions,
    invalidate_other_user_sessions, invalidate_postgres_session_by_id,
    invalidate_postgres_session_by_token_hash, invalidate_session_by_id,
    invalidate_session_by_token_hash, list_application_cloud_settings,
    list_postgres_application_cloud_settings, list_postgres_user_data_categories,
    list_postgres_user_external_auths, list_postgres_user_sessions, list_user_data_categories,
    list_user_external_auths, list_user_sessions, load_postgres_user_data_export,
    load_user_data_export, postgres_auth_account_belongs_to_user,
    postgres_auth_category_belongs_to_user, postgres_auth_email_exists,
    postgres_auth_email_exists_for_other_user, postgres_auth_username_exists,
    replace_postgres_two_factor_recovery_codes, rotate_postgres_refresh_token_session,
    rotate_refresh_token_session, set_postgres_user_email_verified, set_user_email_verified,
    update_application_cloud_settings, update_auth_user_profile,
    update_auth_user_profile_with_auth_log, update_postgres_application_cloud_settings,
    update_postgres_auth_user_profile, update_postgres_auth_user_profile_with_auth_log,
    update_postgres_user_last_login, update_postgres_user_password_hash, update_user_last_login,
    update_user_password_hash, ApplicationCloudSettingDraft, ApplicationCloudSettingRow,
    AuthLogDraft, AuthLoginUserRow, AuthUserProfileRow, AuthUserProfileUpdate, BillCategoryFilter,
    BillFilters, CreateTokenSessionDraft, DbError, ExternalAuthRow, PostgresRepositoryRuntime,
    PostgresUserDataAuditEvent, RegisterPresetCategory, RegisterPresetSubCategory,
    RegisterUserDraft, SqliteRuntime, TokenSessionRow, UserDataExportBundle,
    UserDataExportCategory,
};
use chrono::{Duration as ChronoDuration, Local, NaiveDateTime, TimeZone, Utc};
use qrcodegen::{QrCode, QrCodeEcc};
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
    state::HttpAppState,
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const TOKEN_PASSWORD_FAILURE_LIMIT: i64 = 5;
const TOKEN_PASSWORD_FAILURE_WINDOW_MINUTES: i64 = 15;
const SENSITIVE_AUTH_FAILURE_LIMIT: i64 = 5;
const SENSITIVE_AUTH_FAILURE_WINDOW_MINUTES: i64 = 15;
const STEP_UP_AUTH_FAILED_EVENT: &str = "step_up_auth_failed";
const USER_DATA_CLEAR_AUTH_FAILED_EVENT: &str = "user_data_clear_auth_failed";
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
    ("GET", "/api/data/export.{file_type}"),
    ("POST", "/api/data/clear/transactions"),
    ("POST", "/api/data/clear/all"),
    ("POST", "/api/security/step-up/verify"),
    ("GET", "/api/2fa/status"),
    ("POST", "/api/2fa/verify"),
    ("POST", "/api/2fa/enable/request"),
    ("POST", "/api/2fa/enable/confirm"),
    ("POST", "/api/2fa/disable"),
    ("POST", "/api/2fa/recovery/regenerate"),
    ("POST", "/api/2fa/recovery/verify"),
];

#[tracing::instrument(level = "debug", skip_all)]
pub fn auth_token_runtime_router() -> Router<HttpAppState> {
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
            "/api/data/export.:file_type",
            get(export_user_data_handler).options(auth_options_handler),
        )
        .route(
            "/api/data/clear/transactions",
            post(clear_user_transactions_handler).options(auth_options_handler),
        )
        .route(
            "/api/data/clear/all",
            post(clear_all_user_data_handler).options(auth_options_handler),
        )
        .route(
            "/api/security/step-up/verify",
            post(verify_security_step_up_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/status",
            get(get_two_factor_status_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/verify",
            post(verify_two_factor_login_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/enable/request",
            post(request_two_factor_enable_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/enable/confirm",
            post(confirm_two_factor_enable_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/disable",
            post(disable_two_factor_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/recovery/regenerate",
            post(regenerate_two_factor_recovery_codes_handler).options(auth_options_handler),
        )
        .route(
            "/api/2fa/recovery/verify",
            post(verify_two_factor_recovery_login_handler).options(auth_options_handler),
        )
        .route(
            "/api/tokens",
            get(list_tokens_handler).delete(revoke_other_tokens_handler),
        )
        .route("/api/tokens/:token_id", delete(revoke_token_handler))
        .layer(middleware::from_fn(auth_cors_middleware))
}

include!("public_auth_handlers.rs");
include!("profile_data_handlers.rs");
include!("two_factor_handlers.rs");
include!("token_session_handlers.rs");
include!("jwt_totp_helpers.rs");
include!("data_export_helpers.rs");
include!("profile_payload_helpers.rs");
include!("runtime_audit_helpers.rs");
