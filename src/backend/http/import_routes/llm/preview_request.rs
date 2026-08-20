// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_preview_recommend_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_preview_recommend_runtime_handler", "business operation entered");
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
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let Some(session_id) = first_text_from_object(object, &["session_id", "sessionId"]) else {
        return route_response(llm_contract_error_response(
            "session_id is required",
            "INVALID_REQUEST",
            400,
        ));
    };
    let limit = match llm_limit_from_object(object, 20, 20) {
        Ok(limit) => limit,
        Err(response) => return route_response(response),
    };

    let prepared = {
        let mut runtime = match open_runtime(&state) {
            Ok(runtime) => runtime,
            Err(response) => return route_response(response),
        };
        if let Err(response) = init_import_runtime_schema(&runtime) {
            return route_response(response);
        }
        if let Err(response) = ensure_import_session_exists(&runtime, &session_id, user_id) {
            return route_response(response);
        }
        if let Err(response) = validate_llm_preview_selection_limits(&payload, limit) {
            return route_response(response);
        }
        let action_scope = match import_preview_action_scope_from_payload(&payload) {
            Ok(scope) => scope,
            Err(response) => return route_response(response),
        };
        let patches = match preview_patches_from_payload(
            runtime.connection(),
            user_id,
            &payload,
            limit,
        ) {
            Ok(patches) => patches,
            Err(response) => return route_response(response),
        };
        let mut rows = match preflush_import_preview_action(
            &mut runtime,
            &session_id,
            user_id,
            &action_scope,
            &patches,
            limit,
        ) {
            Ok(result) => result.rows,
            Err(response) => return route_response(response),
        };
        if rows.is_empty() {
            return route_response(ImportV2RouteResponse {
                status_code: 200,
                body: build_llm_preview_recommend_response(
                    &session_id,
                    Vec::new(),
                ),
            });
        }
        rows.retain(preview_row_needs_llm_identity);
        if rows.is_empty() {
            return route_response(ImportV2RouteResponse {
                status_code: 200,
                body: build_llm_preview_recommend_response(
                    &session_id,
                    Vec::new(),
                ),
            });
        }
        let config = match effective_llm_runtime_config(&state, user_id_value).await {
            Ok(config) => config,
            Err(response) => return route_response(response),
        };
        let provider = match llm_provider_context_from_config(&config) {
            Ok(provider) => provider,
            Err(response) => return route_response(response),
        };
        let categories = match load_existing_category_values(runtime.connection(), user_id_value).await {
            Ok(categories) => categories,
            Err(response) => return route_response(response),
        };
        let accounts = match load_existing_account_values(runtime.connection(), user_id_value).await {
            Ok(accounts) => accounts,
            Err(response) => return route_response(response),
        };
        let account_ids = match load_account_id_map(runtime.connection(), user_id_value).await {
            Ok(accounts) => accounts,
            Err(response) => return route_response(response),
        };
        let memory_context = match load_llm_memory_prompt_context(runtime.connection(), user_id) {
            Ok(memory) => memory,
            Err(response) => return route_response(response),
        };
        let transactions = rows
            .iter()
            .map(preview_row_prompt_value)
            .collect::<Vec<_>>();
        let prompt = build_llm_import_preview_recommendation_prompt(
            &transactions,
            &categories,
            &accounts,
            &memory_context,
        );
        LlmPreviewPrepared {
            session_id,
            provider,
            prompt,
            selected_preview_ids: rows.iter().map(|row| row.id).collect(),
            account_ids,
            missing_identities: rows
                .iter()
                .map(|row| {
                    (
                        row.id,
                        LlmPreviewMissingIdentity {
                            category: row.category_id.is_none(),
                            source_account: row.preview_source_account_id.is_none(),
                            destination_account: row.preview_type == "转账"
                                && row.preview_destination_account_id.is_none(),
                        },
                    )
                })
                .collect(),
        }
    };

    if let Err(response) = reserve_llm_rate_limit(user_id_value, 1) {
        return route_response(response);
    }
    let provider_response =
        match execute_llm_provider_request(&state, &prepared.provider, &prepared.prompt).await {
            Ok(response) => response,
            Err(response) => return route_response(response),
        };
    let raw_suggestions = match parse_llm_json_array_response(&provider_response.content) {
        Ok(values) => values,
        Err(error) => {
            return route_response(llm_contract_error_response(
                &error,
                "LLM_PROVIDER_UNAVAILABLE",
                503,
            ));
        }
    };
    let selected_ids = prepared
        .selected_preview_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut suggestions = Vec::new();
    let mut seen_preview_ids = BTreeSet::new();
    for value in raw_suggestions {
        let preview_id = preview_id_from_llm_value(&value);
        if preview_id <= 0
            || !selected_ids.contains(&preview_id)
            || !seen_preview_ids.insert(preview_id)
        {
            continue;
        }
        let suggestion_value = fill_llm_suggestion_account_ids(value, &prepared.account_ids);
        let Some(mut suggestion) = llm_suggestion_from_value(&suggestion_value) else {
            continue;
        };
        if let Some(missing) = prepared.missing_identities.get(&preview_id) {
            restrict_llm_suggestion_to_missing_identities(&mut suggestion, *missing);
        }
        let result = match apply_preview_llm_recommendation(
            runtime.connection_mut(),
            &bill_analyser_db::ImportPreviewLlmApplyRequest {
                session_id: &prepared.session_id,
                preview_id,
                user_id,
                suggestion: &suggestion,
                prompt_text: Some(&prepared.prompt),
                llm_provider: Some(&provider_response.provider),
                llm_model: Some(&provider_response.model),
            },
        ) {
            Ok(result) => result,
            Err(error) => return route_response(db_error_response(error)),
        };
        if let Some(value) = llm_preview_recommendation_item(result, preview_id) {
            suggestions.push(value);
        }
        if suggestions.len() >= selected_ids.len() {
            break;
        }
    }
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: build_llm_preview_recommend_response(
            &prepared.session_id,
            suggestions,
        ),
    })
}
