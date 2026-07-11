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
    let history_status =
        if !history_planned_operation.is_empty() || history_destructive_ack_required {
            Some("pending".to_string())
        } else {
            None
        };

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

pub fn is_import_preview_visible_signal_family(family: &str) -> bool {
    ImportPreviewSignalFamily::parse(family).is_some()
}

pub fn import_preview_recommendation_feedback_family_is_meaningful(
    feedback: &Value,
    family: &str,
) -> bool {
    let Some(family) = ImportPreviewSignalFamily::parse(family) else {
        return false;
    };
    if !matches!(
        family,
        ImportPreviewSignalFamily::Learning | ImportPreviewSignalFamily::Llm
    ) {
        return false;
    }
    let Some(section) = feedback.get(family.as_str()).and_then(Value::as_object) else {
        return false;
    };

    match family {
        ImportPreviewSignalFamily::Learning => learning_matching_section_is_meaningful(section),
        ImportPreviewSignalFamily::Llm => llm_matching_section_is_meaningful(section),
        _ => false,
    }
}

fn learning_matching_section_is_meaningful(section: &Map<String, Value>) -> bool {
    if matching_section_is_suppressed_or_none(section) {
        return false;
    }
    if matching_section_has_unknown_review_status(
        section,
        IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES,
    ) {
        return false;
    }
    if matching_section_has_terminal_review_status(section) {
        return true;
    }
    if matching_section_has_non_pending_review_status(section) {
        return true;
    }
    matching_section_has_any_meaningful_key(
        section,
        IMPORT_PREVIEW_LEARNING_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LEARNING_TEXT_EVIDENCE_FIELDS,
    )
}

fn llm_matching_section_is_meaningful(section: &Map<String, Value>) -> bool {
    if matching_section_is_suppressed_or_none(section) {
        return false;
    }
    if matching_section_has_unknown_review_status(section, IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES)
    {
        return false;
    }
    if matching_section_has_terminal_review_status(section) {
        return true;
    }
    if matching_section_has_non_pending_review_status(section) {
        return true;
    }
    matching_section_has_any_meaningful_key(
        section,
        IMPORT_PREVIEW_LLM_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LLM_TEXT_EVIDENCE_FIELDS,
    )
}

fn is_suppressed_by_truthy_flag(section: &Map<String, Value>) -> bool {
    section
        .get("suppressed")
        .map(import_preview_signal_value_is_truthy)
        .unwrap_or(false)
}

fn matching_section_is_suppressed_or_none(section: &Map<String, Value>) -> bool {
    if is_suppressed_by_truthy_flag(section) {
        return true;
    }
    let resolved = resolve_first_nonempty_status(section);
    matches!(resolved.as_str(), "none" | "suppressed")
}

fn matching_section_has_terminal_review_status(section: &Map<String, Value>) -> bool {
    let resolved = resolve_first_nonempty_status(section);
    if resolved.is_empty() {
        return false;
    }
    IMPORT_PREVIEW_SIGNAL_TERMINAL_STATUSES.contains(&resolved.as_str())
}

fn matching_section_has_non_pending_review_status(section: &Map<String, Value>) -> bool {
    let resolved = resolve_first_nonempty_status(section);
    !resolved.is_empty()
        && !IMPORT_PREVIEW_SIGNAL_NON_PENDING_EXCLUDED_STATUSES.contains(&resolved.as_str())
}

fn matching_section_has_unknown_review_status(
    section: &Map<String, Value>,
    canonical_statuses: &[&str],
) -> bool {
    let resolved = resolve_first_nonempty_status(section);
    !resolved.is_empty() && !canonical_statuses.contains(&resolved.as_str())
}

fn matching_section_has_any_meaningful_key(
    section: &Map<String, Value>,
    numeric_keys: &[&str],
    text_keys: &[&str],
) -> bool {
    numeric_keys
        .iter()
        .any(|key| section.get(*key).is_some_and(json_number_is_positive))
        || text_keys
            .iter()
            .any(|key| section.get(*key).is_some_and(json_text_is_meaningful))
}

fn json_number_is_positive(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_f64().is_some_and(|value| value > 0.0),
        Value::String(text) => strict_decimal_is_positive(text),
        _ => false,
    }
}

fn json_text_is_meaningful(value: &Value) -> bool {
    let Some(text) = value
        .as_str()
        .map(trim_import_preview_signal_text)
        .filter(|text| !text.is_empty())
    else {
        return false;
    };
    let normalized = text.to_ascii_lowercase();
    if matches!(normalized.as_str(), "none" | "suppressed" | "null") {
        return false;
    }
    if strict_decimal_matches_grammar(text) {
        return strict_decimal_is_positive(text);
    }
    true
}

pub fn trim_import_preview_signal_text(value: &str) -> &str {
    value.trim_matches(|ch| IMPORT_PREVIEW_SIGNAL_TRIM_CHARS.contains(ch))
}

pub fn import_preview_signal_value_to_lowercase(value: &Value) -> String {
    trim_import_preview_signal_text(&value_to_trimmed_string(Some(value))).to_ascii_lowercase()
}

/// 中文说明：统一信号真值判断。bool true、文本 true/yes/y、以及有限非零十进制数视为真值；
/// 零、负零、畸形数字文本、falsey 词、null、容器和空白串视为假值。
pub fn import_preview_signal_value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(number) => number
            .as_f64()
            .is_some_and(|value| value.is_finite() && value != 0.0),
        Value::String(text) => {
            let normalized = trim_import_preview_signal_text(text).to_ascii_lowercase();
            if IMPORT_PREVIEW_SIGNAL_FALSEY_TEXT_VALUES.contains(&normalized.as_str()) {
                return false;
            }
            if IMPORT_PREVIEW_SIGNAL_TRUTHY_TEXT_VALUES.contains(&normalized.as_str()) {
                return true;
            }
            strict_decimal_has_non_zero(&normalized)
        }
        _ => false,
    }
}

pub fn parse_import_preview_decimal_number(value: &str) -> Option<f64> {
    let text = trim_import_preview_signal_text(value);
    if text.is_empty() {
        return None;
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    let (before_dot, after_dot) = match unsigned.split_once('.') {
        Some((before, after)) if !after.is_empty() => (before, Some(after)),
        Some(_) => return None,
        None => (unsigned, None),
    };
    let before_digits = before_dot.chars().all(|ch| ch.is_ascii_digit());
    let after_digits = after_dot.is_none_or(|after| after.chars().all(|ch| ch.is_ascii_digit()));
    let has_digit = before_dot.chars().any(|ch| ch.is_ascii_digit())
        || after_dot.is_some_and(|after| after.chars().any(|ch| ch.is_ascii_digit()));
    if before_digits && after_digits && has_digit {
        text.parse::<f64>().ok().filter(|value| value.is_finite())
    } else {
        None
    }
}

/// 中文说明：快速检查字符串是否匹配严格十进制语法——无指数、无 hex、无 NaN/Infinity。
/// 格式：^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$
fn strict_decimal_matches_grammar(text: &str) -> bool {
    let text = trim_import_preview_signal_text(text);
    if text.is_empty() {
        return false;
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    if unsigned.is_empty() {
        return false;
    }
    let bytes = unsigned.as_bytes();
    let dot_pos = bytes.iter().position(|&b| b == b'.');
    let (before, after, has_dot) = if let Some(pos) = dot_pos {
        (&bytes[..pos], &bytes[pos + 1..], true)
    } else {
        (bytes, &[][..], false)
    };
    let before_ok = before.iter().all(u8::is_ascii_digit);
    let after_ok = after.iter().all(u8::is_ascii_digit);
    let has_digit = before.iter().any(u8::is_ascii_digit) || after.iter().any(u8::is_ascii_digit);
    before_ok
        && after_ok
        && has_digit
        && !(before.is_empty() && after.is_empty())
        && (!has_dot || !after.is_empty())
}

/// 中文说明：检查十进制字符串（已语法验证后）是否含有至少一个非零数字。
fn strict_decimal_unsigned_has_non_zero(text: &str) -> bool {
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    unsigned.contains('1')
        || unsigned.contains('2')
        || unsigned.contains('3')
        || unsigned.contains('4')
        || unsigned.contains('5')
        || unsigned.contains('6')
        || unsigned.contains('7')
        || unsigned.contains('8')
        || unsigned.contains('9')
}

/// 中文说明：词法检查十进制字符串是否含有至少一个非零数字（先验证语法，不依赖 f64）。
pub fn strict_decimal_has_non_zero(text: &str) -> bool {
    if !strict_decimal_matches_grammar(text) {
        return false;
    }
    strict_decimal_unsigned_has_non_zero(text)
}

/// 中文说明：词法检查十进制字符串是否为正数（先验证语法，非负且至少有一个非零数字）。
pub fn strict_decimal_is_positive(text: &str) -> bool {
    if !strict_decimal_matches_grammar(text) {
        return false;
    }
    let trimmed = trim_import_preview_signal_text(text);
    if trimmed.starts_with('-') {
        return false;
    }
    strict_decimal_unsigned_has_non_zero(trimmed)
}

/// 中文说明：按优先级从 review_status/status/lifecycle_status/signal_state 取第一个非空规范化值。
pub fn resolve_first_nonempty_status(section: &Map<String, Value>) -> String {
    for field in IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS {
        let value = section
            .get(*field)
            .map(import_preview_signal_value_to_lowercase)
            .unwrap_or_default();
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}

/// 中文说明：根据 LLM matching 状态判断建议是否在可见信号列中展示。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_llm_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let llm_matching = matching.and_then(|matching| object_field(matching.get("llm")))?;
    // Suppressed/none check first
    if matching_section_is_suppressed_or_none(llm_matching) {
        return None;
    }
    if matching_section_has_unknown_review_status(
        llm_matching,
        IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES,
    ) {
        return None;
    }
    if !llm_matching_section_is_meaningful(llm_matching) {
        return None;
    }
    let status = resolve_first_nonempty_status(llm_matching);
    if matches!(status.as_str(), "accepted" | "rejected" | "skipped") {
        return Some(status);
    }
    if matches!(status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }
    Some("pending".to_string())
}

/// 中文说明：根据 matching payload 与旧字段判断转账建议是否仍待用户处理，避免已接受或已拒绝的建议重复提示。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_transfer_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let transfer_matching = matching.and_then(|matching| object_field(matching.get("transfer")));
    if transfer_matching.is_some_and(matching_section_is_suppressed_or_none) {
        return None;
    }
    let resolved_status = transfer_matching
        .map(resolve_first_nonempty_status)
        .unwrap_or_default();
    if !resolved_status.is_empty()
        && !IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES.contains(&resolved_status.as_str())
    {
        return None;
    }
    if matches!(
        resolved_status.as_str(),
        "accepted" | "rejected" | "skipped"
    ) {
        return Some(resolved_status);
    }
    if matches!(resolved_status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }
    if resolved_status == "pending" {
        return Some("pending".to_string());
    }
    let transfer_has_candidate = transfer_matching.is_some_and(|matching| {
        matching
            .get("candidate_type")
            .and_then(Value::as_str)
            .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty())
            || json_number_is_positive(matching.get("score").unwrap_or(&Value::Null))
            || matching
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|value| !trim_import_preview_signal_text(value).is_empty())
    });

    let suggested_preview_type =
        string_field_from_map(preview_item, "suggested_preview_type").to_ascii_lowercase();
    let preview_type = string_field_from_map(preview_item, "preview_type").to_ascii_lowercase();
    let transfer_score = json_number_is_positive(
        preview_item
            .get("transfer_suggestion_score")
            .unwrap_or(&Value::Null),
    );
    if (transfer_has_candidate || matches!(suggested_preview_type.trim(), "转账" | "transfer"))
        && !matches!(preview_type.trim(), "转账" | "transfer" | "4")
        && (transfer_score || transfer_has_candidate)
    {
        Some("pending".to_string())
    } else {
        None
    }
}

/// 中文说明：根据 learning matching 状态和旧推荐字段判断学习建议展示状态，兼容 auto-applied 与 suppressed 语义。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_import_preview_learning_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let learning_matching = matching.and_then(|matching| object_field(matching.get("learning")));
    // Suppressed check first: explicit suppressed flag OR resolved none/suppressed hides everything
    if learning_matching.is_some_and(matching_section_is_suppressed_or_none) {
        return None;
    }
    let review_status = learning_matching
        .map(resolve_first_nonempty_status)
        .unwrap_or_default();
    if !review_status.is_empty()
        && !IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES.contains(&review_status.as_str())
    {
        return None;
    }
    if matches!(review_status.as_str(), "accepted" | "rejected" | "skipped") {
        return Some(review_status);
    }
    if matches!(review_status.as_str(), "auto_applied" | "auto-applied") {
        return Some("accepted".to_string());
    }
    if !learning_matching.is_some_and(learning_matching_section_is_meaningful)
        && !legacy_learning_fields_are_meaningful(preview_item)
    {
        return None;
    }
    if !review_status.is_empty() && !matches!(review_status.as_str(), "pending") {
        return Some("pending".to_string());
    }

    let learning_rule_id = learning_matching
        .and_then(|matching| matching.get("rule_id"))
        .map(value_to_i64)
        .unwrap_or_default();
    let has_pending_learning = (learning_matching
        .is_some_and(learning_matching_section_is_meaningful)
        || json_number_is_positive(
            preview_item
                .get("learning_recommendation_score")
                .unwrap_or(&Value::Null),
        )
        || learning_matching.is_some_and(|matching| {
            json_number_is_positive(matching.get("score").unwrap_or(&Value::Null))
        })
        || !trim_import_preview_signal_text(&string_field_from_map(
            preview_item,
            "learning_recommendation_summary",
        ))
        .is_empty()
        || !trim_import_preview_signal_text(&string_field_from_map(
            preview_item,
            "learning_recommendation_reason",
        ))
        .is_empty()
        || !trim_import_preview_signal_text(&string_field_from_map(
            preview_item,
            "learning_recommendation_type",
        ))
        .is_empty()
        || learning_rule_id > 0)
        && !learning_matching
            .and_then(|matching| matching.get("suppressed"))
            .map(import_preview_signal_value_is_truthy)
            .unwrap_or(false);

    if has_pending_learning {
        Some("pending".to_string())
    } else {
        None
    }
}

fn legacy_learning_fields_are_meaningful(preview_item: &Map<String, Value>) -> bool {
    json_number_is_positive(
        preview_item
            .get("learning_recommendation_score")
            .unwrap_or(&Value::Null),
    ) || json_text_is_meaningful(
        preview_item
            .get("learning_recommendation_summary")
            .unwrap_or(&Value::Null),
    ) || json_text_is_meaningful(
        preview_item
            .get("learning_recommendation_reason")
            .unwrap_or(&Value::Null),
    ) || json_text_is_meaningful(
        preview_item
            .get("learning_recommendation_type")
            .unwrap_or(&Value::Null),
    )
}

fn build_import_preview_llm_category_path(preview_item: &Map<String, Value>) -> String {
    [
        matching_section_string_field(preview_item, "llm", "suggested_main_category")
            .unwrap_or_default(),
        matching_section_string_field(preview_item, "llm", "suggested_sub_category")
            .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("/")
}
