// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const FEATURE_SCHEMA_VERSION: &str = "import-learning-features-v1";
pub const DEFAULT_FEATURE_DIMENSION: usize = 96;
pub const MODEL_KEY: &str = "import-learning-dual-head";
pub const MODEL_FAMILY: &str = "shared-hidden-dual-softmax-v1";
pub const MIN_TRAINING_SAMPLES: usize = 3;
pub const HIDDEN_DIMENSION: usize = 16;
pub const POLICY_VERSION: &str = "learning-green-blue-policy-v1";
pub const GREEN_CONFIDENCE_THRESHOLD: f64 = 0.52;
pub const GREEN_MARGIN_THRESHOLD: f64 = 0.02;
pub const BLUE_CONFIDENCE_THRESHOLD: f64 = 0.70;
pub const BLUE_MARGIN_THRESHOLD: f64 = 0.05;
pub const BLUE_ACCEPT_CONFIRMATION_THRESHOLD: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningTrainingSample {
    pub sample_id: i64,
    pub features: BTreeMap<String, String>,
    pub semantic_label: String,
    pub route_label: String,
}

pub fn normalize_learning_text(raw_value: Option<&Value>) -> String {
    let text = value_to_string(raw_value).trim().to_lowercase();
    collapse_whitespace(&text)
}

pub fn normalize_import_learning_text(raw_value: Option<&Value>) -> String {
    let text = value_to_string(raw_value).trim().to_lowercase();
    if text.is_empty() {
        return String::new();
    }
    let pipe_parts: Vec<String> = text
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let normalized = if pipe_parts.is_empty() {
        text
    } else {
        pipe_parts.join(" | ")
    };
    collapse_whitespace(&normalized)
}

pub fn normalize_import_learning_suggestion_id(raw_value: Option<&Value>) -> Option<i64> {
    let normalized = optional_positive_i64(raw_value);
    normalized.filter(|value| *value > 0)
}

pub fn build_composite_match_features(
    parser_id: &str,
    counterparty: &str,
    description: &str,
    payment_method: &str,
) -> Option<BTreeMap<String, String>> {
    let mut features = BTreeMap::new();
    for (key, raw_value) in [
        ("parser_id", parser_id),
        ("counterparty", counterparty),
        ("description", description),
        ("payment_method", payment_method),
    ] {
        let normalized =
            normalize_import_learning_text(Some(&Value::String(raw_value.to_string())));
        if !normalized.is_empty() {
            features.insert(key.to_string(), normalized);
        }
    }
    if features.len() < 2 {
        None
    } else {
        Some(features)
    }
}

pub fn build_composite_match_hash(
    parser_id: &str,
    counterparty: &str,
    description: &str,
    payment_method: &str,
) -> Option<String> {
    build_composite_match_features(parser_id, counterparty, description, payment_method)
        .map(|features| composite_hash_from_features(&features))
}

pub fn composite_hash_from_features(features: &BTreeMap<String, String>) -> String {
    features
        .iter()
        .filter_map(|(key, value)| {
            let alias = match key.as_str() {
                "parser_id" => "p",
                "counterparty" => "c",
                "description" => "d",
                "payment_method" => "m",
                _ => return None,
            };
            Some(format!("{alias}={value}"))
        })
        .collect::<Vec<_>>()
        .join("|")
}

pub fn parse_composite_match_value(raw_value: Option<&Value>) -> Option<BTreeMap<String, String>> {
    let mut features = BTreeMap::new();
    for part in value_to_string(raw_value).split('|') {
        let Some((raw_key, raw_feature_value)) = part.split_once('=') else {
            continue;
        };
        let key = match normalize_import_learning_text(Some(&Value::String(raw_key.to_string())))
            .as_str()
        {
            "p" | "parser" | "parser_id" => "parser_id",
            "c" | "counterparty" => "counterparty",
            "d" | "description" => "description",
            "m" | "payment" | "payment_method" => "payment_method",
            _ => continue,
        };
        let value =
            normalize_import_learning_text(Some(&Value::String(raw_feature_value.to_string())));
        if !value.is_empty() {
            features.insert(key.to_string(), value);
        }
    }
    if features.len() < 2 {
        None
    } else {
        Some(features)
    }
}

pub fn amount_bucket(raw_amount: Option<&Value>) -> &'static str {
    let amount = value_to_f64(raw_amount).abs();
    if amount == 0.0 {
        "zero"
    } else if amount < 20.0 {
        "lt20"
    } else if amount < 100.0 {
        "lt100"
    } else if amount < 500.0 {
        "lt500"
    } else {
        "gte500"
    }
}

pub fn build_semantic_label(row: &Map<String, Value>) -> String {
    let learned_type = normalize_learning_text(row.get("annotated_type"));
    let category_id = optional_nonnegative_i64(row.get("annotated_category_id")).unwrap_or(0);
    format!("type={learned_type}|category={category_id}")
}

pub fn build_route_label(row: &Map<String, Value>) -> String {
    let source_id = optional_nonnegative_i64(row.get("annotated_source_account_id")).unwrap_or(0);
    let destination_id =
        optional_nonnegative_i64(row.get("annotated_destination_account_id")).unwrap_or(0);
    format!("source={source_id}|destination={destination_id}")
}

pub fn parse_semantic_label(label: &str) -> SemanticLabelParts {
    let mut result = SemanticLabelParts::default();
    for part in label.split('|') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "type" => result.transaction_type = value.to_string(),
            "category" => {
                result.category_id =
                    optional_nonnegative_i64(Some(&Value::String(value.to_string())))
                        .filter(|value| *value > 0);
            }
            _ => {}
        }
    }
    result
}

pub fn parse_route_label(label: &str) -> RouteLabelParts {
    let mut result = RouteLabelParts::default();
    for part in label.split('|') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let normalized_id = optional_nonnegative_i64(Some(&Value::String(value.to_string())))
            .filter(|value| *value > 0);
        match key {
            "source" => result.source_account_id = normalized_id,
            "destination" => result.destination_account_id = normalized_id,
            _ => {}
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SemanticLabelParts {
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RouteLabelParts {
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

pub fn build_feature_payload(row: &Map<String, Value>) -> BTreeMap<String, String> {
    let snapshot = snapshot_payload(row.get("source_snapshot_json"));
    let preview_type = normalize_learning_text(snapshot.get("preview_type"));
    let preview_type = if preview_type.is_empty() {
        normalize_learning_text(row.get("annotated_type"))
    } else {
        preview_type
    };
    BTreeMap::from([
        (
            "parser_id".to_string(),
            normalize_learning_text(row.get("parser_id")),
        ),
        (
            "counterparty".to_string(),
            normalize_learning_text(row.get("counterparty")),
        ),
        (
            "description".to_string(),
            normalize_learning_text(row.get("description")),
        ),
        (
            "payment_method".to_string(),
            normalize_learning_text(row.get("payment_method")),
        ),
        (
            "amount_bucket".to_string(),
            amount_bucket(snapshot.get("preview_amount")).to_string(),
        ),
        ("preview_type".to_string(), preview_type),
    ])
}

pub fn prepare_training_samples(rows: &[Value]) -> Vec<ImportLearningTrainingSample> {
    rows.iter()
        .filter_map(Value::as_object)
        .filter_map(|row| {
            let sample_id = optional_nonnegative_i64(row.get("id")).unwrap_or(0);
            if sample_id <= 0 {
                return None;
            }
            let semantic_label = build_semantic_label(row);
            let route_label = build_route_label(row);
            if semantic_label == "type=|category=0" && route_label == "source=0|destination=0" {
                return None;
            }
            let features = build_feature_payload(row);
            let text_feature_count = ["parser_id", "counterparty", "description", "payment_method"]
                .iter()
                .filter(|key| features.get(**key).is_some_and(|value| !value.is_empty()))
                .count();
            if text_feature_count < 2 {
                return None;
            }
            Some(ImportLearningTrainingSample {
                sample_id,
                features,
                semantic_label,
                route_label,
            })
        })
        .collect()
}

pub fn iter_feature_tokens(features: &BTreeMap<String, String>) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Some(parser_id) = features
        .get("parser_id")
        .map(|value| normalize_learning_text(Some(&Value::String(value.clone()))))
        .filter(|value| !value.is_empty())
    {
        tokens.push(format!("parser_id={parser_id}"));
    }
    for field in ["counterparty", "description", "payment_method"] {
        tokens.extend(iter_text_tokens(
            field,
            features.get(field).map(String::as_str).unwrap_or(""),
        ));
    }
    if let Some(amount_bucket) = features
        .get("amount_bucket")
        .map(|value| normalize_learning_text(Some(&Value::String(value.clone()))))
        .filter(|value| !value.is_empty())
    {
        tokens.push(format!("amount_bucket={amount_bucket}"));
    }
    if let Some(preview_type) = features
        .get("preview_type")
        .map(|value| normalize_learning_text(Some(&Value::String(value.clone()))))
        .filter(|value| !value.is_empty())
    {
        tokens.push(format!("preview_type={preview_type}"));
    }
    tokens
}

pub fn build_label_confirmation_counts(
    samples: &[ImportLearningTrainingSample],
) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for sample in samples {
        let key = format!("{}||{}", sample.semantic_label, sample.route_label);
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

pub fn build_semantic_label_counts(
    samples: &[ImportLearningTrainingSample],
) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for sample in samples {
        *counts.entry(sample.semantic_label.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportLearningPrediction {
    pub semantic_label: String,
    pub route_label: String,
    pub semantic_confidence: f64,
    pub route_confidence: f64,
    pub semantic_margin: f64,
    pub route_margin: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningPolicyDecision {
    pub mode: String,
    pub score: f64,
    pub confidence: f64,
    pub margin: f64,
    pub auto_apply: bool,
    pub level: String,
    pub rejection_reasons: Vec<String>,
}

pub fn evaluate_learning_policy(
    prediction: &ImportLearningPrediction,
    confirmation_count: i64,
    conflict_reasons: &[String],
) -> LearningPolicyDecision {
    let confidence = prediction
        .semantic_confidence
        .min(prediction.route_confidence);
    let margin = prediction.semantic_margin.min(prediction.route_margin);
    let score = round4((prediction.semantic_confidence + prediction.route_confidence) / 2.0);
    let mut rejection_reasons = Vec::new();

    if confidence < GREEN_CONFIDENCE_THRESHOLD {
        rejection_reasons.push("green_confidence".to_string());
    }
    if margin < GREEN_MARGIN_THRESHOLD {
        rejection_reasons.push("green_margin".to_string());
    }
    if !rejection_reasons.is_empty() {
        return LearningPolicyDecision {
            mode: "none".to_string(),
            score,
            confidence: round4(confidence),
            margin: round4(margin),
            auto_apply: false,
            level: String::new(),
            rejection_reasons,
        };
    }

    let mut blue_rejection_reasons = Vec::new();
    if confirmation_count <= BLUE_ACCEPT_CONFIRMATION_THRESHOLD {
        blue_rejection_reasons.push("confirmations".to_string());
    }
    if confidence < BLUE_CONFIDENCE_THRESHOLD {
        blue_rejection_reasons.push("blue_confidence".to_string());
    }
    if margin < BLUE_MARGIN_THRESHOLD {
        blue_rejection_reasons.push("blue_margin".to_string());
    }
    blue_rejection_reasons.extend(
        conflict_reasons
            .iter()
            .filter(|reason| !reason.is_empty())
            .map(|reason| format!("conflict:{reason}")),
    );

    let mode = if blue_rejection_reasons.is_empty() {
        "blue"
    } else {
        "green"
    };
    let level = if score >= 0.86 {
        "high"
    } else if score >= 0.72 {
        "medium"
    } else {
        "low"
    };
    LearningPolicyDecision {
        mode: mode.to_string(),
        score,
        confidence: round4(confidence),
        margin: round4(margin),
        auto_apply: mode == "blue",
        level: level.to_string(),
        rejection_reasons: blue_rejection_reasons,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningDatasetSnapshotPayload {
    pub feature_schema_version: String,
    pub policy_version: String,
    pub sample_ids: Vec<i64>,
    pub semantic_label_counts: BTreeMap<String, i64>,
    pub joint_label_confirmation_counts: BTreeMap<String, i64>,
}

pub fn build_dataset_snapshot_payload(
    samples: &[ImportLearningTrainingSample],
) -> ImportLearningDatasetSnapshotPayload {
    ImportLearningDatasetSnapshotPayload {
        feature_schema_version: FEATURE_SCHEMA_VERSION.to_string(),
        policy_version: POLICY_VERSION.to_string(),
        sample_ids: samples.iter().map(|sample| sample.sample_id).collect(),
        semantic_label_counts: build_semantic_label_counts(samples),
        joint_label_confirmation_counts: build_label_confirmation_counts(samples),
    }
}

pub fn import_learning_model_version(dataset_snapshot_id: i64) -> String {
    format!("v{dataset_snapshot_id}")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportLearningModelRegistryPayload {
    pub feature_schema_version: String,
    pub policy_version: String,
    pub parameter_ref: String,
    pub model_parameters: Value,
    pub training_metrics: Value,
    pub joint_label_confirmation_counts: BTreeMap<String, i64>,
}

pub fn build_model_registry_payload(
    model_parameters: Value,
    training_metrics: Value,
    confirmation_counts: BTreeMap<String, i64>,
) -> ImportLearningModelRegistryPayload {
    ImportLearningModelRegistryPayload {
        feature_schema_version: FEATURE_SCHEMA_VERSION.to_string(),
        policy_version: POLICY_VERSION.to_string(),
        parameter_ref: "metrics_json.model_parameters".to_string(),
        model_parameters,
        training_metrics,
        joint_label_confirmation_counts: confirmation_counts,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LlmPreviewSnapshot {
    pub preview_main_category: String,
    pub preview_sub_category: String,
    pub preview_source_account_id: Option<i64>,
    pub preview_destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LlmPreviewSuggestion {
    pub suggested_main_category: String,
    pub suggested_sub_category: String,
    pub suggested_source_account: String,
    pub suggested_destination_account: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmPreviewApplyPlan {
    pub previous_preview_snapshot: LlmPreviewSnapshot,
    pub applied_preview_snapshot: LlmPreviewSnapshot,
    pub applied_fields: Vec<String>,
    pub resolved_source_account_id: Option<i64>,
    pub resolved_destination_account_id: Option<i64>,
}

pub fn build_llm_preview_apply_plan(
    current: LlmPreviewSnapshot,
    suggestion: &LlmPreviewSuggestion,
    resolved_source_account_id: Option<i64>,
    resolved_destination_account_id: Option<i64>,
) -> LlmPreviewApplyPlan {
    let normalized_current = LlmPreviewSnapshot {
        preview_main_category: current.preview_main_category.trim().to_string(),
        preview_sub_category: current.preview_sub_category.trim().to_string(),
        preview_source_account_id: current.preview_source_account_id,
        preview_destination_account_id: current.preview_destination_account_id,
    };
    let mut applied = normalized_current.clone();
    if normalized_current.preview_main_category.is_empty()
        && normalized_current.preview_sub_category.is_empty()
        && !suggestion.suggested_main_category.trim().is_empty()
    {
        applied.preview_main_category = suggestion.suggested_main_category.trim().to_string();
        applied.preview_sub_category = suggestion.suggested_sub_category.trim().to_string();
    }
    if normalized_current.preview_source_account_id.is_none()
        && resolved_source_account_id.is_some()
    {
        applied.preview_source_account_id = resolved_source_account_id;
    }
    if normalized_current.preview_destination_account_id.is_none()
        && resolved_destination_account_id.is_some()
    {
        applied.preview_destination_account_id = resolved_destination_account_id;
    }

    let mut applied_fields = Vec::new();
    if applied.preview_main_category != normalized_current.preview_main_category {
        applied_fields.push("preview_main_category".to_string());
    }
    if applied.preview_sub_category != normalized_current.preview_sub_category {
        applied_fields.push("preview_sub_category".to_string());
    }
    if applied.preview_source_account_id != normalized_current.preview_source_account_id {
        applied_fields.push("preview_source_account_id".to_string());
    }
    if applied.preview_destination_account_id != normalized_current.preview_destination_account_id {
        applied_fields.push("preview_destination_account_id".to_string());
    }

    LlmPreviewApplyPlan {
        previous_preview_snapshot: current,
        applied_preview_snapshot: applied,
        applied_fields,
        resolved_source_account_id,
        resolved_destination_account_id,
    }
}

pub fn normalize_llm_preview_review_decision(decision: &str) -> Option<&'static str> {
    match decision.trim().to_lowercase().as_str() {
        "accept" => Some("accept"),
        "reject" => Some("reject"),
        _ => None,
    }
}

pub fn should_restore_llm_previous_preview(
    decision: &str,
    previous_preview_snapshot: Option<&LlmPreviewSnapshot>,
    applied_preview_snapshot: Option<&LlmPreviewSnapshot>,
    current_preview_snapshot: &LlmPreviewSnapshot,
) -> bool {
    normalize_llm_preview_review_decision(decision) == Some("reject")
        && previous_preview_snapshot.is_some()
        && applied_preview_snapshot
            .map(|snapshot| snapshot == current_preview_snapshot)
            .unwrap_or(true)
}

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

pub fn learning_error_response(status_code: u16, error: &str) -> LearningRouteResponse {
    LearningRouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

pub fn learning_data_response<T>(data: T) -> LearningRouteResponse
where
    T: Serialize,
{
    LearningRouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

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

pub fn legacy_learning_rules_page_response(
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
        let suggestion_id = coerce_python_int(value)
            .map_err(|message| format!("Invalid suggestionIds: {message}"))?;
        if seen.insert(suggestion_id, ()).is_none() {
            result.push(suggestion_id);
        }
    }
    Ok(result)
}

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

pub fn llm_memory_events_success(
    events: Vec<LlmMemoryEventContract>,
    total: i64,
) -> LearningRouteResponse {
    LearningRouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": events, "total": total}),
    }
}

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

fn iter_text_tokens(field: &str, value: &str) -> Vec<String> {
    let normalized_value = normalize_learning_text(Some(&Value::String(value.to_string())));
    if normalized_value.is_empty() {
        return Vec::new();
    }
    let mut tokens = vec![format!("{field}={normalized_value}")];
    for part in split_learning_token_parts(&normalized_value) {
        tokens.push(format!("{field}:tok={part}"));
        let chars: Vec<char> = part.chars().collect();
        if chars.len() >= 2 {
            for index in 0..(chars.len() - 1) {
                tokens.push(format!("{}:bi={}{}", field, chars[index], chars[index + 1]));
            }
        }
    }
    tokens
}

fn split_learning_token_parts(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '|' | ','
                        | '，'
                        | '/'
                        | '、'
                        | '_'
                        | '-'
                        | ':'
                        | '：'
                        | ';'
                        | '；'
                        | '('
                        | ')'
                        | '（'
                        | '）'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                )
        })
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn snapshot_payload(value: Option<&Value>) -> Map<String, Value> {
    match value {
        Some(Value::Object(object)) => object.clone(),
        Some(Value::String(text)) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| match value {
                Value::Object(object) => Some(object),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Map::new(),
    }
}

fn value_to_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_to_f64(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn optional_positive_i64(value: Option<&Value>) -> Option<i64> {
    optional_nonnegative_i64(value).filter(|value| *value > 0)
}

fn optional_nonnegative_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.trim().is_empty() || text.trim() == "0" => None,
        Some(Value::Number(number)) if number.as_i64() == Some(0) || number.as_u64() == Some(0) => {
            None
        }
        Some(value) => Some(value_to_i64(value).max(0)),
    }
}

fn value_to_i64(value: &Value) -> i64 {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or_default(),
        Value::String(text) => text.trim().parse::<i64>().unwrap_or_default(),
        Value::Bool(true) => 1,
        _ => 0,
    }
}

fn coerce_python_int(value: &Value) -> Result<i64, String> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .ok_or_else(|| "cannot convert value to int".to_string()),
        Value::String(text) => text
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("invalid literal for int() with base 10: '{}'", text)),
        Value::Bool(value) => Ok(if *value { 1 } else { 0 }),
        Value::Null => Err(
            "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
                .to_string(),
        ),
        Value::Array(_) => Err(
            "int() argument must be a string, a bytes-like object or a real number, not 'list'"
                .to_string(),
        ),
        Value::Object(_) => Err(
            "int() argument must be a string, a bytes-like object or a real number, not 'dict'"
                .to_string(),
        ),
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}
