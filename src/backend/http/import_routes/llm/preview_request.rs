pub async fn llm_preview_recommend_runtime_handler(
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
        if let Err(response) = init_llm_config_runtime_schema(&runtime) {
            return route_response(response);
        }
        if let Err(response) = ensure_import_session_exists(&runtime, &session_id, user_id) {
            return route_response(response);
        }
        if let Err(response) = validate_llm_preview_selection_limits(&payload, limit) {
            return route_response(response);
        }
        let patches = match preview_patches_from_payload(
            runtime.connection(),
            user_id,
            &payload,
            limit,
        ) {
            Ok(patches) => patches,
            Err(response) => return route_response(response),
        };
        if !patches.is_empty() {
            if let Err(error) =
                update_preview_bills_batch(runtime.connection_mut(), &session_id, user_id, &patches)
            {
                return route_response(db_error_response(error));
            }
        }
        let rows = match selected_preview_rows_for_llm(
            runtime.connection(),
            &payload,
            &session_id,
            user_id,
            limit,
        ) {
            Ok(rows) => rows,
            Err(response) => return route_response(response),
        };
        if rows.is_empty() {
            return route_response(llm_contract_error_response(
                "No preview rows selected",
                "PREVIEW_SELECTION_EMPTY",
                400,
            ));
        }
        let config = match effective_llm_runtime_config(&state, &runtime, user_id_value) {
            Ok(config) => config,
            Err(response) => return route_response(response),
        };
        let provider = match llm_provider_context_from_config(&config) {
            Ok(provider) => provider,
            Err(response) => return route_response(response),
        };
        let categories = match load_existing_category_paths(runtime.connection(), user_id_value) {
            Ok(categories) => categories,
            Err(response) => return route_response(response),
        };
        let accounts = match load_existing_account_names(runtime.connection(), user_id_value) {
            Ok(accounts) => accounts,
            Err(response) => return route_response(response),
        };
        let account_ids = match load_account_id_map(runtime.connection(), user_id_value) {
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
        let Some(suggestion) = llm_suggestion_from_value(&suggestion_value) else {
            continue;
        };
        let result = match apply_preview_llm_recommendation(
            runtime.connection_mut(),
            bill_analyser_db::ImportPreviewLlmApplyRequest {
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
        body: bill_analyser_core::build_llm_preview_recommend_response(
            &prepared.session_id,
            suggestions,
        ),
    })
}

