use std::time::Duration;

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use bill_analyser_http::{bill_runtime_router, HttpAppState, HttpShellConfig};
use serde_json::{json, Value};
use tower::ServiceExt;

fn bill_router() -> axum::Router {
    let state = HttpAppState::new(
        HttpShellConfig::new("", Duration::from_millis(100), 1024).expect("test HTTP config"),
    )
    .expect("test HTTP state");
    bill_runtime_router().with_state(state)
}

async fn post_json(path: &str, body: &'static str) -> (StatusCode, Value) {
    let response = bill_router()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("bill route request"),
        )
        .await
        .expect("bill route response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    let value = serde_json::from_slice(&body).expect("bill route JSON response");
    (status, value)
}

#[tokio::test]
async fn malformed_json_is_a_client_error_for_bill_body_helpers() {
    for path in [
        "/api/bills/category/quick-add-rule",
        "/api/bills/category/refresh",
    ] {
        let (status, body) = post_json(path, "{").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(
            body,
            json!({"success": false, "error": "Invalid JSON body"}),
            "{path} must not expose serde parser details"
        );
    }
}

#[tokio::test]
async fn valid_json_preserves_existing_bill_route_domain_errors() {
    let (status, body) = post_json("/api/bills/category/quick-add-rule", "{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({
            "success": false,
            "error": "Request body is required"
        })
    );

    let (status, body) = post_json("/api/bills/category/refresh", "{}").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["success"], false);
}
