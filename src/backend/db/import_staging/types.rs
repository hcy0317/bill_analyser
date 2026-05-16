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
        amount: Money::from_yuan_str(&python_float_text(template.parser_amount))
            .unwrap_or(Money::ZERO),
        transaction_type: template.parser_type.clone(),
        source_account_id: template.parser_account_id.clone(),
        parser_id: template.parser_id.clone(),
        source: template.parser_id.clone(),
        counterparty: template.parser_counterparty.clone(),
        payment_method,
        description: template.parser_description.clone(),
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

pub fn preview_draft_from_dedup_bill(bill: &DedupBill) -> ImportPreviewDraft {
    let amount = money_to_yuan_f64(bill.amount);
    let preview_payment_method =
        first_non_empty([bill.payment_method.as_str(), bill.parser_id.as_str()]);
    ImportPreviewDraft {
        preview_date: bill.date.clone(),
        preview_type: bill.transaction_type.clone(),
        preview_amount: amount.abs(),
        preview_destination_amount: preview_destination_amount_for_bill(bill, amount),
        preview_main_category: bill.main_category.clone(),
        preview_sub_category: bill.sub_category.clone(),
        preview_source_account_id: parse_positive_i64(&bill.source_account_id),
        preview_destination_account_id: bill
            .destination_account_id
            .as_deref()
            .and_then(parse_positive_i64),
        preview_counterparty: bill.counterparty.clone(),
        preview_payment_method,
        preview_description: bill.description.clone(),
        preview_parser_id: bill.parser_id.clone(),
        preview_parser_tags: dedup_bill_parser_tags_value(bill),
        dedup_type: Some(
            bill.dedup_type
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "remaining".to_string()),
        ),
        dedup_source_ids: bill
            .dedup_source_ids()
            .iter()
            .filter_map(|value| parse_positive_i64(value))
            .collect(),
        preview_matching_feedback: preview_matching_feedback_from_dedup_bill(bill),
        ..ImportPreviewDraft::default()
    }
}

fn preview_matching_feedback_from_dedup_bill(bill: &DedupBill) -> Value {
    let dedup_type = bill
        .dedup_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("remaining");
    let mut feedback = serde_json::Map::new();
    feedback.insert(
        "parser".to_string(),
        serde_json::json!({
            "parser_id": bill.parser_id,
            "parser_tags": &bill.parser_tags,
            "payment_method": bill.payment_method,
            "counterparty": bill.counterparty,
        }),
    );
    feedback.insert(
        "dedup".to_string(),
        serde_json::json!({
            "type": dedup_type,
            "source_ids": bill.dedup_source_ids(),
            "source_count": bill.dedup_source_ids().len(),
        }),
    );
    if dedup_type == "transfer" || dedup_type == "transfer_cross_batch" {
        feedback.insert(
            "transfer".to_string(),
            serde_json::json!({
                "candidate_type": dedup_type,
                "score": 1.0,
                "level": "high",
                "reason": "smart_dedup transfer pair",
                "review_status": "pending",
                "pair_order": bill.transfer_pair_order,
                "source_chain": &bill.transfer_pair_sources,
            }),
        );
    }
    Value::Object(feedback)
}

pub fn preview_drafts_from_dedup_bills(bills: &[DedupBill]) -> Vec<ImportPreviewDraft> {
    bills.iter().map(preview_draft_from_dedup_bill).collect()
}

impl Default for ImportPreviewDraft {
    fn default() -> Self {
        Self {
            preview_date: String::new(),
            preview_type: String::new(),
            preview_amount: 0.0,
            preview_destination_amount: 0.0,
            preview_main_category: String::new(),
            preview_sub_category: String::new(),
            preview_source_account_id: None,
            preview_destination_account_id: None,
            preview_counterparty: String::new(),
            preview_payment_method: String::new(),
            preview_description: String::new(),
            preview_parser_id: String::new(),
            preview_parser_tags: None,
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            dedup_type: None,
            dedup_source_ids: Vec::new(),
            preview_matching_feedback: Value::Object(Default::default()),
        }
    }
}

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

impl ImportPreviewPatchField {
    fn column_name(self) -> &'static str {
        match self {
            Self::Date => "preview_date",
            Self::Type => "preview_type",
            Self::Amount => "preview_amount",
            Self::DestinationAmount => "preview_destination_amount",
            Self::MainCategory => "preview_main_category",
            Self::SubCategory => "preview_sub_category",
            Self::SourceAccountId => "preview_source_account_id",
            Self::DestinationAccountId => "preview_destination_account_id",
            Self::Counterparty => "preview_counterparty",
            Self::PaymentMethod => "preview_payment_method",
            Self::Description => "preview_description",
            Self::RecurringId => "preview_recurring_id",
            Self::RecurringName => "preview_recurring_name",
            Self::RecurringCandidateCount => "preview_recurring_candidate_count",
            Self::RecurringMatchScore => "preview_recurring_match_score",
            Self::RecurringMatchReasons => "preview_recurring_match_reasons",
            Self::RecurringMatchedDate => "preview_recurring_matched_date",
            Self::Selected => "preview_selected",
            Self::MatchingFeedback => "preview_matching_feedback_json",
        }
    }
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

impl ImportPreviewPatchValue {
    fn into_sql_value(self) -> SqlValue {
        match self {
            Self::Null => SqlValue::Null,
            Self::Text(value) => SqlValue::Text(value),
            Self::Real(value) => SqlValue::Real(value),
            Self::Integer(value) => SqlValue::Integer(value),
            Self::Bool(value) => SqlValue::Integer(i64::from(value)),
            Self::Json(value) => {
                if value.is_null() {
                    SqlValue::Null
                } else {
                    SqlValue::Text(value.to_string())
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreviewPatch {
    pub preview_id: i64,
    pub changes: Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    pub clear_transfer_decision: bool,
}

impl ImportPreviewPatch {
    pub fn new(preview_id: i64) -> Self {
        Self {
            preview_id,
            changes: Vec::new(),
            clear_transfer_decision: false,
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
    pub restored: bool,
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
