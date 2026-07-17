use std::time::{Duration, Instant};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use bill_analyser_http::{build_router, HttpAppState, HttpShellConfig};
use serde_json::Value;
use tower::ServiceExt;

fn unavailable_postgres_state() -> HttpAppState {
    let mut config = HttpShellConfig::new("", Duration::from_millis(100), 1024)
        .expect("test config")
        .with_postgres_url("postgres://health:secret@127.0.0.1:1/bill_analyser")
        .expect("test postgres URL");
    config.weaviate.enabled = false;
    config.weaviate.endpoint = None;
    HttpAppState::new(config).expect("test state")
}

async fn request_health_with_state(state: HttpAppState, path: &str) -> (StatusCode, Value) {
    let response = build_router(state)
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("health body");
    let payload = serde_json::from_slice(&body).expect("health JSON");
    (status, payload)
}

async fn request_health(path: &str) -> (StatusCode, Value) {
    request_health_with_state(unavailable_postgres_state(), path).await
}

#[tokio::test]
async fn liveness_stays_healthy_when_dependencies_are_unavailable() {
    let (status, payload) = request_health("/api/health/live").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["details"]["probe"], "liveness");
}

#[tokio::test]
async fn readiness_fails_fast_when_postgres_is_unavailable() {
    let started = Instant::now();
    let (status, payload) = request_health("/api/health/ready").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(payload["status"], "unhealthy");
    assert_eq!(payload["details"]["probe"], "readiness");
    assert!(payload["details"]["postgres_authority_status"]
        .as_str()
        .is_some_and(|value| value.starts_with("unhealthy:")));
    assert!(started.elapsed() <= Duration::from_secs(2));
    assert!(!payload.to_string().contains("health:secret"));
}

#[tokio::test]
async fn legacy_health_path_uses_readiness_semantics() {
    let (status, payload) = request_health("/api/health").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(payload["details"]["probe"], "readiness");
}

#[tokio::test]
async fn readiness_succeeds_against_real_postgres_and_weaviate_fixtures_when_configured() {
    let (Ok(postgres_url), Ok(weaviate_endpoint)) = (
        std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL"),
        std::env::var("BILL_ANALYSER_TEST_WEAVIATE_ENDPOINT"),
    ) else {
        return;
    };
    let mut config = HttpShellConfig::new("", Duration::from_secs(1), 1024)
        .expect("test config")
        .with_postgres_url(postgres_url)
        .expect("test postgres URL");
    config.weaviate.enabled = true;
    config.weaviate.endpoint = Some(weaviate_endpoint);
    config.weaviate.timeout = Duration::from_secs(1);
    let state = HttpAppState::new(config).expect("real dependency state");

    let (status, payload) = request_health_with_state(state, "/api/health/ready").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["details"]["postgres_authority_status"], "healthy");
    assert_eq!(payload["details"]["weaviate_status"], "healthy");
}
