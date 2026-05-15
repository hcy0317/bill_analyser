pub async fn llm_rule_synthesis_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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
    let limit = match llm_limit_from_object(object, 8, 20) {
        Ok(limit) => limit,
        Err(response) => return route_response(response),
    };
    let prepared = {
        let runtime = match open_runtime(&state) {
            Ok(runtime) => runtime,
            Err(response) => return route_response(response),
        };
        if let Err(response) = init_import_runtime_schema(&runtime) {
            return route_response(response);
        }
        if let Err(response) = init_global_learning_runtime_schema(&runtime) {
            return route_response(response);
        }
        if let Err(response) = init_llm_config_runtime_schema(&runtime) {
            return route_response(response);
        }
        let categories = match load_existing_category_values(runtime.connection(), user_id_value) {
            Ok(categories) => categories,
            Err(response) => return route_response(response),
        };
        let knowledge_pack =
            match build_rule_synthesis_knowledge_pack(runtime.connection(), user_id, &categories) {
                Ok(pack) => pack,
                Err(response) => return route_response(response),
            };
        if categories.is_empty() || !rule_synthesis_has_learning_evidence(&knowledge_pack) {
            return route_response(rule_synthesis_empty_response(knowledge_pack));
        }
        let config = match effective_llm_runtime_config(&state, &runtime, user_id_value) {
            Ok(config) => config,
            Err(response) => return route_response(response),
        };
        let provider = match llm_provider_context_from_config(&config) {
            Ok(provider) => provider,
            Err(response) => return route_response(response),
        };
        LlmRuleSynthesisPrepared {
            provider,
            knowledge_pack,
            categories,
            limit,
        }
    };
    if let Err(response) = reserve_llm_rate_limit(user_id_value, 1) {
        return route_response(response);
    }
    let prompt =
        build_llm_rule_expression_synthesis_prompt(&prepared.knowledge_pack, prepared.limit);
    let provider_response =
        match execute_llm_provider_request(&state, &prepared.provider, &prompt).await {
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
    let category_paths = prepared
        .categories
        .iter()
        .filter_map(|value| value.get("path").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_llm_config_runtime_schema(&runtime) {
        return route_response(response);
    }
    let mut candidates = Vec::new();
    for value in parsed {
        let main_category = main_category_from_llm_value(&value);
        let sub_category = sub_category_from_llm_value(&value);
        let category_path = category_path(&main_category, &sub_category);
        if !category_paths.contains(&category_path) {
            continue;
        }
        let expression = rule_expression_from_llm_value(&value);
        if !valid_rule_expression(&expression) {
            continue;
        }
        if match rule_candidate_duplicate(
            runtime.connection(),
            user_id_value,
            &main_category,
            &sub_category,
            &expression,
        ) {
            Ok(duplicate) => duplicate,
            Err(response) => return route_response(response),
        } {
            continue;
        }
        let candidate = match create_llm_candidate(
            runtime.connection(),
            &LlmCandidateDraft {
                user_id: user_id_value,
                candidate_type: "rule_synthesis".to_string(),
                source_bill_ids: Vec::new(),
                suggested_main_category: main_category,
                suggested_sub_category: sub_category,
                suggested_rule_expression: expression,
                confidence: confidence_from_value(&value),
                llm_provider: provider_response.provider.clone(),
                llm_model: provider_response.model.clone(),
                llm_response_raw: llm_candidate_raw_response(&value),
            },
        ) {
            Ok(candidate) => candidate,
            Err(error) => return route_response(db_error_response(error)),
        };
        candidates.push(candidate);
        if candidates.len() >= prepared.limit {
            break;
        }
    }
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": {
                "mode": "rule_synthesis",
                "knowledge_summary_pack": prepared.knowledge_pack,
                "candidates_created": candidates.len(),
                "candidates": candidates,
            },
            "total": candidates.len(),
        }),
    })
}

