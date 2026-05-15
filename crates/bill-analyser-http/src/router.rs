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
    runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity},
    state::HttpAppState,
    statistics_routes::statistics_runtime_router,
    taxonomy_routes::taxonomy_runtime_router,
};

pub fn build_router(state: HttpAppState) -> Router {
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

pub async fn health_handler(State(state): State<HttpAppState>) -> Json<HttpShellHealth> {
    Json(http_shell_health(&state.config))
}

pub async fn metadata_handler(State(state): State<HttpAppState>) -> Json<HttpShellIdentity> {
    Json(HttpShellIdentity::for_import_route_mode(
        state.config.import_route_mode,
    ))
}

async fn not_found_handler() -> Response<Body> {
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
