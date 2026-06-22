// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Map, Value};

use super::types::AiRouteResponse;

/// 构建 LLM 合同错误响应，同时保留 code 与 error_code 以兼容前端旧字段。
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

/// 构建导入预览 LLM 推荐响应，按 session_id 回传建议数量和建议列表。
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

/// 构建候选列表响应，保持 data 数组和 total 分页字段的接口合同。
pub fn build_llm_candidate_list_response(candidates: Vec<Value>, total: i64) -> Value {
    json!({
        "success": true,
        "data": candidates,
        "total": total,
    })
}

/// 构建候选拒绝响应，明确返回本次状态更新是否成功。
pub fn build_llm_candidate_reject_response(rejected: bool) -> Value {
    json!({
        "success": true,
        "data": {
            "rejected": rejected,
        },
    })
}

/// 判断指定 review endpoint 是否需要真实 provider，接受/拒绝类动作只更新本地状态。
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

/// 构建 LLM 分析响应，按上下文区分导入 session、持久化选择或未分类交易模式。
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
