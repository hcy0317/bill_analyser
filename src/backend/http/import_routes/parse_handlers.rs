// 中文导读：import v2 解析入口，支持 multipart 与 JSON 映射解析。
// 维护重点：只暴露 /api/bills/import/v2/* 当前接口。
// 不变式：解析阶段只创建当前 session/staging 响应，不读历史 non-Postgres 数据。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_parse_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_parse_runtime_handler", "business operation entered");
    let content_type = content_type_from_headers(&headers);
    let content_type_lower = content_type.to_ascii_lowercase();
    if content_type_lower.contains("multipart/form-data") {
        return import_parse_multipart_runtime_response(&state, &headers, &content_type, &body)
            .await;
    }
    if content_type_lower.contains("application/json") || content_type.is_empty() {
        let payload = match serde_json::from_slice::<Value>(&body) {
            Ok(payload) => payload,
            Err(_) => return route_response(import_v2_error_response(400, "Invalid JSON request")),
        };
        return import_parse_json_runtime_response(&state, &headers, &payload, false);
    }
    route_response(import_v2_error_response(
        415,
        "Unsupported import parse content type",
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_parse_generic_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_parse_generic_runtime_handler", "business operation entered");
    import_parse_json_runtime_response(&state, &headers, &payload, true)
}
