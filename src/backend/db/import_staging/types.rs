// 中文导读：Postgres import staging 类型层，定义当前导入 session、preview 与学习/LLM DTO。
// 维护重点：类型保持当前 v2 API 所需形状，存储实现由 Postgres adapter 负责。
// 不变式：金额字段明确区分预览元单位与 standard row 分单位。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSessionDraft {
    pub session_id: String,
    pub user_id: UserId,
    pub file_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSessionStatusUpdate {
    pub session_id: String,
    pub user_id: UserId,
    pub status: String,
    pub total_parsed: Option<i64>,
    pub total_preview: Option<i64>,
    pub total_confirmed: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportParseStagingResult {
    pub inserted_count: usize,
    pub total_parsed: i64,
    pub session_found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSessionRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub status: String,
    pub file_count: i64,
    pub total_parsed: i64,
    pub total_preview: i64,
    pub total_confirmed: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewDraft {
    pub preview_date: String,
    pub preview_type: String,
    pub preview_amount: f64,
    pub preview_destination_amount: f64,
    pub preview_main_category: String,
    pub preview_sub_category: String,
    pub preview_source_account_id: Option<i64>,
    pub preview_destination_account_id: Option<i64>,
    pub preview_counterparty: String,
    pub preview_payment_method: String,
    pub preview_description: String,
    pub preview_parser_id: String,
    pub preview_parser_tags: Option<Value>,
    pub preview_recurring_id: Option<i64>,
    pub preview_recurring_name: String,
    pub preview_recurring_candidate_count: i64,
    pub preview_recurring_match_score: f64,
    pub preview_recurring_match_reasons: String,
    pub preview_recurring_matched_date: String,
    pub preview_selected: bool,
    pub dedup_type: Option<String>,
    pub dedup_source_ids: Vec<i64>,
    pub preview_matching_feedback: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportParserTemplateDraft {
    pub parser_date: String,
    pub parser_amount: f64,
    pub parser_type: String,
    pub parser_description: String,
    pub parser_id: String,
    pub parser_tags: Option<Value>,
    pub parser_counterparty: String,
    pub parser_payment_method: String,
    pub parser_original_type: String,
    pub parser_original_category: String,
    pub parser_account_id: String,
}

impl Default for ImportParserTemplateDraft {
    fn default() -> Self {
        Self {
            parser_date: String::new(),
            parser_amount: 0.0,
            parser_type: String::new(),
            parser_description: String::new(),
            parser_id: String::new(),
            parser_tags: None,
            parser_counterparty: String::new(),
            parser_payment_method: String::new(),
            parser_original_type: String::new(),
            parser_original_category: String::new(),
            parser_account_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportParserTemplateRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub parser_date: String,
    pub parser_amount: f64,
    pub parser_type: String,
    pub parser_description: String,
    pub parser_id: String,
    pub parser_tags: Vec<String>,
    pub parser_counterparty: String,
    pub parser_payment_method: String,
    pub parser_original_type: String,
    pub parser_original_category: String,
    pub parser_account_id: String,
    pub parser_is_processed: bool,
    pub created_at: String,
}

pub fn parser_template_draft_from_standard_bill(
    bill: &StandardBill,
    parser_id: &str,
) -> ImportParserTemplateDraft {
    ImportParserTemplateDraft {
        parser_date: bill.date.clone(),
        parser_amount: bill
            .amount
            .to_yuan_string()
            .parse::<f64>()
            .unwrap_or_default(),
        parser_type: parser_template_type(&bill.transaction_type, bill.amount),
        parser_description: bill.description.clone(),
        parser_id: parser_id.to_string(),
        parser_tags: standard_bill_parser_tags_value(bill),
        parser_counterparty: bill.counterparty.clone(),
        parser_payment_method: bill.payment_method.clone(),
        parser_original_type: bill.original_type.clone(),
        parser_original_category: bill.original_category.clone(),
        parser_account_id: bill.source_account_id.clone(),
    }
}

pub fn parser_template_drafts_from_standard_bills(
    bills: &[StandardBill],
    parser_id: &str,
) -> Vec<ImportParserTemplateDraft> {
    bills
        .iter()
        .map(|bill| parser_template_draft_from_standard_bill(bill, parser_id))
        .collect()
}

pub fn dedup_bill_from_parser_template(template: &ImportParserTemplateRow) -> DedupBill {
    let payment_method = first_non_empty([
        template.parser_payment_method.as_str(),
        template.parser_id.as_str(),
    ]);
    DedupBill {
        date: template.parser_date.clone(),
        amount: Money::from_yuan_str(&finite_float_text(template.parser_amount))
            .unwrap_or(Money::ZERO),
        transaction_type: template.parser_type.clone(),
        source_account_id: template.parser_account_id.clone(),
        parser_id: template.parser_id.clone(),
        source: template.parser_id.clone(),
        counterparty: template.parser_counterparty.clone(),
        payment_method,
        description: template.parser_description.clone(),
        original_type: template.parser_original_type.clone(),
        original_category: template.parser_original_category.clone(),
        template_id: Some(template.id.to_string()),
        parser_tags: template.parser_tags.clone(),
        session_id: Some(template.session_id.clone()),
        ..DedupBill::default()
    }
}

pub fn dedup_bills_from_parser_templates(templates: &[ImportParserTemplateRow]) -> Vec<DedupBill> {
    templates
        .iter()
        .map(dedup_bill_from_parser_template)
        .collect()
}

include!("preview_drafts.rs");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub preview_date: String,
    pub preview_type: String,
    pub preview_amount: f64,
    pub preview_destination_amount: f64,
    pub preview_main_category: String,
    pub preview_sub_category: String,
    pub preview_source_account_id: Option<i64>,
    pub preview_destination_account_id: Option<i64>,
    pub preview_counterparty: String,
    pub preview_payment_method: String,
    pub preview_description: String,
    pub preview_parser_id: String,
    pub preview_parser_tags: Vec<String>,
    pub preview_recurring_id: Option<i64>,
    pub preview_recurring_name: String,
    pub preview_recurring_candidate_count: i64,
    pub preview_recurring_match_score: f64,
    pub preview_recurring_match_reasons: String,
    pub preview_recurring_matched_date: String,
    pub preview_selected: bool,
    pub dedup_type: String,
    pub dedup_source_ids: Vec<i64>,
    pub preview_matching_feedback: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewFilterIndexRow {
    pub id: i64,
    pub preview_date: String,
    pub preview_type: String,
    pub preview_amount: f64,
    pub preview_main_category: String,
    pub preview_sub_category: String,
    pub preview_source_account_id: Option<i64>,
    pub preview_destination_account_id: Option<i64>,
    pub preview_counterparty: String,
    pub preview_payment_method: String,
    pub preview_description: String,
    pub preview_parser_id: String,
    pub preview_parser_tags: Vec<String>,
    pub preview_recurring_id: Option<i64>,
    pub preview_recurring_candidate_count: i64,
    pub preview_recurring_match_reasons: String,
    pub preview_recurring_matched_date: String,
    pub preview_selected: bool,
    pub dedup_type: String,
    pub dedup_source_ids: Vec<i64>,
    pub preview_matching_feedback: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewPageRequest {
    pub page: usize,
    pub page_size: usize,
    pub sort_by: String,
    pub sort_direction: String,
    pub preview_ids: Vec<i64>,
    pub filters: ImportPreviewQueryFilters,
}

impl Default for ImportPreviewPageRequest {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 50,
            sort_by: String::new(),
            sort_direction: "asc".to_string(),
            preview_ids: Vec::new(),
            filters: ImportPreviewQueryFilters::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewQueryFilters {
    pub min_datetime: Option<String>,
    pub max_datetime: Option<String>,
    pub transaction_type: Option<String>,
    pub category: Option<String>,
    pub account: Option<String>,
    pub tag: Option<String>,
    pub signal: Option<String>,
    pub annotation: Option<String>,
    pub description: Option<String>,
    pub selected_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewPageResult {
    pub rows: Vec<ImportPreviewRow>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub metadata: ImportPreviewMetadata,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewMetadata {
    pub facets: ImportPreviewFacets,
    pub counts: ImportPreviewCounts,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewFacets {
    pub categories: Vec<ImportPreviewFacetEntry>,
    pub accounts: Vec<ImportPreviewFacetEntry>,
    pub tags: Vec<ImportPreviewFacetEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewCounts {
    pub annotations: std::collections::BTreeMap<String, usize>,
    pub signals: std::collections::BTreeMap<String, usize>,
    pub selected: usize,
    pub selected_invalid: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewFacetEntry {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPreviewPatchField {
    Date,
    Type,
    Amount,
    DestinationAmount,
    MainCategory,
    SubCategory,
    SourceAccountId,
    DestinationAccountId,
    Counterparty,
    PaymentMethod,
    Description,
    RecurringId,
    RecurringName,
    RecurringCandidateCount,
    RecurringMatchScore,
    RecurringMatchReasons,
    RecurringMatchedDate,
    Selected,
    MatchingFeedback,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportPreviewPatchValue {
    Null,
    Text(String),
    Real(f64),
    Integer(i64),
    Bool(bool),
    Json(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewPatch {
    pub preview_id: i64,
    pub changes: Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    pub clear_transfer_decision: bool,
    pub clear_learning_decision: bool,
    pub clear_llm_decision: bool,
}

impl ImportPreviewPatch {
    pub fn new(preview_id: i64) -> Self {
        Self {
            preview_id,
            changes: Vec::new(),
            clear_transfer_decision: false,
            clear_learning_decision: false,
            clear_llm_decision: false,
        }
    }

    pub fn with_change(
        mut self,
        field: ImportPreviewPatchField,
        value: ImportPreviewPatchValue,
    ) -> Self {
        self.changes.push((field, value));
        self
    }

    pub fn with_changes(
        mut self,
        changes: impl IntoIterator<Item = (ImportPreviewPatchField, ImportPreviewPatchValue)>,
    ) -> Self {
        self.changes.extend(changes);
        self
    }

    pub fn with_transfer_decision_cleared(mut self) -> Self {
        self.clear_transfer_decision = true;
        self
    }

    pub fn with_learning_decision_cleared(mut self) -> Self {
        self.clear_learning_decision = true;
        self
    }

    pub fn with_llm_decision_cleared(mut self) -> Self {
        self.clear_llm_decision = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewClassificationUpdate {
    pub preview_id: i64,
    pub preview_type: String,
    pub preview_main_category: String,
    pub preview_sub_category: String,
    pub preview_source_account_id: Option<i64>,
    pub preview_destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPreviewExpectedState {
    pub session_id: Option<String>,
    pub preview_type: Option<String>,
    pub preview_main_category: Option<String>,
    pub preview_sub_category: Option<String>,
    pub preview_recurring_id: Option<Option<i64>>,
    pub preview_source_account_id: Option<Option<i64>>,
    pub preview_destination_account_id: Option<Option<i64>>,
    pub preview_matching_feedback: Option<Value>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmPreviewResult {
    pub confirmed_count: usize,
    pub skipped_count: usize,
    pub duplicate_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportHistoryRewriteAcknowledgement {
    #[serde(default)]
    pub acknowledged: bool,
    #[serde(default, alias = "selectedPreviewIds")]
    pub selected_preview_ids: Vec<i64>,
    #[serde(default)]
    pub operations: Vec<ImportHistoryRewriteAcknowledgementOperation>,
    #[serde(default, alias = "selectionScope")]
    pub selection_scope: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportHistoryRewriteAcknowledgementOperation {
    #[serde(alias = "previewId")]
    pub preview_id: i64,
    #[serde(alias = "operationId")]
    pub operation_id: String,
    #[serde(alias = "plannedOperation")]
    pub planned_operation: String,
    #[serde(alias = "historyBillId")]
    pub history_bill_id: i64,
    #[serde(alias = "historyBillVersion")]
    pub history_bill_version: i64,
    #[serde(alias = "acknowledgementToken")]
    pub acknowledgement_token: String,
}
