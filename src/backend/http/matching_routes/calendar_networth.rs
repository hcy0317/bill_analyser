#[tracing::instrument(level = "debug", skip_all)]
async fn calendar_events_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "calendar_events_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let start_date = match required_date(query.start_date.as_deref(), "start_date") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let end_date = match required_date(query.end_date.as_deref(), "end_date") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "matching calendar") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_calendar_events_payload(
        runtime.pool(),
        user_id,
        start_date,
        end_date,
    )
    .await
    {
        Ok(data) => success_data(StatusCode::OK, json!(data)),
        Err(error) => db_error_response(error),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn networth_snapshot_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "networth_snapshot_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "networth") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_net_worth_payload(runtime.pool(), user_id).await {
        Ok(snapshot) => success_data(StatusCode::OK, snapshot),
        Err(error) => db_error_response(error),
    };
}
