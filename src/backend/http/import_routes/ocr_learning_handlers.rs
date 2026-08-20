// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
/// 读取当前用户 OCR 配置，返回前会脱敏 provider 参数和凭据。
pub async fn ocr_config_get_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "ocr_config_get_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match load_postgres_ocr_config_setting(runtime.pool(), user_id_value).await {
        Ok(config) => route_response(build_ocr_config_success_response(&config)),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 保存当前用户 OCR 配置，保持 provider/lang/model/base_url/credential 合同一致。
pub async fn ocr_config_put_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "ocr_config_put_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match store_postgres_ocr_config_setting(runtime.pool(), user_id_value, Some(&payload)).await {
        Ok(config) => route_response(build_ocr_config_success_response(&config)),
        Err(bill_analyser_db::DbError::InvalidOperation(message))
            if message == "unknown OCR provider" =>
        {
            route_response(build_unknown_ocr_provider_response())
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 执行 OCR 识别请求，负责 user-scope、取消状态、rate limit 和 provider 分发。
pub async fn ocr_recognition_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "ocr_recognition_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let postgres_runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let mut config = match load_postgres_ocr_config_setting(postgres_runtime.pool(), user_id_value).await
    {
        Ok(config) => config,
        Err(error) => return route_response(db_error_response(error)),
    };
    let input = match ocr_recognition_input_from_request(&headers, &body) {
        Ok(input) => input,
        Err(response) => return route_response(response),
    };
    if input.cancelled {
        return route_response(build_ocr_error_response(
            "cancelled",
            Some("request cancelled by client"),
        ));
    }
    if config.provider == OCR_DISABLED_PROVIDER_NAME {
        return route_response(build_ocr_error_response(
            "provider_unconfigured",
            Some("ocr provider not configured"),
        ));
    }
    if !ocr_rate_limit_try_acquire(user_id_value) {
        return route_response(build_ocr_error_response(
            "rate_limited",
            Some("ocr per-user rate limit exceeded"),
        ));
    }
    if config.provider == NETWORK_OCR_PROVIDER_NAME
        && provider_auth_is_expired(&config.credential_config, Utc::now())
    {
        if validate_llm_vision_base_url(&config.base_url).is_err() {
            return route_response(build_ocr_error_response(
                "provider_unconfigured",
                Some("OCR provider base URL is not allowed"),
            ));
        }
        let client = match reqwest::Client::builder()
            .timeout(StdDuration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        {
            Ok(client) => client,
            Err(_) => {
                return route_response(build_ocr_error_response(
                    "provider_unconfigured",
                    Some("OCR provider unavailable"),
                ))
            }
        };
        let refreshed =
            match refresh_provider_auth_profile(&client, &config.credential_config, &config.base_url)
                .await
            {
                Ok(refreshed) => refreshed,
                Err(_) => {
                    return route_response(build_ocr_error_response(
                        "provider_relogin_required",
                        Some(
                            "OCR provider authorization expired; sign in again or refresh credentials",
                        ),
                    ))
                }
            };
        let stored_config = json!({
            "provider": config.provider,
            "lang": config.lang,
            "model": config.model,
            "base_url": config.base_url,
            "parameters": config.parameters,
            "credential_config": refreshed,
        });
        config = match store_postgres_ocr_config_setting(
            postgres_runtime.pool(),
            user_id_value,
            Some(&stored_config),
        )
        .await
        {
            Ok(config) => config,
            Err(error) => return route_response(db_error_response(error)),
        };
    }
    let request_id = format!(
        "rust-ocr-{}",
        OCR_REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let provider_result =
        match run_ocr_provider(&config, input.image_bytes, input.mime.clone()).await {
            Ok(result) => result,
            Err(failure) => return route_response(ocr_provider_failure_response(failure)),
        };
    let draft_context = match load_receipt_draft_context(runtime.connection(), user_id_value).await {
        Ok(context) => context,
        Err(response) => return route_response(response),
    };
    route_response(build_ocr_recognition_success_response_with_context(
        &config.provider,
        &provider_result,
        &request_id,
        &draft_context,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_receipt_draft_context(
    connection: &Connection,
    user_id: i64,
) -> Result<ReceiptDraftContext, ImportV2RouteResponse> {
    let categories = load_import_intelligence_categories(connection, user_id)
        .await
        .map_err(db_error_response)?;
    let categories_by_id = categories
        .iter()
        .map(|category| (category.id, category.clone()))
        .collect::<BTreeMap<_, _>>();
    let category_rules =
        load_import_intelligence_category_rules(connection, user_id, &categories_by_id)
            .await
            .map_err(db_error_response)?;
    let account_rules = load_import_intelligence_account_rules(connection, user_id)
        .await
        .map_err(db_error_response)?;
    let accounts = load_import_intelligence_accounts(connection, user_id)
        .await
        .map_err(db_error_response)?;
    let tags = load_receipt_draft_tags(connection, user_id).map_err(db_error_response)?;
    Ok(ReceiptDraftContext {
        categories: categories
            .into_iter()
            .map(|category| ReceiptDraftCategory {
                id: category.id.to_string(),
                type_code: category.type_code,
                label: category_label(&category.main_category, &category.sub_category),
            })
            .collect(),
        category_rules: category_rules
            .into_iter()
            .map(|rule| ReceiptDraftCategoryRule {
                id: rule.id.to_string(),
                category_id: rule.category_id.to_string(),
                category_type: i64::from(rule.category_type),
                label: category_label(&rule.main_category, &rule.sub_category),
                priority: i64::from(rule.priority),
                rule_expression: rule.rule_expression,
                regex_enabled: rule.regex_enabled,
            })
            .collect(),
        account_rules,
        accounts: accounts
            .into_iter()
            .map(|account| ReceiptDraftAccount {
                id: account.id.to_string(),
                name: account.name,
            })
            .collect(),
        tags,
    })
}

fn category_label(main_category: &str, sub_category: &str) -> String {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if main_category.is_empty() {
        sub_category.to_string()
    } else if sub_category.is_empty() {
        main_category.to_string()
    } else {
        format!("{main_category} / {sub_category}")
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_receipt_draft_tags(
    _connection: &Connection,
    _user_id: i64,
) -> Result<Vec<ReceiptDraftTag>, bill_analyser_db::DbError> {
    Ok(Vec::new())
}

#[tracing::instrument(level = "debug", skip_all)]
/// 查询导入学习建议列表，按当前用户和筛选条件返回分页数据。
pub async fn learning_suggestions_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<LearningCenterListQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_suggestions_list_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0);
    let status = query
        .status
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let total = match count_learning_suggestions(runtime.connection(), user_id, status) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let items =
        match load_learning_suggestions(runtime.connection(), user_id, status, limit, offset) {
            Ok(items) => items,
            Err(response) => return route_response(response),
        };
    route_response(learning_data_response(json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 触发生成导入学习建议，保持规则/建议生成仍在本地运行态内完成。
pub async fn learning_suggestions_generate_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_suggestions_generate_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match mine_learning_suggestions(runtime.connection_mut(), user_id) {
        Ok(stats) => route_response(learning_data_response(stats)),
        Err(response) => route_response(response),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 接受单条导入学习建议，并记录 lifecycle feedback。
pub async fn learning_suggestion_accept_runtime_handler(
    State(state): State<HttpAppState>,
    Path(suggestion_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_suggestion_accept_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match accept_learning_suggestion(runtime.connection_mut(), suggestion_id, user_id) {
        Ok(LearningSuggestionDecision::NotFound) => {
            route_response(learning_error_response(404, "suggestion_not_found"))
        }
        Err(response) => route_response(response),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 批量接受导入学习建议，统一校验 preview_ids 与 suggestion_ids。
pub async fn learning_suggestions_batch_accept_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_suggestions_batch_accept_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(raw_ids) = first_value(object, &["suggestionIds", "suggestion_ids"]) else {
        return route_response(learning_error_response(
            400,
            "suggestionIds must be a non-empty array",
        ));
    };
    let Some(raw_ids) = raw_ids.as_array() else {
        return route_response(learning_error_response(
            400,
            "suggestionIds must be a non-empty array",
        ));
    };
    if raw_ids.is_empty() {
        return route_response(learning_error_response(
            400,
            "suggestionIds must be a non-empty array",
        ));
    }
    if raw_ids.len() > 100 {
        return route_response(learning_error_response(
            400,
            "batch size must not exceed 100",
        ));
    }
    let mut suggestion_ids = Vec::new();
    for item in raw_ids {
        let Some(id) = value_to_i64(item).filter(|value| *value > 0) else {
            return route_response(learning_error_response(400, "Invalid suggestionIds"));
        };
        if !suggestion_ids.contains(&id) {
            suggestion_ids.push(id);
        }
    }

    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let accepted: Vec<Value> = Vec::new();
    let mut failed = Vec::new();
    for suggestion_id in suggestion_ids {
        match accept_learning_suggestion(runtime.connection_mut(), suggestion_id, user_id) {
            Ok(LearningSuggestionDecision::NotFound) => {
                failed.push(json!({"id": suggestion_id, "error": "suggestion_not_found"}));
            }
            Err(response) => return route_response(response),
        }
    }
    route_response(learning_data_response(json!({
        "accepted": accepted,
        "failed": failed,
        "acceptedCount": accepted.len(),
        "failedCount": failed.len(),
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 拒绝单条导入学习建议，并写入 lifecycle reject feedback。
pub async fn learning_suggestion_reject_runtime_handler(
    State(state): State<HttpAppState>,
    Path(suggestion_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_suggestion_reject_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match reject_learning_suggestion(runtime.connection_mut(), suggestion_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true}),
        }),
        Ok(false) => route_response(learning_error_response(
            404,
            "suggestion_not_found_or_not_pending",
        )),
        Err(response) => route_response(response),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 查询当前用户导入学习规则列表，保持 learning center 所需分页响应。
pub async fn learning_rules_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<LearningCenterListQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_rules_list_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0);
    let enabled_only = query.enabled_only();
    let total = match bill_analyser_db::taxonomy::postgres_reads::count_postgres_learning_rules(
        runtime.pool(),
        user_id,
        enabled_only,
    )
    .await
    {
        Ok(total) => total,
        Err(error) => return route_response(db_error_response(error)),
    };
    let items = match bill_analyser_db::taxonomy::postgres_reads::list_postgres_learning_rules(
        runtime.pool(),
        user_id,
        enabled_only,
        limit,
        offset,
    )
    .await
    {
        Ok(items) => items,
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(learning_data_response(json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 启停当前用户导入学习规则，权限和 user-scope 由运行时上下文控制。
pub async fn learning_rule_toggle_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_rule_toggle_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let enabled = first_value(object, &["enabled"])
        .and_then(value_to_bool)
        .unwrap_or(true);
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let update = bill_analyser_db::taxonomy::postgres_reads::PostgresLearningRuleUpdate {
        enabled: Some(enabled),
        ..Default::default()
    };
    match bill_analyser_db::taxonomy::postgres_reads::update_postgres_learning_rule(
        runtime.pool(),
        user_id,
        rule_id,
        &update,
    )
    .await
    {
        Ok(Some(_)) => route_response(learning_data_response(json!({
            "ruleId": rule_id,
            "enabled": enabled,
        }))),
        Ok(None) => route_response(learning_error_response(404, "rule_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

include!("learning_rule_update.rs");

#[tracing::instrument(level = "debug", skip_all)]
/// 删除当前用户导入学习规则，未命中时返回标准学习域错误。
pub async fn learning_rule_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "learning_rule_delete_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match bill_analyser_db::taxonomy::postgres_reads::delete_postgres_learning_rule(
        runtime.pool(),
        user_id,
        rule_id,
    )
    .await
    {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true}),
        }),
        Ok(false) => route_response(learning_error_response(404, "rule_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}
