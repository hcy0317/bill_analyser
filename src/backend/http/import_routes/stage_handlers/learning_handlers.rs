#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_suggestions_get_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_learning_suggestions_get_runtime_handler", "business operation entered");
    import_learning_suggestions_response(state, session_id, headers, None).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_suggestions_post_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_learning_suggestions_post_runtime_handler", "business operation entered");
    import_learning_suggestions_response(state, session_id, headers, Some(payload)).await
}

/// 从当前 preview rows 构建 learning 建议响应，保持候选、统计和 expected-state 同源。
#[tracing::instrument(level = "debug", skip_all)]
async fn import_learning_suggestions_response(
    state: HttpAppState,
    session_id: String,
    headers: HeaderMap,
    payload: Option<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates = match payload.as_ref() {
        Some(payload) => {
            match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, payload) {
                Ok(updated) => updated,
                Err(response) => return route_response(response),
            }
        }
        None => 0,
    };
    let preview_ids = payload
        .as_ref()
        .map(preview_ids_from_payload)
        .unwrap_or_default();
    let suggestions = match get_preview_by_session(runtime.connection(), &session_id, user_id, false)
    {
        Ok(preview) => build_import_learning_suggestions_from_preview(&preview, &preview_ids),
        Err(error) => return route_response(db_error_response(error)),
    };
    let suggestion_count = suggestions.len();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "suggestions": suggestions,
        "count": suggestion_count,
        "preview_ids": preview_ids,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": false,
        "runtime": "rust-import-db-runtime",
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_import_learning_suggestions_from_preview(
    preview: &[ImportPreviewRow],
    preview_ids: &[i64],
) -> Vec<Value> {
    let selected_ids = preview_ids.iter().copied().collect::<BTreeSet<_>>();
    preview
        .iter()
        .filter(|row| selected_ids.is_empty() || selected_ids.contains(&row.id))
        .filter_map(|row| {
            let learning = row.preview_matching_feedback.get("learning")?.as_object()?;
            Some(json!({
                "preview_id": row.id,
                "previewId": row.id,
                "rule_id": learning.get("rule_id").cloned().unwrap_or(Value::Null),
                "score": learning.get("score").cloned().unwrap_or_else(|| json!(0)),
                "level": learning.get("level").cloned().unwrap_or_else(|| json!("")),
                "reason": learning.get("reason").cloned().unwrap_or_else(|| json!("")),
                "recommended_type": learning
                    .get("recommended_type")
                    .cloned()
                    .unwrap_or_else(|| json!(row.preview_type.clone())),
                "summary": learning.get("summary").cloned().unwrap_or_else(|| json!("")),
                "review_status": learning
                    .get("review_status")
                    .cloned()
                    .unwrap_or_else(|| json!("pending")),
                "source": learning
                    .get("source")
                    .cloned()
                    .unwrap_or_else(|| json!("import_learning_rules")),
                "mode": learning.get("mode").cloned().unwrap_or_else(|| json!("")),
                "auto_apply": learning
                    .get("auto_apply")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "recommendation_key": learning
                    .get("recommendation_key")
                    .cloned()
                    .unwrap_or_else(|| json!("")),
                "lifecycle_status": learning
                    .get("lifecycle_status")
                    .cloned()
                    .unwrap_or_else(|| json!("yellow")),
                "signal_state": learning
                    .get("signal_state")
                    .cloned()
                    .unwrap_or_else(|| json!("yellow")),
                "accepted_count": learning
                    .get("accepted_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
                "rejected_count": learning
                    .get("rejected_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
                "auto_applied_count": learning
                    .get("auto_applied_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
            }))
        })
        .collect()
}

/// 把用户选中的 preview rows 晋升为 learning 样本，并记录后续自动应用所需特征。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_promote_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_learning_promote_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates =
        match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, &payload) {
            Ok(updated) => updated,
            Err(response) => return route_response(response),
        };
    let annotation_samples = annotation_samples_from_payload(&payload);
    let saved_samples = if annotation_samples.is_empty() {
        0
    } else {
        match save_import_annotation_samples(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &annotation_samples,
        ) {
            Ok(saved) => saved,
            Err(error) => return route_response(db_error_response(error)),
        }
    };
    let mut preview_ids = preview_ids_from_payload(&payload);
    preview_ids.extend(annotation_samples.iter().map(|sample| sample.preview_id));
    preview_ids.sort_unstable();
    preview_ids.dedup();
    let promoted = match promote_import_learning_rules(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_ids,
    ) {
        Ok(result) => result,
        Err(response) => return route_response(response),
    };
    route_response(import_v2_data_response(json!({
        "success": true,
        "session_id": session_id,
        "selected_samples": preview_ids.len(),
        "saved_samples": saved_samples,
        "rules_total": promoted.rules_total,
        "created": promoted.created,
        "updated": promoted.updated,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_rules_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<ImportLearningRulesQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_learning_rules_list_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size().clamp(1, 500);
    let enabled_only = query.enabled_only();
    let total = match count_import_learning_rules(runtime.connection(), user_id, enabled_only) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    let rules = match load_import_learning_rules(
        runtime.connection(),
        user_id,
        enabled_only,
        page_size,
        offset,
    ) {
        Ok(rules) => rules,
        Err(response) => return route_response(response),
    };
    let total_pages = if total <= 0 {
        0
    } else {
        (total as usize).div_ceil(page_size) as i64
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": rules,
            "totalCount": total,
            "page": page,
            "pageSize": page_size,
            "totalPages": total_pages,
        }),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_rule_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_learning_rule_update_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    if let Err(response) =
        update_import_learning_rule(runtime.connection(), rule_id, user_id, object)
    {
        return route_response(response);
    }
    match get_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(Some(rule)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": rule}),
        }),
        Ok(None) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_rule_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_learning_rule_delete_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match delete_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": true}),
        }),
        Ok(false) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}
