#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportStageParseData {
    pub session_id: String,
    pub parsed_count: usize,
    pub files: Vec<Value>,
    pub unmatched_files: Vec<Value>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportStageDedupData {
    pub session_id: String,
    pub preview: Vec<Value>,
    pub preview_included: bool,
    pub total: usize,
    pub after_dedup: usize,
    pub dedup_stats: Value,
    pub match_stats: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportStageConfirmData {
    pub imported_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ImportPreviewMatchingPayload {
    pub transfer: TransferMatchingPayload,
    pub investment: InvestmentMatchingPayload,
    pub learning: LearningMatchingPayload,
    pub llm: LlmRecommendationPayload,
    pub recurring: RecurringMatchingPayload,
    pub dedup: DedupMatchingPayload,
    pub parser: ParserMatchingPayload,
    pub annotation: AnnotationMatchingPayload,
    pub reconciliation: ReconciliationMatchingPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_validation: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TransferMatchingPayload {
    pub candidate_type: String,
    pub score: f64,
    pub level: String,
    pub reason: String,
    pub review_status: String,
    pub reviewed_type: String,
    pub suppressed: bool,
    pub pair_order: String,
    pub source_chain: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InvestmentMatchingPayload {
    pub score: f64,
    pub level: String,
    pub reason: String,
    pub platform: String,
    pub product: String,
    pub review_status: String,
    pub suppressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RecurringMatchingPayload {
    pub id: Option<i64>,
    pub name: String,
    pub candidate_count: i64,
    pub match_score: f64,
    pub match_reasons: String,
    pub matched_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DedupMatchingPayload {
    #[serde(rename = "type")]
    pub r#type: String,
    pub source_ids: Value,
    pub source_count: i64,
    pub source_labels: Vec<String>,
    pub sources: Vec<Value>,
}

impl Default for DedupMatchingPayload {
    fn default() -> Self {
        Self {
            r#type: String::new(),
            source_ids: json!([]),
            source_count: 0,
            source_labels: Vec::new(),
            sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ParserMatchingPayload {
    pub id: String,
    pub tags: Vec<String>,
    pub source_chain: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnnotationMatchingPayload {
    pub is_manually_annotated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LlmRecommendationPayload {
    pub suggested_type: String,
    pub suggested_category_id: Option<i64>,
    pub suggested_main_category: String,
    pub suggested_sub_category: String,
    pub suggested_source_account: String,
    pub suggested_destination_account: String,
    pub confidence: f64,
    pub reason: String,
    pub review_status: String,
    pub suppressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ReconciliationMatchingPayload {
    pub candidate_id: String,
    pub candidate_type: String,
    pub status: String,
    pub existing_bill_id: Option<i64>,
    pub group_id: Option<i64>,
    pub score: f64,
    pub level: String,
    pub reason: String,
    pub signal_label: String,
    pub source_chain: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedPreviewState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_account_id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_account_id: Option<Value>,
}

/// 中文说明：从前端回传的 JSON 对象解析预览期望状态，用于确认导入前的状态冲突校验。
#[tracing::instrument(level = "debug", skip_all)]
pub fn expected_preview_state_from_value(value: Option<&Value>) -> Option<ExpectedPreviewState> {
    value.and_then(|value| serde_json::from_value(value.clone()).ok())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewPageData {
    pub preview: Vec<Value>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewIndexData {
    pub items: Vec<ImportPreviewFilterIndexItem>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSessionSummary {
    pub session_id: String,
    pub session_version: i64,
    pub status: String,
    pub created_at: String,
    pub parsed_count: usize,
    pub preview_count: usize,
    pub file_paths: Value,
}
