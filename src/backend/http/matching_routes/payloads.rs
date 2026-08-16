async fn matching_session_candidates_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    session_id: &str,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_matching_session_candidates_payload(
        runtime.pool(),
        user_id,
        session_id,
    )
    .await
    {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Import session not found"),
        Err(error) => matching_error_response(error),
    };
}

async fn matching_bill_candidates_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    bill_id: i64,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_matching_bill_candidates_payload(runtime.pool(), user_id, bill_id)
        .await
    {
        Ok(Some(data)) => success_data(StatusCode::OK, data),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Bill not found"),
        Err(error) => matching_error_response(error),
    };
}

fn matching_candidate_action_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    candidate_id: String,
    action: &str,
    payload: Option<Json<Value>>,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return error_response(StatusCode::BAD_REQUEST, "Invalid request");
    };
    let request = match preview_action_request_from_payload(object) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let context = match preview_matching_action_context(&candidate_id, action, &request) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_postgres_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let result = match context.kind.as_str() {
        "learning" => bill_analyser_db::apply_preview_learning_decision(
            runtime.pool(),
            &context.session_id,
            context.preview_id,
            user_id,
            context.decision,
            request.learning_apply.as_ref(),
            request.expected_state.as_ref(),
        ),
        "llm" => bill_analyser_db::import_staging::review_preview_llm_matching_action(
            runtime.pool(),
            &bill_analyser_db::ImportPreviewLlmReviewRequest {
                session_id: &context.session_id,
                preview_id: context.preview_id,
                user_id,
                decision: context.decision,
                suggestion: None,
                user_correction_category: None,
                user_correction_account: None,
                expected_state: request.expected_state.as_ref(),
            },
        ),
        _ => unreachable!("preview action context restricts candidate kinds"),
    };

    match result {
        Ok(result) if result.state_conflict => {
            error_response(StatusCode::CONFLICT, "Preview state is stale")
        }
        Ok(result) if result.invalid_recurring_id => {
            error_response(StatusCode::BAD_REQUEST, "Invalid recurring candidate")
        }
        Ok(result) => {
            let Some(preview) = result.preview else {
                return error_response(StatusCode::NOT_FOUND, "Preview candidate not found");
            };
            if preview.session_id != context.session_id {
                return error_response(StatusCode::CONFLICT, "Preview state is stale");
            }
            let review_status = preview_action_review_status(
                &preview.preview_matching_feedback,
                &context.kind,
                action,
            );
            let preview_item = matching_preview_item_value(preview);
            let mut action_result = Map::from_iter([
                ("candidate_id".to_string(), json!(candidate_id)),
                ("action".to_string(), json!(action)),
                ("session_id".to_string(), json!(context.session_id)),
                ("preview_id".to_string(), json!(context.preview_id)),
                ("review_status".to_string(), json!(review_status)),
            ]);
            if request.response_mode_preview_item {
                action_result.insert("preview_item".to_string(), preview_item);
            } else {
                action_result.insert("preview".to_string(), json!([preview_item]));
            }
            success_data(
                StatusCode::OK,
                bill_analyser_core::matching::build_matching_candidate_action_payload(
                    &candidate_id,
                    &action_result,
                ),
            )
        }
        Err(DbError::PreviewVersionConflict {
            preview_id,
            expected,
            ..
        }) if context.kind == "learning" && preview_id == context.preview_id => {
            match get_preview_bill_by_id(runtime.pool(), preview_id, user_id) {
                Ok(Some(latest)) if latest.session_id == context.session_id => {
                    preview_row_version_conflict_response(expected, latest)
                }
                Ok(_) => error_response(StatusCode::NOT_FOUND, "Preview candidate not found"),
                Err(error) => matching_error_response(error.into()),
            }
        }
        Err(error) => matching_error_response(error.into()),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PreviewMatchingActionContext {
    kind: String,
    session_id: String,
    preview_id: i64,
    decision: bill_analyser_db::ImportPreviewDecision,
}

fn preview_matching_action_context(
    candidate_id: &str,
    action: &str,
    request: &PreviewMatchingActionRequest,
) -> RouteResult<PreviewMatchingActionContext> {
    let Some(descriptor) = bill_analyser_core::matching::parse_matching_candidate_id(candidate_id)
    else {
        return Err(Box::new(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid matching candidate",
        )));
    };
    if descriptor.scope != "preview" || !matches!(descriptor.kind.as_str(), "learning" | "llm") {
        return Err(Box::new(error_response(
            StatusCode::CONFLICT,
            "Matching candidate action is not available for this candidate",
        )));
    }
    let Some(preview_id) = descriptor.preview_id else {
        return Err(Box::new(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid matching candidate",
        )));
    };
    let session_id = request
        .expected_state
        .as_ref()
        .and_then(|expected| expected.session_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Box::new(error_response(
                StatusCode::BAD_REQUEST,
                "sessionId is required",
            ))
        })?;
    let decision = match action.trim().to_ascii_lowercase().as_str() {
        "accept" => bill_analyser_db::ImportPreviewDecision::Accept,
        "reject" => bill_analyser_db::ImportPreviewDecision::Reject,
        "clear" => bill_analyser_db::ImportPreviewDecision::Clear,
        _ => {
            return Err(Box::new(error_response(
                StatusCode::BAD_REQUEST,
                "Invalid matching candidate action",
            )));
        }
    };
    Ok(PreviewMatchingActionContext {
        kind: descriptor.kind,
        session_id,
        preview_id,
        decision,
    })
}

fn preview_action_review_status(feedback: &Value, kind: &str, action: &str) -> String {
    feedback
        .get(kind)
        .and_then(Value::as_object)
        .and_then(|value| first_value(value, &["review_status", "status"]))
        .and_then(value_to_text)
        .unwrap_or_else(|| {
            if action.eq_ignore_ascii_case("clear") {
                "cleared".to_string()
            } else {
                action.trim().to_ascii_lowercase()
            }
        })
}

fn matching_preview_item_value(preview: bill_analyser_db::ImportPreviewRow) -> Value {
    let mut value = serde_json::to_value(preview).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        bill_analyser_core::attach_import_preview_matching_payload(object);
        bill_analyser_core::attach_import_preview_state_snapshot_to_canonical_row(object);
    }
    value
}

fn preview_action_request_from_payload(
    object: &Map<String, Value>,
) -> RouteResult<PreviewMatchingActionRequest> {
    let expected_state = expected_state_from_payload(object)?.or_else(|| {
        first_value(object, &["sessionId", "session_id"])
            .and_then(value_to_text)
            .filter(|value| !value.trim().is_empty())
            .map(|session_id| ImportPreviewExpectedState {
                session_id: Some(session_id),
                ..ImportPreviewExpectedState::default()
            })
    });
    Ok(PreviewMatchingActionRequest {
        expected_state,
        response_mode_preview_item: response_mode_is_preview_item(object),
        reviewed_type: first_value(object, &["reviewedType", "reviewed_type", "type"])
            .and_then(value_to_text),
        recurring_id: first_value(object, &["recurringId", "recurring_id"])
            .and_then(value_to_i64)
            .filter(|value| *value > 0),
        recurring_candidate_count: recurring_candidate_count_from_payload(object),
        recurring_candidate: recurring_candidate_from_payload(object),
        learning_apply: learning_apply_from_payload(object),
    })
}

fn expected_state_from_payload(
    object: &Map<String, Value>,
) -> RouteResult<Option<ImportPreviewExpectedState>> {
    let Some(expected_state_value) = first_value(object, &["expectedState", "expected_state"])
    else {
        return Ok(None);
    };
    let Some(expected_state) = expected_state_value.as_object() else {
        return Err(Box::new(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid request",
        )));
    };
    Ok(Some(ImportPreviewExpectedState {
        session_id: first_value(expected_state, &["sessionId", "session_id"])
            .and_then(value_to_text),
        expected_row_version: positive_row_version_field(
            expected_state,
            &["rowVersion", "row_version"],
        )?,
        review_status: first_value(
            expected_state,
            &["reviewStatus", "review_status", "status"],
        )
        .and_then(value_to_text)
        .map(|value| value.trim().to_ascii_lowercase()),
        preview_type: first_value(expected_state, &["type", "previewType", "preview_type"])
            .and_then(value_to_text)
            .map(|value| normalize_preview_type_text(&value)),
        preview_category_id: optional_id_field_from_object(
            expected_state,
            &[
                "categoryId",
                "category_id",
                "previewCategoryId",
                "preview_category_id",
            ],
        ),
        preview_main_category: first_value(
            expected_state,
            &[
                "mainCategory",
                "previewMainCategory",
                "preview_main_category",
            ],
        )
        .and_then(value_to_text),
        preview_sub_category: first_value(
            expected_state,
            &["subCategory", "previewSubCategory", "preview_sub_category"],
        )
        .and_then(value_to_text),
        preview_recurring_id: optional_id_field_from_object(
            expected_state,
            &[
                "recurringTemplateId",
                "recurringId",
                "previewRecurringId",
                "preview_recurring_id",
            ],
        ),
        preview_source_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "sourceAccountId",
                "previewSourceAccountId",
                "preview_source_account_id",
            ],
        ),
        preview_destination_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "destinationAccountId",
                "previewDestinationAccountId",
                "preview_destination_account_id",
            ],
        ),
        preview_matching_feedback: first_value(
            expected_state,
            &[
                "matchingFeedback",
                "previewMatchingFeedback",
                "preview_matching_feedback",
            ],
        )
        .cloned(),
    }))
}

fn positive_row_version_field(
    object: &Map<String, Value>,
    keys: &[&str],
) -> RouteResult<Option<i64>> {
    let Some(value) = first_value(object, keys) else {
        return Ok(None);
    };
    value
        .as_i64()
        .filter(|version| *version > 0)
        .map(Some)
        .ok_or_else(|| Box::new(error_response(StatusCode::BAD_REQUEST, "Invalid request")))
}

fn learning_apply_from_payload(object: &Map<String, Value>) -> Option<ImportPreviewLearningApply> {
    let apply = ImportPreviewLearningApply {
        preview_type: first_value(object, &["type", "previewType", "preview_type"])
            .and_then(value_to_text)
            .map(|value| normalize_preview_type_text(&value)),
        preview_main_category: first_value(
            object,
            &[
                "mainCategory",
                "previewMainCategory",
                "preview_main_category",
            ],
        )
        .and_then(value_to_text),
        preview_sub_category: first_value(
            object,
            &["subCategory", "previewSubCategory", "preview_sub_category"],
        )
        .and_then(value_to_text),
        preview_source_account_id: optional_id_field_from_object(
            object,
            &[
                "sourceAccountId",
                "previewSourceAccountId",
                "preview_source_account_id",
            ],
        ),
        preview_destination_account_id: optional_id_field_from_object(
            object,
            &[
                "destinationAccountId",
                "previewDestinationAccountId",
                "preview_destination_account_id",
            ],
        ),
        rule_id: first_value(object, &["ruleId", "rule_id"])
            .and_then(value_to_i64)
            .filter(|value| *value > 0),
    };
    if apply.preview_type.is_some()
        || apply.preview_main_category.is_some()
        || apply.preview_sub_category.is_some()
        || apply.preview_source_account_id.is_some()
        || apply.preview_destination_account_id.is_some()
        || apply.rule_id.is_some()
    {
        Some(apply)
    } else {
        None
    }
}

fn recurring_candidate_from_payload(
    object: &Map<String, Value>,
) -> Option<ImportPreviewRecurringCandidate> {
    let recurring_id = first_value(object, &["recurringId", "recurring_id"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)?;
    let candidate = first_value(
        object,
        &[
            "candidate",
            "targetCandidate",
            "target_candidate",
            "recurringCandidate",
            "recurring_candidate",
        ],
    )
    .and_then(Value::as_object);
    Some(ImportPreviewRecurringCandidate {
        id: candidate
            .and_then(|candidate| first_value(candidate, &["id", "recurringId", "recurring_id"]))
            .and_then(value_to_i64)
            .filter(|value| *value > 0)
            .unwrap_or(recurring_id),
        name: candidate
            .and_then(|candidate| {
                first_value(candidate, &["name", "recurringName", "recurring_name"])
            })
            .and_then(value_to_text)
            .unwrap_or_default(),
        match_score: candidate
            .and_then(|candidate| first_value(candidate, &["matchScore", "match_score"]))
            .and_then(value_to_f64)
            .unwrap_or_default(),
        match_reasons: candidate
            .and_then(|candidate| first_value(candidate, &["matchReasons", "match_reasons"]))
            .map(match_reasons_from_value)
            .unwrap_or_default(),
        matched_occurrence_date: candidate
            .and_then(|candidate| {
                first_value(
                    candidate,
                    &[
                        "matchedOccurrenceDate",
                        "matched_occurrence_date",
                        "matchedDate",
                        "matched_date",
                    ],
                )
            })
            .and_then(value_to_text)
            .unwrap_or_default(),
    })
}

fn recurring_candidate_count_from_payload(object: &Map<String, Value>) -> i64 {
    first_value(
        object,
        &[
            "candidateCount",
            "candidate_count",
            "recurringCandidateCount",
            "previewRecurringCandidateCount",
            "preview_recurring_candidate_count",
        ],
    )
    .and_then(value_to_i64)
    .unwrap_or_else(|| {
        i64::from(
            first_value(object, &["recurringId", "recurring_id"])
                .and_then(value_to_i64)
                .is_some_and(|value| value > 0),
        )
    })
}

fn optional_id_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Option<Option<i64>> {
    first_value(object, keys).map(|value| match value_to_i64(value) {
        Some(value) if value > 0 => Some(value),
        _ => None,
    })
}

fn reconciliation_query_to_map(query: ReconciliationCandidatesQuery) -> Map<String, Value> {
    let mut map = Map::new();
    insert_query_string(&mut map, "candidateType", query.candidate_type);
    insert_query_string(&mut map, "status", query.status);
    insert_query_string(&mut map, "sessionId", query.session_id);
    insert_query_string(&mut map, "previewId", query.preview_id);
    insert_query_string(&mut map, "billId", query.bill_id);
    insert_query_string(&mut map, "limit", query.limit);
    map
}

#[tracing::instrument(level = "debug", skip_all)]
fn insert_query_string(map: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        map.insert(key.to_string(), json!(value));
    }
}
