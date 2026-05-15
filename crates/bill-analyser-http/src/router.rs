use axum::{
    extract::{DefaultBodyLimit, State},
    response::Json,
    routing::{any, get},
    Router,
};

use crate::{
    auth_routes::auth_token_runtime_router,
    bill_routes::bill_runtime_router,
    budget_routes::budget_runtime_router,
    config::ImportRouteMode,
    import_routes::{import_runtime_router, import_skeleton_router},
    matching_routes::matching_recurring_calendar_networth_runtime_router,
    proxy::{ownership_aware_proxy_handler, proxy_handler, ProxyState},
    runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity},
    statistics_routes::statistics_runtime_router,
    taxonomy_routes::taxonomy_runtime_router,
};

pub fn build_router(state: ProxyState) -> Router {
    let import_route_mode = state.config.import_route_mode;
    let body_limit_bytes = state.config.body_limit_bytes;
    let router = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/runtime", get(metadata_handler));
    let router = match import_route_mode {
        ImportRouteMode::ImportRouteSkeleton => router.merge(import_skeleton_router()),
        ImportRouteMode::ImportDbRuntime => router
            .merge(import_runtime_router())
            .merge(bill_runtime_router())
            .merge(auth_token_runtime_router())
            .merge(budget_runtime_router())
            .merge(matching_recurring_calendar_networth_runtime_router())
            .merge(statistics_runtime_router())
            .merge(taxonomy_runtime_router()),
        ImportRouteMode::ProxyOnly => router,
    };

    let fallback = match import_route_mode {
        ImportRouteMode::ImportDbRuntime => any(ownership_aware_proxy_handler),
        ImportRouteMode::ImportRouteSkeleton | ImportRouteMode::ProxyOnly => any(proxy_handler),
    };

    router
        .fallback(fallback)
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .with_state(state)
}

pub async fn health_handler(State(state): State<ProxyState>) -> Json<HttpShellHealth> {
    Json(http_shell_health(&state.config))
}

pub async fn metadata_handler(State(state): State<ProxyState>) -> Json<HttpShellIdentity> {
    Json(HttpShellIdentity::for_import_route_mode(
        state.config.import_route_mode,
    ))
}
