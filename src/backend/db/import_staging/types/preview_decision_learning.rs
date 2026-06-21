#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPreviewDecision {
    Accept,
    Reject,
    Clear,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewDecisionResult {
    pub preview: Option<ImportPreviewRow>,
    pub state_conflict: bool,
    pub invalid_recurring_id: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLearningLifecycleView {
    pub recommendation_key: String,
    pub recommendation_type: String,
    pub status: String,
    pub signal_state: String,
    pub accepted_count: i64,
    pub rejected_count: i64,
    pub auto_applied_count: i64,
    pub auto_apply_enabled: bool,
    pub suppressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLearningLifecycleRecordInput {
    pub recommendation_key: String,
    pub recommendation_type: String,
    pub feedback: String,
    pub rule_id: Option<i64>,
    pub suggestion_id: Option<i64>,
    pub session_id: Option<String>,
    pub preview_id: Option<i64>,
    pub bill_id: Option<i64>,
    pub candidate_id: Option<String>,
    pub payload_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPreviewLearningApply {
    pub preview_type: Option<String>,
    pub preview_main_category: Option<String>,
    pub preview_sub_category: Option<String>,
    pub preview_source_account_id: Option<Option<i64>>,
    pub preview_destination_account_id: Option<Option<i64>>,
    pub rule_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPreviewLlmSuggestion {
    pub suggested_type: String,
    pub suggested_category_id: Option<i64>,
    pub suggested_main_category: String,
    pub suggested_sub_category: String,
    pub suggested_source_account: String,
    pub suggested_destination_account: String,
    pub resolved_source_account_id: Option<i64>,
    pub resolved_destination_account_id: Option<i64>,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewLlmDecisionResult {
    pub preview: Option<ImportPreviewRow>,
    pub event_id: Option<i64>,
    pub applied_fields: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ImportPreviewLlmApplyRequest<'a> {
    pub session_id: &'a str,
    pub preview_id: i64,
    pub user_id: UserId,
    pub suggestion: &'a ImportPreviewLlmSuggestion,
    pub prompt_text: Option<&'a str>,
    pub llm_provider: Option<&'a str>,
    pub llm_model: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct ImportPreviewLlmReviewRequest<'a> {
    pub session_id: &'a str,
    pub preview_id: i64,
    pub user_id: UserId,
    pub decision: ImportPreviewDecision,
    pub suggestion: Option<&'a ImportPreviewLlmSuggestion>,
    pub user_correction_category: Option<&'a str>,
    pub user_correction_account: Option<&'a str>,
}
