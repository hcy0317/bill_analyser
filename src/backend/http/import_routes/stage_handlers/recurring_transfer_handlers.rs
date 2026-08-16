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
            expected_state.expected_row_version,
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
            expected_state.expected_row_version,
            json!({"recurringId": Value::Null}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

/// 组级转账决策入口。group 与全部 preview member 的版本共同构成 CAS token。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_decision_group_runtime_handler(
    State(state): State<HttpAppState>,
    Path((session_id, group_id)): Path<(String, i64)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let (command, preview_ids) = match decision_group_command_from_object(
        object,
        session_id.clone(),
        group_id,
    ) {
        Ok(value) => value,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Ok(groups) = get_import_decision_groups_by_session(runtime.connection(), &session_id, user_id) {
        if let Some(group) = groups.iter().find(|group| group.id == group_id) {
            let expected_ids = group.members.iter().filter_map(|member| member.preview_row_id).collect::<std::collections::HashSet<_>>();
            if expected_ids != preview_ids {
                return route_response(import_v2_error_response(400, "expectedPreviewVersions must exactly cover preview members"));
            }
        }
    }
    match apply_import_decision_group_command(runtime.connection(), user_id, &command) {
        Ok(ImportDecisionGroupCommandResult::Applied(mut result)) => {
            let canonical_only = result.upserted_preview_items.is_empty();
            match decision_group_response_items(
                &mut runtime,
                &command.session_id,
                user_id,
                command.group_id,
                &command.operation_id,
                &result.upserted_preview_ids,
                canonical_only,
            )
            .await
            {
                Ok(items) => result.upserted_preview_items = items,
                Err(bill_analyser_db::DbError::InvalidOperation(message))
                    if message.contains("reclassification pending") =>
                {
                    return route_response(import_v2_error_response(
                        409,
                        "Decision group reclassification is pending",
                    ));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
            let replacement_removed_ids = if canonical_only {
                result.upserted_preview_ids.clone()
            } else {
                result.removed_preview_ids.clone()
            };
            route_response(import_v2_data_response(json!({
                "sessionId": command.session_id,
                "groupId": result.group_id,
                "groupVersion": result.group_version,
                "decisionStatus": result.decision_status,
                "removed": result.removed_preview_ids,
                "upserted": result.upserted_preview_ids,
                "removedPreviewIds": replacement_removed_ids,
                "upsertedPreviewItems": result.upserted_preview_items,
            })))
        }
        Ok(ImportDecisionGroupCommandResult::Conflict) => {
            route_response(import_v2_error_response(409, "Decision group state conflict"))
        }
        Ok(ImportDecisionGroupCommandResult::MaterializationPending) => route_response(
            import_v2_error_response(409, "Decision group materialization is pending"),
        ),
        Ok(ImportDecisionGroupCommandResult::MaterializationFailed) => route_response(
            import_v2_error_response(500, "Decision group materialization failed"),
        ),
        Ok(ImportDecisionGroupCommandResult::NotFound) => {
            route_response(import_v2_error_response(404, "Import session not found"))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

fn decision_group_command_from_object(
    object: &serde_json::Map<String, Value>,
    session_id: String,
    group_id: i64,
) -> Result<(ImportDecisionGroupCommand, std::collections::HashSet<i64>), ImportV2RouteResponse> {
    let decision = first_value(object, &["decision"]).and_then(value_to_text).unwrap_or_default();
    let operation_id = first_value(object, &["operationId", "operation_id"]).and_then(value_to_text).unwrap_or_default();
    if operation_id.trim().is_empty() {
        return Err(import_v2_error_response(400, "Missing operationId"));
    }
    let Some(expected_group_version) = first_value(object, &["expectedGroupVersion", "expected_group_version"]).and_then(value_to_i64) else {
        return Err(import_v2_error_response(400, "Missing expectedGroupVersion"));
    };
    let Some(version_items) = first_value(object, &["expectedPreviewVersions", "expected_preview_versions"]).and_then(Value::as_array) else {
        return Err(import_v2_error_response(400, "Invalid expectedPreviewVersions"));
    };
    let mut expected_preview_versions = Vec::with_capacity(version_items.len());
    let mut preview_ids = std::collections::HashSet::new();
    for value in version_items {
        let Some(item) = value.as_object() else {
            return Err(import_v2_error_response(400, "Invalid expectedPreviewVersions item"));
        };
        let (Some(preview_row_id), Some(version)) = (
            first_value(item, &["previewRowId", "preview_row_id"]).and_then(value_to_i64),
            first_value(item, &["version"]).and_then(value_to_i64),
        ) else {
            return Err(import_v2_error_response(400, "Invalid expectedPreviewVersions item"));
        };
        if preview_row_id <= 0 || version <= 0 || !preview_ids.insert(preview_row_id) {
            return Err(import_v2_error_response(400, "Duplicate or invalid preview member version"));
        }
        expected_preview_versions.push(ImportDecisionPreviewVersion { preview_row_id, version });
    }
    Ok((ImportDecisionGroupCommand { operation_id, session_id, group_id, decision, expected_group_version, expected_preview_versions }, preview_ids))
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
    let expected_row_version = expected_state.expected_row_version;
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
    if expected_state_conflicts_for_legacy_route(&preview, &expected_state) {
        return route_response(preview_state_conflict_response());
    }
    let groups = match get_import_decision_groups_by_session(
        runtime.connection(),
        &preview.session_id,
        user_id,
    ) {
        Ok(groups) => groups
            .into_iter()
            .filter(|group| {
                group.group_type.contains("transfer")
                    && group.members.iter().any(|member| member.preview_row_id == Some(preview_id))
            })
            .collect::<Vec<_>>(),
        Err(error) => return route_response(db_error_response(error)),
    };
    let group = match unique_legacy_transfer_group(&groups, preview_id) {
        Ok(group) => group,
        Err(message) => return route_response(import_v2_error_response(409, message)),
    };
    let command = ImportDecisionGroupCommand {
        operation_id: format!("legacy-transfer-{preview_id}-{}-v{}", decision_name(decision), group.version),
        session_id: preview.session_id.clone(),
        group_id: group.id,
        decision: decision_name(decision).to_string(),
        expected_group_version: group.version,
        expected_preview_versions: decision_group_preview_versions(
            group,
            expected_row_version.map(|version| (preview_id, version)),
        ),
    };
    match apply_import_decision_group_command(runtime.connection(), user_id, &command) {
        Ok(ImportDecisionGroupCommandResult::Applied(mut result)) => {
            let canonical_only = result.upserted_preview_items.is_empty();
            match decision_group_response_items(
                &mut runtime,
                &preview.session_id,
                user_id,
                command.group_id,
                &command.operation_id,
                &result.upserted_preview_ids,
                canonical_only,
            )
            .await
            {
                Ok(items) => result.upserted_preview_items = items,
                Err(bill_analyser_db::DbError::InvalidOperation(message))
                    if message.contains("reclassification pending") =>
                {
                    return route_response(import_v2_error_response(
                        409,
                        "Decision group reclassification is pending",
                    ));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
            let replacement_removed_ids = if canonical_only {
                result.upserted_preview_ids.clone()
            } else {
                result.removed_preview_ids.clone()
            };
            route_response(import_v2_data_response(json!({
                "decision": decision_name(decision),
                "sessionId": preview.session_id,
                "groupId": result.group_id,
                "groupVersion": result.group_version,
                "removed": result.removed_preview_ids,
                "upserted": result.upserted_preview_ids,
                "removedPreviewIds": replacement_removed_ids,
                "upsertedPreviewItems": result.upserted_preview_items,
            })))
        },
        Ok(ImportDecisionGroupCommandResult::Conflict) => {
            if let Some(expected) = expected_row_version {
                match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
                    Ok(Some(latest_row)) if latest_row.version != expected => {
                        return route_response(preview_row_version_conflict_response(
                            expected,
                            latest_row,
                        ));
                    }
                    Ok(_) => {}
                    Err(error) => return route_response(db_error_response(error)),
                }
            }
            route_response(import_v2_error_response(409, "Decision group state conflict"))
        }
        Ok(ImportDecisionGroupCommandResult::MaterializationPending) =>
            route_response(import_v2_error_response(409, "Decision group state conflict")),
        Ok(ImportDecisionGroupCommandResult::MaterializationFailed) =>
            route_response(import_v2_error_response(500, "Decision group materialization failed")),
        Ok(ImportDecisionGroupCommandResult::NotFound) =>
            route_response(import_v2_error_response(404, "Import session not found")),
        Err(error) => route_response(db_error_response(error)),
    }
}

fn decision_group_preview_versions(
    group: &bill_analyser_db::ImportDecisionGroupRow,
    expected_anchor: Option<(i64, i64)>,
) -> Vec<ImportDecisionPreviewVersion> {
    let mut versions = group
        .members
        .iter()
        .filter_map(|member| Some((member.preview_row_id?, member.version)))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .map(|(preview_row_id, version)| ImportDecisionPreviewVersion {
            preview_row_id,
            version,
        })
        .collect::<Vec<_>>();
    if let Some((preview_row_id, version)) = expected_anchor {
        if let Some(anchor) = versions
            .iter_mut()
            .find(|item| item.preview_row_id == preview_row_id)
        {
            anchor.version = version;
        }
    }
    versions
}

async fn decision_group_response_items(
    runtime: &mut ImportRuntime,
    session_id: &str,
    user_id: UserId,
    group_id: i64,
    operation_id: &str,
    preview_ids: &[i64],
    load_canonical_only: bool,
) -> Result<Vec<Value>, bill_analyser_db::DbError> {
    if !load_canonical_only {
        return reclassify_dematerialized_preview_items(
            runtime,
            session_id,
            user_id,
            group_id,
            operation_id,
            preview_ids,
        )
        .await;
    }
    let expected_ids = preview_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let items = get_preview_by_session(runtime.connection(), session_id, user_id, false)?
        .into_iter()
        .filter(|row| expected_ids.contains(&row.id))
        .map(preview_row_to_value)
        .collect::<Vec<_>>();
    if items.len() != expected_ids.len() {
        return Err(bill_analyser_db::DbError::InvalidOperation(
            "decision group canonical preview state is incomplete".into(),
        ));
    }
    Ok(items)
}

fn unique_legacy_transfer_group(
    groups: &[bill_analyser_db::ImportDecisionGroupRow],
    preview_id: i64,
) -> Result<&bill_analyser_db::ImportDecisionGroupRow, &'static str> {
    let mut matches = groups.iter().filter(|group| {
        group.group_type.contains("transfer")
            && group.members.iter().any(|member| member.preview_row_id == Some(preview_id))
    });
    let Some(group) = matches.next() else {
        return Err("Decision group materialization is pending");
    };
    if matches.next().is_some() {
        return Err("Preview belongs to multiple decision groups");
    }
    Ok(group)
}

fn expected_state_conflicts_for_legacy_route(
    preview: &ImportPreviewRow,
    expected: &ImportPreviewExpectedState,
) -> bool {
    expected.session_id.as_ref().is_some_and(|value| value != &preview.session_id)
        || expected.preview_type.as_ref().is_some_and(|value| value != &preview.preview_type)
        || expected.preview_main_category.as_ref().is_some_and(|value| value != &preview.preview_main_category)
        || expected.preview_sub_category.as_ref().is_some_and(|value| value != &preview.preview_sub_category)
}
