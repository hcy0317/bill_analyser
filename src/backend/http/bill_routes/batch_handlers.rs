// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn batch_create_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "batch_create_bills_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "bills") {
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
            match create_draft_from_payload_postgres(runtime.pool(), user_id, &item_value).await {
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
        let bill_ids =
            match batch_create_postgres_bills(runtime.pool(), user_id.get() as i64, &drafts).await {
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
            match get_postgres_frontend_bill(runtime.pool(), user_id, bill_id).await {
                Ok(Some(value)) => {
                    ids.push(bill_id.to_string());
                    items.push(value);
                }
                Ok(None) | Err(_) => return db_error_response(),
            }
        }
        return route_contract_response(batch_create_success_route_response(items, ids));
    }
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

#[tracing::instrument(level = "debug", skip_all)]
async fn batch_update_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "batch_update_bills_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
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
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match batch_update_postgres_bills(
            runtime.pool(),
            user_id.get() as i64,
            &bill_ids,
            &fields,
        )
        .await
        {
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
        };
    }
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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

#[tracing::instrument(level = "debug", skip_all)]
async fn batch_delete_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "batch_delete_bills_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(bill_ids) = extract_bill_ids(&payload).filter(|ids| !ids.is_empty()) else {
        return bad_request("No bill IDs provided");
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match batch_delete_postgres_bills(
            runtime.pool(),
            user_id.get() as i64,
            &bill_ids,
        )
        .await
        {
            Ok(deleted_count) => {
                json_response(StatusCode::OK, batch_delete_success_payload(deleted_count))
            }
            Err(_) => db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match batch_delete_bills(runtime.connection_mut(), user_id, &bill_ids) {
        Ok(deleted_count) => {
            json_response(StatusCode::OK, batch_delete_success_payload(deleted_count))
        }
        Err(_) => db_error_response(),
    }
}
