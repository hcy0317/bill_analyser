fn parse_string_vec(raw_json: Option<&str>) -> Vec<String> {
    raw_json
        .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .unwrap_or_default()
}

fn parse_i64_csv(raw_csv: Option<&str>) -> Vec<i64> {
    raw_csv
        .unwrap_or("")
        .split(',')
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .collect()
}

fn parse_json_object(raw_json: Option<&str>) -> Value {
    raw_json
        .filter(|text| !text.trim().is_empty())
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Default::default()))
}

fn parse_optional_json_object(raw_json: Option<&str>) -> Option<Value> {
    raw_json
        .filter(|text| !text.trim().is_empty())
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
}

fn clear_transfer_matching_feedback(raw_json: Option<&str>) -> String {
    match parse_json_object(raw_json) {
        Value::Object(mut object) => {
            object.remove("transfer");
            if object.is_empty() {
                String::new()
            } else {
                Value::Object(object).to_string()
            }
        }
        _ => String::new(),
    }
}

fn preview_decision_not_found() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: false,
        invalid_recurring_id: false,
    }
}

fn preview_decision_state_conflict() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: true,
        invalid_recurring_id: false,
    }
}

fn preview_matches_expected_state(
    preview: &ImportPreviewRow,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> bool {
    let Some(expected) = expected_state else {
        return true;
    };

    if expected
        .session_id
        .as_ref()
        .is_some_and(|value| preview.session_id != *value)
    {
        return false;
    }
    if expected
        .preview_type
        .as_ref()
        .is_some_and(|value| preview.preview_type != *value)
    {
        return false;
    }
    if expected
        .preview_main_category
        .as_ref()
        .is_some_and(|value| preview.preview_main_category != *value)
    {
        return false;
    }
    if expected
        .preview_sub_category
        .as_ref()
        .is_some_and(|value| preview.preview_sub_category != *value)
    {
        return false;
    }
    if expected
        .preview_recurring_id
        .as_ref()
        .is_some_and(|value| preview.preview_recurring_id != *value)
    {
        return false;
    }
    if expected
        .preview_source_account_id
        .as_ref()
        .is_some_and(|value| preview.preview_source_account_id != *value)
    {
        return false;
    }
    if expected
        .preview_destination_account_id
        .as_ref()
        .is_some_and(|value| preview.preview_destination_account_id != *value)
    {
        return false;
    }
    if expected
        .preview_matching_feedback
        .as_ref()
        .is_some_and(|value| preview.preview_matching_feedback != *value)
    {
        return false;
    }

    true
}

fn ensure_json_object(value: &mut Value) {
    if !value.is_object() {
        *value = Value::Object(Default::default());
    }
}

fn remove_json_object_key(value: &mut Value, key: &str) {
    ensure_json_object(value);
    if let Value::Object(object) = value {
        object.remove(key);
    }
}

fn serialize_preview_matching_feedback(value: &Value) -> String {
    match value {
        Value::Object(object) if object.is_empty() => String::new(),
        Value::Object(_) => value.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod row_value_helper_tests {
    use super::*;

    fn preview_row() -> ImportPreviewRow {
        ImportPreviewRow {
            id: 1,
            session_id: "session-a".to_string(),
            user_id: 42,
            preview_date: "2026-05-01".to_string(),
            preview_type: "支出".to_string(),
            preview_amount: 12.5,
            preview_destination_amount: 0.0,
            preview_main_category: "餐饮".to_string(),
            preview_sub_category: "咖啡".to_string(),
            preview_source_account_id: Some(10),
            preview_destination_account_id: None,
            preview_counterparty: "cafe".to_string(),
            preview_payment_method: "card".to_string(),
            preview_description: "latte".to_string(),
            preview_parser_id: "wechat".to_string(),
            preview_parser_tags: vec!["wechat".to_string()],
            preview_recurring_id: Some(7),
            preview_recurring_name: "monthly".to_string(),
            preview_recurring_candidate_count: 1,
            preview_recurring_match_score: 0.8,
            preview_recurring_match_reasons: "amount".to_string(),
            preview_recurring_matched_date: "2026-05-01".to_string(),
            preview_selected: true,
            dedup_type: "remaining".to_string(),
            dedup_source_ids: vec![1],
            preview_matching_feedback: serde_json::json!({"transfer": {"review_status": "accepted"}}),
            created_at: "2026-05-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn decision_result_and_expected_state_helpers_cover_mismatch_edges() {
        let not_found = preview_decision_not_found();
        assert!(not_found.preview.is_none());
        assert!(!not_found.state_conflict);
        assert!(!not_found.invalid_recurring_id);

        let conflict = preview_decision_state_conflict();
        assert!(conflict.preview.is_none());
        assert!(conflict.state_conflict);
        assert!(!conflict.invalid_recurring_id);

        let preview = preview_row();
        assert!(preview_matches_expected_state(&preview, None));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                session_id: Some("other-session".to_string()),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_type: Some("收入".to_string()),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_main_category: Some("交通".to_string()),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_sub_category: Some("地铁".to_string()),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_recurring_id: Some(Some(99)),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_source_account_id: Some(Some(99)),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_destination_account_id: Some(Some(99)),
                ..ImportPreviewExpectedState::default()
            })
        ));
        assert!(!preview_matches_expected_state(
            &preview,
            Some(&ImportPreviewExpectedState {
                preview_matching_feedback: Some(serde_json::json!({"learning": {}})),
                ..ImportPreviewExpectedState::default()
            })
        ));
    }

    #[test]
    fn json_object_helpers_normalize_feedback_shapes() {
        let mut scalar = serde_json::json!("bad");
        ensure_json_object(&mut scalar);
        assert_eq!(scalar, serde_json::json!({}));

        let mut value = serde_json::json!({"transfer": {}, "other": true});
        remove_json_object_key(&mut value, "transfer");
        assert_eq!(value, serde_json::json!({"other": true}));
        assert_eq!(
            clear_transfer_matching_feedback(Some(r#"{"transfer":{},"other":true}"#)),
            serde_json::json!({"other": true}).to_string()
        );
        assert_eq!(clear_transfer_matching_feedback(Some(r#"{"transfer":{}}"#)), "");
        assert_eq!(serialize_preview_matching_feedback(&serde_json::json!({})), "");
        assert_eq!(serialize_preview_matching_feedback(&serde_json::json!("bad")), "");
    }
}
