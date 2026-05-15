pub async fn llm_candidates_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<LlmCandidatesQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let candidates = match list_llm_candidates(
        runtime.connection(),
        user_id_value,
        query.status.as_deref(),
        query.r#type.as_deref(),
        limit,
        offset,
    ) {
        Ok(candidates) => candidates,
        Err(error) => return route_response(db_error_response(error)),
    };
    match count_llm_candidates(
        runtime.connection(),
        user_id_value,
        query.status.as_deref(),
        query.r#type.as_deref(),
    ) {
        Ok(total) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: build_llm_candidate_list_response(candidates, total),
        }),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn llm_candidate_get_runtime_handler(
    State(state): State<HttpAppState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match bill_analyser_db::get_llm_candidate_by_id(
        runtime.connection(),
        candidate_id,
        user_id_value,
    ) {
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

pub async fn llm_candidate_accept_runtime_handler(
    State(state): State<HttpAppState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match accept_llm_candidate(runtime.connection(), candidate_id, user_id_value) {
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

pub async fn llm_candidate_reject_runtime_handler(
    State(state): State<HttpAppState>,
    Path(candidate_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    match reject_llm_candidate(runtime.connection(), candidate_id, user_id_value) {
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

