// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

/// 接受导入预览 LLM 建议，记录 memory 事件并返回更新后的预览行投影。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_preview_recommend_accept_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_preview_recommend_accept_runtime_handler", "business operation entered");
    llm_preview_recommend_review_response(
        state,
        headers,
        payload,
        ImportPreviewDecision::Accept,
        "accept",
    )
    .await
}

/// 拒绝导入预览 LLM 建议，清理预览信号并记录用户反馈 memory。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_preview_recommend_reject_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_preview_recommend_reject_runtime_handler", "business operation entered");
    llm_preview_recommend_review_response(
        state,
        headers,
        payload,
        ImportPreviewDecision::Reject,
        "reject",
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn llm_preview_recommend_review_response(
    state: HttpAppState,
    headers: HeaderMap,
    payload: Value,
    decision: ImportPreviewDecision,
    decision_text: &str,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match first_value(object, &["session_id", "sessionId"])
        .and_then(value_to_text)
        .filter(|value| !value.trim().is_empty())
    {
        Some(value) => value,
        None => {
            return route_response(import_v2_error_response(
                400,
                "session_id and preview_id are required",
            ));
        }
    };
    let preview_id = match first_value(object, &["preview_id", "previewId"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => {
            return route_response(import_v2_error_response(
                400,
                "session_id and preview_id are required",
            ));
        }
    };
    let suggestion_value = first_value(object, &["suggestion"]).cloned();
    let expected_state = if first_value(object, &["expectedState", "expected_state"]).is_some() {
        match expected_state_from_payload(object) {
            Ok(expected_state) => Some(expected_state),
            Err(response) => return route_response(response),
        }
    } else {
        None
    };
    let user_correction =
        first_value(object, &["user_correction", "userCorrection"]).and_then(Value::as_object);
    let user_correction_category = user_correction.and_then(|object| {
        first_value(
            object,
            &[
                "category",
                "categoryName",
                "mainCategory",
                "main_category",
                "suggested_main_category",
            ],
        )
        .and_then(value_to_text)
    });
    let user_correction_account = user_correction.and_then(|object| {
        first_value(
            object,
            &[
                "account",
                "accountName",
                "sourceAccount",
                "source_account",
                "suggested_source_account",
            ],
        )
        .and_then(value_to_text)
    });
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let account_ids = match load_account_id_map(runtime.connection(), user_id_value).await {
        Ok(account_ids) => account_ids,
        Err(response) => return route_response(response),
    };
    let suggestion = suggestion_value
        .map(|value| fill_llm_suggestion_account_ids(value, &account_ids))
        .and_then(|value| llm_suggestion_from_value(&value));
    match review_preview_llm_recommendation(
        runtime.connection_mut(),
        &ImportPreviewLlmReviewRequest {
            session_id: &session_id,
            preview_id,
            user_id,
            decision,
            suggestion: suggestion.as_ref(),
            user_correction_category: user_correction_category.as_deref(),
            user_correction_account: user_correction_account.as_deref(),
            expected_state: expected_state.as_ref(),
        },
    ) {
        Ok(result) if result.state_conflict => {
            route_response(import_v2_error_response(409, "Preview state is stale"))
        }
        Ok(result) => route_response(llm_decision_result_response(
            result,
            &session_id,
            preview_id,
            decision_text,
        )),
        Err(bill_analyser_db::DbError::PreviewVersionConflict {
            preview_id: conflict_preview_id,
            expected,
            ..
        }) if conflict_preview_id == preview_id => {
            let latest_row = match get_preview_bill_by_id(
                runtime.connection(),
                conflict_preview_id,
                user_id,
            ) {
                Ok(Some(row)) if row.session_id == session_id => row,
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(
                        404,
                        "Preview recommendation is no longer available",
                    ));
                }
                Err(error) => return route_response(db_error_response(error)),
            };
            route_response(preview_row_version_conflict_response(expected, latest_row))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 查询用户 LLM memory 事件，供学习中心和 prompt 上下文复用。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_memory_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<LlmMemoryQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_memory_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    match get_llm_memory_events(
        runtime.connection(),
        user_id,
        query.session_id.as_deref(),
        limit,
    ) {
        Ok(events) => {
            let total = events.len();
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true, "data": events, "total": total}),
            })
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 读取当前用户有效 LLM runtime 配置，优先使用内存覆盖，再回退已保存配置。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_config_get_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_config_get_runtime_handler", "business operation entered");
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
    let config = if let Some(config) = state.get_llm_runtime_config(user_id_value) {
        config
    } else {
        match effective_postgres_llm_config_from_saved(runtime.pool(), user_id_value).await {
            Ok(config) => config,
            Err(error) => return route_response(db_error_response(error)),
        }
    };
    ai_route_response(build_llm_config_get_response(&config))
}

/// 更新当前用户临时 runtime LLM 配置，不直接写入 saved config 表。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_config_post_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_config_post_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let payload = match json_body_or_empty(&body) {
        Ok(payload) => payload,
        Err(response) => return route_response(response),
    };
    let Some(object) = payload.as_object().filter(|object| !object.is_empty()) else {
        return route_response(ImportV2RouteResponse {
            status_code: 400,
            body: json!({"success": false, "error": "No data provided"}),
        });
    };
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let base_config = if let Some(config) = state.get_llm_runtime_config(user_id_value) {
        config
    } else {
        match effective_postgres_llm_config_from_saved(runtime.pool(), user_id_value).await {
            Ok(config) => config,
            Err(error) => return route_response(db_error_response(error)),
        }
    };
    let updated = update_runtime_llm_config_payload(&base_config, object);
    state.set_llm_runtime_config(user_id_value, updated.clone());
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": llm_runtime_config_response_data(&updated, false)}),
    })
}

/// 查询当前用户 saved LLM 配置列表，返回前统一脱敏。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_configs_list_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_configs_list_runtime_handler", "business operation entered");
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
    match list_postgres_llm_configs(runtime.pool(), user_id_value).await {
        Ok(configs) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({
                "success": true,
                "data": configs.iter().map(safe_llm_config_payload).collect::<Vec<_>>(),
            }),
        }),
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 创建当前用户 saved LLM 配置，支持 credential_config 和 advanced_settings 持久化。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_configs_create_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_configs_create_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let payload = match json_body_or_empty(&body) {
        Ok(payload) => payload,
        Err(response) => return route_response(response),
    };
    let object = payload.as_object().cloned().unwrap_or_default();
    let name = text_from_map(&object, "name").trim().to_string();
    if name.is_empty() {
        return route_response(ImportV2RouteResponse {
            status_code: 400,
            body: json!({"success": false, "error": "name is required"}),
        });
    }
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let draft = LlmConfigDraft {
        name,
        provider: text_from_map_or(&object, "provider", "openai"),
        model: text_from_map_or(&object, "model", ""),
        api_key: text_from_map_or(&object, "api_key", ""),
        base_url: text_from_map_or(&object, "base_url", ""),
        credential_config: first_value(
            &object,
            &["credential_config", "credentialConfig", "auth_profile", "authProfile"],
        )
        .cloned()
        .unwrap_or_else(|| json!({})),
        advanced_settings: object
            .get("advanced_settings")
            .cloned()
            .unwrap_or_else(|| json!({})),
        is_active: object
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    match create_postgres_llm_config(runtime.pool(), user_id_value, &draft).await {
        Ok(config) => {
            if config
                .get("is_active")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                state.clear_llm_runtime_config(user_id_value);
            }
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true, "data": safe_llm_config_payload(&config)}),
            })
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 更新当前用户 saved LLM 配置，保留密钥脱敏占位符“不变更”语义。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_config_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_config_update_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let payload = match json_body_or_empty(&body) {
        Ok(payload) => payload,
        Err(response) => return route_response(response),
    };
    let object = payload.as_object().cloned().unwrap_or_default();
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    match update_postgres_llm_config(
        runtime.pool(),
        config_id,
        user_id_value,
        &llm_config_update_from_map(&object),
    )
    .await
    {
        Ok(Some(config)) => {
            if config
                .get("is_active")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                state.clear_llm_runtime_config(user_id_value);
            }
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true, "data": safe_llm_config_payload(&config)}),
            })
        }
        Ok(None) => route_response(llm_not_found_response("config_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 删除当前用户 saved LLM 配置，未命中时返回 config_not_found。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_config_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_config_delete_runtime_handler", "business operation entered");
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
    match delete_postgres_llm_config(runtime.pool(), config_id, user_id_value).await {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true}),
        }),
        Ok(false) => route_response(llm_not_found_response("config_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 激活当前用户指定 saved LLM 配置，并清理 runtime 覆盖缓存。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_config_activate_runtime_handler(
    State(state): State<HttpAppState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_config_activate_runtime_handler", "business operation entered");
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
    match activate_postgres_llm_config(runtime.pool(), config_id, user_id_value).await {
        Ok(true) => {
            state.clear_llm_runtime_config(user_id_value);
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: json!({"success": true}),
            })
        }
        Ok(false) => route_response(llm_not_found_response("config_not_found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 使用当前用户保存的指定配置执行一次最小 provider 探测，不改变激活状态或运行时覆盖。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_config_test_runtime_handler(
    State(state): State<HttpAppState>,
    Path(config_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let user_id_value = match user_id_i64_value(user_id) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    if let Err(response) = reserve_llm_rate_limit(user_id_value, 1) {
        return route_response(response);
    }
    let runtime = match open_postgres_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    let saved_config = match get_postgres_llm_config(runtime.pool(), config_id, user_id_value).await {
        Ok(Some(config)) => config,
        Ok(None) => return route_response(llm_not_found_response("config_not_found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    let runtime_config = build_runtime_llm_config_from_saved_config(&saved_config);
    let mut provider = match llm_provider_context_from_config(&runtime_config) {
        Ok(provider) => provider,
        Err(response) => return route_response(response),
    };
    provider.system_prompt.clear();
    provider.temperature = 0.0;
    provider.max_tokens = 16;
    provider.reasoning_depth.clear();

    let started_at = Instant::now();
    match execute_llm_provider_request(
        &state,
        &provider,
        "Reply with exactly this tiny JSON object: {\"ok\":true}",
    )
    .await
    {
        Ok(response) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({
                "success": true,
                "data": {
                    "provider": response.provider,
                    "model": response.model,
                    "latency_ms": started_at.elapsed().as_millis(),
                },
            }),
        }),
        Err(response) => route_response(response),
    }
}
