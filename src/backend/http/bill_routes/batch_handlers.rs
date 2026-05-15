async fn batch_create_bills_handler(
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
    let item_objects = match batch_create_transaction_items(&payload) {
        Ok(value) => value,
        Err(error) => {
            return route_contract_response(batch_create_prepare_error_route_response(
                error.to_string(),
                0,
            ))
        }
    };
    let mut drafts = Vec::with_capacity(item_objects.len());
    for (index, item) in item_objects.iter().enumerate() {
        let item_value = Value::Object((*item).clone());
        match create_draft_from_payload(runtime.connection(), user_id, &item_value) {
            Ok(draft) => drafts.push(draft),
            Err(response) => {
                let error = response_error_text(*response)
                    .unwrap_or_else(|| "Invalid bill payload".to_string());
                return route_contract_response(batch_create_prepare_error_route_response(
                    error, index,
                ));
            }
        }
    }
    let bill_ids = match batch_create_bills(runtime.connection_mut(), user_id, &drafts) {
        Ok(value) => value,
        Err(error) => {
            return route_contract_response(batch_create_persist_error_route_response(
                error.to_string(),
                0,
                Vec::new(),
                Vec::new(),
            ))
        }
    };
    let mut items = Vec::with_capacity(bill_ids.len());
    let mut ids = Vec::with_capacity(bill_ids.len());
    for bill_id in bill_ids {
        match get_frontend_bill(runtime.connection(), user_id, bill_id) {
            Ok(Some(value)) => {
                ids.push(bill_id.to_string());
                items.push(value);
            }
            Ok(None) | Err(_) => return db_error_response(),
        }
    }
    route_contract_response(batch_create_success_route_response(items, ids))
}

async fn batch_update_bills_handler(
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
    let Some(bill_ids) = extract_bill_ids(&payload).filter(|ids| !ids.is_empty()) else {
        return bad_request("No bill IDs provided");
    };
    let mut fields = match payload_object(&payload) {
        Ok(value) => value.clone(),
        Err(response) => return *response,
    };
    let _ = fields.remove("bill_ids");
    let _ = fields.remove("billIds");
    let _ = fields.remove("ids");
    if let Some(Value::Object(updates)) = fields.remove("updates") {
        fields = updates;
    }
    sanitize_backend_update_fields(&mut fields, &payload);
    if fields.is_empty() {
        return bad_request("No fields to update");
    }
    match batch_update_bills(runtime.connection_mut(), user_id, &bill_ids, &fields) {
        Ok(result) => success_result(
            StatusCode::OK,
            serde_json::to_value(batch_update_response(
                result.success_count,
                result.failed_count,
                result.failed_ids,
            ))
            .expect("batch update response serializes"),
        ),
        Err(error) => bad_request(error.to_string()),
    }
}

async fn batch_delete_bills_handler(
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
    let Some(bill_ids) = extract_bill_ids(&payload).filter(|ids| !ids.is_empty()) else {
        return bad_request("No bill IDs provided");
    };
    match batch_delete_bills(runtime.connection_mut(), user_id, &bill_ids) {
        Ok(deleted_count) => {
            json_response(StatusCode::OK, batch_delete_success_payload(deleted_count))
        }
        Err(_) => db_error_response(),
    }
}
