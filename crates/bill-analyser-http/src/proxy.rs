use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
};
use bill_analyser_core::{ApiResponse, ErrorCode, RuntimeError};
use bytes::Bytes;
use serde::Serialize;

use crate::config::HttpShellConfig;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

#[derive(Debug, Clone)]
pub struct ProxyState {
    pub config: HttpShellConfig,
    client: reqwest::Client,
    request_counter: Arc<AtomicU64>,
}

impl ProxyState {
    pub fn new(config: HttpShellConfig) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().timeout(config.timeout).build()?;
        Ok(Self {
            config,
            client,
            request_counter: Arc::new(AtomicU64::new(1)),
        })
    }

    fn next_request_id(&self) -> HeaderValue {
        let next_id = self.request_counter.fetch_add(1, Ordering::SeqCst);
        HeaderValue::from_str(&format!("rust-http-{next_id}"))
            .expect("generated request id is a valid header value")
    }
}

pub async fn proxy_request(state: ProxyState, request: Request<Body>) -> Response<Body> {
    match forward_request(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

pub async fn proxy_handler(
    State(state): State<ProxyState>,
    request: Request<Body>,
) -> Response<Body> {
    proxy_request(state, request).await
}

async fn forward_request(
    state: ProxyState,
    request: Request<Body>,
) -> Result<Response<Body>, ProxyErrorBody> {
    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let upstream_url = build_upstream_url(&state.config.python_upstream, path_and_query)?;
    let request_id = parts
        .headers
        .get(REQUEST_ID_HEADER)
        .cloned()
        .unwrap_or_else(|| state.next_request_id());
    let body_bytes = to_bytes(body, state.config.body_limit_bytes)
        .await
        .map_err(|error| ProxyErrorBody::gateway("proxy_body_read_failed", error.to_string()))?;

    let mut upstream_request = state.client.request(parts.method, upstream_url);
    upstream_request = upstream_request.headers(filter_proxy_request_headers(&parts.headers));
    upstream_request = upstream_request.header(REQUEST_ID_HEADER, request_id.clone());
    upstream_request = upstream_request.body(body_bytes);

    let upstream_response = upstream_request
        .send()
        .await
        .map_err(|error| proxy_send_error(error, state.config.timeout))?;
    let status = upstream_response.status();
    let response_headers = filter_proxy_response_headers(upstream_response.headers());
    let response_body = upstream_response
        .bytes()
        .await
        .map_err(|error| ProxyErrorBody::gateway("proxy_body_read_failed", error.to_string()))?;

    let mut response = Response::builder().status(status);
    for (name, value) in &response_headers {
        response = response.header(name, value);
    }
    if !response
        .headers_ref()
        .is_some_and(|headers| headers.contains_key(REQUEST_ID_HEADER))
    {
        response = response.header(REQUEST_ID_HEADER, request_id);
    }

    response
        .body(Body::from(response_body))
        .map_err(|error| ProxyErrorBody::gateway("proxy_response_build_failed", error.to_string()))
}

pub fn build_upstream_url(
    python_upstream: &str,
    path_and_query: &str,
) -> Result<String, ProxyErrorBody> {
    if !(python_upstream.starts_with("http://") || python_upstream.starts_with("https://")) {
        return Err(ProxyErrorBody::gateway(
            "proxy_invalid_upstream",
            "Python upstream must start with http:// or https://",
        ));
    }
    let suffix = if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    };
    Ok(format!(
        "{}{}",
        python_upstream.trim_end_matches('/'),
        suffix
    ))
}

pub fn filter_proxy_request_headers(headers: &HeaderMap) -> HeaderMap {
    filter_headers(headers, true)
}

pub fn filter_proxy_response_headers(headers: &HeaderMap) -> HeaderMap {
    filter_headers(headers, false)
}

fn filter_headers(headers: &HeaderMap, is_request: bool) -> HeaderMap {
    let mut filtered = HeaderMap::new();
    for (name, value) in headers {
        if should_forward_header(name, is_request) {
            filtered.append(name.clone(), value.clone());
        }
    }
    filtered
}

fn should_forward_header(name: &HeaderName, is_request: bool) -> bool {
    if is_hop_by_hop_header(name) {
        return false;
    }
    if is_request && *name == header::HOST {
        return false;
    }
    if is_request && name.as_str().eq_ignore_ascii_case(REQUEST_ID_HEADER) {
        return false;
    }
    if *name == header::CONTENT_LENGTH {
        return false;
    }
    true
}

pub fn is_hop_by_hop_header(name: &HeaderName) -> bool {
    HOP_BY_HOP_HEADERS
        .iter()
        .any(|blocked| name.as_str().eq_ignore_ascii_case(blocked))
}

fn proxy_send_error(error: reqwest::Error, timeout: Duration) -> ProxyErrorBody {
    if error.is_timeout() {
        ProxyErrorBody::gateway(
            "proxy_upstream_timeout",
            format!(
                "Python upstream did not respond within {} ms",
                timeout.as_millis()
            ),
        )
    } else {
        ProxyErrorBody::gateway("proxy_upstream_unavailable", error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProxyErrorBody {
    pub success: bool,
    pub error: ProxyErrorDetail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProxyErrorDetail {
    pub code: String,
    pub message: String,
}

impl ProxyErrorBody {
    pub fn gateway(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: ProxyErrorDetail {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for ProxyErrorBody {
    fn into_response(self) -> Response<Body> {
        let runtime_error = RuntimeError::new(ErrorCode::InternalError, self.error.message.clone());
        let _core_envelope: ApiResponse<()> = ApiResponse::failure(runtime_error);
        let body = serde_json::to_vec(&self).unwrap_or_else(|_| {
            br#"{"success":false,"error":{"code":"proxy_serialization_failed","message":"proxy error serialization failed"}}"#
                .to_vec()
        });

        let mut response = Response::new(Body::from(Bytes::from(body)));
        *response.status_mut() = StatusCode::BAD_GATEWAY;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    }
}
