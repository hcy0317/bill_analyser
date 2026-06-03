// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};

use crate::{
    auth_routes::auth_token_runtime_router,
    backup_routes::backup_ops_runtime_router,
    bill_routes::bill_runtime_router,
    budget_routes::budget_runtime_router,
    import_routes::import_runtime_router,
    matching_routes::matching_recurring_calendar_networth_runtime_router,
    runtime::{http_shell_health_with_weaviate_status, HttpShellHealth, HttpShellIdentity},
    state::HttpAppState,
    statistics_routes::statistics_runtime_router,
    taxonomy_routes::taxonomy_runtime_router,
    weaviate::probe_weaviate_health,
};

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_router(state: HttpAppState) -> Router {
    // 注册顺序体现 Rust-only `/api/...` 主链：业务 router 在 fallback 之前合并，
    // 未知 API 统一由 Rust 返回结构化 404，不能重新透传sidecar。
    let body_limit_bytes = state.config.body_limit_bytes;
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/runtime", get(metadata_handler))
        .merge(import_runtime_router())
        .merge(bill_runtime_router())
        .merge(auth_token_runtime_router())
        .merge(backup_ops_runtime_router())
        .merge(budget_runtime_router())
        .merge(matching_recurring_calendar_networth_runtime_router())
        .merge(statistics_runtime_router())
        .merge(taxonomy_runtime_router())
        .fallback(not_found_handler)
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .with_state(state)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn health_handler(State(state): State<HttpAppState>) -> Json<HttpShellHealth> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "runtime",
        operation = "health_handler",
        "business operation entered"
    );
    let weaviate_status = probe_weaviate_health(&state.config).await;
    Json(http_shell_health_with_weaviate_status(
        &state.config,
        &weaviate_status.health_detail_value(),
    ))
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
