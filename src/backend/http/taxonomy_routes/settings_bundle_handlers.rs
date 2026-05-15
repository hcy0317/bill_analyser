async fn export_settings_bundle_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy settings bundle export") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match build_settings_bundle(runtime.connection_mut(), db_user_id(user_id)) {
        Ok(bundle) => settings_bundle_download_response(bundle, "bill-analyser-settings.json"),
        Err(_) => settings_bundle_db_error_response(),
    }
}

async fn export_settings_bundle_section_get_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(section_key): Path<String>,
) -> Response {
    if !is_valid_settings_bundle_section(&section_key) {
        return settings_bundle_section_not_found(&section_key);
    }
    if is_sensitive_settings_export_section(&section_key) {
        return bad_request("password is required");
    }
    export_settings_bundle_section(&state, &headers, &section_key)
}

async fn export_settings_bundle_section_post_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(section_key): Path<String>,
    body: Bytes,
) -> Response {
    if !is_valid_settings_bundle_section(&section_key) {
        return settings_bundle_section_not_found(&section_key);
    }

    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy settings bundle export") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = db_user_id(user_id);

    if is_sensitive_settings_export_section(&section_key) {
        let payload = optional_json_body(body);
        let password = payload
            .as_ref()
            .and_then(|value| value.get("password"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if password.is_empty() {
            return bad_request("password is required");
        }
        match verify_sensitive_export_password(runtime.connection_mut(), user_id, password) {
            Ok(true) => {}
            Ok(false) => {
                return json_response(
                    StatusCode::UNAUTHORIZED,
                    json!({"success": false, "error": "Invalid password"}),
                )
            }
            Err(_) => return settings_bundle_db_error_response(),
        }
    }

    match build_settings_bundle(runtime.connection_mut(), user_id) {
        Ok(bundle) => settings_bundle_download_response(
            filter_settings_bundle_section(&bundle, &section_key),
            &format!("bill-analyser-settings-{section_key}.json"),
        ),
        Err(_) => settings_bundle_db_error_response(),
    }
}

fn export_settings_bundle_section(
    state: &HttpAppState,
    headers: &HeaderMap,
    section_key: &str,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(state, "taxonomy settings bundle export") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match build_settings_bundle(runtime.connection_mut(), db_user_id(user_id)) {
        Ok(bundle) => settings_bundle_download_response(
            filter_settings_bundle_section(&bundle, section_key),
            &format!("bill-analyser-settings-{section_key}.json"),
        ),
        Err(_) => settings_bundle_db_error_response(),
    }
}

async fn preview_import_settings_bundle_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    import_settings_bundle_response(
        &state,
        &headers,
        body,
        true,
        "taxonomy settings bundle import",
        "Failed to preview settings bundle",
    )
}

async fn import_settings_bundle_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    import_settings_bundle_response(
        &state,
        &headers,
        body,
        false,
        "taxonomy settings bundle import",
        "Failed to import settings bundle",
    )
}

async fn preview_import_settings_bundle_section_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(section_key): Path<String>,
    body: Bytes,
) -> Response {
    if !is_valid_settings_bundle_section(&section_key) {
        return settings_bundle_section_not_found(&section_key);
    }
    import_settings_bundle_section_response(
        &state,
        &headers,
        &section_key,
        body,
        true,
        "Failed to preview settings section",
    )
}

async fn import_settings_bundle_section_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(section_key): Path<String>,
    body: Bytes,
) -> Response {
    if !is_valid_settings_bundle_section(&section_key) {
        return settings_bundle_section_not_found(&section_key);
    }
    import_settings_bundle_section_response(
        &state,
        &headers,
        &section_key,
        body,
        false,
        "Failed to import settings section",
    )
}

fn import_settings_bundle_section_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    section_key: &str,
    body: Bytes,
    dry_run: bool,
    internal_error_message: &'static str,
) -> Response {
    let body = match parse_settings_bundle_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let section_bundle = settings_bundle_section_from_request(&body, section_key);
    import_settings_bundle_value_response(
        state,
        headers,
        &section_bundle,
        dry_run,
        "taxonomy settings bundle import",
        internal_error_message,
    )
}

fn import_settings_bundle_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    body: Bytes,
    dry_run: bool,
    runtime_label: &'static str,
    internal_error_message: &'static str,
) -> Response {
    let body = match parse_settings_bundle_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    import_settings_bundle_value_response(
        state,
        headers,
        &body,
        dry_run,
        runtime_label,
        internal_error_message,
    )
}

fn import_settings_bundle_value_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    bundle: &Value,
    dry_run: bool,
    runtime_label: &'static str,
    internal_error_message: &'static str,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(state, runtime_label) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match import_settings_bundle(
        runtime.connection_mut(),
        bundle,
        db_user_id(user_id),
        dry_run,
    ) {
        Ok(result) => success_result(StatusCode::OK, result),
        Err(bill_analyser_db::DbError::InvalidOperation(message)) => bad_request(message),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, internal_error_message),
    }
}

fn parse_settings_bundle_body(body: Bytes) -> RouteResult<Value> {
    let value = if body.is_empty() {
        return Err(Box::new(bad_request("Invalid JSON bundle")));
    } else {
        serde_json::from_slice::<Value>(&body)
            .map_err(|_| Box::new(bad_request("Invalid JSON bundle")))?
    };
    if !value.is_object() {
        return Err(Box::new(bad_request("Invalid JSON bundle")));
    }
    Ok(value)
}

fn settings_bundle_section_from_request(data: &Value, section_key: &str) -> Value {
    let Some(object) = data.as_object() else {
        return json!({
            "schemaVersion": Value::Null,
            "sections": { section_key: Value::Array(Vec::new()) },
        });
    };
    let sections = object
        .get("sections")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| object.clone());
    let section_items = sections
        .get(section_key)
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    json!({
        "schemaVersion": object.get("schemaVersion").cloned().unwrap_or(Value::Null),
        "sections": { section_key: section_items },
    })
}

async fn import_categories_handler(
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
    let categories = if let Some(object) = body.as_object() {
        object
            .get("categories")
            .or_else(|| object.get("result"))
            .unwrap_or(&body)
    } else {
        &body
    };
    let Some(categories) = categories.as_array() else {
        return bad_request("Invalid format, expected list of categories");
    };

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);
    let mut imported = 0;
    let mut updated = 0;
    let mut skipped = 0;

    for category in categories {
        let main_category = string_or_default(category.get("main_category"), "");
        if main_category.trim().is_empty() {
            skipped += 1;
            continue;
        }
        let sub_category = string_or_default(category.get("sub_category"), "");
        let payload = Value::Object(import_category_payload(
            category,
            &main_category,
            &sub_category,
        ));
        match repository.get_category_by_name(&main_category, &sub_category, user_id) {
            Ok(Some(existing)) => {
                if let Some(category_id) = existing.get("id").and_then(value_as_i64) {
                    if repository
                        .update_category(category_id, &payload, user_id)
                        .is_err()
                    {
                        return category_db_error_response();
                    }
                }
                updated += 1;
            }
            Ok(None) => {
                if repository.create_category(&payload, user_id).is_err() {
                    return category_db_error_response();
                }
                imported += 1;
            }
            Err(_) => return category_db_error_response(),
        }
    }

    success_result(
        StatusCode::OK,
        json!({ "imported": imported, "updated": updated, "skipped": skipped }),
    )
}

