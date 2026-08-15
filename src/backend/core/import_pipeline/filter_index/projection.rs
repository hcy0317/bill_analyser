#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CategoryLookup {
    pub name: String,
    pub sub_category: String,
    pub main_category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AccountLookup {
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImportPreviewSignalFamily {
    Parser,
    PlatformDuplicate,
    Transfer,
    History,
    Learning,
    Llm,
}

impl ImportPreviewSignalFamily {
    pub const ORDER: [Self; 6] = [
        Self::Parser,
        Self::PlatformDuplicate,
        Self::Transfer,
        Self::History,
        Self::Learning,
        Self::Llm,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parser => "parser",
            Self::PlatformDuplicate => "platform_duplicate",
            Self::Transfer => "transfer",
            Self::History => "history",
            Self::Learning => "learning",
            Self::Llm => "llm",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match trim_import_preview_signal_text(value) {
            "parser" => Some(Self::Parser),
            "platform_duplicate" => Some(Self::PlatformDuplicate),
            "transfer" => Some(Self::Transfer),
            "history" => Some(Self::History),
            "learning" => Some(Self::Learning),
            "llm" => Some(Self::Llm),
            _ => None,
        }
    }
}
pub const IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILY_NAMES: [&str; 6] = [
    ImportPreviewSignalFamily::Parser.as_str(),
    ImportPreviewSignalFamily::PlatformDuplicate.as_str(),
    ImportPreviewSignalFamily::Transfer.as_str(),
    ImportPreviewSignalFamily::History.as_str(),
    ImportPreviewSignalFamily::Learning.as_str(),
    ImportPreviewSignalFamily::Llm.as_str(),
];

pub const IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES: &[&str] =
    &IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILY_NAMES;

pub const IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS: &[&str] = &[
    "review_status",
    "status",
    "lifecycle_status",
    "signal_state",
];
pub const IMPORT_PREVIEW_SIGNAL_SUPPRESSED_STATUSES: &[&str] = &["none", "suppressed"];
pub const IMPORT_PREVIEW_SIGNAL_TERMINAL_STATUSES: &[&str] = &[
    "accepted",
    "rejected",
    "skipped",
    "auto_applied",
    "auto-applied",
];
pub const IMPORT_PREVIEW_SIGNAL_NON_PENDING_EXCLUDED_STATUSES: &[&str] = &[
    "pending",
    "none",
    "suppressed",
    "accepted",
    "rejected",
    "skipped",
    "auto_applied",
    "auto-applied",
];
pub const IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES: &[&str] = &[
    "pending",
    "none",
    "suppressed",
    "accepted",
    "rejected",
    "skipped",
    "auto_applied",
    "auto-applied",
];
pub const IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES: &[&str] = &[
    "pending",
    "needs_review",
    "none",
    "suppressed",
    "accepted",
    "rejected",
    "skipped",
    "auto_applied",
    "auto-applied",
];
pub const IMPORT_PREVIEW_LEARNING_NUMERIC_EVIDENCE_FIELDS: &[&str] = &[
    "rule_id",
    "score",
    "confidence",
    "margin",
    "accepted_count",
    "rejected_count",
    "auto_applied_count",
];
pub const IMPORT_PREVIEW_LEARNING_TEXT_EVIDENCE_FIELDS: &[&str] = &[
    "reason",
    "recommended_type",
    "summary",
    "source",
    "mode",
    "model_version",
    "recommendation_key",
];
pub const IMPORT_PREVIEW_LLM_NUMERIC_EVIDENCE_FIELDS: &[&str] =
    &["confidence", "suggested_category_id"];
pub const IMPORT_PREVIEW_LLM_TEXT_EVIDENCE_FIELDS: &[&str] = &[
    "suggested_type",
    "suggested_main_category",
    "suggested_sub_category",
    "suggested_source_account",
    "suggested_destination_account",
    "reason",
];
pub const IMPORT_PREVIEW_HISTORY_OPERATION_NAMES: &[&str] = &[
    "update_history",
    "merge_transfer_history",
    "update_current_bill",
    "merge_current_bill_transfer",
    "merge_transfer",
];
pub const IMPORT_PREVIEW_SIGNAL_TRUTHY_TEXT_VALUES: &[&str] = &["true", "1", "yes", "y"];
pub const IMPORT_PREVIEW_SIGNAL_FALSEY_TEXT_VALUES: &[&str] =
    &["false", "0", "no", "n", "none", "suppressed", "null", ""];
/// Strict finite decimal string grammar: optional sign, decimal digits with at most one dot,
/// at least one digit, no exponent, no hex, no NaN/Infinity. Positive means strictly `> 0`.
pub const IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_GRAMMAR: &str =
    r"^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$";
pub const IMPORT_PREVIEW_SIGNAL_TRIM_CHARS: &str = " \t\n\u{000B}\u{000C}\r\u{0085}\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}";
/// Postgres regex for at least one non-zero digit (lexical truthy check, no numeric cast).
pub const IMPORT_PREVIEW_DECIMAL_HAS_NON_ZERO: &str = "[1-9]";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewFilterIndexItem {
    pub id: i64,
    pub preview_date: String,
    #[serde(rename = "type")]
    pub frontend_type: i64,
    pub source_amount_cents: i64,
    pub category_id: String,
    pub actual_category_name: String,
    pub source_account_id: String,
    pub destination_account_id: String,
    pub actual_source_account_name: String,
    pub actual_destination_account_name: String,
    pub comment: String,
    pub counterparty: String,
    pub payment_method: String,
    pub selected: bool,
    pub is_manually_annotated: bool,
    pub parser_source: String,
    pub parser_tags: Vec<Value>,
    pub dedup_type: String,
    pub dedup_source_ids: Vec<Value>,
    pub transfer_status: Option<String>,
    pub transfer_title: String,
    pub learning_status: Option<String>,
    pub learning_title: String,
    pub learning_summary: String,
    pub learning_mode: String,
    pub llm_status: Option<String>,
    pub llm_title: String,
    pub llm_confidence: f64,
    pub llm_category_path: String,
    pub llm_source_account: String,
    pub llm_destination_account: String,
    pub history_status: Option<String>,
    pub history_title: String,
    pub history_planned_operation: String,
    pub history_bill_id: i64,
    pub history_bill_version: i64,
    pub history_operation_id: String,
    pub history_acknowledgement_token: String,
    pub history_destructive_ack_required: bool,
    pub history_summary: Value,
    pub recurring_template_id: String,
    pub recurring_candidate_count: i64,
    pub recurring_match_reasons: String,
    pub recurring_matched_date: String,
}

/// 中文说明：把一行导入预览原始 JSON 投影为前端筛选索引，统一分类、账户、parser、去重和信号状态字段。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_preview_filter_index_item(
    preview_item: &Map<String, Value>,
    categories_by_id: &BTreeMap<i64, CategoryLookup>,
    accounts_by_id: &BTreeMap<i64, AccountLookup>,
) -> ImportPreviewFilterIndexItem {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_import_preview_filter_index_item",
        "business operation entered"
    );
    let category_id_value = preview_item.get("category_id");
    let category_id_key = integer_lookup_key(category_id_value);
    let category_row =
        category_id_key.and_then(|id| categories_by_id.get(&id).map(|row| (id, row)));
    let category_id = category_row
        .map(|(id, _)| id.to_string())
        .unwrap_or_default();
    let actual_category_name = category_row
        .and_then(|(_, row)| first_non_empty([&row.name, &row.sub_category, &row.main_category]))
        .map(ToOwned::to_owned)
        .unwrap_or_default();

    let source_account_id_value = preview_item.get("preview_source_account_id");
    let source_account_id_key = integer_lookup_key(source_account_id_value);
    let source_account_row =
        source_account_id_key.and_then(|id| accounts_by_id.get(&id).map(|row| (id, row)));
    let source_account_id = source_account_row
        .map(|(id, _)| id.to_string())
        .unwrap_or_default();
    let actual_source_account_name = source_account_row
        .map(|(_, row)| row.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();

    let destination_account_id_value = preview_item.get("preview_destination_account_id");
    let destination_account_id_key = integer_lookup_key(destination_account_id_value);
    let destination_account_row =
        destination_account_id_key.and_then(|id| accounts_by_id.get(&id).map(|row| (id, row)));
    let destination_account_id = destination_account_row
        .map(|(id, _)| id.to_string())
        .unwrap_or_default();
    let actual_destination_account_name = destination_account_row
        .map(|(_, row)| row.name.clone())
        .unwrap_or_default();
    let llm_category_path = build_import_preview_llm_category_path(preview_item);
    let llm_source_account =
        matching_section_string_field(preview_item, "llm", "suggested_source_account")
            .unwrap_or_default();
    let llm_destination_account =
        matching_section_string_field(preview_item, "llm", "suggested_destination_account")
            .unwrap_or_default();
    let normalized_matching_payload = build_import_preview_matching_payload(preview_item);
    let reconciliation = normalized_matching_payload
        .as_object()
        .and_then(|matching| object_field(matching.get("reconciliation")));
    let history_planned_operation = reconciliation
        .and_then(|section| section.get("planned_operation"))
        .and_then(Value::as_str)
        .and_then(normalize_history_operation)
        .map(|operation| operation.as_str().to_string())
        .unwrap_or_default();
    let history_destructive_ack_required = reconciliation
        .and_then(|section| section.get("destructive_ack_required"))
        .map(import_preview_signal_value_is_truthy)
        .unwrap_or(false);
    let history_has_unknown_status = reconciliation.is_some_and(|section| {
        matching_section_has_unknown_review_status(section, IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES)
    });
    let history_status = (!history_has_unknown_status
        && (!history_planned_operation.is_empty() || history_destructive_ack_required))
        .then(|| "pending".to_string());

    ImportPreviewFilterIndexItem {
        id: integer_field_from_map(preview_item, "id"),
        preview_date: string_field_from_map(preview_item, "preview_date"),
        frontend_type: map_import_preview_type_to_frontend_value(preview_item.get("preview_type")),
        source_amount_cents: integer_field_from_map(preview_item, "preview_amount_cents"),
        category_id,
        actual_category_name,
        source_account_id,
        destination_account_id,
        actual_source_account_name,
        actual_destination_account_name,
        comment: string_field_from_map(preview_item, "preview_description"),
        counterparty: string_field_from_map(preview_item, "preview_counterparty"),
        payment_method: string_field_from_map(preview_item, "preview_payment_method"),
        selected: preview_item
            .get("preview_selected")
            .map(json_value_is_truthy)
            .unwrap_or(true),
        is_manually_annotated: preview_item
            .get("preview_is_manually_annotated")
            .map(json_value_is_truthy)
            .unwrap_or(false),
        parser_source: string_field_from_map(preview_item, "preview_parser_id"),
        parser_tags: list_field_from_map(preview_item, "preview_parser_tags"),
        dedup_type: string_field_from_map(preview_item, "dedup_type"),
        dedup_source_ids: parse_dedup_source_ids(preview_item.get("dedup_source_ids")),
        transfer_status: resolve_import_preview_transfer_signal_status(preview_item),
        transfer_title: matching_section_string_field(preview_item, "transfer", "reason")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| string_field_from_map(preview_item, "transfer_suggestion_reason")),
        learning_status: resolve_import_preview_learning_signal_status(preview_item),
        learning_title: matching_section_string_field(preview_item, "learning", "reason")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                string_field_from_map(preview_item, "learning_recommendation_reason")
            }),
        learning_summary: matching_section_string_field(preview_item, "learning", "summary")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                string_field_from_map(preview_item, "learning_recommendation_summary")
            }),
        learning_mode: matching_section_string_field(preview_item, "learning", "mode")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| string_field_from_map(preview_item, "learning_recommendation_mode")),
        llm_status: resolve_import_preview_llm_signal_status(preview_item),
        llm_title: matching_section_string_field(preview_item, "llm", "reason").unwrap_or_default(),
        llm_confidence: matching_section_float_field(preview_item, "llm", "confidence"),
        llm_category_path,
        llm_source_account,
        llm_destination_account,
        history_status,
        history_title: reconciliation
            .map(|section| {
                get_first_non_empty_string([
                    string_field_from_map(section, "notice"),
                    string_field_from_map(section, "reason"),
                ])
            })
            .unwrap_or_default(),
        history_planned_operation,
        history_bill_id: reconciliation
            .and_then(|section| section.get("history_bill_id"))
            .map(value_to_i64)
            .unwrap_or_default(),
        history_bill_version: reconciliation
            .and_then(|section| section.get("history_bill_version"))
            .map(value_to_i64)
            .unwrap_or(1)
            .max(1),
        history_operation_id: reconciliation
            .map(|section| string_field_from_map(section, "operation_id"))
            .unwrap_or_default(),
        history_acknowledgement_token: reconciliation
            .map(|section| string_field_from_map(section, "acknowledgement_token"))
            .unwrap_or_default(),
        history_destructive_ack_required,
        history_summary: reconciliation
            .and_then(|section| section.get("history_summary"))
            .filter(|summary| summary.is_object())
            .cloned()
            .unwrap_or(Value::Null),
        recurring_template_id: normalize_id_text(preview_item.get("preview_recurring_id")),
        recurring_candidate_count: integer_field_from_map(
            preview_item,
            "preview_recurring_candidate_count",
        ),
        recurring_match_reasons: string_field_from_map(
            preview_item,
            "preview_recurring_match_reasons",
        ),
        recurring_matched_date: string_field_from_map(
            preview_item,
            "preview_recurring_matched_date",
        ),
    }
}
