// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

fn build_llm_feedback_payload(
    suggestion: &ImportPreviewLlmSuggestion,
    review_status: &str,
    suppressed: bool,
    previous_preview: &Value,
    applied_preview: &Value,
) -> Value {
    serde_json::json!({
        "review_status": review_status,
        "suppressed": suppressed,
        "suggested_main_category": suggestion.suggested_main_category,
        "suggested_sub_category": suggestion.suggested_sub_category,
        "suggested_source_account": suggestion.suggested_source_account,
        "suggested_destination_account": suggestion.suggested_destination_account,
        "confidence": suggestion.confidence,
        "reason": suggestion.reason,
        "previous_preview": previous_preview,
        "applied_preview": applied_preview,
    })
}

fn truncated_llm_memory_prompt_text(prompt_text: &str) -> String {
    if prompt_text.len() <= LLM_MEMORY_PROMPT_TEXT_MAX_BYTES {
        return prompt_text.to_string();
    }
    prompt_text
        .char_indices()
        .take_while(|(index, _)| *index < LLM_MEMORY_PROMPT_TEXT_MAX_BYTES)
        .map(|(_, ch)| ch)
        .collect()
}

fn llm_suggestion_payload(suggestion: &ImportPreviewLlmSuggestion) -> Value {
    serde_json::json!({
        "suggested_main_category": suggestion.suggested_main_category,
        "suggested_sub_category": suggestion.suggested_sub_category,
        "suggested_source_account": suggestion.suggested_source_account,
        "suggested_destination_account": suggestion.suggested_destination_account,
        "confidence": suggestion.confidence,
        "reason": suggestion.reason,
    })
}

fn llm_suggestion_from_feedback(feedback: &Value) -> ImportPreviewLlmSuggestion {
    ImportPreviewLlmSuggestion {
        suggested_main_category: snapshot_text(feedback, "suggested_main_category"),
        suggested_sub_category: snapshot_text(feedback, "suggested_sub_category"),
        suggested_source_account: snapshot_text(feedback, "suggested_source_account"),
        suggested_destination_account: snapshot_text(feedback, "suggested_destination_account"),
        resolved_source_account_id: None,
        resolved_destination_account_id: None,
        confidence: snapshot_f64(feedback, "confidence"),
        reason: snapshot_text(feedback, "reason"),
    }
}

fn optional_non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn json_option_text(value: Option<&Value>) -> Option<String> {
    value.map(Value::to_string)
}

fn snapshot_text(snapshot: &Value, field: &str) -> String {
    snapshot
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn snapshot_i64(snapshot: &Value, field: &str) -> i64 {
    snapshot
        .get(field)
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn snapshot_f64(snapshot: &Value, field: &str) -> f64 {
    snapshot
        .get(field)
        .and_then(Value::as_f64)
        .unwrap_or_default()
}

fn snapshot_optional_i64(snapshot: &Value, field: &str) -> Option<i64> {
    snapshot.get(field).and_then(|value| {
        if value.is_null() {
            None
        } else {
            value.as_i64()
        }
    })
}

fn snapshot_account_id(snapshot: &Value, field: &str) -> Option<i64> {
    snapshot.get(field).and_then(|value| {
        if value.is_null() {
            None
        } else if let Some(number) = value.as_i64() {
            Some(number)
        } else {
            value.as_str().and_then(parse_positive_i64)
        }
    })
}

fn normalize_rule_id(rule_id: Option<i64>) -> Option<i64> {
    rule_id.filter(|value| *value > 0)
}

fn feedback_rule_id(feedback: &Value) -> Option<i64> {
    feedback.get("rule_id").and_then(|value| {
        if let Some(number) = value.as_i64() {
            normalize_rule_id(Some(number))
        } else {
            value.as_str().and_then(parse_positive_i64)
        }
    })
}
