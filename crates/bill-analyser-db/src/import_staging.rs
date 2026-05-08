use bill_analyser_core::{
    normalize_bill_date_text, serialize_parser_tags, DedupBill, Money, StandardBill, UserId,
};
use chrono::Utc;
use rusqlite::ffi::{SQLITE_CONSTRAINT_PRIMARYKEY, SQLITE_CONSTRAINT_UNIQUE};
use rusqlite::{params, types::Value as SqlValue, Connection, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{run_transaction, DbError, DbResult};

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
        ..ImportPreviewDraft::default()
    }
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

pub fn init_import_staging_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS import_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            status TEXT NOT NULL DEFAULT 'parsing',
            file_count INTEGER DEFAULT 0,
            total_parsed INTEGER DEFAULT 0,
            total_preview INTEGER DEFAULT 0,
            total_confirmed INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_import_sessions_user_session
            ON import_sessions(user_id, session_id);
        CREATE INDEX IF NOT EXISTS idx_import_sessions_session ON import_sessions(session_id);
        CREATE INDEX IF NOT EXISTS idx_import_sessions_user ON import_sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_import_sessions_status ON import_sessions(status);

        CREATE TABLE IF NOT EXISTS bills_preview (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            preview_date TEXT NOT NULL,
            preview_type TEXT NOT NULL,
            preview_amount REAL NOT NULL,
            preview_destination_amount REAL DEFAULT 0,
            preview_main_category TEXT,
            preview_sub_category TEXT,
            preview_source_account_id INTEGER,
            preview_destination_account_id INTEGER,
            preview_counterparty TEXT,
            preview_payment_method TEXT,
            preview_description TEXT,
            preview_parser_id TEXT,
            preview_parser_tags_json TEXT,
            preview_recurring_id INTEGER,
            preview_recurring_name TEXT,
            preview_recurring_candidate_count INTEGER DEFAULT 0,
            preview_recurring_match_score REAL DEFAULT 0,
            preview_recurring_match_reasons TEXT,
            preview_recurring_matched_date TEXT,
            preview_selected INTEGER DEFAULT 1,
            dedup_type TEXT,
            dedup_source_ids TEXT,
            preview_matching_feedback_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_preview_session ON bills_preview(session_id);
        CREATE INDEX IF NOT EXISTS idx_preview_selected ON bills_preview(preview_selected);
        CREATE INDEX IF NOT EXISTS idx_preview_type ON bills_preview(preview_type);
        CREATE INDEX IF NOT EXISTS idx_preview_date ON bills_preview(preview_date);

        CREATE TABLE IF NOT EXISTS bills_parser_template (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            parser_date TEXT NOT NULL,
            parser_amount REAL NOT NULL,
            parser_type TEXT NOT NULL,
            parser_description TEXT,
            parser_id TEXT NOT NULL,
            parser_tags_json TEXT,
            parser_counterparty TEXT,
            parser_payment_method TEXT,
            parser_original_type TEXT,
            parser_original_category TEXT,
            parser_account_id TEXT,
            parser_is_processed TEXT DEFAULT '0',
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_parser_template_session ON bills_parser_template(session_id);
        CREATE INDEX IF NOT EXISTS idx_parser_template_processed ON bills_parser_template(parser_is_processed);
        CREATE INDEX IF NOT EXISTS idx_parser_template_date ON bills_parser_template(parser_date);
        CREATE INDEX IF NOT EXISTS idx_parser_template_parser_id ON bills_parser_template(parser_id);

        CREATE TABLE IF NOT EXISTS import_annotation_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            preview_id INTEGER NOT NULL,
            annotated_type TEXT,
            annotated_category_id INTEGER,
            annotated_source_account_id INTEGER,
            annotated_destination_account_id INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(session_id, preview_id),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_annotation_samples_session ON import_annotation_samples(session_id);
        CREATE INDEX IF NOT EXISTS idx_annotation_samples_user ON import_annotation_samples(user_id);

        CREATE TABLE IF NOT EXISTS llm_memory_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            session_id TEXT,
            preview_id INTEGER,
            event_type TEXT NOT NULL DEFAULT 'recommendation',
            decision TEXT,
            prompt_text TEXT,
            llm_response_raw TEXT,
            llm_provider TEXT,
            llm_model TEXT,
            suggested_main_category TEXT,
            suggested_sub_category TEXT,
            suggested_source_account TEXT,
            suggested_destination_account TEXT,
            confidence REAL DEFAULT 0.0,
            user_correction_category TEXT,
            user_correction_account TEXT,
            snapshot_before TEXT,
            snapshot_after TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_user
            ON llm_memory_events(user_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_user_order
            ON llm_memory_events(user_id, created_at DESC, id DESC);
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session
            ON llm_memory_events(user_id, session_id);
        CREATE INDEX IF NOT EXISTS idx_llm_memory_events_session_order
            ON llm_memory_events(user_id, session_id, created_at DESC, id DESC);
        ",
    )?;
    apply_legacy_staging_alters(connection)?;
    Ok(())
}

pub fn create_import_session(connection: &Connection, draft: &ImportSessionDraft) -> DbResult<i64> {
    let now = now_text();
    let user_id = user_id_i64(draft.user_id)?;
    connection.execute(
        "
        INSERT INTO import_sessions (
            session_id, user_id, status, file_count,
            total_parsed, total_preview, total_confirmed,
            created_at, updated_at
        ) VALUES (?1, ?2, 'parsing', ?3, 0, 0, 0, ?4, ?4)
        ",
        params![draft.session_id, user_id, draft.file_count, now],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn update_import_session_status(
    connection: &Connection,
    update: &ImportSessionStatusUpdate,
) -> DbResult<bool> {
    let changed = connection.execute(
        "
        UPDATE import_sessions
        SET status = ?1,
            updated_at = ?2,
            total_parsed = COALESCE(?3, total_parsed),
            total_preview = COALESCE(?4, total_preview),
            total_confirmed = COALESCE(?5, total_confirmed)
        WHERE session_id = ?6 AND user_id = ?7
        ",
        params![
            update.status,
            now_text(),
            update.total_parsed,
            update.total_preview,
            update.total_confirmed,
            update.session_id,
            user_id_i64(update.user_id)?,
        ],
    )?;
    Ok(changed > 0)
}

pub fn get_import_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Option<ImportSessionRow>> {
    connection
        .query_row(
            "SELECT * FROM import_sessions WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id_i64(user_id)?],
            import_session_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn insert_preview_bill(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    insert_preview_bill_on_connection(connection, session_id, user_id, draft)?;
    Ok(connection.last_insert_rowid())
}

pub fn insert_preview_bills_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        for draft in drafts {
            insert_preview_bill_on_connection(tx, session_id, user_id, draft)?;
        }
        Ok(drafts.len())
    })
}

pub fn get_preview_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    let mut query =
        "SELECT * FROM bills_preview WHERE session_id = ?1 AND user_id = ?2".to_string();
    if selected_only {
        query.push_str(" AND preview_selected = 1");
    }
    query.push_str(" ORDER BY preview_date ASC, id ASC");

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(params![session_id, user_id_i64(user_id)?], preview_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn count_preview_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<i64> {
    let selected_clause = if selected_only {
        " AND preview_selected = 1"
    } else {
        ""
    };
    connection
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause}"
            ),
            params![session_id, user_id_i64(user_id)?],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

pub fn get_preview_page_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    page: i64,
    page_size: i64,
    selected_only: bool,
) -> DbResult<(Vec<ImportPreviewRow>, i64)> {
    let normalized_page = page.max(1);
    let normalized_page_size = page_size.max(1);
    let offset = (normalized_page - 1) * normalized_page_size;
    let selected_clause = if selected_only {
        " AND preview_selected = 1"
    } else {
        ""
    };
    let total: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause}"
        ),
        params![session_id, user_id_i64(user_id)?],
        |row| row.get(0),
    )?;
    let mut statement = connection.prepare(&format!(
        "SELECT * FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause} \
         ORDER BY preview_date ASC, id ASC LIMIT ?3 OFFSET ?4"
    ))?;
    let rows = statement.query_map(
        params![
            session_id,
            user_id_i64(user_id)?,
            normalized_page_size,
            offset
        ],
        preview_from_row,
    )?;
    Ok((rows.collect::<Result<Vec<_>, _>>()?, total))
}

pub fn get_preview_bill_by_id(
    connection: &Connection,
    preview_id: i64,
    user_id: UserId,
) -> DbResult<Option<ImportPreviewRow>> {
    connection
        .query_row(
            "SELECT * FROM bills_preview WHERE id = ?1 AND user_id = ?2",
            params![preview_id, user_id_i64(user_id)?],
            preview_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_preview_by_ids(
    connection: &Connection,
    session_id: &str,
    preview_ids: &[i64],
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewRow>> {
    let normalized_ids: Vec<i64> = preview_ids
        .iter()
        .copied()
        .filter(|preview_id| *preview_id > 0)
        .collect();
    if normalized_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", normalized_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut statement = connection.prepare(&format!(
        "SELECT * FROM bills_preview WHERE session_id = ? AND user_id = ? AND id IN ({placeholders})"
    ))?;
    let user_id = user_id_i64(user_id)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(normalized_ids.len() + 2);
    params.push(&session_id);
    params.push(&user_id);
    for preview_id in &normalized_ids {
        params.push(preview_id);
    }
    let rows = statement.query_map(params.as_slice(), preview_from_row)?;
    let lookup = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|preview| (preview.id, preview))
        .collect::<std::collections::HashMap<_, _>>();
    Ok(normalized_ids
        .iter()
        .filter_map(|preview_id| lookup.get(preview_id).cloned())
        .collect())
}

pub fn insert_parser_template(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportParserTemplateDraft,
) -> DbResult<i64> {
    insert_parser_template_on_connection(connection, session_id, user_id, draft)?;
    Ok(connection.last_insert_rowid())
}

pub fn insert_parser_templates_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportParserTemplateDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        for draft in drafts {
            insert_parser_template_on_connection(tx, session_id, user_id, draft)?;
        }
        Ok(drafts.len())
    })
}

pub fn stage_import_parser_templates(
    connection: &mut Connection,
    session: &ImportSessionDraft,
    drafts: &[ImportParserTemplateDraft],
    require_existing_session: bool,
) -> DbResult<ImportParseStagingResult> {
    run_transaction(connection, |tx| {
        let existing_session = get_import_session(tx, &session.session_id, session.user_id)?;
        if require_existing_session && existing_session.is_none() {
            return Ok(ImportParseStagingResult {
                inserted_count: 0,
                total_parsed: 0,
                session_found: false,
            });
        }
        let previous_parsed = existing_session
            .as_ref()
            .map(|row| row.total_parsed)
            .unwrap_or_default();
        if existing_session.is_none() {
            create_import_session(tx, session)?;
        }
        let mut inserted_count = 0;
        for draft in drafts {
            insert_parser_template_on_connection(tx, &session.session_id, session.user_id, draft)?;
            inserted_count += 1;
        }
        let total_parsed = previous_parsed.saturating_add(usize_to_i64_saturating(inserted_count));
        update_import_session_status(
            tx,
            &ImportSessionStatusUpdate {
                session_id: session.session_id.clone(),
                user_id: session.user_id,
                status: "parsed".to_string(),
                total_parsed: Some(total_parsed),
                total_preview: None,
                total_confirmed: None,
            },
        )?;
        Ok(ImportParseStagingResult {
            inserted_count,
            total_parsed,
            session_found: true,
        })
    })
}

pub fn get_parser_templates_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    processed_only: Option<bool>,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    let mut query =
        "SELECT * FROM bills_parser_template WHERE session_id = ?1 AND user_id = ?2".to_string();
    match processed_only {
        Some(true) => query.push_str(" AND parser_is_processed = '1'"),
        Some(false) => query.push_str(" AND parser_is_processed = '0'"),
        None => {}
    }
    query.push_str(" ORDER BY parser_date ASC, id ASC");

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(
        params![session_id, user_id_i64(user_id)?],
        parser_template_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn get_unprocessed_templates_for_dedup(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    get_parser_templates_by_session(connection, session_id, user_id, Some(false))
}

pub fn update_parser_template_status(
    connection: &mut Connection,
    template_ids: &[i64],
    processed: bool,
    account_id: Option<&str>,
    user_id: UserId,
) -> DbResult<usize> {
    let normalized_ids: Vec<i64> = template_ids
        .iter()
        .copied()
        .filter(|template_id| *template_id > 0)
        .collect();
    if normalized_ids.is_empty() {
        return Ok(0);
    }

    let processed_value = if processed { "1" } else { "0" };
    let user_id = user_id_i64(user_id)?;
    let placeholders = std::iter::repeat_n("?", normalized_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let account_update = if account_id.is_some() {
        ", parser_account_id = ?"
    } else {
        ""
    };
    let query = format!(
        "UPDATE bills_parser_template SET parser_is_processed = ?{account_update} \
         WHERE id IN ({placeholders}) AND user_id = ?"
    );

    run_transaction(connection, |tx| {
        let mut params = Vec::with_capacity(normalized_ids.len() + 3);
        params.push(SqlValue::Text(processed_value.to_string()));
        if let Some(account_id) = account_id {
            params.push(SqlValue::Text(account_id.to_string()));
        }
        for template_id in &normalized_ids {
            params.push(SqlValue::Integer(*template_id));
        }
        params.push(SqlValue::Integer(user_id));
        tx.execute(&query, rusqlite::params_from_iter(params))
            .map_err(DbError::from)
    })
}

pub fn update_preview_bill(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<bool> {
    Ok(execute_preview_patch(connection, session_id, user_id, patch)? > 0)
}

pub fn update_preview_bills_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    if patches.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let mut updated_count = 0;
        for patch in patches {
            if execute_preview_patch(tx, session_id, user_id, patch)? > 0 {
                updated_count += 1;
            }
        }
        Ok(updated_count)
    })
}

pub fn batch_update_preview_classification(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    updates: &[ImportPreviewClassificationUpdate],
) -> DbResult<usize> {
    if updates.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let mut updated_count = 0;
        for update in updates {
            if update.preview_id <= 0 {
                continue;
            }
            let changed = tx.execute(
                "
                UPDATE bills_preview SET
                    preview_type = ?1,
                    preview_main_category = ?2,
                    preview_sub_category = ?3,
                    preview_source_account_id = ?4,
                    preview_destination_account_id = ?5
                WHERE id = ?6 AND session_id = ?7 AND user_id = ?8
                ",
                params![
                    update.preview_type,
                    update.preview_main_category,
                    update.preview_sub_category,
                    update.preview_source_account_id,
                    update.preview_destination_account_id,
                    update.preview_id,
                    session_id,
                    user_id
                ],
            )?;
            if changed > 0 {
                updated_count += 1;
            }
        }
        Ok(updated_count)
    })
}

pub fn apply_preview_transfer_decision(
    connection: &mut Connection,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    reviewed_type: &str,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_decision_not_found());
        };
        if !preview_matches_expected_state(&preview, expected_state) {
            return Ok(preview_decision_state_conflict());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        let transfer_feedback = feedback
            .get("transfer")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let previous_snapshot = transfer_feedback
            .get("previous_preview")
            .and_then(normalize_transfer_snapshot);
        let current_review_status = transfer_feedback
            .get("review_status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let should_restore =
            current_review_status == "accepted" && previous_snapshot.as_ref().is_some();

        let mut patch = ImportPreviewPatch::new(preview_id);
        match decision {
            ImportPreviewDecision::Accept => {
                let snapshot =
                    previous_snapshot.unwrap_or_else(|| build_transfer_previous_snapshot(&preview));
                patch = patch
                    .with_change(
                        ImportPreviewPatchField::Type,
                        ImportPreviewPatchValue::Text(reviewed_type.to_string()),
                    )
                    .with_change(
                        ImportPreviewPatchField::MainCategory,
                        ImportPreviewPatchValue::Text(String::new()),
                    )
                    .with_change(
                        ImportPreviewPatchField::SubCategory,
                        ImportPreviewPatchValue::Text(String::new()),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringId,
                        ImportPreviewPatchValue::Null,
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringName,
                        ImportPreviewPatchValue::Text(String::new()),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringCandidateCount,
                        ImportPreviewPatchValue::Integer(0),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringMatchScore,
                        ImportPreviewPatchValue::Real(0.0),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringMatchReasons,
                        ImportPreviewPatchValue::Text(String::new()),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringMatchedDate,
                        ImportPreviewPatchValue::Text(String::new()),
                    );
                feedback["transfer"] = serde_json::json!({
                    "review_status": "accepted",
                    "reviewed_type": reviewed_type,
                    "suppressed": false,
                    "previous_preview": snapshot,
                });
            }
            ImportPreviewDecision::Reject => {
                if should_restore {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(transfer_snapshot_restore_changes(snapshot));
                    }
                }
                feedback["transfer"] = serde_json::json!({
                    "review_status": "rejected",
                    "reviewed_type": "",
                    "suppressed": true,
                });
            }
            ImportPreviewDecision::Clear => {
                if should_restore {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(transfer_snapshot_restore_changes(snapshot));
                    }
                }
                remove_json_object_key(&mut feedback, "transfer");
            }
        }

        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, &preview.session_id, user_id, &patch)? < 1 {
            return Ok(preview_decision_not_found());
        }
        Ok(ImportPreviewDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

pub fn update_preview_recurring_match_decision(
    connection: &mut Connection,
    preview_id: i64,
    user_id: UserId,
    update: &ImportPreviewRecurringMatchUpdate,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_decision_not_found());
        };
        if !preview_matches_expected_state(&preview, expected_state) {
            return Ok(preview_decision_state_conflict());
        }

        let target_candidate = match update.recurring_id {
            Some(recurring_id) => match update.target_candidate.as_ref() {
                Some(candidate) if candidate.id == recurring_id => Some(candidate),
                _ => {
                    return Ok(ImportPreviewDecisionResult {
                        preview: None,
                        state_conflict: false,
                        invalid_recurring_id: true,
                    })
                }
            },
            None => None,
        };

        let mut feedback = preview.preview_matching_feedback.clone();
        if preview.preview_recurring_id != update.recurring_id {
            remove_json_object_key(&mut feedback, "transfer");
        }
        let patch = ImportPreviewPatch::new(preview_id)
            .with_change(
                ImportPreviewPatchField::RecurringId,
                update
                    .recurring_id
                    .map(ImportPreviewPatchValue::Integer)
                    .unwrap_or(ImportPreviewPatchValue::Null),
            )
            .with_change(
                ImportPreviewPatchField::RecurringName,
                ImportPreviewPatchValue::Text(
                    target_candidate
                        .map(|candidate| candidate.name.clone())
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringCandidateCount,
                ImportPreviewPatchValue::Integer(update.candidate_count),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchScore,
                ImportPreviewPatchValue::Real(
                    target_candidate
                        .map(|candidate| candidate.match_score)
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchReasons,
                ImportPreviewPatchValue::Text(
                    target_candidate
                        .map(|candidate| candidate.match_reasons.join("|"))
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchedDate,
                ImportPreviewPatchValue::Text(
                    target_candidate
                        .map(|candidate| candidate.matched_occurrence_date.clone())
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::MatchingFeedback,
                ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
            );
        if execute_preview_patch(tx, &preview.session_id, user_id, &patch)? < 1 {
            return Ok(preview_decision_not_found());
        }
        Ok(ImportPreviewDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

pub fn apply_preview_learning_decision(
    connection: &mut Connection,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    applied_result: Option<&ImportPreviewLearningApply>,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_decision_not_found());
        };
        if !preview_matches_expected_state(&preview, expected_state) {
            return Ok(preview_decision_state_conflict());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        let learning_feedback = feedback
            .get("learning")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let previous_snapshot = learning_feedback
            .get("previous_preview")
            .and_then(normalize_learning_snapshot);
        let applied_snapshot = learning_feedback
            .get("applied_preview")
            .and_then(normalize_learning_snapshot);
        let current_review_status = learning_feedback
            .get("review_status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let should_restore = current_review_status == "accepted"
            && previous_snapshot.as_ref().is_some()
            && applied_snapshot
                .as_ref()
                .is_none_or(|snapshot| learning_preview_matches_snapshot(&preview, snapshot));

        let mut patch = ImportPreviewPatch::new(preview_id);
        match decision {
            ImportPreviewDecision::Accept => {
                let previous =
                    previous_snapshot.unwrap_or_else(|| build_learning_previous_snapshot(&preview));
                let applied = learning_accept_preview_snapshot(applied_result, &preview);
                patch = patch.with_changes(learning_snapshot_restore_changes(&applied));
                feedback["learning"] = serde_json::json!({
                    "review_status": "accepted",
                    "suppressed": false,
                    "previous_preview": previous,
                    "applied_preview": applied,
                });
                if let Some(rule_id) =
                    applied_result.and_then(|value| normalize_rule_id(value.rule_id))
                {
                    feedback["learning"]["rule_id"] = serde_json::json!(rule_id);
                }
            }
            ImportPreviewDecision::Reject => {
                if should_restore {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(learning_snapshot_restore_changes(snapshot));
                    }
                }
                feedback["learning"] = serde_json::json!({
                    "review_status": "rejected",
                    "suppressed": true,
                });
                let rule_id = applied_result
                    .and_then(|value| normalize_rule_id(value.rule_id))
                    .or_else(|| feedback_rule_id(&learning_feedback));
                if let Some(rule_id) = rule_id {
                    feedback["learning"]["rule_id"] = serde_json::json!(rule_id);
                }
            }
            ImportPreviewDecision::Clear => {
                if should_restore {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(learning_snapshot_restore_changes(snapshot));
                    }
                }
                remove_json_object_key(&mut feedback, "learning");
            }
        }

        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, &preview.session_id, user_id, &patch)? < 1 {
            return Ok(preview_decision_not_found());
        }
        Ok(ImportPreviewDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

pub fn create_llm_memory_event(
    connection: &Connection,
    draft: &LlmMemoryEventDraft,
) -> DbResult<i64> {
    connection.execute(
        "
        INSERT INTO llm_memory_events (
            user_id, session_id, preview_id, event_type, decision,
            prompt_text, llm_response_raw, llm_provider, llm_model,
            suggested_main_category, suggested_sub_category,
            suggested_source_account, suggested_destination_account,
            confidence, user_correction_category, user_correction_account,
            snapshot_before, snapshot_after, metadata, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
        ",
        params![
            user_id_i64(draft.user_id)?,
            draft.session_id,
            draft.preview_id,
            draft.event_type,
            draft.decision,
            draft.prompt_text,
            draft.llm_response_raw,
            draft.llm_provider,
            draft.llm_model,
            draft.suggested_main_category,
            draft.suggested_sub_category,
            draft.suggested_source_account,
            draft.suggested_destination_account,
            draft.confidence,
            draft.user_correction_category,
            draft.user_correction_account,
            json_option_text(draft.snapshot_before.as_ref()),
            json_option_text(draft.snapshot_after.as_ref()),
            json_option_text(draft.metadata.as_ref()),
            now_text(),
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn get_llm_memory_events(
    connection: &Connection,
    user_id: UserId,
    session_id: Option<&str>,
    event_type: Option<&str>,
    limit: usize,
    offset: usize,
) -> DbResult<Vec<LlmMemoryEventRow>> {
    let mut query = "SELECT * FROM llm_memory_events WHERE user_id = ?".to_string();
    let mut params = vec![SqlValue::Integer(user_id_i64(user_id)?)];
    if let Some(session_id) = session_id {
        query.push_str(" AND session_id = ?");
        params.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(event_type) = event_type {
        query.push_str(" AND event_type = ?");
        params.push(SqlValue::Text(event_type.to_string()));
    }
    query.push_str(" ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?");
    params.push(SqlValue::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    params.push(SqlValue::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(
        rusqlite::params_from_iter(params),
        llm_memory_event_from_row,
    )?;
    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    Ok(events)
}

pub fn apply_preview_llm_recommendation(
    connection: &mut Connection,
    request: ImportPreviewLlmApplyRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let ImportPreviewLlmApplyRequest {
        session_id,
        preview_id,
        user_id,
        suggestion,
        prompt_text,
        llm_provider,
        llm_model,
    } = request;
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_llm_not_found());
        };
        if preview.session_id != session_id {
            return Ok(preview_llm_not_found());
        }

        let previous_snapshot = build_llm_previous_snapshot(&preview);
        let current_main_category = preview.preview_main_category.trim().to_string();
        let current_sub_category = preview.preview_sub_category.trim().to_string();
        let mut next_main_category = current_main_category.clone();
        let mut next_sub_category = current_sub_category.clone();
        if current_main_category.is_empty()
            && current_sub_category.is_empty()
            && !suggestion.suggested_main_category.trim().is_empty()
        {
            next_main_category = suggestion.suggested_main_category.trim().to_string();
            next_sub_category = suggestion.suggested_sub_category.trim().to_string();
        }

        let mut next_source_account_id = preview.preview_source_account_id;
        if next_source_account_id.is_none() && suggestion.resolved_source_account_id.is_some() {
            next_source_account_id = suggestion.resolved_source_account_id;
        }
        let mut next_destination_account_id = preview.preview_destination_account_id;
        if next_destination_account_id.is_none()
            && suggestion.resolved_destination_account_id.is_some()
        {
            next_destination_account_id = suggestion.resolved_destination_account_id;
        }

        let applied_snapshot = serde_json::json!({
            "preview_main_category": next_main_category,
            "preview_sub_category": next_sub_category,
            "preview_source_account_id": next_source_account_id,
            "preview_destination_account_id": next_destination_account_id,
        });
        let mut applied_fields = Vec::new();
        let mut patch = ImportPreviewPatch::new(preview_id);
        if next_main_category != current_main_category {
            patch = patch.with_change(
                ImportPreviewPatchField::MainCategory,
                ImportPreviewPatchValue::Text(next_main_category.clone()),
            );
            applied_fields.push("preview_main_category".to_string());
        }
        if next_sub_category != current_sub_category {
            patch = patch.with_change(
                ImportPreviewPatchField::SubCategory,
                ImportPreviewPatchValue::Text(next_sub_category.clone()),
            );
            applied_fields.push("preview_sub_category".to_string());
        }
        if next_source_account_id != preview.preview_source_account_id {
            patch = patch.with_change(
                ImportPreviewPatchField::SourceAccountId,
                next_source_account_id
                    .map(ImportPreviewPatchValue::Integer)
                    .unwrap_or(ImportPreviewPatchValue::Null),
            );
            applied_fields.push("preview_source_account_id".to_string());
        }
        if next_destination_account_id != preview.preview_destination_account_id {
            patch = patch.with_change(
                ImportPreviewPatchField::DestinationAccountId,
                next_destination_account_id
                    .map(ImportPreviewPatchValue::Integer)
                    .unwrap_or(ImportPreviewPatchValue::Null),
            );
            applied_fields.push("preview_destination_account_id".to_string());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        if !applied_fields.is_empty() {
            remove_json_object_key(&mut feedback, "transfer");
        }
        feedback["llm"] = build_llm_feedback_payload(
            suggestion,
            "pending",
            false,
            &previous_snapshot,
            &applied_snapshot,
        );
        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, session_id, user_id, &patch)? < 1 {
            return Ok(preview_llm_not_found());
        }

        let event_id = create_llm_memory_event(
            tx,
            &LlmMemoryEventDraft {
                user_id,
                session_id: Some(session_id.to_string()),
                preview_id: Some(preview_id),
                event_type: "recommendation".to_string(),
                decision: None,
                prompt_text: prompt_text.map(str::to_string),
                llm_response_raw: Some(llm_suggestion_payload(suggestion).to_string()),
                llm_provider: llm_provider.map(str::to_string),
                llm_model: llm_model.map(str::to_string),
                suggested_main_category: optional_non_empty(&suggestion.suggested_main_category),
                suggested_sub_category: optional_non_empty(&suggestion.suggested_sub_category),
                suggested_source_account: optional_non_empty(&suggestion.suggested_source_account),
                suggested_destination_account: optional_non_empty(
                    &suggestion.suggested_destination_account,
                ),
                confidence: suggestion.confidence,
                user_correction_category: None,
                user_correction_account: None,
                snapshot_before: Some(previous_snapshot),
                snapshot_after: Some(applied_snapshot),
                metadata: Some(serde_json::json!({
                    "reason": suggestion.reason,
                    "counterparty": preview.preview_counterparty,
                    "payment_method": preview.preview_payment_method,
                    "description": preview.preview_description,
                    "applied_fields": applied_fields,
                    "resolved_source_account_id": suggestion.resolved_source_account_id,
                    "resolved_destination_account_id": suggestion.resolved_destination_account_id,
                })),
            },
        )?;
        Ok(ImportPreviewLlmDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            event_id: Some(event_id),
            applied_fields,
            restored: false,
        })
    })
}

pub fn review_preview_llm_recommendation(
    connection: &mut Connection,
    request: ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let ImportPreviewLlmReviewRequest {
        session_id,
        preview_id,
        user_id,
        decision,
        suggestion,
        user_correction_category,
        user_correction_account,
    } = request;
    run_transaction(connection, |tx| {
        if decision == ImportPreviewDecision::Clear {
            return Ok(preview_llm_not_found());
        }
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_llm_not_found());
        };
        if preview.session_id != session_id {
            return Ok(preview_llm_not_found());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        let llm_feedback = feedback
            .get("llm")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let previous_snapshot = llm_feedback
            .get("previous_preview")
            .and_then(normalize_llm_snapshot);
        let applied_snapshot = llm_feedback
            .get("applied_preview")
            .and_then(normalize_llm_snapshot);
        let should_restore = decision == ImportPreviewDecision::Reject
            && previous_snapshot.as_ref().is_some()
            && applied_snapshot
                .as_ref()
                .is_none_or(|snapshot| llm_preview_matches_snapshot(&preview, snapshot));
        let current_snapshot = build_llm_previous_snapshot(&preview);
        let refreshed_snapshot = if should_restore {
            previous_snapshot
                .clone()
                .unwrap_or_else(|| current_snapshot.clone())
        } else {
            applied_snapshot
                .clone()
                .unwrap_or_else(|| current_snapshot.clone())
        };
        let resolved_suggestion = suggestion
            .cloned()
            .unwrap_or_else(|| llm_suggestion_from_feedback(&llm_feedback));

        let mut patch = ImportPreviewPatch::new(preview_id);
        if should_restore {
            patch = patch.with_changes(llm_snapshot_restore_changes(&refreshed_snapshot));
        }
        let review_status = if decision == ImportPreviewDecision::Accept {
            "accepted"
        } else {
            "rejected"
        };
        feedback["llm"] = build_llm_feedback_payload(
            &resolved_suggestion,
            review_status,
            decision == ImportPreviewDecision::Reject,
            &previous_snapshot.unwrap_or_else(|| current_snapshot.clone()),
            &applied_snapshot.unwrap_or_else(|| current_snapshot.clone()),
        );
        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, session_id, user_id, &patch)? < 1 {
            return Ok(preview_llm_not_found());
        }

        let event_id = create_llm_memory_event(
            tx,
            &LlmMemoryEventDraft {
                user_id,
                session_id: Some(session_id.to_string()),
                preview_id: Some(preview_id),
                event_type: "feedback".to_string(),
                decision: Some(review_status.trim_end_matches("ed").to_string()),
                prompt_text: None,
                llm_response_raw: Some(llm_suggestion_payload(&resolved_suggestion).to_string()),
                llm_provider: None,
                llm_model: None,
                suggested_main_category: optional_non_empty(
                    &resolved_suggestion.suggested_main_category,
                ),
                suggested_sub_category: optional_non_empty(
                    &resolved_suggestion.suggested_sub_category,
                ),
                suggested_source_account: optional_non_empty(
                    &resolved_suggestion.suggested_source_account,
                ),
                suggested_destination_account: optional_non_empty(
                    &resolved_suggestion.suggested_destination_account,
                ),
                confidence: resolved_suggestion.confidence,
                user_correction_category: user_correction_category.and_then(optional_non_empty),
                user_correction_account: user_correction_account.and_then(optional_non_empty),
                snapshot_before: Some(current_snapshot),
                snapshot_after: Some(refreshed_snapshot),
                metadata: Some(serde_json::json!({
                    "reason": resolved_suggestion.reason,
                    "rollback": should_restore,
                })),
            },
        )?;
        Ok(ImportPreviewLlmDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            event_id: Some(event_id),
            applied_fields: Vec::new(),
            restored: should_restore,
        })
    })
}

pub fn update_preview_selection(
    connection: &mut Connection,
    preview_ids: &[i64],
    selected: bool,
    user_id: UserId,
) -> DbResult<usize> {
    if preview_ids.is_empty() {
        return Ok(0);
    }
    let selected_value = i64::from(selected);
    run_transaction(connection, |tx| {
        for preview_id in preview_ids {
            tx.execute(
                "UPDATE bills_preview SET preview_selected = ?1 WHERE user_id = ?2 AND id = ?3",
                params![selected_value, user_id_i64(user_id)?, preview_id],
            )?;
        }
        Ok(preview_ids.len())
    })
}

pub fn reset_session_preview_selection(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    let changed = connection.execute(
        "UPDATE bills_preview SET preview_selected = 0 WHERE session_id = ?1 AND user_id = ?2",
        params![session_id, user_id_i64(user_id)?],
    )?;
    Ok(changed)
}

pub fn replace_preview_selection_with_patches(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    run_transaction(connection, |tx| {
        tx.execute(
            "UPDATE bills_preview SET preview_selected = 0 WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id_i64(user_id)?],
        )?;
        let mut updated_count = 0;
        for patch in patches {
            if execute_preview_patch(tx, session_id, user_id, patch)? > 0 {
                updated_count += 1;
            }
        }
        Ok(updated_count)
    })
}

pub fn save_import_annotation_samples(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    samples: &[ImportAnnotationSampleDraft],
) -> DbResult<usize> {
    if samples.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let now = now_text();
        let mut saved_count = 0;
        for sample in samples {
            if sample.preview_id <= 0 {
                continue;
            }
            let changed = tx.execute(
                "
                INSERT INTO import_annotation_samples (
                    session_id, user_id, preview_id,
                    annotated_type, annotated_category_id,
                    annotated_source_account_id, annotated_destination_account_id,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                ON CONFLICT(session_id, preview_id) DO UPDATE SET
                    annotated_type = excluded.annotated_type,
                    annotated_category_id = excluded.annotated_category_id,
                    annotated_source_account_id = excluded.annotated_source_account_id,
                    annotated_destination_account_id = excluded.annotated_destination_account_id,
                    updated_at = excluded.updated_at
                WHERE import_annotation_samples.user_id = excluded.user_id
                ",
                params![
                    session_id,
                    user_id,
                    sample.preview_id,
                    sample.annotated_type,
                    sample.annotated_category_id,
                    sample.annotated_source_account_id,
                    sample.annotated_destination_account_id,
                    now
                ],
            )?;
            if changed > 0 {
                saved_count += 1;
            }
        }
        Ok(saved_count)
    })
}

pub fn get_import_annotation_samples(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportAnnotationSampleRow>> {
    let mut statement = connection.prepare(
        "
        SELECT * FROM import_annotation_samples
        WHERE session_id = ?1 AND user_id = ?2
        ORDER BY updated_at ASC, id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![session_id, user_id_i64(user_id)?],
        annotation_sample_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn clear_session_data(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ClearSessionDataResult> {
    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let parser_count = tx.execute(
            "DELETE FROM bills_parser_template WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        let preview_count = tx.execute(
            "DELETE FROM bills_preview WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        let annotation_count = tx.execute(
            "DELETE FROM import_annotation_samples WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        Ok(ClearSessionDataResult {
            parser_count,
            preview_count,
            annotation_count,
        })
    })
}

pub fn confirm_preview_to_bills(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ConfirmPreviewResult> {
    let user_scope = user_id;
    let user_id = user_id_i64(user_scope)?;
    let now = now_text();
    let batch_id = Utc::now().format("%Y%m%d%H%M%S").to_string();

    run_transaction(connection, |tx| {
        if let Some(session) = get_import_session(tx, session_id, user_scope)? {
            if session.status == "completed" {
                return Ok(ConfirmPreviewResult {
                    confirmed_count: i64_to_usize_saturating(session.total_confirmed),
                    skipped_count: 0,
                    duplicate_count: 0,
                    errors: Vec::new(),
                });
            }
        }
        let previews = get_preview_by_session(tx, session_id, user_scope, true)?;
        let mut result = ConfirmPreviewResult {
            confirmed_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            errors: Vec::new(),
        };

        for preview in &previews {
            let bill_type = normalize_confirm_bill_type(&preview.preview_type);
            let preview_date_text = normalize_bill_date_text(&preview.preview_date);
            let amount = confirm_amount_for_type(&bill_type, preview.preview_amount);
            let bill_hash = calculate_import_bill_hash(
                &preview_date_text,
                &bill_type,
                amount,
                &preview.preview_counterparty,
                &preview.preview_description,
            );
            let insert_result = tx.execute(
                "
                INSERT INTO bills (
                    user_id, date, type, amount, counterparty, description,
                    payment_method, main_category, sub_category,
                    source_account_id, destination_account_id, destination_amount,
                    batch_id, hash, created_from_recurring, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                ",
                params![
                    user_id,
                    preview_date_text,
                    bill_type,
                    amount,
                    preview.preview_counterparty,
                    preview.preview_description,
                    preview.preview_payment_method,
                    preview.preview_main_category,
                    preview.preview_sub_category,
                    preview.preview_source_account_id,
                    preview.preview_destination_account_id,
                    preview.preview_destination_amount,
                    batch_id,
                    bill_hash,
                    preview.preview_recurring_id,
                    now,
                    now,
                ],
            );

            match insert_result {
                Ok(_) => result.confirmed_count += 1,
                Err(error) if is_sqlite_constraint_error(&error) => result.duplicate_count += 1,
                Err(error) => return Err(DbError::from(error)),
            }
        }

        tx.execute(
            "
            UPDATE import_sessions
            SET status = 'completed',
                updated_at = ?1,
                total_confirmed = MAX(total_confirmed, ?2)
            WHERE session_id = ?3 AND user_id = ?4
            ",
            params![now, result.confirmed_count as i64, session_id, user_id],
        )?;
        Ok(result)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearSessionDataResult {
    pub parser_count: usize,
    pub preview_count: usize,
    pub annotation_count: usize,
}

fn insert_preview_bill_on_connection(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<()> {
    let parser_tags_json = serialize_parser_tags(
        draft.preview_parser_tags.as_ref(),
        &draft.preview_parser_id,
        &draft.preview_payment_method,
        "",
    );
    let dedup_source_ids = draft
        .dedup_source_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");

    connection.execute(
        "
        INSERT INTO bills_preview (
            session_id, user_id, preview_date, preview_type,
            preview_amount, preview_destination_amount,
            preview_main_category, preview_sub_category,
            preview_source_account_id, preview_destination_account_id,
            preview_counterparty, preview_payment_method, preview_description,
            preview_parser_id, preview_parser_tags_json,
            preview_recurring_id, preview_recurring_name,
            preview_recurring_candidate_count, preview_recurring_match_score,
            preview_recurring_match_reasons, preview_recurring_matched_date,
            preview_selected, dedup_type, dedup_source_ids, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, 1, ?22, ?23, ?24)
        ",
        params![
            session_id,
            user_id_i64(user_id)?,
            normalize_bill_date_text(&draft.preview_date),
            draft.preview_type,
            draft.preview_amount,
            draft.preview_destination_amount,
            draft.preview_main_category,
            draft.preview_sub_category,
            draft.preview_source_account_id,
            draft.preview_destination_account_id,
            draft.preview_counterparty,
            draft.preview_payment_method,
            draft.preview_description,
            draft.preview_parser_id,
            parser_tags_json,
            draft.preview_recurring_id,
            draft.preview_recurring_name,
            draft.preview_recurring_candidate_count,
            draft.preview_recurring_match_score,
            draft.preview_recurring_match_reasons,
            draft.preview_recurring_matched_date,
            draft.dedup_type,
            dedup_source_ids,
            now_text(),
        ],
    )?;
    Ok(())
}

fn insert_parser_template_on_connection(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportParserTemplateDraft,
) -> DbResult<()> {
    let parser_tags_json = serialize_parser_tags(
        draft.parser_tags.as_ref(),
        &draft.parser_id,
        &draft.parser_payment_method,
        "",
    );
    connection.execute(
        "
        INSERT INTO bills_parser_template (
            session_id, user_id, parser_date, parser_amount,
            parser_type, parser_description, parser_id,
            parser_tags_json, parser_counterparty, parser_payment_method,
            parser_original_type, parser_original_category,
            parser_account_id, parser_is_processed, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, '0', ?14)
        ",
        params![
            session_id,
            user_id_i64(user_id)?,
            normalize_bill_date_text(&draft.parser_date),
            draft.parser_amount,
            draft.parser_type,
            draft.parser_description,
            draft.parser_id,
            parser_tags_json,
            draft.parser_counterparty,
            draft.parser_payment_method,
            draft.parser_original_type,
            draft.parser_original_category,
            draft.parser_account_id,
            now_text(),
        ],
    )?;
    Ok(())
}

fn execute_preview_patch(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<usize> {
    if patch.preview_id <= 0 {
        return Ok(0);
    }

    let mut assignments = Vec::new();
    let mut params = Vec::new();
    for (field, value) in &patch.changes {
        assignments.push(format!("{} = ?", field.column_name()));
        params.push(value.clone().into_sql_value());
    }

    if patch.clear_transfer_decision {
        if let Some(raw_feedback) = connection
            .query_row(
                "
                SELECT preview_matching_feedback_json
                FROM bills_preview
                WHERE id = ?1 AND session_id = ?2 AND user_id = ?3
                ",
                params![patch.preview_id, session_id, user_id_i64(user_id)?],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
        {
            assignments.push("preview_matching_feedback_json = ?".to_string());
            params.push(SqlValue::Text(clear_transfer_matching_feedback(Some(
                raw_feedback.as_deref().unwrap_or(""),
            ))));
        }
    }

    if assignments.is_empty() {
        return Ok(0);
    }

    params.push(SqlValue::Integer(patch.preview_id));
    params.push(SqlValue::Text(session_id.to_string()));
    params.push(SqlValue::Integer(user_id_i64(user_id)?));
    let query = format!(
        "UPDATE bills_preview SET {} WHERE id = ? AND session_id = ? AND user_id = ?",
        assignments.join(", ")
    );
    connection
        .execute(&query, rusqlite::params_from_iter(params))
        .map_err(DbError::from)
}

fn import_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportSessionRow> {
    Ok(ImportSessionRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        status: row.get("status")?,
        file_count: row.get("file_count")?,
        total_parsed: row.get("total_parsed")?,
        total_preview: row.get("total_preview")?,
        total_confirmed: row.get("total_confirmed")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn annotation_sample_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ImportAnnotationSampleRow> {
    Ok(ImportAnnotationSampleRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        preview_id: row.get("preview_id")?,
        annotated_type: row.get("annotated_type")?,
        annotated_category_id: row.get("annotated_category_id")?,
        annotated_source_account_id: row.get("annotated_source_account_id")?,
        annotated_destination_account_id: row.get("annotated_destination_account_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn llm_memory_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LlmMemoryEventRow> {
    let snapshot_before: Option<String> = row.get("snapshot_before")?;
    let snapshot_after: Option<String> = row.get("snapshot_after")?;
    let metadata: Option<String> = row.get("metadata")?;
    Ok(LlmMemoryEventRow {
        id: row.get("id")?,
        user_id: row.get("user_id")?,
        session_id: row.get("session_id")?,
        preview_id: row.get("preview_id")?,
        event_type: row.get("event_type")?,
        decision: row.get("decision")?,
        prompt_text: row.get("prompt_text")?,
        llm_response_raw: row.get("llm_response_raw")?,
        llm_provider: row.get("llm_provider")?,
        llm_model: row.get("llm_model")?,
        suggested_main_category: row.get("suggested_main_category")?,
        suggested_sub_category: row.get("suggested_sub_category")?,
        suggested_source_account: row.get("suggested_source_account")?,
        suggested_destination_account: row.get("suggested_destination_account")?,
        confidence: row.get("confidence")?,
        user_correction_category: row.get("user_correction_category")?,
        user_correction_account: row.get("user_correction_account")?,
        snapshot_before: parse_optional_json_object(snapshot_before.as_deref()),
        snapshot_after: parse_optional_json_object(snapshot_after.as_deref()),
        metadata: parse_optional_json_object(metadata.as_deref()),
        created_at: row.get("created_at")?,
    })
}

fn parser_template_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportParserTemplateRow> {
    let parser_tags_json: Option<String> = row.get("parser_tags_json")?;
    Ok(ImportParserTemplateRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        parser_date: row.get("parser_date")?,
        parser_amount: row.get("parser_amount")?,
        parser_type: row.get("parser_type")?,
        parser_description: row
            .get::<_, Option<String>>("parser_description")?
            .unwrap_or_default(),
        parser_id: row.get("parser_id")?,
        parser_tags: parse_string_vec(parser_tags_json.as_deref()),
        parser_counterparty: row
            .get::<_, Option<String>>("parser_counterparty")?
            .unwrap_or_default(),
        parser_payment_method: row
            .get::<_, Option<String>>("parser_payment_method")?
            .unwrap_or_default(),
        parser_original_type: row
            .get::<_, Option<String>>("parser_original_type")?
            .unwrap_or_default(),
        parser_original_category: row
            .get::<_, Option<String>>("parser_original_category")?
            .unwrap_or_default(),
        parser_account_id: row
            .get::<_, Option<String>>("parser_account_id")?
            .unwrap_or_default(),
        parser_is_processed: row.get::<_, String>("parser_is_processed")? == "1",
        created_at: row.get("created_at")?,
    })
}

fn preview_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportPreviewRow> {
    let parser_tags_json: Option<String> = row.get("preview_parser_tags_json")?;
    let dedup_source_ids: Option<String> = row.get("dedup_source_ids")?;
    let feedback_json: Option<String> = row.get("preview_matching_feedback_json")?;

    Ok(ImportPreviewRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        preview_date: row.get("preview_date")?,
        preview_type: row.get("preview_type")?,
        preview_amount: row.get("preview_amount")?,
        preview_destination_amount: row.get("preview_destination_amount")?,
        preview_main_category: row
            .get::<_, Option<String>>("preview_main_category")?
            .unwrap_or_default(),
        preview_sub_category: row
            .get::<_, Option<String>>("preview_sub_category")?
            .unwrap_or_default(),
        preview_source_account_id: row.get("preview_source_account_id")?,
        preview_destination_account_id: row.get("preview_destination_account_id")?,
        preview_counterparty: row
            .get::<_, Option<String>>("preview_counterparty")?
            .unwrap_or_default(),
        preview_payment_method: row
            .get::<_, Option<String>>("preview_payment_method")?
            .unwrap_or_default(),
        preview_description: row
            .get::<_, Option<String>>("preview_description")?
            .unwrap_or_default(),
        preview_parser_id: row
            .get::<_, Option<String>>("preview_parser_id")?
            .unwrap_or_default(),
        preview_parser_tags: parse_string_vec(parser_tags_json.as_deref()),
        preview_recurring_id: row.get("preview_recurring_id")?,
        preview_recurring_name: row
            .get::<_, Option<String>>("preview_recurring_name")?
            .unwrap_or_default(),
        preview_recurring_candidate_count: row
            .get::<_, Option<i64>>("preview_recurring_candidate_count")?
            .unwrap_or_default(),
        preview_recurring_match_score: row
            .get::<_, Option<f64>>("preview_recurring_match_score")?
            .unwrap_or_default(),
        preview_recurring_match_reasons: row
            .get::<_, Option<String>>("preview_recurring_match_reasons")?
            .unwrap_or_default(),
        preview_recurring_matched_date: row
            .get::<_, Option<String>>("preview_recurring_matched_date")?
            .unwrap_or_default(),
        preview_selected: row.get::<_, i64>("preview_selected")? != 0,
        dedup_type: row
            .get::<_, Option<String>>("dedup_type")?
            .unwrap_or_default(),
        dedup_source_ids: parse_i64_csv(dedup_source_ids.as_deref()),
        preview_matching_feedback: parse_json_object(feedback_json.as_deref()),
        created_at: row.get("created_at")?,
    })
}

fn parse_string_vec(raw_json: Option<&str>) -> Vec<String> {
    raw_json
        .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
        .unwrap_or_default()
}

fn parse_i64_csv(raw_csv: Option<&str>) -> Vec<i64> {
    raw_csv
        .unwrap_or("")
        .split(',')
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .collect()
}

fn parse_json_object(raw_json: Option<&str>) -> Value {
    raw_json
        .filter(|text| !text.trim().is_empty())
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Default::default()))
}

fn parse_optional_json_object(raw_json: Option<&str>) -> Option<Value> {
    raw_json
        .filter(|text| !text.trim().is_empty())
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
}

fn clear_transfer_matching_feedback(raw_json: Option<&str>) -> String {
    match parse_json_object(raw_json) {
        Value::Object(mut object) => {
            object.remove("transfer");
            if object.is_empty() {
                String::new()
            } else {
                Value::Object(object).to_string()
            }
        }
        _ => String::new(),
    }
}

fn preview_decision_not_found() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: false,
        invalid_recurring_id: false,
    }
}

fn preview_decision_state_conflict() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: true,
        invalid_recurring_id: false,
    }
}

fn preview_matches_expected_state(
    preview: &ImportPreviewRow,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> bool {
    let Some(expected) = expected_state else {
        return true;
    };

    if expected
        .session_id
        .as_ref()
        .is_some_and(|value| preview.session_id != *value)
    {
        return false;
    }
    if expected
        .preview_type
        .as_ref()
        .is_some_and(|value| preview.preview_type != *value)
    {
        return false;
    }
    if expected
        .preview_main_category
        .as_ref()
        .is_some_and(|value| preview.preview_main_category != *value)
    {
        return false;
    }
    if expected
        .preview_sub_category
        .as_ref()
        .is_some_and(|value| preview.preview_sub_category != *value)
    {
        return false;
    }
    if expected
        .preview_recurring_id
        .as_ref()
        .is_some_and(|value| preview.preview_recurring_id != *value)
    {
        return false;
    }
    if expected
        .preview_source_account_id
        .as_ref()
        .is_some_and(|value| preview.preview_source_account_id != *value)
    {
        return false;
    }
    if expected
        .preview_destination_account_id
        .as_ref()
        .is_some_and(|value| preview.preview_destination_account_id != *value)
    {
        return false;
    }
    if expected
        .preview_matching_feedback
        .as_ref()
        .is_some_and(|value| preview.preview_matching_feedback != *value)
    {
        return false;
    }

    true
}

fn ensure_json_object(value: &mut Value) {
    if !value.is_object() {
        *value = Value::Object(Default::default());
    }
}

fn remove_json_object_key(value: &mut Value, key: &str) {
    ensure_json_object(value);
    if let Value::Object(object) = value {
        object.remove(key);
    }
}

fn serialize_preview_matching_feedback(value: &Value) -> String {
    match value {
        Value::Object(object) if object.is_empty() => String::new(),
        Value::Object(_) => value.to_string(),
        _ => String::new(),
    }
}

fn normalize_transfer_snapshot(value: &Value) -> Option<Value> {
    value
        .as_object()
        .map(|object| Value::Object(object.clone()))
        .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
}

fn build_transfer_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_type": preview.preview_type,
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_recurring_id": preview.preview_recurring_id,
        "preview_recurring_name": preview.preview_recurring_name,
        "preview_recurring_candidate_count": preview.preview_recurring_candidate_count,
        "preview_recurring_match_score": preview.preview_recurring_match_score,
        "preview_recurring_match_reasons": preview.preview_recurring_match_reasons,
        "preview_recurring_matched_date": preview.preview_recurring_matched_date,
    })
}

fn transfer_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_type")),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::RecurringId,
            snapshot_optional_i64(snapshot, "preview_recurring_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_recurring_name")),
        ),
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(snapshot_i64(
                snapshot,
                "preview_recurring_candidate_count",
            )),
        ),
        (
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(snapshot_f64(snapshot, "preview_recurring_match_score")),
        ),
        (
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text(snapshot_text(
                snapshot,
                "preview_recurring_match_reasons",
            )),
        ),
        (
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text(snapshot_text(
                snapshot,
                "preview_recurring_matched_date",
            )),
        ),
    ]
}

fn normalize_learning_snapshot(value: &Value) -> Option<Value> {
    value.as_object().map(|_| {
        serde_json::json!({
            "preview_type": snapshot_text(value, "preview_type"),
            "preview_main_category": snapshot_text(value, "preview_main_category"),
            "preview_sub_category": snapshot_text(value, "preview_sub_category"),
            "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
            "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
        })
    })
}

fn build_learning_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_type": preview.preview_type,
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
    })
}

fn learning_accept_preview_snapshot(
    applied_result: Option<&ImportPreviewLearningApply>,
    preview: &ImportPreviewRow,
) -> Value {
    serde_json::json!({
        "preview_type": applied_result
            .and_then(|value| value.preview_type.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_type.clone()),
        "preview_main_category": applied_result
            .and_then(|value| value.preview_main_category.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_main_category.clone()),
        "preview_sub_category": applied_result
            .and_then(|value| value.preview_sub_category.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_sub_category.clone()),
        "preview_source_account_id": applied_result
            .and_then(|value| value.preview_source_account_id)
            .unwrap_or(preview.preview_source_account_id),
        "preview_destination_account_id": applied_result
            .and_then(|value| value.preview_destination_account_id)
            .unwrap_or(preview.preview_destination_account_id),
    })
}

fn learning_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_type")),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
    ]
}

fn learning_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_type") == preview.preview_type
        && snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
}

fn preview_llm_not_found() -> ImportPreviewLlmDecisionResult {
    ImportPreviewLlmDecisionResult {
        preview: None,
        event_id: None,
        applied_fields: Vec::new(),
        restored: false,
    }
}

fn build_llm_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
    })
}

fn normalize_llm_snapshot(value: &Value) -> Option<Value> {
    value.as_object().map(|_| {
        serde_json::json!({
            "preview_main_category": snapshot_text(value, "preview_main_category"),
            "preview_sub_category": snapshot_text(value, "preview_sub_category"),
            "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
            "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
        })
    })
}

fn llm_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
    ]
}

fn llm_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
}

fn build_llm_feedback_payload(
    suggestion: &ImportPreviewLlmSuggestion,
    review_status: &str,
    suppressed: bool,
    previous_preview: &Value,
    applied_preview: &Value,
) -> Value {
    serde_json::json!({
        "review_status": review_status,
        "suppressed": suppressed,
        "suggested_main_category": suggestion.suggested_main_category,
        "suggested_sub_category": suggestion.suggested_sub_category,
        "suggested_source_account": suggestion.suggested_source_account,
        "suggested_destination_account": suggestion.suggested_destination_account,
        "confidence": suggestion.confidence,
        "reason": suggestion.reason,
        "previous_preview": previous_preview,
        "applied_preview": applied_preview,
    })
}

fn llm_suggestion_payload(suggestion: &ImportPreviewLlmSuggestion) -> Value {
    serde_json::json!({
        "suggested_main_category": suggestion.suggested_main_category,
        "suggested_sub_category": suggestion.suggested_sub_category,
        "suggested_source_account": suggestion.suggested_source_account,
        "suggested_destination_account": suggestion.suggested_destination_account,
        "confidence": suggestion.confidence,
        "reason": suggestion.reason,
    })
}

fn llm_suggestion_from_feedback(feedback: &Value) -> ImportPreviewLlmSuggestion {
    ImportPreviewLlmSuggestion {
        suggested_main_category: snapshot_text(feedback, "suggested_main_category"),
        suggested_sub_category: snapshot_text(feedback, "suggested_sub_category"),
        suggested_source_account: snapshot_text(feedback, "suggested_source_account"),
        suggested_destination_account: snapshot_text(feedback, "suggested_destination_account"),
        resolved_source_account_id: None,
        resolved_destination_account_id: None,
        confidence: snapshot_f64(feedback, "confidence"),
        reason: snapshot_text(feedback, "reason"),
    }
}

fn optional_non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn json_option_text(value: Option<&Value>) -> Option<String> {
    value.map(Value::to_string)
}

fn snapshot_text(snapshot: &Value, field: &str) -> String {
    snapshot
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn snapshot_i64(snapshot: &Value, field: &str) -> i64 {
    snapshot
        .get(field)
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn snapshot_f64(snapshot: &Value, field: &str) -> f64 {
    snapshot
        .get(field)
        .and_then(Value::as_f64)
        .unwrap_or_default()
}

fn snapshot_optional_i64(snapshot: &Value, field: &str) -> Option<i64> {
    snapshot.get(field).and_then(|value| {
        if value.is_null() {
            None
        } else {
            value.as_i64()
        }
    })
}

fn snapshot_account_id(snapshot: &Value, field: &str) -> Option<i64> {
    snapshot.get(field).and_then(|value| {
        if value.is_null() {
            None
        } else if let Some(number) = value.as_i64() {
            Some(number)
        } else {
            value.as_str().and_then(parse_positive_i64)
        }
    })
}

fn normalize_rule_id(rule_id: Option<i64>) -> Option<i64> {
    rule_id.filter(|value| *value > 0)
}

fn feedback_rule_id(feedback: &Value) -> Option<i64> {
    feedback.get("rule_id").and_then(|value| {
        if let Some(number) = value.as_i64() {
            normalize_rule_id(Some(number))
        } else {
            value.as_str().and_then(parse_positive_i64)
        }
    })
}

fn standard_bill_parser_tags_value(bill: &StandardBill) -> Option<Value> {
    if bill.parser_tags.is_empty() {
        None
    } else {
        serde_json::to_value(&bill.parser_tags).ok()
    }
}

fn dedup_bill_parser_tags_value(bill: &DedupBill) -> Option<Value> {
    let mut tags = bill.parser_tags.clone();
    if !bill.destination_parser_id.trim().is_empty() {
        tags.push(format!("parser:{}", bill.destination_parser_id.trim()));
    }
    if tags.is_empty() {
        None
    } else {
        serde_json::to_value(tags).ok()
    }
}

fn parser_template_type(transaction_type: &str, amount: Money) -> String {
    let transaction_type = transaction_type.trim();
    if !transaction_type.is_empty() {
        return transaction_type.to_string();
    }

    if amount.is_positive() {
        "收入".to_string()
    } else if amount.is_negative() {
        "支出".to_string()
    } else {
        "其他".to_string()
    }
}

fn preview_destination_amount_for_bill(bill: &DedupBill, amount: f64) -> f64 {
    if is_investment_type(&bill.transaction_type) {
        amount.abs()
    } else {
        0.0
    }
}

fn is_investment_type(bill_type: &str) -> bool {
    matches!(
        bill_type.trim().to_ascii_lowercase().as_str(),
        "投资" | "investment" | "5"
    )
}

fn money_to_yuan_f64(amount: Money) -> f64 {
    amount.to_yuan_string().parse::<f64>().unwrap_or_default()
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn first_non_empty(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    values
        .into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

pub fn calculate_import_bill_hash(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
) -> String {
    let source = format!(
        "{}|{}|{}|{}|{}",
        date,
        bill_type,
        python_float_text(amount),
        counterparty,
        description
    );
    format!("{:x}", md5::compute(source.as_bytes()))
}

fn confirm_amount_for_type(bill_type: &str, preview_amount: f64) -> f64 {
    let amount = preview_amount.abs();
    if matches!(bill_type, "支出" | "expense") {
        -amount
    } else {
        amount
    }
}

fn normalize_confirm_bill_type(bill_type: &str) -> String {
    let trimmed = bill_type.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "3" | "expense" => "支出".to_string(),
        "2" | "income" => "收入".to_string(),
        "4" | "transfer" => "转账".to_string(),
        "5" | "investment" => "投资".to_string(),
        _ => trimmed.to_string(),
    }
}

fn python_float_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

fn apply_legacy_staging_alters(connection: &Connection) -> DbResult<()> {
    for statement in [
        "ALTER TABLE bills_parser_template ADD COLUMN parser_tags_json TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_parser_id TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_parser_tags_json TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_id INTEGER",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_name TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_candidate_count INTEGER DEFAULT 0",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_score REAL DEFAULT 0",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_match_reasons TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_recurring_matched_date TEXT",
        "ALTER TABLE bills_preview ADD COLUMN preview_matching_feedback_json TEXT",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_type TEXT",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_category_id INTEGER",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_source_account_id INTEGER",
        "ALTER TABLE import_annotation_samples ADD COLUMN annotated_destination_account_id INTEGER",
        "ALTER TABLE import_annotation_samples ADD COLUMN created_at TEXT",
        "ALTER TABLE import_annotation_samples ADD COLUMN updated_at TEXT",
    ] {
        match connection.execute(statement, []) {
            Ok(_) => {}
            Err(error) if is_duplicate_column_error(&error) => {}
            Err(error) => return Err(DbError::from(error)),
        }
    }
    connection.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_annotation_samples_session_preview_unique \
         ON import_annotation_samples(session_id, preview_id)",
        [],
    )?;
    Ok(())
}

fn is_duplicate_column_error(error: &rusqlite::Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("duplicate column")
}

fn is_sqlite_constraint_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(sqlite_error, _)
            if sqlite_error.code == ErrorCode::ConstraintViolation
                && matches!(
                    sqlite_error.extended_code,
                    SQLITE_CONSTRAINT_UNIQUE | SQLITE_CONSTRAINT_PRIMARYKEY
                )
    )
}

fn i64_to_usize_saturating(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

fn usize_to_i64_saturating(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("user id exceeds sqlite integer range".to_string()))
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}
