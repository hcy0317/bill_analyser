// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

async fn list_tags_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());

    match repository.list_tags(db_user_id(user_id)) {
        Ok(tags) => success_result(StatusCode::OK, format_tag_list_response(tags)),
        Err(_) => tag_db_error_response(),
    }
}

async fn get_tag_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(tag_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());

    match repository.get_tag(tag_id, db_user_id(user_id)) {
        Ok(Some(tag)) => {
            success_result(StatusCode::OK, Value::Object(backend_tag_to_frontend(tag)))
        }
        Ok(None) => not_found("Tag not found"),
        Err(_) => tag_db_error_response(),
    }
}

async fn create_tag_handler(
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
    if !tag_name_is_present(&body) {
        return bad_request("name is required");
    }
    let payload = match frontend_tag_to_backend(&body) {
        Ok(value) => Value::Object(value),
        Err(message) => return bad_request(message),
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let tag_id = match repository.create_tag(&payload, user_id) {
        Ok(value) => value,
        Err(_) => return tag_db_error_response(),
    };
    match repository.get_tag(tag_id, user_id) {
        Ok(Some(tag)) => success_result(
            StatusCode::CREATED,
            Value::Object(backend_tag_to_frontend(tag)),
        ),
        Ok(None) => success_result(StatusCode::CREATED, json!({ "id": tag_id.to_string() })),
        Err(_) => tag_db_error_response(),
    }
}

async fn update_tag_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(tag_id): Path<i64>,
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
    let payload = match frontend_tag_to_backend(&body) {
        Ok(value) => Value::Object(value),
        Err(message) => return bad_request(message),
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.update_tag(tag_id, &payload, user_id) {
        Ok(true) => match repository.get_tag(tag_id, user_id) {
            Ok(Some(tag)) => {
                success_result(StatusCode::OK, Value::Object(backend_tag_to_frontend(tag)))
            }
            Ok(None) => success_result(StatusCode::OK, json!({ "id": tag_id.to_string() })),
            Err(_) => tag_db_error_response(),
        },
        Ok(false) => not_found("Tag not found"),
        Err(_) => tag_db_error_response(),
    }
}

async fn delete_tag_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(tag_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());

    match repository.delete_tag(tag_id, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Tag not found"),
        Err(_) => tag_db_error_response(),
    }
}

async fn update_tag_display_orders_handler(
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
    let Some(new_display_orders) = body.get("newDisplayOrders") else {
        return bad_request("newDisplayOrders is required");
    };
    let Some(items) = new_display_orders.as_array() else {
        return bad_request("newDisplayOrders must be an array");
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(object) = item.as_object() else {
            return bad_request("Each item must have id and displayOrder");
        };
        if !object.contains_key("id") || !object.contains_key("displayOrder") {
            return bad_request("Each item must have id and displayOrder");
        }
        let tag_id = match object.get("id").and_then(parse_python_int) {
            Some(value) => value,
            None => return bad_request(invalid_tag_order_value_error(object.get("id"))),
        };
        let display_order = match object.get("displayOrder").and_then(parse_python_int) {
            Some(value) => value,
            None => return bad_request(invalid_tag_order_value_error(object.get("displayOrder"))),
        };
        orders.push(TagDisplayOrder {
            tag_id,
            display_order,
        });
    }

    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    match repository.update_display_orders(&orders, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => tag_db_error_response(),
        Err(_) => tag_db_error_response(),
    }
}

async fn batch_create_tags_handler(
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
    let Some(tags) = body
        .get("tags")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
    else {
        return bad_request("tags is required and must be a non-empty array");
    };
    let skip_exists = body.get("skipExists").is_some_and(value_truthy);

    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);
    let mut existing_by_name = match repository.list_tags(user_id) {
        Ok(existing_tags) => existing_tags
            .into_iter()
            .map(|tag| {
                let key = tag.name.trim().to_lowercase();
                let value = Value::Object(backend_tag_to_frontend(tag));
                (key, value)
            })
            .collect::<BTreeMap<_, _>>(),
        Err(_) => return tag_db_error_response(),
    };
    let mut created_tags = Vec::new();

    for item in tags {
        let Some(normalized_name) = tag_batch_normalized_name(item) else {
            return bad_request("Each tag item must contain a non-empty name");
        };
        if let Some(existing_tag) = existing_by_name.get(&normalized_name) {
            if skip_exists {
                created_tags.push(existing_tag.clone());
                continue;
            }
            return error_response(
                StatusCode::CONFLICT,
                format!("Tag already exists: {}", tag_batch_display_name(item)),
            );
        }

        let payload = match frontend_tag_to_backend(item) {
            Ok(value) => Value::Object(value),
            Err(_) => return bad_request("Each tag item must contain a non-empty name"),
        };
        let tag_id = match repository.create_tag(&payload, user_id) {
            Ok(value) => value,
            Err(_) => return tag_db_error_response(),
        };
        match repository.get_tag(tag_id, user_id) {
            Ok(Some(tag)) => {
                let tag_value = Value::Object(backend_tag_to_frontend(tag));
                existing_by_name.insert(normalized_name, tag_value.clone());
                created_tags.push(tag_value);
            }
            Ok(None) => {}
            Err(_) => return tag_db_error_response(),
        }
    }

    success_result(StatusCode::CREATED, Value::Array(created_tags))
}

async fn list_templates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, None, None);
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.list_templates(db_user_id(user_id), template_type) {
        Ok(templates) => success_result(StatusCode::OK, format_template_list_response(templates)),
        Err(_) => template_db_error_response(),
    }
}

async fn get_template_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    Path(template_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, None, None);
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.get_template_by_id(template_id, db_user_id(user_id), template_type) {
        Ok(Some(template)) => success_result(StatusCode::OK, Value::Object(template)),
        Ok(None) => not_found("Template not found"),
        Err(_) => template_db_error_response(),
    }
}

async fn create_template_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
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
    let template_type = template_type_from_query_body(&query, Some(&body), Some(1));
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let template_id = match repository.create_template(&body, user_id) {
        Ok(value) => value,
        Err(_) => return template_db_error_response(),
    };
    match repository.get_template_by_id(template_id, user_id, template_type) {
        Ok(Some(template)) => success_result(StatusCode::CREATED, Value::Object(template)),
        Ok(None) => success_result(
            StatusCode::CREATED,
            json!({ "id": template_id.to_string() }),
        ),
        Err(_) => template_db_error_response(),
    }
}

async fn update_template_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    Path(template_id): Path<i64>,
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
    let template_type = template_type_from_query_body(&query, Some(&body), Some(1));
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.update_template(template_id, &body, user_id, template_type) {
        Ok(true) => match repository.get_template_by_id(template_id, user_id, template_type) {
            Ok(Some(template)) => success_result(StatusCode::OK, Value::Object(template)),
            Ok(None) => success_result(StatusCode::OK, Value::Null),
            Err(_) => template_db_error_response(),
        },
        Ok(false) => not_found("Template not found"),
        Err(_) => template_db_error_response(),
    }
}

async fn delete_template_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    Path(template_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, None, Some(1));
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.delete_template(template_id, db_user_id(user_id), template_type) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Template not found"),
        Err(_) => template_db_error_response(),
    }
}

async fn update_template_display_orders_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
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
    let orders = match template_display_orders_from_body(&body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, Some(&body), Some(1)).unwrap_or(1);
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.update_display_orders(&orders, template_type, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => template_db_error_response(),
        Err(_) => template_db_error_response(),
    }
}

