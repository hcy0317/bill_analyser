async fn list_suggestions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<RecurringSuggestionsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "list_suggestions_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let limit = match parse_limit(query.limit.as_deref()) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let offset = match parse_offset(query.offset.as_deref()) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let total = match count_postgres_recurring_suggestions(runtime.pool(), user_id, status).await {
        Ok(value) => value,
        Err(error) => return db_error_response(error),
    };
    return match list_postgres_recurring_suggestions(runtime.pool(), user_id, status, limit, offset)
        .await
    {
        Ok(items) => success_data(
            StatusCode::OK,
            json!({
                "total": total,
                "items": serialize_recurring_suggestions(&items),
                "limit": limit,
                "offset": offset,
            }),
        ),
        Err(error) => db_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn detect_suggestions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "detect_suggestions_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bills =
        match list_postgres_recent_bills_for_recurring_detection(runtime.pool(), user_id, 10_000)
            .await
        {
            Ok(value) => value,
            Err(error) => return db_error_response(error),
        };
    let linked_ids = match get_postgres_bills_linked_to_recurring(runtime.pool(), user_id).await {
        Ok(value) => value,
        Err(error) => return db_error_response(error),
    };
    let patterns = detect_recurring_patterns(&bills, 3, &linked_ids);
    return match detect_and_save_postgres_recurring_suggestions(runtime.pool(), user_id, &patterns)
        .await
    {
        Ok(summary) => success_data(
            StatusCode::OK,
            json!({
                "detected": patterns.len(),
                "created": summary.created,
                "updated": summary.updated,
                "skipped": summary.skipped,
            }),
        ),
        Err(error) => db_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn accept_suggestion_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "accept_suggestion_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match accept_postgres_recurring_suggestion(runtime.pool(), user_id, suggestion_id).await
    {
        Ok(Some(value)) => success_data(StatusCode::OK, value),
        Ok(None) => not_found("Suggestion not found or already processed"),
        Err(error) => db_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reject_suggestion_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(suggestion_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reject_suggestion_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "recurring suggestions") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match reject_postgres_recurring_suggestion(runtime.pool(), user_id, suggestion_id).await
    {
        Ok(true) => success_data(StatusCode::OK, json!({ "status": "rejected" })),
        Ok(false) => not_found("Suggestion not found or already processed"),
        Err(error) => db_error_response(error),
    };
}
