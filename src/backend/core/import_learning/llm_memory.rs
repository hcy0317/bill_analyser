use super::*;

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

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_llm_preview_apply_plan(
    current: LlmPreviewSnapshot,
    suggestion: &LlmPreviewSuggestion,
    resolved_source_account_id: Option<i64>,
    resolved_destination_account_id: Option<i64>,
) -> LlmPreviewApplyPlan {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_llm_preview_apply_plan",
        "business operation entered"
    );
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_llm_preview_review_decision(decision: &str) -> Option<&'static str> {
    match decision.trim().to_lowercase().as_str() {
        "accept" => Some("accept"),
        "reject" => Some("reject"),
        _ => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn should_revert_llm_preview_application(
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
