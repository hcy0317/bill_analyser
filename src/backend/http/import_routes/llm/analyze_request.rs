// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn llm_analyze_transactions_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "llm_analyze_transactions_runtime_handler", "business operation entered");
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
        Err(_) => {
            return route_response(ImportV2RouteResponse {
                status_code: 400,
                body: json!({"success": false, "error": "Invalid request"}),
            });
        }
    };
    let limit = match llm_limit_from_object(object, 20, 20) {
        Ok(limit) => limit,
        Err(response) => return route_response(response),
    };
    let session_id = first_text_from_object(object, &["session_id", "sessionId"]);
    let bill_ids = match limited_id_list_field_from_object(object, &["bill_ids", "billIds"], limit)
    {
        Ok(ids) => ids,
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
        if let Some(session_id) = session_id.as_deref() {
            if let Err(response) = ensure_import_session_exists(&runtime, session_id, user_id) {
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
            let rows = match preflush_import_preview_action(
                &mut runtime,
                session_id,
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
                    body: build_llm_analysis_response(
                        Vec::new(),
                        &json!({"session_id": session_id}),
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
            let account_names = match load_account_id_map(runtime.connection(), user_id_value).await {
                Ok(accounts) => accounts
                    .into_iter()
                    .map(|(name, id)| (id, name))
                    .collect::<BTreeMap<_, _>>(),
                Err(response) => return route_response(response),
            };
            let groups = match rule_induction_groups(rows, &account_names) {
                Ok(groups) => groups,
                Err(response) => return route_response(response),
            };
            let template = llm_advanced_prompt_template(&config, "rule_prompt_template");
            LlmAnalyzePrepared::ImportSession {
                provider,
                session_id: session_id.to_string(),
                groups,
                rule_prompt_template: template,
            }
        } else {
            let config = match effective_llm_runtime_config(&state, user_id_value).await {
                Ok(config) => config,
                Err(response) => return route_response(response),
            };
            let provider = match llm_provider_context_from_config(&config) {
                Ok(provider) => provider,
                Err(response) => return route_response(response),
            };
            let postgres_runtime = match open_postgres_runtime(&state) {
                Ok(runtime) => runtime,
                Err(response) => return route_response(response),
            };
            let transactions = match load_postgres_persisted_bill_prompt_values(
                postgres_runtime.pool(),
                user_id_value,
                bill_ids.as_deref(),
                limit,
            )
            .await
            {
                Ok(transactions) => transactions,
                Err(response) => return route_response(response),
            };
            let template = llm_advanced_prompt_template(&config, "classification_prompt_template");
            LlmAnalyzePrepared::Persisted {
                provider,
                transactions,
                bill_ids,
                classification_prompt_template: template,
            }
        }
    };

    let slots = prepared.provider_call_count();
    if let Err(response) = reserve_llm_rate_limit(user_id_value, slots) {
        return route_response(response);
    }
    let mut candidates = Vec::new();
    match prepared {
        LlmAnalyzePrepared::ImportSession {
            provider,
            session_id,
            groups,
            rule_prompt_template,
        } => {
            for group in groups {
                let default_prompt = if let Some(account_id) = group.account_id {
                    build_llm_account_rule_induction_prompt(
                        account_id,
                        &group.account_name,
                        &group.account_role,
                        &group.transactions,
                    )
                } else {
                    build_llm_category_rule_induction_prompt(
                        group.category_id.unwrap_or_default(),
                        &group.category_name,
                        &group.transactions,
                    )
                };
                let template_target = if group.account_id.is_some() {
                    &group.account_name
                } else {
                    &group.category_name
                };
                let prompt = render_llm_prompt_template(
                    &rule_prompt_template,
                    &default_prompt,
                    &group.transactions,
                    template_target,
                );
                let provider_response =
                    match execute_llm_provider_request(&state, &provider, &prompt).await {
                        Ok(response) => response,
                        Err(response) => return route_response(response),
                    };
                let parsed = match parse_llm_json_array_response(&provider_response.content) {
                    Ok(values) => values,
                    Err(error) => {
                        return route_response(llm_contract_error_response(
                            &error,
                            "LLM_PROVIDER_UNAVAILABLE",
                            503,
                        ));
                    }
                };
                let runtime = match open_runtime(&state) {
                    Ok(runtime) => runtime,
                    Err(response) => return route_response(response),
                };
                let postgres_runtime = match open_postgres_runtime(&state) {
                    Ok(runtime) => runtime,
                    Err(response) => return route_response(response),
                };
                if let Err(response) = init_import_runtime_schema(&runtime) {
                    return route_response(response);
                }
                let mut seen_expressions = BTreeSet::new();
                let mut group_candidate_count = 0usize;
                for value in parsed {
                    let expression = rule_expression_from_llm_value(&value);
                    if !valid_rule_expression(&expression)
                        || !seen_expressions.insert(expression.trim().to_string())
                    {
                        continue;
                    }
                    let duplicate_result = if let Some(account_id) = group.account_id {
                        has_postgres_llm_account_rule_candidate_duplicate(
                            postgres_runtime.pool(),
                            user_id_value,
                            account_id,
                            &expression,
                        )
                        .await
                    } else {
                        has_postgres_llm_rule_candidate_duplicate(
                            postgres_runtime.pool(),
                            user_id_value,
                            &group.main_category,
                            &group.sub_category,
                            &expression,
                        )
                        .await
                    };
                    if match duplicate_result {
                        Ok(duplicate) => duplicate,
                        Err(error) => return route_response(db_error_response(error)),
                    } {
                        continue;
                    }
                    let candidate = match create_postgres_llm_candidate(
                        postgres_runtime.pool(),
                        &LlmCandidateDraft {
                            user_id: user_id_value,
                            candidate_type: group.candidate_type.to_string(),
                            source_bill_ids: group.source_ids.clone(),
                            suggested_main_category: group.main_category.clone(),
                            suggested_sub_category: group.sub_category.clone(),
                            suggested_account_id: group.account_id,
                            suggested_account_name: group.account_name.clone(),
                            suggested_rule_expression: expression,
                            confidence: confidence_from_value(&value),
                            llm_provider: provider_response.provider.clone(),
                            llm_model: provider_response.model.clone(),
                            llm_response_raw: llm_candidate_raw_response(&value),
                        },
                    )
                    .await
                    {
                        Ok(candidate) => candidate,
                        Err(error) => return route_response(db_error_response(error)),
                    };
                    candidates.push(candidate);
                    group_candidate_count += 1;
                    if group_candidate_count >= LLM_RULE_INDUCTION_MAX_CANDIDATES_PER_GROUP {
                        break;
                    }
                }
            }
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: build_llm_analysis_response(
                    candidates,
                    &json!({"session_id": session_id}),
                ),
            })
        }
        LlmAnalyzePrepared::Persisted {
            provider,
            transactions,
            bill_ids,
            classification_prompt_template,
        } => {
            if transactions.is_empty() {
                return route_response(ImportV2RouteResponse {
                    status_code: 200,
                    body: build_llm_analysis_response(
                        Vec::new(),
                        &json!({"bill_ids": bill_ids}),
                    ),
                });
            }
            let default_prompt = build_llm_classification_prompt(&transactions);
            let prompt = render_llm_prompt_template(
                &classification_prompt_template,
                &default_prompt,
                &transactions,
                "",
            );
            let provider_response =
                match execute_llm_provider_request(&state, &provider, &prompt).await {
                    Ok(response) => response,
                    Err(response) => return route_response(response),
                };
            let parsed = match parse_llm_json_array_response(&provider_response.content) {
                Ok(values) => values,
                Err(error) => {
                    return route_response(llm_contract_error_response(
                        &error,
                        "LLM_PROVIDER_UNAVAILABLE",
                        503,
                    ));
                }
            };
            let selected_ids = transactions
                .iter()
                .filter_map(|value| value.get("id").and_then(Value::as_i64))
                .collect::<BTreeSet<_>>();
            let postgres_runtime = match open_postgres_runtime(&state) {
                Ok(runtime) => runtime,
                Err(response) => return route_response(response),
            };
            let mut seen_bill_ids = BTreeSet::new();
            for value in parsed {
                let bill_id = value
                    .as_object()
                    .and_then(|object| first_value(object, &["bill_id", "billId", "id"]))
                    .and_then(value_to_i64)
                    .unwrap_or_default();
                if bill_id <= 0
                    || !selected_ids.contains(&bill_id)
                    || !seen_bill_ids.insert(bill_id)
                {
                    continue;
                }
                let candidate = match create_postgres_llm_candidate(
                    postgres_runtime.pool(),
                    &LlmCandidateDraft {
                        user_id: user_id_value,
                        candidate_type: "classification".to_string(),
                        source_bill_ids: vec![bill_id],
                        suggested_main_category: main_category_from_llm_value(&value),
                        suggested_sub_category: sub_category_from_llm_value(&value),
                        suggested_account_id: None,
                        suggested_account_name: String::new(),
                        suggested_rule_expression: String::new(),
                        confidence: confidence_from_value(&value),
                        llm_provider: provider_response.provider.clone(),
                        llm_model: provider_response.model.clone(),
                        llm_response_raw: llm_candidate_raw_response(&value),
                    },
                )
                .await
                {
                    Ok(candidate) => candidate,
                    Err(error) => return route_response(db_error_response(error)),
                };
                candidates.push(candidate);
                if candidates.len() >= selected_ids.len() {
                    break;
                }
            }
            route_response(ImportV2RouteResponse {
                status_code: 200,
                body: build_llm_analysis_response(
                    candidates,
                    &json!({"bill_ids": bill_ids}),
                ),
            })
        }
    }
}
