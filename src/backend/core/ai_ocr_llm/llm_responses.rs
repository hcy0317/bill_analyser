use serde_json::{json, Map, Value};

use super::types::AiRouteResponse;

pub fn build_llm_contract_error_response(
    message: &str,
    code: &str,
    status_code: u16,
) -> AiRouteResponse {
    AiRouteResponse {
        status_code,
        body: json!({
            "success": false,
            "error": message,
            "code": code,
            "error_code": code,
        }),
    }
}

pub fn build_llm_preview_recommend_response(session_id: &str, suggestions: Vec<Value>) -> Value {
    json!({
        "success": true,
        "data": {
            "session_id": session_id,
            "count": suggestions.len(),
            "suggestions": suggestions,
        },
    })
}

pub fn build_llm_candidate_list_response(candidates: Vec<Value>, total: i64) -> Value {
    json!({
        "success": true,
        "data": candidates,
        "total": total,
    })
}

pub fn build_llm_candidate_reject_response(rejected: bool) -> Value {
    json!({
        "success": true,
        "data": {
            "rejected": rejected,
        },
    })
}

pub fn llm_review_endpoint_requires_live_provider(endpoint: &str) -> bool {
    let normalized = endpoint.trim_matches('/');
    if matches!(
        normalized,
        "preview-recommend/accept"
            | "preview-recommend/reject"
            | "candidates/accept"
            | "candidates/reject"
    ) {
        return false;
    }

    let endpoint_parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    !matches!(
        endpoint_parts.as_slice(),
        ["candidates", _, "accept"] | ["candidates", _, "reject"]
    )
}

pub fn build_llm_analysis_response(candidates: Vec<Value>, context: &Value) -> Value {
    let context_object = context.as_object();
    let session_id = context_object
        .and_then(|item| item.get("session_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let bill_ids_present = context_object
        .and_then(|item| item.get("bill_ids"))
        .is_some_and(|value| !value.is_null());
    let mode = if !session_id.is_empty() {
        "import_session"
    } else if bill_ids_present {
        "persisted_selection"
    } else {
        "persisted_uncategorized"
    };
    let mut response_payload = Map::new();
    response_payload.insert("candidates_created".to_string(), json!(candidates.len()));
    response_payload.insert("candidates".to_string(), Value::Array(candidates.clone()));
    response_payload.insert("mode".to_string(), json!(mode));
    if !session_id.is_empty() {
        response_payload.insert("session_id".to_string(), json!(session_id));
    }

    json!({
        "success": true,
        "data": response_payload,
        "total": candidates.len(),
    })
}
