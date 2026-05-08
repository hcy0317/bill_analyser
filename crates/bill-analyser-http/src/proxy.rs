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
use bill_analyser_core::{
    endpoints_by_owner, ApiResponse, ErrorCode, MigrationState, RuntimeError,
};
use bytes::Bytes;
use serde::Serialize;

use crate::config::{HttpShellConfig, ImportRouteMode};

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

pub async fn ownership_aware_proxy_handler(
    State(state): State<ProxyState>,
    request: Request<Body>,
) -> Response<Body> {
    let method = request.method().as_str();
    let path = request.uri().path();
    if proxy_allowed_for_request(&state, method, path) {
        return proxy_request(state, request).await;
    }

    let mut response = ProxyErrorBody::not_manifest_python_proxied(method, path).into_response();
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

pub fn proxy_allowed_for_request(state: &ProxyState, method: &str, path: &str) -> bool {
    if state.config.import_route_mode == ImportRouteMode::ProxyOnly {
        return true;
    }
    is_manifest_python_proxied_route(method, path)
}

pub fn is_manifest_python_proxied_route(method: &str, path: &str) -> bool {
    endpoints_by_owner(MigrationState::PythonProxied)
        .iter()
        .any(|endpoint| {
            method_matches(endpoint.method, method) && route_pattern_matches(endpoint.pattern, path)
        })
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

    pub fn not_manifest_python_proxied(method: &str, path: &str) -> Self {
        Self::gateway(
            "route_not_manifest_python_proxied",
            format!(
                "{method} {path} is not declared as PythonProxied in the Rust migration manifest"
            ),
        )
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

fn method_matches(manifest_method: &str, request_method: &str) -> bool {
    manifest_method.eq_ignore_ascii_case(request_method)
        || request_method.eq_ignore_ascii_case("OPTIONS")
        || (manifest_method.eq_ignore_ascii_case("GET")
            && request_method.eq_ignore_ascii_case("HEAD"))
}

fn route_pattern_matches(pattern: &str, path: &str) -> bool {
    if !pattern.starts_with("/api/") {
        return false;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path == prefix || path.starts_with(&format!("{}/", prefix.trim_end_matches('/')));
    }

    let pattern_segments = pattern.trim_matches('/').split('/').collect::<Vec<_>>();
    let path_segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    route_segments_match(&pattern_segments, &path_segments)
}

fn route_segments_match(pattern_segments: &[&str], path_segments: &[&str]) -> bool {
    match pattern_segments.split_first() {
        None => path_segments.is_empty(),
        Some((pattern_segment, remaining_pattern)) => {
            if is_rest_placeholder(pattern_segment) {
                if remaining_pattern.is_empty() {
                    return !path_segments.is_empty();
                }
                return (1..=path_segments.len())
                    .any(|count| route_segments_match(remaining_pattern, &path_segments[count..]));
            }

            path_segments
                .split_first()
                .is_some_and(|(path_segment, remaining_path)| {
                    route_segment_matches(pattern_segment, path_segment)
                        && route_segments_match(remaining_pattern, remaining_path)
                })
        }
    }
}

fn route_segment_matches(pattern_segment: &str, path_segment: &str) -> bool {
    if pattern_segment == path_segment {
        return true;
    }
    if let Some(parameter_name) = single_segment_placeholder_name(pattern_segment) {
        return !path_segment.is_empty()
            && (!is_numeric_route_parameter(parameter_name)
                || path_segment.bytes().all(|byte| byte.is_ascii_digit()));
    }
    embedded_placeholder_bounds(pattern_segment).is_some_and(|(prefix, suffix)| {
        path_segment.starts_with(prefix)
            && path_segment.ends_with(suffix)
            && path_segment.len() > prefix.len() + suffix.len()
    })
}

fn is_rest_placeholder(pattern_segment: &str) -> bool {
    pattern_segment.starts_with("{*") && pattern_segment.ends_with('}')
}

fn single_segment_placeholder_name(pattern_segment: &str) -> Option<&str> {
    if pattern_segment.starts_with('{')
        && pattern_segment.ends_with('}')
        && !is_rest_placeholder(pattern_segment)
    {
        return Some(&pattern_segment[1..pattern_segment.len() - 1]);
    }
    None
}

fn is_numeric_route_parameter(parameter_name: &str) -> bool {
    matches!(
        parameter_name,
        "account_id"
            | "bill_id"
            | "budget_id"
            | "candidate_id"
            | "config_id"
            | "pair_id"
            | "preview_id"
            | "rule_id"
            | "suggestion_id"
            | "tag_id"
            | "template_id"
    )
}

fn embedded_placeholder_bounds(pattern_segment: &str) -> Option<(&str, &str)> {
    let start = pattern_segment.find('{')?;
    let end = pattern_segment[start..].find('}')? + start;
    if end == pattern_segment.len() - 1 && start == 0 {
        return None;
    }
    Some((&pattern_segment[..start], &pattern_segment[end + 1..]))
}
