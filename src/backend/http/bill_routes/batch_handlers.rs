// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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
    if let Err(response) = validate_bill_money_payload(&payload) {
        return *response;
    }

        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let item_objects = match batch_create_transaction_items(&payload) {
            Ok(value) => value,
            Err(error) => {
                return batch_create_prepare_error_response(error.to_string(), 0)
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
                    return batch_create_prepare_error_response(error, index);
                }
            }
        }
        let bill_ids =
            match batch_create_postgres_bills(runtime.pool(), user_id.get() as i64, &drafts).await {
                Ok(value) => value,
                Err(error) => {
                    return batch_create_persist_error_response(
                        error.to_string(),
                        0,
                        Vec::new(),
                        Vec::new(),
                    )
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
        return batch_create_success_response_projection(items, ids);
}

fn batch_create_success_response_projection(items: Vec<Value>, ids: Vec<String>) -> Response {
    let result = serde_json::to_value(batch_create_success_response(items, ids))
        .expect("batch create result should serialize");
    success_result(StatusCode::CREATED, result)
}

fn batch_create_prepare_error_response(
    error: impl ToString,
    failed_index: usize,
) -> Response {
    error_result(
        StatusCode::BAD_REQUEST,
        error,
        json!({
            "failedIndex": failed_index,
            "createdCount": 0,
            "items": [],
        }),
    )
}

fn batch_create_persist_error_response(
    error: impl ToString,
    failed_index: usize,
    created_items: Vec<Value>,
    created_ids: Vec<String>,
) -> Response {
    let result = serde_json::to_value(batch_create_failure_response(
        failed_index,
        created_items,
        created_ids,
    ))
    .expect("batch create failure result should serialize");
    error_result(StatusCode::INTERNAL_SERVER_ERROR, error, result)
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
    if let Err(response) = validate_bill_money_payload(&payload) {
        return *response;
    }
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
                success_result(StatusCode::OK, json!({"deleted_count": deleted_count}))
            }
            Err(_) => db_error_response(),
        };
}

#[cfg(test)]
mod batch_response_tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_value(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("batch response bytes");
        (
            status,
            serde_json::from_slice(&body).expect("batch response JSON"),
        )
    }

    #[tokio::test]
    async fn batch_create_response_projection_preserves_status_and_compatibility_fields() {
        let (status, success) = response_value(batch_create_success_response_projection(
            vec![json!({"id": "1"}), json!({"id": "2"})],
            vec!["1".to_string(), "2".to_string()],
        ))
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(success["success"], true);
        assert_eq!(success["result"]["createdCount"], 2);
        assert_eq!(success["result"]["ids"], json!(["1", "2"]));

        let (status, prepare_error) =
            response_value(batch_create_prepare_error_response("bad input", 3)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(prepare_error["success"], false);
        assert_eq!(prepare_error["error"], "bad input");
        assert_eq!(prepare_error["result"]["failedIndex"], 3);
        assert_eq!(prepare_error["result"]["createdCount"], 0);
        assert_eq!(prepare_error["result"]["items"], json!([]));
        assert!(prepare_error["result"].get("ids").is_none());

        let (status, persist_error) = response_value(batch_create_persist_error_response(
            "db failed",
            2,
            vec![json!({"id": "1"})],
            vec!["1".to_string()],
        ))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(persist_error["success"], false);
        assert_eq!(persist_error["error"], "db failed");
        assert_eq!(persist_error["result"]["failedIndex"], 2);
        assert_eq!(persist_error["result"]["createdCount"], 1);
        assert_eq!(persist_error["result"]["ids"], json!(["1"]));
    }
}
