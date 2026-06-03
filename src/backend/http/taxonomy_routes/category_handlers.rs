// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn list_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "list_categories_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_categories(runtime.pool(), db_user_id(user_id)).await {
            Ok(categories) => success_result(StatusCode::OK, format_category_tree_response(categories)),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn flat_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "flat_categories_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_categories(runtime.pool(), db_user_id(user_id)).await {
            Ok(categories) => success_result(StatusCode::OK, format_category_flat_response(categories)),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn all_categories_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "all_categories_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_categories(runtime.pool(), db_user_id(user_id)).await {
            Ok(categories) => success_result(StatusCode::OK, categories_to_value(categories)),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_all_categories_handler(
    headers: HeaderMap,
    State(state): State<HttpAppState>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_all_categories_handler", "business operation entered");
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return *response;
    }
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if body
        .as_object()
        .and_then(|object| object.get("categories"))
        .is_none()
    {
        return bad_request("categories are required");
    }

    json_response(
        StatusCode::OK,
        json!({ "success": true, "message": "Categories updated successfully" }),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "create_category_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(name) = category_name_from_body(&body).filter(|value| !value.is_empty()) else {
        return bad_request("Category name is required");
    };
    let parent_id = value_string(body.get("parentId"), "0");

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        if parent_id == "0" {
            if let Ok(Some(existing)) =
                get_postgres_category_by_name(runtime.pool(), &name, "", user_id).await
            {
                return success_result_with_message(
                    StatusCode::OK,
                    Value::Object(backend_category_to_frontend(&existing, "0")),
                    "Category already exists",
                );
            }

            let category_type = body.get("type").and_then(value_as_i64).unwrap_or(1);
            let payload = Value::Object(frontend_category_to_backend(
                &body,
                &name,
                "",
                category_type,
                CategoryPayloadMode::FrontendDefaults,
            ));
            let category_id = match create_postgres_category(runtime.pool(), &payload, user_id).await
            {
                Ok(Some(value)) => value,
                Ok(None) => return category_db_error_response(),
                Err(_) => return category_db_error_response(),
            };
            return match get_postgres_category_by_id(runtime.pool(), category_id, user_id).await {
                Ok(Some(category)) => success_result(
                    StatusCode::CREATED,
                    Value::Object(backend_category_to_frontend(&category, "0")),
                ),
                Ok(None) => category_db_error_response(),
                Err(_) => category_db_error_response(),
            };
        }

        let (main_category, parent_type) =
            match resolve_postgres_parent_category(runtime.pool(), &parent_id, user_id).await {
                Ok(Some(value)) => value,
                Ok(None) => return not_found("Parent category not found"),
                Err(_) => return category_db_error_response(),
            };
        if let Ok(Some(existing)) =
            get_postgres_category_by_name(runtime.pool(), &main_category, &name, user_id).await
        {
            return success_result_with_message(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&existing, &parent_id)),
                "Category already exists",
            );
        }

        let payload = Value::Object(frontend_category_to_backend(
            &body,
            &main_category,
            &name,
            parent_type,
            CategoryPayloadMode::FrontendDefaults,
        ));
        let category_id = match create_postgres_category(runtime.pool(), &payload, user_id).await {
            Ok(Some(value)) => value,
            Ok(None) => return category_db_error_response(),
            Err(_) => return category_db_error_response(),
        };
        return match get_postgres_category_by_id(runtime.pool(), category_id, user_id).await {
            Ok(Some(category)) => success_result(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&category, &parent_id)),
            ),
            Ok(None) => category_db_error_response(),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "get_category_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if let Some(main_category_name) = virtual_category_name(&category_id) {
        return success_result(
            StatusCode::OK,
            json!({
                "id": category_id,
                "name": main_category_name,
                "parentId": "0",
                "type": 1,
                "icon": "",
                "color": "",
                "comment": "",
                "displayOrder": 0,
                "visible": true,
                "keywords": ""
            }),
        );
    }
    let category_id = match category_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid category ID"),
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        return match get_postgres_category_by_id(runtime.pool(), category_id, user_id).await {
            Ok(Some(category)) => {
                let parent_id =
                    category_parent_id_for_postgres_get(runtime.pool(), &category, user_id).await;
                success_result(
                    StatusCode::OK,
                    Value::Object(backend_category_to_frontend(&category, &parent_id)),
                )
            }
            Ok(None) => not_found("Category not found"),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_category_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !body.is_object() {
        return bad_request("Invalid request");
    }

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);

        if let Some(old_name) = virtual_category_name(&category_id) {
            return update_postgres_virtual_category_handler(
                runtime.pool(),
                user_id,
                &old_name,
                &body,
            )
            .await;
        }

        let category_id = match category_id.parse::<i64>() {
            Ok(value) => value,
            Err(_) => return bad_request("Invalid category ID"),
        };
        return update_postgres_real_category_handler(
            runtime.pool(),
            user_id,
            category_id,
            &body,
        )
        .await;
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "delete_category_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let deleted = if let Some(main_category) = virtual_category_name(&category_id) {
            delete_postgres_categories_by_main_category(runtime.pool(), &main_category, user_id)
                .await
        } else {
            match category_id.parse::<i64>() {
                Ok(category_id) => {
                    delete_postgres_category(runtime.pool(), category_id, user_id).await
                }
                Err(_) => return bad_request("Invalid category ID"),
            }
        };

        return match deleted {
            Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
            Ok(false) => not_found("Category not found or delete failed"),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn move_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "move_categories_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(new_display_orders) = body.get("newDisplayOrders").and_then(Value::as_array) else {
        return success_result(StatusCode::OK, Value::Bool(true));
    };
    if new_display_orders.is_empty() {
        return success_result(StatusCode::OK, Value::Bool(true));
    }


        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        for item in new_display_orders {
            let Some(category_id) = item.get("id").and_then(value_as_i64) else {
                continue;
            };
            let Some(display_order) = item.get("displayOrder").and_then(value_as_i64) else {
                continue;
            };
            if update_postgres_category_display_order(
                runtime.pool(),
                category_id,
                display_order,
                user_id,
            )
            .await
            .is_err()
            {
                return category_db_error_response();
            }
        }

        return success_result(StatusCode::OK, Value::Bool(true));
}

#[tracing::instrument(level = "debug", skip_all)]
async fn batch_create_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "batch_create_categories_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(categories) = body
        .as_object()
        .and_then(|object| object.get("categories"))
        .and_then(Value::as_array)
    else {
        return bad_request("No categories provided");
    };


        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);

        for category in categories {
            let Some(main_name) =
                category_name_from_body(category).filter(|value| !value.is_empty())
            else {
                continue;
            };
            let category_type = category.get("type").and_then(value_as_i64).unwrap_or(3);
            match get_postgres_category_by_name(runtime.pool(), &main_name, "", user_id).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let payload = Value::Object(frontend_category_to_backend(
                        category,
                        &main_name,
                        "",
                        category_type,
                        CategoryPayloadMode::FrontendDefaults,
                    ));
                    if create_postgres_category(runtime.pool(), &payload, user_id)
                        .await
                        .is_err()
                    {
                        return category_db_error_response();
                    }
                }
                Err(_) => return category_db_error_response(),
            }

            let Some(sub_categories) = category.get("subCategories").and_then(Value::as_array)
            else {
                continue;
            };
            for sub_category in sub_categories {
                let Some(sub_name) =
                    category_name_from_body(sub_category).filter(|value| !value.is_empty())
                else {
                    continue;
                };
                let sub_type = sub_category
                    .get("type")
                    .and_then(value_as_i64)
                    .unwrap_or(category_type);
                match get_postgres_category_by_name(runtime.pool(), &main_name, &sub_name, user_id)
                    .await
                {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        let payload = Value::Object(frontend_category_to_backend(
                            sub_category,
                            &main_name,
                            &sub_name,
                            sub_type,
                            CategoryPayloadMode::FrontendDefaults,
                        ));
                        if create_postgres_category(runtime.pool(), &payload, user_id)
                            .await
                            .is_err()
                        {
                            return category_db_error_response();
                        }
                    }
                    Err(_) => return category_db_error_response(),
                }
            }
        }

        return match list_postgres_categories(runtime.pool(), user_id).await {
            Ok(categories) => {
                success_result(StatusCode::OK, format_category_tree_response(categories))
            }
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn export_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "export_categories_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_categories(runtime.pool(), db_user_id(user_id)).await {
            Ok(categories) => success_result(
                StatusCode::OK,
                Value::Array(
                    categories
                        .iter()
                        .map(category_export_record)
                        .map(Value::Object)
                        .collect(),
                ),
            ),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn category_statistics_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "category_statistics_handler", "business operation entered");
    let _ignored_current_filters = (query.period.as_deref(), query.category_type.as_deref());
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_category_statistics(
            runtime.pool(),
            query.start_date.as_deref(),
            query.end_date.as_deref(),
            db_user_id(user_id),
        )
        .await
        {
            Ok(statistics) => success_result(
                StatusCode::OK,
                format_category_statistics_response(statistics),
            ),
            Err(_) => category_db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn recategorize_all_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "recategorize_all_bills_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let force = body
        .as_object()
        .and_then(|object| object.get("force"))
        .is_some_and(value_truthy);

    let runtime = match open_postgres_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match recategorize_bills_with_category_rules_postgres(runtime.pool(), user_id, force).await {
        Ok(result) => success_result(
            StatusCode::OK,
            json!({
                "total": result.total,
                "updated": result.updated,
            }),
        ),
        Err(_) => category_db_error_response(),
    }
}
