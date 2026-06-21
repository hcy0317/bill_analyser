pub fn build_matching_candidate_action_payload(
    candidate_id: &str,
    result: &Map<String, Value>,
) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_matching_candidate_action_payload", "business operation entered");
    let mut payload = json!({
        "candidateId": value_to_string(result.get("candidate_id")).if_empty(candidate_id),
        "action": value_to_string(result.get("action")),
    });
    let object = payload.as_object_mut().expect("json object");
    insert_i64_if_present(object, "previewId", result.get("preview_id"));
    insert_string_if_present(object, "sessionId", result.get("session_id"));
    insert_i64_if_present(object, "recurringId", result.get("recurring_id"));
    insert_i64_if_present(object, "keptBillId", result.get("keptBillId"));
    insert_i64_if_present(object, "mergedBillId", result.get("mergedBillId"));
    insert_string_if_present(object, "reviewStatus", result.get("review_status"));
    insert_string_if_present(object, "effect", result.get("effect"));
    if let Some(value) = result.get("suppressed") {
        object.insert(
            "suppressed".to_string(),
            json!(value.as_bool().unwrap_or(false)),
        );
    }
    for (source_key, target_key) in [
        ("preview_item", "previewItem"),
        ("projection", "projection"),
    ] {
        if let Some(value) = result.get(source_key).filter(|value| value.is_object()) {
            object.insert(target_key.to_string(), value.clone());
        }
    }
    if let Some(bill) = result.get("bill").and_then(Value::as_object) {
        object.insert("bill".to_string(), serialize_bill_snapshot(bill));
    }
    if let Some(value) = result.get("preview").filter(|value| value.is_array()) {
        object.insert("preview".to_string(), value.clone());
    }
    if let Some(pair) = result.get("pair").and_then(Value::as_object) {
        object.insert("pair".to_string(), serialize_bill_pair(pair));
    }
    payload
}
