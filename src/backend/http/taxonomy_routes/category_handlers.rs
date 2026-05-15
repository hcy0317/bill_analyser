async fn list_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(StatusCode::OK, format_category_tree_response(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn flat_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(StatusCode::OK, format_category_flat_response(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn all_categories_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(StatusCode::OK, categories_to_value(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn update_all_categories_handler(
    headers: HeaderMap,
    State(state): State<HttpAppState>,
    body: Bytes,
) -> Response {
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

async fn create_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    if parent_id == "0" {
        if let Ok(Some(existing)) = repository.get_category_by_name(&name, "", user_id) {
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
        let category_id = match repository.create_category(&payload, user_id) {
            Ok(Some(value)) => value,
            Ok(None) => return category_db_error_response(),
            Err(_) => return category_db_error_response(),
        };
        return match repository.get_category_by_id(category_id, user_id) {
            Ok(Some(category)) => success_result(
                StatusCode::CREATED,
                Value::Object(backend_category_to_frontend(&category, "0")),
            ),
            Ok(None) => category_db_error_response(),
            Err(_) => category_db_error_response(),
        };
    }

    let (main_category, parent_type) =
        match resolve_parent_category(&mut repository, &parent_id, user_id) {
            Ok(Some(value)) => value,
            Ok(None) => return not_found("Parent category not found"),
            Err(_) => return category_db_error_response(),
        };
    if let Ok(Some(existing)) = repository.get_category_by_name(&main_category, &name, user_id) {
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
    let category_id = match repository.create_category(&payload, user_id) {
        Ok(Some(value)) => value,
        Ok(None) => return category_db_error_response(),
        Err(_) => return category_db_error_response(),
    };
    match repository.get_category_by_id(category_id, user_id) {
        Ok(Some(category)) => success_result(
            StatusCode::OK,
            Value::Object(backend_category_to_frontend(&category, &parent_id)),
        ),
        Ok(None) => category_db_error_response(),
        Err(_) => category_db_error_response(),
    }
}

async fn get_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Response {
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
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.get_category_by_id(category_id, user_id) {
        Ok(Some(category)) => {
            let parent_id = category_parent_id_for_get(&mut repository, &category, user_id);
            success_result(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&category, &parent_id)),
            )
        }
        Ok(None) => not_found("Category not found"),
        Err(_) => category_db_error_response(),
    }
}

async fn update_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
    body: Bytes,
) -> Response {
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
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    if let Some(old_name) = virtual_category_name(&category_id) {
        return update_virtual_category_handler(&mut repository, user_id, &old_name, &body);
    }

    let category_id = match category_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid category ID"),
    };
    update_real_category_handler(&mut repository, user_id, category_id, &body)
}

async fn delete_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let deleted = if let Some(main_category) = virtual_category_name(&category_id) {
        repository.delete_categories_by_main_category(&main_category, user_id)
    } else {
        match category_id.parse::<i64>() {
            Ok(category_id) => repository.delete_category(category_id, user_id),
            Err(_) => return bad_request("Invalid category ID"),
        }
    };

    match deleted {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Category not found or delete failed"),
        Err(_) => category_db_error_response(),
    }
}

async fn move_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    for item in new_display_orders {
        let Some(category_id) = item.get("id").and_then(value_as_i64) else {
            continue;
        };
        let Some(display_order) = item.get("displayOrder").and_then(value_as_i64) else {
            continue;
        };
        if repository
            .update_category(category_id, &json!({ "priority": display_order }), user_id)
            .is_err()
        {
            return category_db_error_response();
        }
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

async fn batch_create_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    for category in categories {
        let Some(main_name) = category_name_from_body(category).filter(|value| !value.is_empty())
        else {
            continue;
        };
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(3);
        if repository
            .get_category_by_name(&main_name, "", user_id)
            .map(|existing| existing.is_none())
            .unwrap_or(false)
        {
            let payload = Value::Object(frontend_category_to_backend(
                category,
                &main_name,
                "",
                category_type,
                CategoryPayloadMode::FrontendDefaults,
            ));
            if repository.create_category(&payload, user_id).is_err() {
                return category_db_error_response();
            }
        }

        let Some(sub_categories) = category.get("subCategories").and_then(Value::as_array) else {
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
            if repository
                .get_category_by_name(&main_name, &sub_name, user_id)
                .map(|existing| existing.is_none())
                .unwrap_or(false)
            {
                let payload = Value::Object(frontend_category_to_backend(
                    sub_category,
                    &main_name,
                    &sub_name,
                    sub_type,
                    CategoryPayloadMode::FrontendDefaults,
                ));
                if repository.create_category(&payload, user_id).is_err() {
                    return category_db_error_response();
                }
            }
        }
    }

    match repository.list_categories(user_id) {
        Ok(categories) => success_result(StatusCode::OK, format_category_tree_response(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn export_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
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
    }
}

async fn category_statistics_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    let _ignored_legacy_filters = (query.period.as_deref(), query.category_type.as_deref());
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.category_statistics(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        db_user_id(user_id),
    ) {
        Ok(statistics) => success_result(
            StatusCode::OK,
            format_category_statistics_response(statistics),
        ),
        Err(_) => category_db_error_response(),
    }
}

async fn recategorize_all_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match recategorize_bills_with_category_rules(runtime.connection_mut(), user_id, force) {
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

