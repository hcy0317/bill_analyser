// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn list_category_rules_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryRulesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "list_category_rules_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_category_rules(
            runtime.pool(),
            db_user_id(user_id),
            query.category_id,
            category_rules_enabled_only(&query),
        )
        .await
        {
            Ok(rules) => json_response(StatusCode::OK, format_category_rules_response(rules)),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.list_rules(
        db_user_id(user_id),
        query.category_id,
        category_rules_enabled_only(&query),
    ) {
        Ok(rules) => json_response(StatusCode::OK, format_category_rules_response(rules)),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_category_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "create_category_rule_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(object) = body.as_object() else {
        return bad_request("No data provided");
    };
    if !object.contains_key("category_id")
        || object.get("rule_expression").is_none_or(Value::is_null)
    {
        return bad_request("category_id and rule_expression are required");
    }

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let rule_id = match create_postgres_category_rule(runtime.pool(), &body, user_id).await {
            Ok(Some(value)) => value,
            Ok(None) => return bad_request("Failed to create rule"),
            Err(_) => return category_rule_db_error_response(),
        };
        return match get_postgres_category_rule(runtime.pool(), rule_id, user_id).await {
            Ok(Some(rule)) => category_rule_data_response(StatusCode::CREATED, rule),
            Ok(None) => category_rule_db_error_response(),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let rule_id = match repository.create_rule(&body, user_id) {
        Ok(Some(value)) => value,
        Ok(None) => return bad_request("Failed to create rule"),
        Err(_) => return category_rule_db_error_response(),
    };
    match repository.get_rule(rule_id, user_id) {
        Ok(Some(rule)) => category_rule_data_response(StatusCode::CREATED, rule),
        Ok(None) => category_rule_db_error_response(),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_legacy_category_rules_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "get_legacy_category_rules_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy legacy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let config_key = legacy_category_rules_setting_key(user_id);
        match get_postgres_legacy_category_rules_setting(runtime.pool(), &config_key, user_id)
            .await
        {
            Ok(Some(rules)) => return success_result(StatusCode::OK, rules),
            Ok(None) => {}
            Err(_) => return category_rule_db_error_response(),
        }
        return match list_postgres_legacy_category_engine_rules(runtime.pool(), user_id).await {
            Ok(rules) => success_result(StatusCode::OK, Value::Array(rules)),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy legacy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = db_user_id(user_id);
    let config_key = legacy_category_rules_setting_key(user_id);

    match get_app_setting(runtime.connection_mut(), &config_key) {
        Ok(Some(stored)) => {
            if let Ok(rules) = serde_json::from_str::<Value>(&stored) {
                return success_result(StatusCode::OK, rules);
            }
        }
        Ok(None) => {}
        Err(_) => return category_rule_db_error_response(),
    }

    match list_legacy_category_engine_rules(runtime.connection_mut(), user_id) {
        Ok(rules) => success_result(StatusCode::OK, Value::Array(rules)),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_legacy_category_rules_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_legacy_category_rules_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(rules) = body.as_object().and_then(|object| object.get("rules")) else {
        return bad_request("rules are required");
    };
    let stored_value = match serde_json::to_string(rules) {
        Ok(value) => value,
        Err(_) => return category_rule_db_error_response(),
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy legacy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let config_key = legacy_category_rules_setting_key(user_id);
        return match set_postgres_legacy_category_rules_setting(
            runtime.pool(),
            &config_key,
            rules,
            user_id,
        )
        .await
        {
            Ok(_) => json_response(
                StatusCode::OK,
                json!({"success": true, "message": "Category rules updated successfully"}),
            ),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy legacy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let draft = AppSettingDraft {
        key: legacy_category_rules_setting_key(db_user_id(user_id)),
        value: stored_value,
        value_type: "json".to_string(),
        description: Some("Legacy category rules config cache".to_string()),
        is_encrypted: false,
    };

    match set_app_setting(runtime.connection_mut(), &draft) {
        Ok(_) => json_response(
            StatusCode::OK,
            json!({"success": true, "message": "Category rules updated successfully"}),
        ),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_category_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_category_rule_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if body
        .as_object()
        .and_then(|object| object.get("rule_expression"))
        .is_some_and(Value::is_null)
    {
        return bad_request("rule_expression is required");
    }
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        return match update_postgres_category_rule(runtime.pool(), rule_id, &body, user_id).await {
            Ok(true) => match get_postgres_category_rule(runtime.pool(), rule_id, user_id).await {
                Ok(Some(rule)) => category_rule_data_response(StatusCode::OK, rule),
                Ok(None) => category_rule_db_error_response(),
                Err(_) => category_rule_db_error_response(),
            },
            Ok(false) => not_found("Rule not found or no change"),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.update_rule(rule_id, &body, user_id) {
        Ok(true) => match repository.get_rule(rule_id, user_id) {
            Ok(Some(rule)) => category_rule_data_response(StatusCode::OK, rule),
            Ok(None) => category_rule_db_error_response(),
            Err(_) => category_rule_db_error_response(),
        },
        Ok(false) => not_found("Rule not found or no change"),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_category_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "delete_category_rule_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match delete_postgres_category_rule(runtime.pool(), rule_id, db_user_id(user_id))
            .await
        {
            Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
            Ok(false) => not_found("Rule not found"),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.delete_rule(rule_id, db_user_id(user_id)) {
        Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
        Ok(false) => not_found("Rule not found"),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reorder_category_rules_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "reorder_category_rules_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(rule_ids) = body.get("rule_ids") else {
        return bad_request("rule_ids is required");
    };
    let Some(rule_ids) = rule_ids.as_array() else {
        return bad_request("rule_ids must be a list");
    };
    let mut parsed_rule_ids = Vec::with_capacity(rule_ids.len());
    for value in rule_ids {
        let Some(rule_id) = parse_python_int(value) else {
            return bad_request("rule_ids must be a list");
        };
        parsed_rule_ids.push(rule_id);
    }

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match reorder_postgres_category_rules(
            runtime.pool(),
            &parsed_rule_ids,
            db_user_id(user_id),
        )
        .await
        {
            Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
            Ok(false) => category_rule_db_error_response(),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());
    match repository.reorder_rules(&parsed_rule_ids, db_user_id(user_id)) {
        Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
        Ok(false) => category_rule_db_error_response(),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn ensure_category_rule_defaults_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "ensure_category_rule_defaults_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match ensure_postgres_category_rule_defaults(runtime.pool(), db_user_id(user_id))
            .await
        {
            Ok(summary) => json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "categories": {
                            "created": summary.categories_created,
                            "skipped": summary.categories_skipped,
                        },
                        "rules": {
                            "created": summary.rules_created,
                            "skipped": summary.rules_skipped,
                            "missingCategories": summary.rules_missing_categories,
                        }
                    }
                }),
            ),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.ensure_default_seed(db_user_id(user_id)) {
        Ok(summary) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "data": {
                    "categories": {
                        "created": summary.categories_created,
                        "skipped": summary.categories_skipped,
                    },
                    "rules": {
                        "created": summary.rules_created,
                        "skipped": summary.rules_skipped,
                        "missingCategories": summary.rules_missing_categories,
                    }
                }
            }),
        ),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn migrate_category_keywords_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "migrate_category_keywords_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match migrate_postgres_category_keywords_to_rules(
            runtime.pool(),
            db_user_id(user_id),
        )
        .await
        {
            Ok(summary) => json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "migrated": summary.migrated,
                        "skipped": summary.skipped,
                    }
                }),
            ),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.migrate_keywords_to_rules(db_user_id(user_id)) {
        Ok(summary) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "data": {
                    "migrated": summary.migrated,
                    "skipped": summary.skipped,
                }
            }),
        ),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn test_category_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "test_category_rule_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "text is required") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(text) = body.get("text").and_then(Value::as_str) else {
        return bad_request("text is required");
    };

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy category rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match get_postgres_category_rule(runtime.pool(), rule_id, db_user_id(user_id)).await {
            Ok(Some(rule)) => {
                let rule_expression = string_or_default(rule.get("rule_expression"), "");
                let regex_enabled = rule.get("regex_enabled").is_some_and(value_truthy);
                json_response(
                    StatusCode::OK,
                    json!({
                        "success": true,
                        "data": {
                            "matched": match_rule_expression(text, &rule_expression, regex_enabled)
                        }
                    }),
                )
            }
            Ok(None) => not_found("Rule not found"),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.get_rule(rule_id, db_user_id(user_id)) {
        Ok(Some(rule)) => {
            let rule_expression = string_or_default(rule.get("rule_expression"), "");
            let regex_enabled = rule.get("regex_enabled").is_some_and(value_truthy);
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "matched": match_rule_expression(text, &rule_expression, regex_enabled)
                    }
                }),
            )
        }
        Ok(None) => not_found("Rule not found"),
        Err(_) => category_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn rules_overview_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "rules_overview_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy rules overview") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_rules_overview_payload(runtime.pool(), db_user_id(user_id))
            .await
        {
            Ok(payload) => json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": payload,
                }),
            ),
            Err(_) => category_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy rules overview") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = db_user_id(user_id);

    let learning_count =
        count_rules_overview_learning_rules(runtime.connection_mut(), user_id).unwrap_or(0);
    let learning_rules =
        list_rules_overview_learning_rules(runtime.connection_mut(), user_id).unwrap_or_default();
    let category_rule_count = {
        let mut repository = CategoryRulesRepository::new(runtime.connection_mut());
        repository
            .list_rules(user_id, None, true)
            .map(|rules| rules.len() as i64)
            .unwrap_or(0)
    };
    let recurring_rules =
        list_rules_overview_recurring_rules(runtime.connection_mut(), user_id).unwrap_or_default();
    let recurring_rule_count = recurring_rules.len() as i64;

    json_response(
        StatusCode::OK,
        json!({
            "success": true,
            "data": {
                "learningRules": learning_rules,
                "learningRuleCount": learning_count,
                "categoryRuleCount": category_rule_count,
                "recurringRules": recurring_rules,
                "recurringRuleCount": recurring_rule_count,
                "totalRuleCount": learning_count + category_rule_count + recurring_rule_count,
            }
        }),
    )
}

