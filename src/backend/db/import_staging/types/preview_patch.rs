#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPreviewPatchField {
    Date,
    Type,
    Amount,
    DestinationAmount,
    MainCategory,
    SubCategory,
    CategoryId,
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
    ManualAnnotation,
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
    pub review_status: Option<String>,
    pub preview_type: Option<String>,
    pub preview_category_id: Option<Option<i64>>,
    pub preview_main_category: Option<String>,
    pub preview_sub_category: Option<String>,
    pub preview_recurring_id: Option<Option<i64>>,
    pub preview_source_account_id: Option<Option<i64>>,
    pub preview_destination_account_id: Option<Option<i64>>,
    pub preview_matching_feedback: Option<Value>,
}
