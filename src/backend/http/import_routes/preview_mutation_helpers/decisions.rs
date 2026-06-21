/// 解析人工决策 payload，统一 accept/reject/clear 等动作名到内部 decision enum。
fn decision_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewDecision, ImportV2RouteResponse> {
    let decision = first_value(object, &["decision"])
        .and_then(value_to_text)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match decision.as_str() {
        "accept" | "accepted" => Ok(ImportPreviewDecision::Accept),
        "reject" | "rejected" => Ok(ImportPreviewDecision::Reject),
        "clear" | "cleared" => Ok(ImportPreviewDecision::Clear),
        _ => Err(import_v2_error_response(400, "Invalid decision")),
    }
}

fn decision_name(decision: ImportPreviewDecision) -> &'static str {
    match decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    }
}

fn recurring_candidate_from_payload(
    object: &Map<String, Value>,
    recurring_id: i64,
) -> Option<ImportPreviewRecurringCandidate> {
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
    .and_then(Value::as_object)?;
    Some(ImportPreviewRecurringCandidate {
        id: first_value(candidate, &["id", "recurringId", "recurring_id"])
            .and_then(value_to_i64)
            .unwrap_or(recurring_id),
        name: first_value(candidate, &["name", "recurringName", "recurring_name"])
            .and_then(value_to_text)
            .unwrap_or_default(),
        match_score: first_value(candidate, &["matchScore", "match_score"])
            .and_then(value_to_f64)
            .unwrap_or_default(),
        match_reasons: match_reasons_from_value(first_value(
            candidate,
            &["matchReasons", "match_reasons"],
        )),
        matched_occurrence_date: first_value(
            candidate,
            &["matchedOccurrenceDate", "matched_occurrence_date"],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
    })
}

fn recurring_candidate_count_from_payload(
    object: &Map<String, Value>,
    target_candidate: Option<&ImportPreviewRecurringCandidate>,
) -> i64 {
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
    .unwrap_or_else(|| i64::from(target_candidate.is_some()))
}

fn match_reasons_from_value(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(value_to_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect(),
        Some(value) => value_to_text(value)
            .unwrap_or_default()
            .split(['|', ','])
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    }
}

/// 把 preview 决策结果投影为 HTTP response，保留 not_found/conflict 的前端可识别状态。
fn preview_decision_result_response(
    result: ImportPreviewDecisionResult,
    extra: Value,
) -> ImportV2RouteResponse {
    if result.state_conflict {
        return preview_state_conflict_response();
    }
    if result.invalid_recurring_id {
        return import_v2_error_response(400, "Invalid recurringId");
    }
    let Some(preview) = result.preview else {
        return import_v2_error_response(404, "Preview bill not found");
    };
    let preview_id = preview.id;
    let session_id = preview.session_id.clone();
    let preview_item = preview_row_to_value(preview);
    let mut data = json!({
        "previewId": preview_id,
        "sessionId": session_id,
        "previewItem": preview_item.clone(),
        "preview": [preview_item],
    });
    if let (Some(data), Some(extra)) = (data.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            data.insert(key.clone(), value.clone());
        }
    }
    import_v2_data_response(data)
}
