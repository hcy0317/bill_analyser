#[tracing::instrument(level = "debug", skip_all)]
async fn sync_account_balances_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "sync_account_balances_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match sync_all_postgres_account_balances(runtime.pool(), user_id).await {
        Ok(result) => success_result(
            StatusCode::OK,
            format_sync_account_balances_response(result),
        ),
        Err(_) => db_error_response(),
    }
}
