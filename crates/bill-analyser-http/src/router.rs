use axum::{
    extract::State,
    response::Json,
    routing::{any, get},
    Router,
};

use crate::{
    bill_routes::bill_runtime_router,
    budget_routes::budget_runtime_router,
    config::ImportRouteMode,
    import_routes::{import_runtime_router, import_skeleton_router},
    proxy::{proxy_handler, ProxyState},
    runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity},
    statistics_routes::statistics_runtime_router,
};

pub fn build_router(state: ProxyState) -> Router {
    let import_route_mode = state.config.import_route_mode;
    let router = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/runtime", get(metadata_handler));
    let router = match import_route_mode {
        ImportRouteMode::ImportRouteSkeleton => router.merge(import_skeleton_router()),
        ImportRouteMode::ImportDbRuntime => router
            .merge(import_runtime_router())
            .merge(bill_runtime_router())
            .merge(budget_runtime_router())
            .merge(statistics_runtime_router()),
        ImportRouteMode::ProxyOnly => router,
    };

    router.fallback(any(proxy_handler)).with_state(state)
}

pub async fn health_handler(State(state): State<ProxyState>) -> Json<HttpShellHealth> {
    Json(http_shell_health(&state.config))
}

pub async fn metadata_handler(State(state): State<ProxyState>) -> Json<HttpShellIdentity> {
    Json(HttpShellIdentity::for_import_route_mode(
        state.config.import_route_mode,
    ))
}
