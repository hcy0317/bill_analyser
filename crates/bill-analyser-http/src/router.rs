use axum::{
    extract::State,
    response::Json,
    routing::{any, get},
    Router,
};

use crate::{
    proxy::{proxy_handler, ProxyState},
    runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity},
};

pub fn build_router(state: ProxyState) -> Router {
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/runtime", get(metadata_handler))
        .fallback(any(proxy_handler))
        .with_state(state)
}

pub async fn health_handler(State(state): State<ProxyState>) -> Json<HttpShellHealth> {
    Json(http_shell_health(&state.config))
}

pub async fn metadata_handler() -> Json<HttpShellIdentity> {
    Json(HttpShellIdentity::current())
}
