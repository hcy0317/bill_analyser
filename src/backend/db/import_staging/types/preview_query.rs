#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewRow {
    pub id: i64,
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
