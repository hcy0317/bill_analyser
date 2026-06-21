#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_recurring_candidates_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "preview_recurring_candidates_runtime_handler", "business operation entered");
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
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    let user_id_i64 = match user_id_i64_for_sql(user_id) {
        Ok(value) => value,
        Err(error) => return route_response(db_error_response(error)),
    };
    let draft = import_preview_draft_from_row(&preview);
    let candidates = match load_import_intelligence_recurring_templates(runtime.connection(), user_id_i64)
        .await
    {
        Ok(templates) => {
            let mut candidates = templates
                .iter()
                .filter_map(|template| recurring_template_match(&draft, template))
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                right
                    .match_score
                    .partial_cmp(&left.match_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            candidates
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    let candidate_count = candidates.len();
    let candidates = candidates
        .into_iter()
        .map(|candidate| {
            json!({
                "id": candidate.id,
                "name": candidate.name,
                "matchScore": candidate.match_score,
                "match_score": candidate.match_score,
                "matchReasons": candidate.match_reasons,
                "match_reasons": candidate.match_reasons,
                "matchedOccurrenceDate": candidate.matched_occurrence_date,
                "matched_occurrence_date": candidate.matched_occurrence_date,
            })
        })
        .collect::<Vec<_>>();
    route_response(import_v2_data_response(json!({
        "previewId": preview.id,
        "sessionId": preview.session_id,
        "linkedRecurringId": preview.preview_recurring_id,
        "linkedRecurringName": preview.preview_recurring_name,
        "candidates": candidates,
        "candidate_count": candidate_count,
        "provider_bypassed": false,
        "runtime": "rust-import-db-runtime",
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_recurring_match_put_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "preview_recurring_match_put_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let recurring_id = match first_value(object, &["recurringId", "recurring_id"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => return route_response(import_v2_error_response(400, "Missing recurringId")),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let target_candidate = recurring_candidate_from_payload(object, recurring_id);
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: Some(recurring_id),
        candidate_count: recurring_candidate_count_from_payload(object, target_candidate.as_ref()),
        target_candidate,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        &preview.session_id,
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": recurring_id}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_recurring_match_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "preview_recurring_match_delete_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: None,
        candidate_count: 0,
        target_candidate: None,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        &preview.session_id,
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": Value::Null}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 处理人工转账候选决策，确保转账 feedback 与 preview row 状态同步落库。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_transfer_decision_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "preview_transfer_decision_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let decision = match decision_from_payload(object) {
        Ok(decision) => decision,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    match apply_preview_transfer_decision(
        runtime.connection_mut(),
        &preview.session_id,
        preview_id,
        user_id,
        decision,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"decision": decision_name(decision)}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}
