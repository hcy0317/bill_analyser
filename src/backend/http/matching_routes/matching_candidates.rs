async fn matching_session_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_session_candidates_handler",
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
    return match query_postgres_matching_session_candidates_payload(
        runtime.pool(),
        user_id,
        &session_id,
    )
    .await
    {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Import session not found"),
        Err(error) => matching_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_bill_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_bill_candidates_handler",
        "business operation entered"
    );
    matching_bill_candidates_response(&state, &headers, bill_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<MatchingCandidatesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_candidates_handler",
        "business operation entered"
    );
    let session_id = query
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let raw_bill_id = query
        .bill_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if session_id.is_some() == raw_bill_id.is_some() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Exactly one of sessionId or billId is required",
        );
    }
    if let Some(session_id) = session_id {
        return matching_session_candidates_response(&state, &headers, session_id).await;
    }
    let bill_id = match raw_bill_id.and_then(|value| value.parse::<i64>().ok()) {
        Some(value) if value > 0 => value,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid billId"),
    };
    matching_bill_candidates_response(&state, &headers, bill_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn matching_bill_feedback_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "matching_bill_feedback_handler",
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
    return match query_postgres_matching_bill_feedback_payload(runtime.pool(), user_id, bill_id)
        .await
    {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Bill not found"),
        Err(error) => matching_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reconciliation_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<ReconciliationCandidatesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reconciliation_candidates_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let query_map = reconciliation_query_to_map(query);
    let parsed_query = match parse_reconciliation_candidates_query(&query_map) {
        Ok(value) => value,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let _filters = reconciliation_filters_from_value(&parsed_query);

    let runtime = match open_postgres_runtime(&state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_reconciliation_candidates_payload(runtime.pool(), user_id).await {
        Ok(data) => success_data(StatusCode::OK, data),
        Err(error) => matching_error_response(error),
    };
}
