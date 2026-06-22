use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmMemoryEventContract {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub user_id: i64,
    pub session_id: Option<String>,
    pub preview_id: Option<i64>,
    pub event_type: String,
    pub decision: Option<String>,
    pub prompt_text: Option<String>,
    pub llm_response_raw: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub suggested_main_category: Option<String>,
    pub suggested_sub_category: Option<String>,
    pub suggested_source_account: Option<String>,
    pub suggested_destination_account: Option<String>,
    pub confidence: f64,
    pub user_correction_category: Option<String>,
    pub user_correction_account: Option<String>,
    pub snapshot_before: Option<String>,
    pub snapshot_after: Option<String>,
    pub metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearningRouteResponse {
    pub status_code: u16,
    pub body: Value,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn learning_error_response(status_code: u16, error: &str) -> LearningRouteResponse {
    LearningRouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn learning_data_response<T>(data: T) -> LearningRouteResponse
where
    T: Serialize,
{
    LearningRouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn learning_center_page_response(
    items: Vec<Value>,
    total: i64,
    limit: i64,
    offset: i64,
) -> LearningRouteResponse {
    learning_data_response(json!({
        "items": items,
        "total": total,
        "limit": limit.min(1000),
        "offset": offset.max(0),
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn learning_rules_page_response(
    result: Vec<Value>,
    total_count: i64,
    page: i64,
    page_size: i64,
) -> LearningRouteResponse {
    let effective_page = page.max(1);
    let mut effective_page_size = page_size;
    if effective_page_size == 0 {
        effective_page_size = 100;
    }
    effective_page_size = effective_page_size.clamp(-1, 500);
    let total_pages = if effective_page_size > 0 {
        ((total_count + effective_page_size - 1) / effective_page_size).max(1)
    } else {
        1
    };
    let effective_page = if total_count > 0 {
        effective_page.min(total_pages)
    } else {
        1
    };
    LearningRouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": result,
            "totalCount": total_count,
            "page": effective_page,
            "pageSize": effective_page_size,
            "totalPages": total_pages,
        }),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_preview_ids(value: Option<&Value>) -> Result<Option<Vec<i64>>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Value::Array(values) = value else {
        return Err("previewIds must be an array");
    };
    let mut result = Vec::new();
    for value in values {
        match value {
            Value::Number(number) => {
                let Some(preview_id) = number.as_i64() else {
                    return Err("previewIds must contain positive integers");
                };
                if preview_id <= 0 {
                    return Err("previewIds must contain positive integers");
                }
                result.push(preview_id);
            }
            _ => return Err("previewIds must contain positive integers"),
        }
    }
    Ok(Some(result))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_learning_suggestion_ids(value: Option<&Value>) -> Result<Vec<i64>, String> {
    let Some(Value::Array(values)) = value else {
        return Err("suggestionIds must be a non-empty array".to_string());
    };
    if values.is_empty() {
        return Err("suggestionIds must be a non-empty array".to_string());
    }
    if values.len() > 100 {
        return Err("batch size must not exceed 100".to_string());
    }

    let mut seen = BTreeMap::new();
    let mut result = Vec::new();
    for value in values {
        let suggestion_id = coerce_json_int(value)
            .map_err(|message| format!("Invalid suggestionIds: {message}"))?;
        if seen.insert(suggestion_id, ()).is_none() {
            result.push(suggestion_id);
        }
    }
    Ok(result)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn learning_batch_accept_response(
    accepted: Vec<Value>,
    failed: Vec<Value>,
) -> LearningRouteResponse {
    let accepted_count = accepted.len();
    let failed_count = failed.len();
    learning_data_response(json!({
        "accepted": accepted,
        "failed": failed,
        "acceptedCount": accepted_count,
        "failedCount": failed_count,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn llm_memory_events_success(
    events: Vec<LlmMemoryEventContract>,
    total: i64,
) -> LearningRouteResponse {
    LearningRouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": events, "total": total}),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn llm_error_response(status_code: u16, message: &str, code: &str) -> LearningRouteResponse {
    LearningRouteResponse {
        status_code,
        body: json!({
            "success": false,
            "error": message,
            "code": code,
            "error_code": code,
        }),
    }
}
