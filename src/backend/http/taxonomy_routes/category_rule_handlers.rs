// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
/// 查询当前用户的分类规则列表，并按可选分类与启用状态过滤。
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

#[tracing::instrument(level = "debug", skip_all)]
/// 创建分类规则并返回当前前端使用的 category rule 投影。
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

#[tracing::instrument(level = "debug", skip_all)]
/// 更新分类规则，保持 rule_expression 必填校验和 user-scope 写入约束。
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

#[tracing::instrument(level = "debug", skip_all)]
/// 删除当前用户拥有的分类规则，不跨用户暴露是否存在。
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

#[tracing::instrument(level = "debug", skip_all)]
/// 按前端传入顺序重排分类规则，要求 rule_ids 全部属于当前用户。
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
        let Some(rule_id) = parse_json_int(value) else {
            return bad_request("rule_ids must be a list");
        };
        parsed_rule_ids.push(rule_id);
    }


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

#[tracing::instrument(level = "debug", skip_all)]
/// 初始化内置分类规则和缺失分类，作为规则中心的手动修复入口。
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

#[tracing::instrument(level = "debug", skip_all)]
/// 只读测试单条分类规则是否命中文本，不更新规则应用计数。
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

#[tracing::instrument(level = "debug", skip_all)]
/// 聚合分类、学习和周期规则概览，为规则中心 overview tab 提供只读数据。
async fn rules_overview_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "rules_overview_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

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
