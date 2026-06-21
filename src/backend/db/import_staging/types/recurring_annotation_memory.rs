#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewRecurringCandidate {
    pub id: i64,
    pub name: String,
    pub match_score: f64,
    pub match_reasons: Vec<String>,
    pub matched_occurrence_date: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewRecurringMatchUpdate {
    pub recurring_id: Option<i64>,
    pub candidate_count: i64,
    pub target_candidate: Option<ImportPreviewRecurringCandidate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportAnnotationSampleDraft {
    pub preview_id: i64,
    pub annotated_type: Option<String>,
    pub annotated_category_id: Option<i64>,
    pub annotated_source_account_id: Option<i64>,
    pub annotated_destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportAnnotationSampleRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub preview_id: i64,
    pub annotated_type: Option<String>,
    pub annotated_category_id: Option<i64>,
    pub annotated_source_account_id: Option<i64>,
    pub annotated_destination_account_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmMemoryEventDraft {
    pub user_id: UserId,
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
    pub snapshot_before: Option<Value>,
    pub snapshot_after: Option<Value>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmMemoryEventRow {
    pub id: i64,
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
    pub snapshot_before: Option<Value>,
    pub snapshot_after: Option<Value>,
    pub metadata: Option<Value>,
    pub created_at: String,
}
