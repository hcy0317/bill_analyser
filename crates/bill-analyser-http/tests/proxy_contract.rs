use std::{net::SocketAddr, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use bill_analyser_http::{
    build_router, build_upstream_url, filter_proxy_request_headers, http_shell_health,
    HttpShellConfig, HttpShellConfigError, ProxyState, REQUEST_ID_HEADER,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[test]
fn config_defaults_keep_python_as_proxy_fallback() {
    let config = HttpShellConfig::default();
    let health = http_shell_health(&config);

    assert_eq!(config.python_upstream, "http://127.0.0.1:5000");
    assert_eq!(config.timeout, Duration::from_millis(30_000));
    assert_eq!(config.body_limit_bytes, 10 * 1024 * 1024);
    assert_eq!(
        health.identity.runtime_boundary,
        "rust-http-shell:proxy-only"
    );
    assert_eq!(health.identity.business_migration, "none");
    assert!(!health.identity.api_takeover);
    assert_eq!(
        health.details.get("proxied_routes"),
        Some(&"unowned /api/*".to_string())
    );
}

#[test]
fn config_from_env_reads_explicit_proxy_values() {
    let config = HttpShellConfig::from_env_with(|name| match name {
        "BILL_ANALYSER_PYTHON_UPSTREAM" => Some("http://127.0.0.1:5999/".to_string()),
        "BILL_ANALYSER_HTTP_TIMEOUT_MS" => Some("1234".to_string()),
        "BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES" => Some("4096".to_string()),
        _ => None,
    })
    .expect("env config parses");

    assert_eq!(config.python_upstream, "http://127.0.0.1:5999");
    assert_eq!(config.timeout, Duration::from_millis(1234));
    assert_eq!(config.body_limit_bytes, 4096);
}

#[test]
fn config_rejects_invalid_upstream_body_limit_and_env_integer() {
    assert_eq!(
        HttpShellConfig::new("ftp://127.0.0.1", Duration::from_secs(1), 1).unwrap_err(),
        HttpShellConfigError::InvalidUpstream
    );
    assert_eq!(
        HttpShellConfig::new("http://127.0.0.1:5000", Duration::from_secs(1), 0).unwrap_err(),
        HttpShellConfigError::InvalidBodyLimit
    );

    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_HTTP_TIMEOUT_MS" => Some("not-a-number".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_HTTP_TIMEOUT_MS")
    );
}

#[test]
fn upstream_url_preserves_path_and_query() {
    let url =
        build_upstream_url("http://127.0.0.1:5000/", "/api/bills?x=1&y=2").expect("url builds");

    assert_eq!(url, "http://127.0.0.1:5000/api/bills?x=1&y=2");
}

#[test]
fn request_header_filter_removes_hop_by_hop_host_and_content_length() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("example.test"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("close"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("12"));
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer token"),
    );
    headers.insert(header::COOKIE, HeaderValue::from_static("session=abc"));
    headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("rid-client"));

    let filtered = filter_proxy_request_headers(&headers);

    assert!(!filtered.contains_key(header::HOST));
    assert!(!filtered.contains_key(header::CONNECTION));
    assert!(!filtered.contains_key(header::CONTENT_LENGTH));
    assert!(!filtered.contains_key(REQUEST_ID_HEADER));
    assert_eq!(
        filtered.get(header::AUTHORIZATION),
        Some(&HeaderValue::from_static("Bearer token"))
    );
    assert_eq!(
        filtered.get(header::COOKIE),
        Some(&HeaderValue::from_static("session=abc"))
    );
}

#[test]
fn response_header_filter_preserves_set_cookie_and_drops_hop_by_hop() {
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, HeaderValue::from_static("upstream=ok"));
    headers.insert(
        header::TRANSFER_ENCODING,
        HeaderValue::from_static("chunked"),
    );

    let filtered = bill_analyser_http::filter_proxy_response_headers(&headers);

    assert_eq!(
        filtered.get(header::SET_COOKIE),
        Some(&HeaderValue::from_static("upstream=ok"))
    );
    assert!(!filtered.contains_key(header::TRANSFER_ENCODING));
}

#[tokio::test]
async fn rust_owned_routes_take_precedence_over_proxy() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runtime")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    let body = read_json(response).await;
    assert_eq!(body["crate_name"], "bill-analyser-http");
    assert_eq!(body["business_migration"], "none");
    assert_eq!(body["api_takeover"], false);
}

#[tokio::test]
async fn health_route_reports_proxy_only_runtime_boundary() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    let body = read_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(
        body["identity"]["runtime_boundary"],
        "rust-http-shell:proxy-only"
    );
    assert_eq!(body["identity"]["business_migration"], "none");
    assert_eq!(body["identity"]["api_takeover"], false);
}

#[tokio::test]
async fn proxy_preserves_method_query_headers_cookies_and_json_body() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse?stage=parse")
                .header(header::AUTHORIZATION, "Bearer token")
                .header(header::COOKIE, "session=abc")
                .header(REQUEST_ID_HEADER, "rid-from-client")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"amount":123}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(REQUEST_ID_HEADER),
        Some(&HeaderValue::from_static("rid-from-client"))
    );
    assert_eq!(
        response.headers().get(header::SET_COOKIE),
        Some(&HeaderValue::from_static("upstream=ok"))
    );

    let body = read_json(response).await;
    assert_eq!(body["method"], "POST");
    assert_eq!(body["path"], "/api/bills/import/v2/parse");
    assert_eq!(body["query"], "stage=parse");
    assert_eq!(body["authorization"], "Bearer token");
    assert_eq!(body["cookie"], "session=abc");
    assert_eq!(body["request_id"], "rid-from-client");
    assert_eq!(body["content_type"], "application/json");
    assert_eq!(body["body"], r#"{"amount":123}"#);
}

#[tokio::test]
async fn proxy_preserves_non_json_body_and_content_type() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/upload")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("plain body"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    let body = read_json(response).await;
    assert_eq!(body["method"], "PUT");
    assert_eq!(body["content_type"], "text/plain");
    assert_eq!(body["body"], "plain body");
}

#[tokio::test]
async fn proxy_forwards_non_success_status_and_body_unchanged() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/upstream-status/418")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
    let body = read_string(response).await;
    assert_eq!(body, "status:418");
}

#[tokio::test]
async fn proxy_generates_request_id_when_missing() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/no-request-id")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert!(body["request_id"]
        .as_str()
        .is_some_and(|value| value.starts_with("rust-http-")));
}

#[tokio::test]
async fn proxy_wraps_upstream_down_as_infrastructure_error() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    let app = test_router(format!("http://{addr}"), Duration::from_millis(200));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/down")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert!(body["error"]["code"]
        .as_str()
        .is_some_and(|code| code.starts_with("proxy_upstream_")));
}

#[tokio::test]
async fn proxy_wraps_timeout_as_infrastructure_error() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_millis(10));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/slow")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "proxy_upstream_timeout");
}

#[tokio::test]
async fn proxy_wraps_body_limit_overflow_as_infrastructure_error() {
    let upstream = spawn_fake_upstream().await;
    let config = HttpShellConfig::new(upstream.url(), Duration::from_secs(5), 4).expect("config");
    let state = ProxyState::new(config).expect("proxy state");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/too-large")
                .body(Body::from("larger than four bytes"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "proxy_body_read_failed");
}

fn test_router(upstream: String, timeout: Duration) -> Router {
    let config = HttpShellConfig::new(upstream, timeout, 1024 * 1024).expect("config");
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

struct FakeUpstream {
    addr: SocketAddr,
}

impl FakeUpstream {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

async fn spawn_fake_upstream() -> FakeUpstream {
    let app = Router::new()
        .route("/slow", get(slow_handler))
        .route("/upstream-status/:code", any(status_handler))
        .fallback(any(echo_handler));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("fake upstream serves");
    });

    FakeUpstream { addr }
}

async fn slow_handler() -> impl IntoResponse {
    tokio::time::sleep(Duration::from_millis(100)).await;
    "slow"
}

async fn status_handler(axum::extract::Path(code): axum::extract::Path<u16>) -> impl IntoResponse {
    (
        StatusCode::from_u16(code).expect("test status code"),
        format!("status:{code}"),
    )
}

async fn echo_handler(request: Request<Body>) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.expect("body bytes");
    let body_text = String::from_utf8_lossy(&body).to_string();
    let response = json!({
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query().unwrap_or(""),
        "authorization": header_value(&parts.headers, header::AUTHORIZATION.as_str()),
        "cookie": header_value(&parts.headers, header::COOKIE.as_str()),
        "request_id": header_value(&parts.headers, REQUEST_ID_HEADER),
        "content_type": header_value(&parts.headers, header::CONTENT_TYPE.as_str()),
        "body": body_text,
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, HeaderValue::from_static("upstream=ok"));

    (headers, response.to_string())
}

fn header_value(headers: &HeaderMap, name: impl AsRef<str>) -> String {
    headers
        .get(name.as_ref())
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

async fn read_string(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}
