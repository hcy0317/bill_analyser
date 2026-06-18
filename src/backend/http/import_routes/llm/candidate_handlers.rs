// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_candidates_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<LlmCandidatesQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_candidates_list_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let candidates = match list_postgres_llm_candidates(
        runtime.pool(),
        user_id_value,
        query.status.as_deref(),
        query.r#type.as_deref(),
        limit,
        offset,
    )
    .await
    {
        Ok(candidates) => candidates,
        Err(error) => return route_response(db_error_response(error)),
    };
    match count_postgres_llm_candidates(
        runtime.pool(),
        user_id_value,
        query.status.as_deref(),
        query.r#type.as_deref(),
    )
    .await
    {
        Ok(total) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: build_llm_candidate_list_response(candidates, total),
        }),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_candidate_get_runtime_handler(
    State(state): State<HttpAppState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_candidate_get_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match get_postgres_llm_candidate_by_id(
        runtime.pool(),
        candidate_id,
        user_id_value,
    )
    .await
    {
        Ok(Some(candidate)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "data": candidate}),
        }),
        Ok(None) => route_response(llm_not_found_response(&format!(
            "Candidate {candidate_id} not found"
        ))),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_candidate_accept_runtime_handler(
    State(state): State<HttpAppState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_candidate_accept_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match accept_postgres_llm_candidate(runtime.pool(), candidate_id, user_id_value).await {
        Ok(Some(result)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "data": result}),
        }),
        Ok(None) => route_response(llm_not_found_response(&format!(
            "Candidate {candidate_id} not found"
        ))),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_candidate_reject_runtime_handler(
    State(state): State<HttpAppState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_candidate_reject_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match reject_postgres_llm_candidate(runtime.pool(), candidate_id, user_id_value).await {
        Ok(Some(rejected)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: build_llm_candidate_reject_response(rejected),
        }),
        Ok(None) => route_response(llm_not_found_response(&format!(
            "Candidate {candidate_id} not found"
        ))),
        Err(error) => route_response(db_error_response(error)),
    }
}
