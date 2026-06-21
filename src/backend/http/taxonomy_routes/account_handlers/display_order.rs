#[tracing::instrument(level = "debug", skip_all)]
async fn update_account_display_orders_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_account_display_orders_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(new_display_orders) = body.get("newDisplayOrders") else {
        return bad_request("Missing newDisplayOrders parameter");
    };
    let Some(items) = new_display_orders.as_array() else {
        return bad_request("newDisplayOrders must be a list");
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(account_id) = item.get("id").and_then(value_as_i64) else {
            return bad_request("Each item must have id and displayOrder");
        };
        let Some(display_order) = item.get("displayOrder").and_then(value_as_i64) else {
            return bad_request("Each item must have id and displayOrder");
        };
        orders.push(AccountDisplayOrder {
            account_id,
            display_order,
        });
    }


        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match update_postgres_account_display_orders(
            runtime.pool(),
            &orders,
            db_user_id(user_id),
        )
        .await
        {
            Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
            Ok(false) => db_error_response(),
            Err(_) => db_error_response(),
        };
}
