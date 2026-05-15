async fn list_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsListQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = match filters_from_query(runtime.connection(), user_id, &query) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let page = query.page();
    let page_size = query.page_size();
    match query_bills(runtime.connection(), user_id, page, page_size, &filters).and_then(
        |bill_page| page_to_frontend(runtime.connection(), user_id, page, page_size, bill_page),
    ) {
        Ok(body) => json_response(StatusCode::OK, body),
        Err(_) => db_error_response(),
    }
}

async fn bills_by_month_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsByMonthQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let (start_date, end_date) = match bill_analyser_core::adapters::transaction::month_date_range(
        query.year,
        query.month,
    ) {
        Ok(value) => value,
        Err(error) => return bad_request(error.to_string()),
    };
    let mut filters = match filters_from_query(runtime.connection(), user_id, &query.common) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    filters.date_from = Some(start_date);
    filters.date_to = Some(end_date);
    match query_bills(runtime.connection(), user_id, 1, 100_000, &filters).and_then(|bill_page| {
        page_to_frontend(runtime.connection(), user_id, 1, 100_000, bill_page)
    }) {
        Ok(body) => json_response(StatusCode::OK, body),
        Err(_) => db_error_response(),
    }
}

async fn create_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let draft = match create_draft_from_payload(runtime.connection(), user_id, &payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bill_id = match create_bill(runtime.connection_mut(), user_id, &draft) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match get_frontend_bill(runtime.connection(), user_id, bill_id) {
        Ok(Some(value)) => success_result(StatusCode::CREATED, value),
        Ok(None) => db_error_response(),
        Err(_) => db_error_response(),
    }
}


async fn get_bill_query_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillIdQuery>,
) -> Response {
    let Some(bill_id) = query.id.as_deref().and_then(parse_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    get_bill_response(state, headers, bill_id).await
}

async fn get_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    get_bill_response(state, headers, bill_id).await
}

async fn get_bill_response(state: HttpAppState, headers: HeaderMap, bill_id: i64) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_frontend_bill(runtime.connection(), user_id, bill_id) {
        Ok(Some(value)) => success_result(StatusCode::OK, value),
        Ok(None) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

async fn update_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if get_bill_by_id(runtime.connection(), user_id, bill_id)
        .map_err(|_| ())
        .ok()
        .flatten()
        .is_none()
    {
        return not_found("Bill not found");
    }
    let draft = match update_draft_from_payload(runtime.connection(), user_id, &payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match update_bill(runtime.connection_mut(), user_id, bill_id, &draft) {
        Ok(true) => match get_frontend_bill(runtime.connection(), user_id, bill_id) {
            Ok(Some(value)) => success_result(StatusCode::OK, value),
            Ok(None) => not_found("Bill not found"),
            Err(_) => db_error_response(),
        },
        Ok(false) => not_found("Bill not found or update failed"),
        Err(_) => db_error_response(),
    }
}

async fn legacy_modify_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let Some(bill_id) = payload.get("id").and_then(value_to_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(old_snapshot) = (match get_bill_update_snapshot(runtime.connection(), user_id, bill_id)
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    }) else {
        return not_found("Bill not found");
    };
    let mut draft = match update_draft_from_payload(runtime.connection(), user_id, &payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    apply_legacy_modify_preserved_fields(&mut draft.fields, &payload, &old_snapshot);
    match update_bill(runtime.connection_mut(), user_id, bill_id, &draft) {
        Ok(true) => json_response(StatusCode::OK, legacy_modify_bill_success_payload(bill_id)),
        Ok(false) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn delete_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    delete_bill_response(state, headers, bill_id, delete_bill_success_payload()).await
}

async fn legacy_delete_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let Some(bill_id) = payload.get("id").and_then(value_to_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    delete_bill_response(
        state,
        headers,
        bill_id,
        legacy_delete_bill_success_payload(),
    )
    .await
}

async fn delete_bill_response(
    state: HttpAppState,
    headers: HeaderMap,
    bill_id: i64,
    success_body: Value,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match delete_bill(runtime.connection_mut(), user_id, bill_id) {
        Ok(true) => json_response(StatusCode::OK, success_body),
        Ok(false) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

#[cfg(test)]
mod crud_handler_tests {
    use super::*;

    #[tokio::test]
    async fn crud_list_handler_rejects_missing_auth_before_db_open() {
        let config = HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            std::time::Duration::from_secs(1),
            1024,
            crate::config::ImportRouteMode::ImportDbRuntime,
        )
        .expect("config builds");
        let state = HttpAppState::new(config).expect("state builds");
        let response =
            list_bills_handler(State(state), HeaderMap::new(), Query(BillsListQuery::default()))
                .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
