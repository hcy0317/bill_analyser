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
    _candidate_id: String,
    _action: &str,
    payload: Option<Json<Value>>,
) -> Response {
    let _user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return error_response(StatusCode::BAD_REQUEST, "Invalid request");
    };
    let _request = match preview_action_request_from_payload(object) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    error_response(
        StatusCode::CONFLICT,
        "PostgreSQL matching candidate actions require a materialized candidate",
    )
}

fn preview_action_request_from_payload(
    object: &Map<String, Value>,
) -> RouteResult<PreviewMatchingActionRequest> {
    Ok(PreviewMatchingActionRequest {
        expected_state: expected_state_from_payload(object)?,
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
        preview_type: first_value(expected_state, &["type", "previewType", "preview_type"])
            .and_then(value_to_text)
            .map(|value| normalize_preview_type_text(&value)),
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
