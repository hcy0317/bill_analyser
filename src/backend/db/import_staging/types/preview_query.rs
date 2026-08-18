#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewRow {
    pub id: i64,
    #[serde(rename = "row_version")]
    pub version: i64,
    pub session_id: String,
    pub user_id: i64,
    pub preview_date: String,
    pub preview_type: String,
    pub preview_amount_cents: i64,
    pub preview_destination_amount_cents: i64,
    pub category_id: Option<i64>,
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
    pub session_id: String,
    pub preview_date: String,
    pub preview_type: String,
    pub preview_amount_cents: i64,
    pub category_id: Option<i64>,
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

impl From<ImportPreviewRow> for ImportPreviewFilterIndexRow {
    fn from(row: ImportPreviewRow) -> Self {
        Self {
            id: row.id,
            session_id: row.session_id,
            preview_date: row.preview_date,
            preview_type: row.preview_type,
            preview_amount_cents: row.preview_amount_cents,
            category_id: row.category_id,
            preview_main_category: row.preview_main_category,
            preview_sub_category: row.preview_sub_category,
            preview_source_account_id: row.preview_source_account_id,
            preview_destination_account_id: row.preview_destination_account_id,
            preview_counterparty: row.preview_counterparty,
            preview_payment_method: row.preview_payment_method,
            preview_description: row.preview_description,
            preview_parser_id: row.preview_parser_id,
            preview_parser_tags: row.preview_parser_tags,
            preview_recurring_id: row.preview_recurring_id,
            preview_recurring_candidate_count: row.preview_recurring_candidate_count,
            preview_recurring_match_reasons: row.preview_recurring_match_reasons,
            preview_recurring_matched_date: row.preview_recurring_matched_date,
            preview_selected: row.preview_selected,
            dedup_type: row.dedup_type,
            dedup_source_ids: row.dedup_source_ids,
            preview_matching_feedback: row.preview_matching_feedback,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewCategoryLookup {
    pub type_code: Option<i64>,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPreviewSelectionMode {
    Select,
    Deselect,
    Invert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPreviewSelectionTarget {
    All,
    Valid,
    NeedsReview,
}

#[derive(Debug, Clone, Copy)]
pub struct ImportPreviewConditionalSelectionCommand<'a> {
    pub mode: ImportPreviewSelectionMode,
    pub target: ImportPreviewSelectionTarget,
    pub request: &'a ImportPreviewPageRequest,
    pub expected_selection_hash: Option<&'a str>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewSignalReadParityReport {
    pub observed_rows: usize,
    pub legacy_total: usize,
    pub typed_total: usize,
    pub rows_mismatch: bool,
    pub total_mismatch: bool,
    pub signal_counts_mismatch: bool,
    pub selection_counts_mismatch: bool,
    pub facets_mismatch: bool,
    pub selection_hash_mismatch: bool,
}

impl ImportPreviewSignalReadParityReport {
    pub fn mismatch_count(&self) -> usize {
        [
            self.rows_mismatch,
            self.total_mismatch,
            self.signal_counts_mismatch,
            self.selection_counts_mismatch,
            self.facets_mismatch,
            self.selection_hash_mismatch,
        ]
        .into_iter()
        .filter(|mismatch| *mismatch)
        .count()
    }

    pub fn is_match(&self) -> bool {
        self.mismatch_count() == 0
    }
}

pub const IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WARMUP_ITERATIONS: u32 = 3;
pub const IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_MEASURED_ITERATIONS: u32 = 10;
pub const IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT: f64 = 15.0;
pub const IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS: u32 = 50_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewSignalPerformanceAuditConfig {
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
    pub query_page_size: u32,
    pub maximum_query_regression_percent: f64,
    pub maximum_write_regression_percent: f64,
}

impl Default for ImportPreviewSignalPerformanceAuditConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 1,
            measured_iterations: 5,
            query_page_size: 100,
            maximum_query_regression_percent: 15.0,
            maximum_write_regression_percent: 15.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewSignalDurationSummary {
    pub samples: u32,
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewSignalPerformanceTimingContract {
    pub clock: String,
    pub jit_disabled: bool,
    pub warmup_excluded: bool,
    pub legacy_typed_order: String,
    pub percentile_method: String,
    pub statement_timeout_ms: u64,
    pub lock_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewSignalQueryPerformanceCase {
    pub session_id: i64,
    pub preview_rows: i64,
    pub signal: String,
    pub parity_match: bool,
    pub legacy: ImportPreviewSignalDurationSummary,
    pub typed: ImportPreviewSignalDurationSummary,
    pub p95_regression_percent: f64,
    pub regression_exceeded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewSignalWritePerformanceReport {
    pub source_session_id: i64,
    pub source_session_rows: i64,
    pub maximum_rows: u32,
    pub rows: i64,
    pub truncated: bool,
    pub base: ImportPreviewSignalDurationSummary,
    pub typed: ImportPreviewSignalDurationSummary,
    pub p95_regression_percent: f64,
    pub maximum_regression_percent: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewSignalPerformanceAuditReport {
    pub expected_migration_version: i64,
    pub actual_migration_version: i64,
    pub migration_count: u64,
    pub snapshot_token: String,
    pub config: ImportPreviewSignalPerformanceAuditConfig,
    pub timing_contract: ImportPreviewSignalPerformanceTimingContract,
    pub corpus_sessions: u64,
    pub query_cases: Vec<ImportPreviewSignalQueryPerformanceCase>,
    pub query_mismatch_cases: u64,
    pub query_regression_cases: u64,
    pub query_regression_tolerance_ms: f64,
    pub write: ImportPreviewSignalWritePerformanceReport,
    pub transaction_rolled_back: bool,
    pub duration_ms: u64,
}

impl ImportPreviewSignalPerformanceAuditReport {
    pub fn passes_performance_gate(&self) -> bool {
        self.actual_migration_version == self.expected_migration_version
            && self.migration_count
                == u64::try_from(self.expected_migration_version).unwrap_or(u64::MAX)
            && !self.query_cases.is_empty()
            && self.query_mismatch_cases == 0
            && self.query_regression_cases == 0
            && self.write.passed
            && self.transaction_rolled_back
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewMetadata {
    pub facets: ImportPreviewFacets,
    pub counts: ImportPreviewCounts,
    pub selection_hash: String,
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
    #[serde(default)]
    pub selected_total: usize,
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
