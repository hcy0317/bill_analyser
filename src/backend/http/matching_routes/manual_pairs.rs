async fn matching_pairs_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_pairs_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_matching_pairs_payload(runtime.pool(), user_id).await {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_manual_pair_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "create_manual_pair_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let request = match parse_manual_pair_request(&payload) {
        Ok(value) => value,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    let runtime = match open_postgres_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match create_postgres_manual_matching_pair(
        runtime.pool(),
        user_id,
        request.bill_id,
        request.candidate_bill_id,
        &request.pair_type,
    )
    .await
    {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_manual_pair_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(pair_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "delete_manual_pair_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match delete_postgres_manual_matching_pair(runtime.pool(), user_id, pair_id).await {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn accept_matching_candidate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "accept_matching_candidate_handler",
        "business operation entered"
    );
    matching_candidate_action_response(&state, &headers, candidate_id, "accept", payload)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reject_matching_candidate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reject_matching_candidate_handler",
        "business operation entered"
    );
    matching_candidate_action_response(&state, &headers, candidate_id, "reject", payload)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_matching_candidate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(candidate_id): Path<String>,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "clear_matching_candidate_handler",
        "business operation entered"
    );
    matching_candidate_action_response(&state, &headers, candidate_id, "clear", payload)
}
