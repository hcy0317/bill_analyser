// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_parse_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_parse_runtime_handler", "business operation entered");
    let content_type = content_type_from_headers(&headers);
    let content_type_lower = content_type.to_ascii_lowercase();
    if content_type_lower.contains("multipart/form-data") {
        return import_parse_multipart_runtime_response(&state, &headers, &content_type, &body)
            .await;
    }
    if content_type_lower.contains("application/json") || content_type.is_empty() {
        let payload = match serde_json::from_slice::<Value>(&body) {
            Ok(payload) => payload,
            Err(_) => return route_response(import_v2_error_response(400, "Invalid JSON request")),
        };
        return import_parse_json_runtime_response(&state, &headers, &payload, false);
    }
    route_response(import_v2_error_response(
        415,
        "Unsupported import parse content type",
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_parse_generic_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_parse_generic_runtime_handler", "business operation entered");
    import_parse_json_runtime_response(&state, &headers, &payload, true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_import_preview_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_import_preview_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let request = match import_preview_request_from_body(&headers, &body) {
        Ok(request) => request,
        Err(response) => return route_response(response),
    };
    let preview = if let Some(temp_path) = request.temp_path {
        let temp_path = match validate_import_temp_path(&temp_path, user_id, None) {
            Ok(path) => path,
            Err(response) => return route_response(response),
        };
        match preview_rows_from_temp_path(&temp_path, request.delimiter.as_deref()) {
            Ok(preview) => preview,
            Err(response) => return route_response(response),
        }
    } else if let Some(uploaded_file) = request.uploaded_file {
        match preview_rows_from_file_bytes(&uploaded_file, request.delimiter.as_deref()) {
            Ok(preview) => preview,
            Err(response) => return route_response(response),
        }
    } else {
        return route_response(import_v2_error_response(400, "Missing temp_path or file"));
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": preview,
        }),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_import_parsers_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_import_parsers_runtime_handler", "business operation entered");
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return route_response(response);
    }
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": legacy_import_parsers(),
        }),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_parse_import_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_parse_import_runtime_handler", "business operation entered");
    let content_type = content_type_from_headers(&headers);
    if !content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        return route_response(import_v2_error_response(
            415,
            "Unsupported parse_import content type",
        ));
    }
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let form = match parse_multipart_form_data(&content_type, &body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let Some(file_part) = form.file_parts().first().copied() else {
        return route_response(import_v2_error_response(400, "No file provided"));
    };
    let filename = file_part
        .filename
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("import-file.csv");
    let request = parse_legacy_import_file(&form, filename, &file_part.body, user_id);
    let parsed = match request {
        Ok(parsed) => parsed,
        Err(response) => return route_response(response),
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": {
                "items": parsed.items,
                "totalCount": parsed.total_count,
                "parserType": parsed.parser_type,
                "detectedParserType": parsed.detected_parser_type,
            }
        }),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_import_upload_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_import_upload_runtime_handler", "business operation entered");
    let content_type = content_type_from_headers(&headers);
    if !content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        return route_response(import_v2_error_response(
            415,
            "Unsupported import upload content type",
        ));
    }
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let form = match parse_multipart_form_data(&content_type, &body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let Some(file_part) = form.file_parts().first().copied() else {
        return route_response(import_v2_error_response(400, "No file provided"));
    };
    let filename = file_part
        .filename
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("import-file.csv");
    let parsed = match parse_legacy_import_file(&form, filename, &file_part.body, user_id) {
        Ok(parsed) => parsed,
        Err(response) => return route_response(response),
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": {
                "preview": parsed.items,
                "total": parsed.total_count,
                "valid": parsed.total_count,
                "invalid": 0,
                "inserted": 0,
                "duplicates": 0,
                "dedup_stats": Value::Null,
                "parser_type": parsed.parser_type,
                "errors": [],
            }
        }),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_import_reclassify_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_import_reclassify_runtime_handler", "business operation entered");
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return route_response(response);
    }
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(transactions) = object.get("transactions").and_then(Value::as_array) else {
        return route_response(import_v2_error_response(400, "缺少transactions字段"));
    };
    let result = transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| legacy_reclassify_passthrough(index, transaction))
        .collect::<Vec<_>>();
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": result}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_configs_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<ImportConfigQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_configs_list_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    if let Some(file_format) = query
        .file_format()
        .map(|value| normalize_config_text(value))
    {
        configs.retain(|config| {
            config_text(config, &["fileFormat", "file_format"])
                .map(|value| normalize_config_text(&value) == file_format)
                .unwrap_or(false)
        });
    }
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    configs.truncate(limit);
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": configs}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_configs_save_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_configs_save_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    if config_text_from_object(object, &["name"]).is_none() {
        return route_response(import_v2_error_response(400, "name is required"));
    }
    if config_text_from_object(object, &["fileFormat", "file_format"]).is_none() {
        return route_response(import_v2_error_response(400, "fileFormat is required"));
    }
    let Some(field_mappings) = first_value(object, &["fieldMappings", "field_mappings"]) else {
        return route_response(import_v2_error_response(400, "fieldMappings is required"));
    };
    if !field_mappings
        .as_object()
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return route_response(import_v2_error_response(400, "fieldMappings is required"));
    }

    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let requested_id = first_value(object, &["id"])
        .and_then(value_to_i64)
        .filter(|id| *id > 0);
    let config_id = requested_id.unwrap_or_else(|| {
        configs
            .iter()
            .filter_map(config_id_value)
            .max()
            .unwrap_or(0)
            + 1
    });
    let saved = build_import_config_from_payload(object, config_id);
    if bool_value(&saved, "isDefault").unwrap_or(false) {
        for config in &mut configs {
            if let Some(object) = config.as_object_mut() {
                object.insert("isDefault".to_string(), json!(false));
            }
        }
    }
    if let Some(existing) = configs
        .iter_mut()
        .find(|config| config_id_value(config) == Some(config_id))
    {
        *existing = saved;
    } else {
        configs.push(saved);
    }
    configs.sort_by_key(|config| config_id_value(config).unwrap_or(i64::MAX));
    if let Err(response) = store_import_configs(runtime.connection(), user_id, &configs) {
        return route_response(response);
    }
    route_response(ImportV2RouteResponse {
        status_code: 201,
        body: json!({"success": true, "result": {"id": config_id}}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_configs_match_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_configs_match_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(file_format) = config_text_from_object(object, &["fileFormat", "file_format"]) else {
        return route_response(import_v2_error_response(400, "fileFormat is required"));
    };
    let Some(headers) = string_array_field_from_object(object, &["headers"]) else {
        return route_response(import_v2_error_response(400, "headers is required"));
    };
    if headers.is_empty() {
        return route_response(import_v2_error_response(400, "headers is required"));
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let result = best_import_config_match(&configs, &file_format, &headers);
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": result}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_configs_suggest_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_configs_suggest_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(file_format) = config_text_from_object(object, &["fileFormat", "file_format"]) else {
        return route_response(import_v2_error_response(400, "fileFormat is required"));
    };
    let Some(headers) = string_array_field_from_object(object, &["headers"]) else {
        return route_response(import_v2_error_response(400, "headers is required"));
    };
    if headers.is_empty() {
        return route_response(import_v2_error_response(400, "headers is required"));
    }
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let matched = best_import_config_match(&configs, &file_format, &headers);
    let suggestion = build_import_config_suggestion(&file_format, &headers, matched.as_ref());
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": suggestion}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_configs_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_configs_delete_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut configs = match load_import_configs(runtime.connection(), user_id) {
        Ok(configs) => configs,
        Err(response) => return route_response(response),
    };
    let before = configs.len();
    configs.retain(|config| config_id_value(config) != Some(config_id));
    if before == configs.len() {
        return route_response(import_v2_error_response(404, "Config not found"));
    }
    if let Err(response) = store_import_configs(runtime.connection(), user_id, &configs) {
        return route_response(response);
    }
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "result": true}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_import_confirm_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_import_confirm_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }

    if let Some(session_id) =
        first_value(object, &["session_id", "sessionId"]).and_then(value_to_text)
    {
        match get_import_session(runtime.connection(), &session_id, user_id) {
            Ok(Some(_)) => {}
            Ok(None) => return route_response(import_session_not_found_response()),
            Err(error) => return route_response(db_error_response(error)),
        }
        let result = match confirm_preview_to_bills(runtime.connection_mut(), &session_id, user_id)
        {
            Ok(result) => result,
            Err(error) => return route_response(db_error_response(error)),
        };
        return route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({
                "success": true,
                "result": {
                    "total": result.confirmed_count + result.skipped_count + result.duplicate_count,
                    "inserted": result.confirmed_count,
                    "duplicates": result.duplicate_count,
                    "skipped": result.skipped_count,
                    "errors": result.errors,
                }
            }),
        });
    }

    let Some(bills) = first_value(object, &["bills", "transactions"]).and_then(Value::as_array)
    else {
        return route_response(import_v2_error_response(400, "bills is required"));
    };
    if bills.is_empty() {
        return route_response(import_v2_error_response(400, "bills is required"));
    }
    if let Err(response) = init_legacy_bills_runtime_schema(&runtime) {
        return route_response(response);
    }
    let result = match insert_legacy_confirmed_bills(runtime.connection_mut(), user_id, bills) {
        Ok(result) => result,
        Err(response) => return route_response(response),
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": result["inserted"].as_i64().unwrap_or_default() >= 0, "result": result}),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn legacy_import_batch_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "legacy_import_batch_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(file_path) = first_text_from_object(object, &["file_path", "filePath"]) else {
        return route_response(import_v2_error_response(400, "file_path is required"));
    };
    let path = match validate_import_temp_path(&file_path, user_id, None) {
        Ok(path) => path,
        Err(response) => return route_response(response),
    };
    let text = match read_import_temp_text(&path) {
        Ok(text) => text,
        Err(response) => return route_response(response),
    };
    let parsed = parse_standard_bills_from_csv_text(&text, "auto", false);
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": {
                "success": true,
                "total": parsed.bills.len(),
                "inserted": 0,
                "duplicates": 0,
                "preview": parsed
                    .bills
                    .iter()
                    .map(|bill| legacy_import_item_from_standard_bill(bill, "auto"))
                    .collect::<Vec<_>>(),
                "runtime": "rust-import-db-runtime-partial",
            }
        }),
    })
}

