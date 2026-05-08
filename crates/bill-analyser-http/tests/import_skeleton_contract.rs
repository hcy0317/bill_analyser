use std::{collections::BTreeSet, net::SocketAddr, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use bill_analyser_core::import_deletion_blocked_endpoints;
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState, IMPORT_SKELETON_ROUTE_PATTERNS,
};
use serde_json::Value;
use tokio::net::TcpListener;
use tower::ServiceExt;

#[test]
fn import_skeleton_registry_covers_all_deletion_blocked_governance_routes() {
    let expected = import_deletion_blocked_endpoints()
        .into_iter()
        .map(|endpoint| (endpoint.method, endpoint.pattern))
        .collect::<BTreeSet<_>>();
    let actual = IMPORT_SKELETON_ROUTE_PATTERNS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert!(actual.contains(&("POST", "/api/llm/preview-recommend/accept")));
    assert!(!actual.contains(&("POST", "/api/llm/rule-synthesis")));
}

#[tokio::test]
async fn import_route_skeleton_intercepts_every_first_phase_route_without_proxy() {
    let app = skeleton_router(unavailable_upstream().await);

    for (method, pattern) in IMPORT_SKELETON_ROUTE_PATTERNS {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::from_bytes(method.as_bytes()).expect("method parses"))
                    .uri(sample_path(pattern))
                    .body(Body::from("{}"))
                    .expect("request builds"),
            )
            .await
            .expect("response");

        assert_ne!(
            response.status(),
            StatusCode::BAD_GATEWAY,
            "{method} {pattern} fell through to the Python proxy"
        );
    }
}

#[tokio::test]
async fn import_skeleton_parse_reports_no_business_ownership_db_writes_or_deletion() {
    let app = skeleton_router(unavailable_upstream().await);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .body(Body::from(r#"{"files":[]}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(
        body["error"],
        "Rust import route skeleton is not business-owned; Python proxy remains authoritative"
    );
    assert_eq!(body["data"]["route_owner"], "python_proxied");
    assert_eq!(body["data"]["business_migration"], "import_route_skeleton");
    assert_eq!(body["data"]["db_write_allowed"], false);
    assert_eq!(body["data"]["python_import_deletion_allowed"], false);
}

#[tokio::test]
async fn import_skeleton_uses_safe_session_missing_fixtures_without_db_state() {
    let app = skeleton_router(unavailable_upstream().await);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/sess-missing")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    let get_body = read_json(get_response).await;
    assert_eq!(get_body["success"], false);
    assert_eq!(get_body["error"], "Session not found or expired");

    let delete_response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/bills/import/v2/session/sess-missing")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_body = read_json(delete_response).await;
    assert_eq!(delete_body["success"], false);
    assert_eq!(delete_body["message"], "Session not found");
}

#[tokio::test]
async fn import_skeleton_runtime_metadata_stays_non_takeover() {
    let app = skeleton_router(unavailable_upstream().await);

    let runtime = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/runtime")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    let runtime_body = read_json(runtime).await;
    assert_eq!(
        runtime_body["runtime_boundary"],
        "rust-http-shell:import-route-skeleton"
    );
    assert_eq!(
        runtime_body["business_migration"],
        "import-route-skeleton-no-db"
    );
    assert_eq!(runtime_body["api_takeover"], true);

    let health = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    let health_body = read_json(health).await;
    assert_eq!(
        health_body["details"]["import_route_mode"],
        "import_route_skeleton"
    );
    assert_eq!(
        health_body["details"]["owned_routes"],
        "/api/health,/api/runtime,import/preview-adjacent runtime routes"
    );
    assert_eq!(health_body["identity"]["api_takeover"], true);
}

fn skeleton_router(upstream: String) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        upstream,
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportRouteSkeleton,
    )
    .expect("config");
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

async fn unavailable_upstream() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    drop(listener);
    format!("http://{addr}")
}

fn sample_path(pattern: &str) -> String {
    pattern
        .replace("{session_id}", "sess-1")
        .replace("{preview_id}", "42")
        .replace("{config_id}", "7")
        .replace("{rule_id}", "9")
        .replace("{suggestion_id}", "11")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}
