async fn recurring_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Query(query): Query<RecurringCandidatesQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let tolerance_days = query.tolerance_days.unwrap_or(3).clamp(0, 31);
    match get_bill_recurring_candidates(runtime.connection(), user_id, bill_id, tolerance_days) {
        Ok(Some(result)) => success_result(
            StatusCode::OK,
            json!({
                "billId": bill_id,
                "linkedRecurringId": result.linked_recurring_id,
                "linkedRecurringName": result.linked_recurring_name,
                "candidates": result.candidates,
            }),
        ),
        Ok(None) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

async fn bind_recurring_match_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    let Some(recurring_id) = payload.get("recurringId").and_then(value_to_positive_i64) else {
        return bad_request("Missing recurringId");
    };
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match bind_bill_to_recurring(runtime.connection_mut(), user_id, bill_id, recurring_id) {
        Ok(Some(result)) => success_result(
            StatusCode::OK,
            json!({
                "billId": result.bill_id,
                "recurringId": result.recurring_id,
                "nextScheduledDate": result.next_scheduled_date,
            }),
        ),
        Ok(None) => not_found("Bill or recurring template not found"),
        Err(_) => db_error_response(),
    }
}

async fn unbind_recurring_match_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match unbind_bill_from_recurring(runtime.connection_mut(), user_id, bill_id) {
        Ok(Some(true)) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(Some(false)) | Ok(None) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}
