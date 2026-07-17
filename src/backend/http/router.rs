// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::time::Duration;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};

use crate::{
    auth::resolve_authoritative_authenticated_user_from_headers,
    auth_routes::auth_token_runtime_router,
    backup_routes::backup_ops_runtime_router,
    bill_routes::bill_runtime_router,
    budget_routes::budget_runtime_router,
    import_routes::import_runtime_router,
    matching_routes::matching_recurring_calendar_networth_runtime_router,
    runtime::{
        http_shell_liveness, http_shell_readiness_with_dependency_statuses, HttpShellHealth,
        HttpShellIdentity,
    },
    state::HttpAppState,
    statistics_routes::statistics_runtime_router,
    taxonomy_routes::taxonomy_runtime_router,
    weaviate::probe_weaviate_health,
};

const POSTGRES_READINESS_TIMEOUT: Duration = Duration::from_secs(1);

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_router(state: HttpAppState) -> Router {
    // 注册顺序体现 Rust-only `/api/...` 主链：业务 router 在 fallback 之前合并，
    // 未知 API 统一由 Rust 返回结构化 404，不能重新透传sidecar。
    let body_limit_bytes = state.config.body_limit_bytes;
    let protected_routes = Router::new()
        .merge(import_runtime_router())
        .merge(bill_runtime_router())
        .merge(auth_token_runtime_router())
        .merge(backup_ops_runtime_router())
        .merge(budget_runtime_router())
        .merge(matching_recurring_calendar_networth_runtime_router())
        .merge(statistics_runtime_router())
        .merge(taxonomy_runtime_router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            session_authority_middleware,
        ));
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/health/live", get(liveness_handler))
        .route("/api/health/ready", get(readiness_handler))
        .route("/api/runtime", get(metadata_handler))
        .merge(protected_routes)
        .fallback(not_found_handler)
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .with_state(state)
}

async fn session_authority_middleware(
    State(state): State<HttpAppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    if auth_session_authority_is_bypassed(request.method(), request.uri().path()) {
        return next.run(request).await;
    }
    match resolve_authoritative_authenticated_user_from_headers(
        request.headers(),
        &state,
        "x-bill-analyser-trusted-user-secret",
    )
    .await
    {
        Ok(authenticated) => {
            request.extensions_mut().insert(authenticated);
            next.run(request).await
        }
        Err(error) => {
            let status =
                StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (
                status,
                Json(serde_json::json!({
                    "success": false,
                    "error": error.message,
                })),
            )
                .into_response()
        }
    }
}

fn auth_session_authority_is_bypassed(method: &Method, path: &str) -> bool {
    if method == Method::OPTIONS {
        return true;
    }
    matches!(
        path,
        "/api/auth/login"
            | "/api/auth/register"
            | "/api/auth/oauth2/authorize"
            | "/api/auth/email/verify"
            | "/api/auth/email/resend-verification"
            | "/api/auth/password/forgot"
            | "/api/auth/password/reset"
            | "/api/tokens/refresh"
            | "/api/2fa/verify"
            | "/api/2fa/recovery/verify"
            | "/api/system/version"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn health_handler(State(state): State<HttpAppState>) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "runtime",
        operation = "health_handler",
        "business operation entered"
    );
    readiness_response(&state).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn readiness_handler(State(state): State<HttpAppState>) -> Response {
    readiness_response(&state).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn liveness_handler(State(state): State<HttpAppState>) -> Json<HttpShellHealth> {
    Json(http_shell_liveness(&state.config))
}

async fn readiness_response(state: &HttpAppState) -> Response {
    let (postgres_status, weaviate_status) = tokio::join!(
        probe_postgres_readiness(state),
        probe_weaviate_health(&state.config)
    );
    let health = http_shell_readiness_with_dependency_statuses(
        &state.config,
        postgres_status,
        &weaviate_status.health_detail_value(),
    );
    let status = if health.status == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(health)).into_response()
}

async fn probe_postgres_readiness(state: &HttpAppState) -> &'static str {
    if !state.config.postgres_configured() {
        return "unhealthy:unconfigured";
    }
    let runtime = match state.open_postgres_repository_runtime("readiness probe") {
        Ok(runtime) => runtime,
        Err(_) => return "unhealthy:runtime_unavailable",
    };
    let status = match tokio::time::timeout(
        POSTGRES_READINESS_TIMEOUT,
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(runtime.pool()),
    )
    .await
    {
        Ok(Ok(1)) => "healthy",
        Ok(Ok(_)) => "unhealthy:unexpected_result",
        Ok(Err(_)) => "unhealthy:unavailable",
        Err(_) => "unhealthy:timeout",
    };
    if status != "healthy" {
        state.invalidate_postgres_repository_runtime();
    }
    status
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn metadata_handler(State(state): State<HttpAppState>) -> Json<HttpShellIdentity> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "runtime",
        operation = "metadata_handler",
        "business operation entered"
    );
    Json(HttpShellIdentity::for_import_route_mode(
        state.config.import_route_mode,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn not_found_handler() -> Response<Body> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "runtime",
        operation = "not_found_handler",
        "business operation entered"
    );
    // fallback 是前端 route ownership contract 的兜底响应；保持机器可读 code，
    // 便于 contract test 区分 Rust 未实现与网络/代理错误。
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "success": false,
            "error": {
                "code": "route_not_found",
                "message": "route is not implemented by the Rust backend"
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use base64::{engine::general_purpose, Engine as _};
    use chrono::Duration as ChronoDuration;
    use ring::hmac;
    use serde_json::json;
    use tower::ServiceExt;

    use crate::config::HttpShellConfig;

    static PROTECTED_HANDLER_INVOCATIONS: AtomicUsize = AtomicUsize::new(0);
    const TEST_JWT_SECRET: &str = "router-session-authority-test-secret";

    #[tokio::test]
    async fn session_authority_failures_do_not_invoke_protected_handler() {
        PROTECTED_HANDLER_INVOCATIONS.store(0, Ordering::SeqCst);
        let state = HttpAppState::new(
            HttpShellConfig::default()
                .with_auth_jwt_secret(TEST_JWT_SECRET)
                .with_postgres_url("postgres://bill_analyser:invalid@127.0.0.1:9/unavailable")
                .expect("test URL"),
        )
        .expect("test state");
        let app = Router::new()
            .route("/api/protected-test", get(counted_protected_handler))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                session_authority_middleware,
            ))
            .with_state(state);

        for (token, expected_status) in [
            ("not-a-jwt".to_string(), StatusCode::UNAUTHORIZED),
            (
                signed_access_token(7, ChronoDuration::minutes(-1)),
                StatusCode::UNAUTHORIZED,
            ),
            (
                signed_access_token(7, ChronoDuration::minutes(10)),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/protected-test")
                        .header("authorization", format!("Bearer {token}"))
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), expected_status);
        }
        assert_eq!(PROTECTED_HANDLER_INVOCATIONS.load(Ordering::SeqCst), 0);
    }

    async fn counted_protected_handler() -> StatusCode {
        PROTECTED_HANDLER_INVOCATIONS.fetch_add(1, Ordering::SeqCst);
        StatusCode::OK
    }

    fn signed_access_token(user_id: i64, expires_in: ChronoDuration) -> String {
        let now = chrono::Utc::now();
        let header = json!({"alg": "HS256", "typ": "JWT"});
        let payload = json!({
            "user_id": user_id,
            "type": "access",
            "iat": now.timestamp(),
            "exp": (now + expires_in).timestamp(),
        });
        let encoded_header = general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&header).expect("header JSON"));
        let encoded_payload = general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).expect("payload JSON"));
        let signing_input = format!("{encoded_header}.{encoded_payload}");
        let key = hmac::Key::new(hmac::HMAC_SHA256, TEST_JWT_SECRET.as_bytes());
        let signature = hmac::sign(&key, signing_input.as_bytes());
        format!(
            "{signing_input}.{}",
            general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref())
        )
    }
}
