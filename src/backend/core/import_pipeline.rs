// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};

pub const IMPORT_PREVIEW_SORT_KEYS: &[&str] = &[
    "time",
    "type",
    "sourceAmount",
    "counterparty",
    "paymentMethod",
    "comment",
];
// 这些字段是 Check Data 前端与 Rust preview page 的排序合同；新增字段时必须同步
// preview query、services.ts 映射和前端 contract 测试。
pub const IMPORT_PREVIEW_SELECTION_KEYS: &[&str] =
    &["selected", "isSelected", "is_selected", "preview_selected"];
pub const IMPORT_V2_PIPELINE_STEPS: &[&str] = &[
    "parse",
    "validation",
    "smart_dedup",
    "category_match",
    "account_match",
    "learning_replay",
    "recurring_projection",
    "preview",
    "confirm",
];
// 导入步骤名用于状态响应和排错，不代表 handler 可以跳过 staging/preview/confirm
// 的数据库生命周期。
pub const IMPORT_STAGING_TABLES: &[&str] =
    &["import_sessions", "bills_parser_template", "bills_preview"];
pub const BILLS_PREVIEW_CONTRACT_FIELDS: &[&str] = &[
    "preview_parser_id",
    "preview_parser_tags_json",
    "preview_selected",
    "preview_is_manually_annotated",
    "dedup_type",
    "dedup_source_ids",
    "preview_recurring_id",
    "preview_recurring_name",
    "preview_recurring_candidate_count",
    "preview_recurring_match_score",
    "preview_recurring_match_reasons",
    "preview_recurring_matched_date",
    "preview_matching_feedback_json",
];
// preview row 的兼容字段清单。任何删改都要同时复核后端投影、前端模型和
// matching feedback 的 sparse payload 兼容性。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportPreviewSortDirection {
    Asc,
    Desc,
}

impl ImportPreviewSortDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewPageQuery {
    pub page: usize,
    pub page_size: usize,
    pub sort_by: String,
    pub sort_direction: ImportPreviewSortDirection,
    pub preview_ids: Vec<i64>,
}

pub fn normalize_import_preview_page_query(
    page: Option<i64>,
    page_size: Option<i64>,
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
    preview_ids: &[i64],
) -> ImportPreviewPageQuery {
    ImportPreviewPageQuery {
        page: normalize_page(page),
        page_size: normalize_page_size(page_size),
        sort_by: normalize_import_preview_page_sort_key(sort_by).to_string(),
        sort_direction: normalize_import_preview_page_sort_direction(sort_direction),
        preview_ids: normalize_preview_ids(preview_ids),
    }
}

pub fn normalize_page(page: Option<i64>) -> usize {
    usize::try_from(page.unwrap_or(1).max(1)).unwrap_or(1)
}

pub fn normalize_page_size(page_size: Option<i64>) -> usize {
    let page_size = page_size.unwrap_or(50).clamp(1, 200);
    usize::try_from(page_size).unwrap_or(50)
}

pub fn normalize_import_preview_page_sort_direction(
    sort_direction: Option<&str>,
) -> ImportPreviewSortDirection {
    if sort_direction
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("desc")
    {
        ImportPreviewSortDirection::Desc
    } else {
        ImportPreviewSortDirection::Asc
    }
}

pub fn normalize_import_preview_page_sort_key(sort_by: Option<&str>) -> &'static str {
    let normalized = sort_by.unwrap_or("").trim();
    IMPORT_PREVIEW_SORT_KEYS
        .iter()
        .copied()
        .find(|key| *key == normalized)
        .unwrap_or("")
}

pub fn normalize_preview_ids(preview_ids: &[i64]) -> Vec<i64> {
    let mut seen = HashSet::new();
    preview_ids
        .iter()
        .copied()
        .filter(|preview_id| *preview_id > 0)
        .filter(|preview_id| seen.insert(*preview_id))
        .collect()
}

pub fn sort_import_preview_page_items(
    items: &[Value],
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
) -> Vec<Value> {
    let normalized_sort_by = normalize_import_preview_page_sort_key(sort_by);
    if normalized_sort_by.is_empty() {
        return items.to_vec();
    }

    let sort_field = match normalized_sort_by {
        "time" => "preview_date",
        "type" => "preview_type",
        "sourceAmount" => "preview_amount",
        "counterparty" => "preview_counterparty",
        "paymentMethod" => "preview_payment_method",
        "comment" => "preview_description",
        _ => return items.to_vec(),
    };

    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| integer_field(item, "id"));
    let descending = normalize_import_preview_page_sort_direction(sort_direction)
        == ImportPreviewSortDirection::Desc;
    if sort_field == "preview_amount" {
        sorted.sort_by(|left, right| {
            let left_key = float_field(left, sort_field);
            let right_key = float_field(right, sort_field);
            if descending {
                right_key
                    .partial_cmp(&left_key)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                left_key
                    .partial_cmp(&right_key)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    } else {
        sorted.sort_by(|left, right| {
            let left_key = string_field(left, sort_field).to_lowercase();
            let right_key = string_field(right, sort_field).to_lowercase();
            if descending {
                right_key.cmp(&left_key)
            } else {
                left_key.cmp(&right_key)
            }
        });
    }
    sorted
}

pub fn coerce_preview_selected_value(value: Option<&Value>, default: bool) -> bool {
    match value {
        None | Some(Value::Null) => default,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(number)) => !json_number_is_zero(number),
        Some(Value::String(text)) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "" | "none" | "null" => default,
                "0" | "false" | "no" | "off" | "n" => false,
                "1" | "true" | "yes" | "on" | "y" => true,
                _ => !text.is_empty(),
            }
        }
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
    }
}

pub fn preview_update_is_selected(update_item: &Map<String, Value>, default: bool) -> bool {
    IMPORT_PREVIEW_SELECTION_KEYS
        .iter()
        .find_map(|key| {
            update_item
                .get(*key)
                .map(|value| coerce_preview_selected_value(Some(value), default))
        })
        .unwrap_or(default)
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreviewFilterIndexItem {
    pub id: i64,
    pub preview_date: String,
    #[serde(rename = "type")]
    pub frontend_type: i64,
    pub source_amount: f64,
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
    pub recurring_template_id: String,
    pub recurring_candidate_count: i64,
    pub recurring_match_reasons: String,
    pub recurring_matched_date: String,
}

pub fn build_import_preview_filter_index_item(
    preview_item: &Map<String, Value>,
    categories_by_id: &BTreeMap<i64, CategoryLookup>,
    accounts_by_id: &BTreeMap<i64, AccountLookup>,
) -> ImportPreviewFilterIndexItem {
    let category_id_value = preview_item.get("category_id");
    let category_id = normalize_id_text(category_id_value);
    let category_row =
        integer_lookup_key(category_id_value).and_then(|id| categories_by_id.get(&id));
    let actual_category_name = category_row
        .and_then(|row| first_non_empty([&row.name, &row.sub_category, &row.main_category]))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            first_non_empty([
                &string_field_from_map(preview_item, "preview_sub_category"),
                &string_field_from_map(preview_item, "preview_main_category"),
            ])
            .unwrap_or("")
            .to_string()
        });

    let source_account_id_value = preview_item.get("preview_source_account_id");
    let source_account_id = normalize_id_text(source_account_id_value);
    let source_account_row =
        integer_lookup_key(source_account_id_value).and_then(|id| accounts_by_id.get(&id));
    let actual_source_account_name = source_account_row
        .map(|row| row.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| string_field_from_map(preview_item, "preview_payment_method"));

    let destination_account_id_value = preview_item.get("preview_destination_account_id");
    let destination_account_id = normalize_id_text(destination_account_id_value);
    let destination_account_row =
        integer_lookup_key(destination_account_id_value).and_then(|id| accounts_by_id.get(&id));
    let actual_destination_account_name = destination_account_row
        .map(|row| row.name.clone())
        .unwrap_or_default();

    ImportPreviewFilterIndexItem {
        id: integer_field_from_map(preview_item, "id"),
        preview_date: string_field_from_map(preview_item, "preview_date"),
        frontend_type: map_import_preview_type_to_frontend_value(preview_item.get("preview_type")),
        source_amount: float_field_from_map(preview_item, "preview_amount"),
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
            .map(python_truthy)
            .unwrap_or(true),
        is_manually_annotated: preview_item
            .get("preview_is_manually_annotated")
            .map(python_truthy)
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

pub fn resolve_import_preview_transfer_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let transfer_matching = matching.and_then(|matching| object_field(matching.get("transfer")));
    let review_status = transfer_matching
        .and_then(|matching| matching.get("review_status"))
        .map(value_to_trimmed_lowercase)
        .unwrap_or_default();
    if matches!(review_status.as_str(), "accepted" | "rejected") {
        return Some(review_status);
    }
    let transfer_has_candidate = transfer_matching.is_some_and(|matching| {
        !string_field_from_map(matching, "candidate_type")
            .trim()
            .is_empty()
            || float_field_from_map(matching, "score") > 0.0
            || !string_field_from_map(matching, "reason").trim().is_empty()
    });

    let suggested_preview_type =
        string_field_from_map(preview_item, "suggested_preview_type").to_lowercase();
    let preview_type = string_field_from_map(preview_item, "preview_type").to_lowercase();
    let transfer_score = float_field_from_map(preview_item, "transfer_suggestion_score");
    let transfer_suppressed = transfer_matching
        .and_then(|matching| matching.get("suppressed"))
        .map(python_truthy)
        .unwrap_or(false);

    if (transfer_has_candidate || matches!(suggested_preview_type.trim(), "转账" | "transfer"))
        && !matches!(preview_type.trim(), "转账" | "transfer")
        && (transfer_score > 0.0 || transfer_has_candidate)
        && !transfer_suppressed
    {
        Some("pending".to_string())
    } else {
        None
    }
}

pub fn resolve_import_preview_learning_signal_status(
    preview_item: &Map<String, Value>,
) -> Option<String> {
    let matching = preview_matching_payload(preview_item);
    let learning_matching = matching.and_then(|matching| object_field(matching.get("learning")));
    let review_status = learning_matching
        .and_then(|matching| matching.get("review_status"))
        .map(value_to_trimmed_lowercase)
        .unwrap_or_default();
    if matches!(review_status.as_str(), "accepted" | "rejected") {
        return Some(review_status);
    }

    let learning_rule_id = learning_matching
        .and_then(|matching| matching.get("rule_id"))
        .map(value_to_i64)
        .unwrap_or_default();
    let has_pending_learning =
        (float_field_from_map(preview_item, "learning_recommendation_score") > 0.0
            || !string_field_from_map(preview_item, "learning_recommendation_summary")
                .trim()
                .is_empty()
            || !string_field_from_map(preview_item, "learning_recommendation_reason")
                .trim()
                .is_empty()
            || !string_field_from_map(preview_item, "learning_recommendation_type")
                .trim()
                .is_empty()
            || learning_rule_id > 0)
            && !learning_matching
                .and_then(|matching| matching.get("suppressed"))
                .map(python_truthy)
                .unwrap_or(false);

    if has_pending_learning {
        Some("pending".to_string())
    } else {
        None
    }
}

pub fn attach_import_preview_matching_payload(preview_item: &mut Map<String, Value>) {
    let matching = build_import_preview_matching_payload(preview_item);
    preview_item.insert("matching".to_string(), matching);
}

pub fn build_import_preview_matching_payload(preview_item: &Map<String, Value>) -> Value {
    let mut payload =
        serde_json::to_value(ImportPreviewMatchingPayload::default()).unwrap_or_else(|_| json!({}));
    let Some(payload_object) = payload.as_object_mut() else {
        return json!({});
    };

    if let Some(feedback) = preview_matching_payload(preview_item) {
        for section in [
            "transfer",
            "investment",
            "learning",
            "llm",
            "recurring",
            "dedup",
            "parser",
            "annotation",
            "reconciliation",
        ] {
            if let Some(feedback_section) = object_field_from_map(feedback, section) {
                merge_object_fields(
                    section_object_mut(payload_object, section),
                    feedback_section,
                );
            }
        }
    }

    populate_parser_matching_section(payload_object, preview_item);
    populate_dedup_matching_section(payload_object, preview_item);
    populate_recurring_matching_section(payload_object, preview_item);
    populate_annotation_matching_section(payload_object, preview_item);

    payload
}

fn preview_matching_payload(preview_item: &Map<String, Value>) -> Option<&Map<String, Value>> {
    object_field_from_map(preview_item, "matching")
        .or_else(|| object_field_from_map(preview_item, "preview_matching_feedback"))
}

fn merge_object_fields(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

fn section_object_mut<'a>(
    payload_object: &'a mut Map<String, Value>,
    section: &str,
) -> &'a mut Map<String, Value> {
    if !payload_object
        .get(section)
        .is_some_and(|value| value.is_object())
    {
        payload_object.insert(section.to_string(), json!({}));
    }

    payload_object
        .get_mut(section)
        .and_then(Value::as_object_mut)
        .expect("matching section object")
}

fn populate_parser_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    let parser = section_object_mut(payload_object, "parser");
    let parser_id = first_non_empty_string_field(
        parser,
        ["id", "parser_id"].into_iter().chain(["preview_parser_id"]),
        preview_item,
    );
    let parser_tags = first_non_empty_value_list(
        [
            parser.get("tags"),
            parser.get("parser_tags"),
            preview_item.get("preview_parser_tags"),
        ]
        .into_iter()
        .flatten(),
    );

    parser.insert("id".to_string(), Value::String(parser_id.clone()));
    parser.insert("parser_id".to_string(), Value::String(parser_id));
    parser.insert("tags".to_string(), Value::Array(parser_tags.clone()));
    parser.insert("parser_tags".to_string(), Value::Array(parser_tags));
}

fn populate_dedup_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    let dedup = section_object_mut(payload_object, "dedup");
    let dedup_type = get_first_non_empty_string([
        string_field_from_map(dedup, "type"),
        string_field_from_map(preview_item, "dedup_type"),
    ]);
    let source_ids = first_non_empty_value_list(
        [
            dedup.get("source_ids"),
            preview_item.get("dedup_source_ids"),
        ]
        .into_iter()
        .flatten(),
    );

    dedup.insert("type".to_string(), Value::String(dedup_type));
    dedup.insert("source_ids".to_string(), Value::Array(source_ids.clone()));
    if integer_field_from_map(dedup, "source_count") <= 0 {
        dedup.insert(
            "source_count".to_string(),
            Value::Number(Number::from(source_ids.len() as i64)),
        );
    }
}

fn populate_recurring_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    let recurring = section_object_mut(payload_object, "recurring");
    if recurring
        .get("id")
        .map(|value| normalize_id_text(Some(value)))
        .unwrap_or_default()
        .is_empty()
    {
        recurring.insert(
            "id".to_string(),
            preview_item
                .get("preview_recurring_id")
                .cloned()
                .unwrap_or(Value::Null),
        );
    }

    let recurring_name = get_first_non_empty_string([
        string_field_from_map(recurring, "name"),
        string_field_from_map(preview_item, "preview_recurring_name"),
    ]);
    recurring.insert("name".to_string(), Value::String(recurring_name));

    if integer_field_from_map(recurring, "candidate_count") <= 0 {
        recurring.insert(
            "candidate_count".to_string(),
            Value::Number(Number::from(integer_field_from_map(
                preview_item,
                "preview_recurring_candidate_count",
            ))),
        );
    }
    if float_field_from_map(recurring, "match_score") <= 0.0 {
        recurring.insert(
            "match_score".to_string(),
            Number::from_f64(float_field_from_map(
                preview_item,
                "preview_recurring_match_score",
            ))
            .map(Value::Number)
            .unwrap_or(Value::Number(Number::from(0))),
        );
    }

    let match_reasons = get_first_non_empty_string([
        string_field_from_map(recurring, "match_reasons"),
        string_field_from_map(preview_item, "preview_recurring_match_reasons"),
    ]);
    let matched_date = get_first_non_empty_string([
        string_field_from_map(recurring, "matched_date"),
        string_field_from_map(preview_item, "preview_recurring_matched_date"),
    ]);
    recurring.insert("match_reasons".to_string(), Value::String(match_reasons));
    recurring.insert("matched_date".to_string(), Value::String(matched_date));
}

fn populate_annotation_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    let annotation = section_object_mut(payload_object, "annotation");
    let is_manually_annotated = annotation
        .get("is_manually_annotated")
        .map(python_truthy)
        .unwrap_or(false)
        || preview_item
            .get("preview_is_manually_annotated")
            .map(python_truthy)
            .unwrap_or(false);
    annotation.insert(
        "is_manually_annotated".to_string(),
        Value::Bool(is_manually_annotated),
    );
}

fn first_non_empty_string_field<'a>(
    section: &Map<String, Value>,
    section_keys: impl Iterator<Item = &'a str>,
    preview_item: &Map<String, Value>,
) -> String {
    get_first_non_empty_string(section_keys.map(|key| {
        let value = section.get(key).or_else(|| preview_item.get(key));
        value_to_trimmed_string(value)
    }))
}

fn get_first_non_empty_string(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn first_non_empty_value_list<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    values
        .into_iter()
        .map(|value| parse_dedup_source_ids(Some(value)))
        .find(|values| !values.is_empty())
        .unwrap_or_default()
}

fn matching_section_string_field(
    preview_item: &Map<String, Value>,
    section: &str,
    key: &str,
) -> Option<String> {
    preview_matching_payload(preview_item)
        .and_then(|matching| object_field(matching.get(section)))
        .map(|section| string_field_from_map(section, key))
}

pub fn map_import_preview_type_to_frontend_value(preview_type: Option<&Value>) -> i64 {
    match value_to_trimmed_string(preview_type).as_str() {
        "收入" | "income" => 2,
        "支出" | "expense" => 3,
        "转账" | "transfer" => 4,
        "投资" | "investment" => 5,
        _ => 1,
    }
}

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
pub struct LearningMatchingPayload {
    pub rule_id: Option<i64>,
    pub score: f64,
    pub level: String,
    pub reason: String,
    pub recommended_type: String,
    pub summary: String,
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
    pub status: String,
    pub created_at: String,
    pub parsed_count: usize,
    pub preview_count: usize,
    pub file_paths: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportV2RouteResponse {
    pub status_code: u16,
    pub body: Value,
}

pub fn import_v2_error_response(status_code: u16, error: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

pub fn import_v2_message_response(
    status_code: u16,
    success: bool,
    message: &str,
) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": success, "message": message}),
    }
}

pub fn import_v2_data_response<T>(data: T) -> ImportV2RouteResponse
where
    T: Serialize,
{
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

pub fn import_stage_parse_success(data: ImportStageParseData) -> ImportV2RouteResponse {
    import_v2_data_response(data)
}

pub fn import_stage_dedup_success(data: ImportStageDedupData) -> ImportV2RouteResponse {
    import_v2_data_response(data)
}

pub fn import_stage_confirm_success(data: ImportStageConfirmData) -> ImportV2RouteResponse {
    import_v2_data_response(data)
}

pub fn import_preview_page_success(data: ImportPreviewPageData) -> ImportV2RouteResponse {
    import_v2_data_response(data)
}

pub fn import_preview_index_success(data: ImportPreviewIndexData) -> ImportV2RouteResponse {
    import_v2_data_response(data)
}

pub fn import_session_success(session: ImportSessionSummary) -> ImportV2RouteResponse {
    import_v2_data_response(session)
}

pub fn import_session_not_found_response() -> ImportV2RouteResponse {
    import_v2_error_response(404, "Session not found or expired")
}

pub fn import_session_cancel_missing_response() -> ImportV2RouteResponse {
    import_v2_message_response(200, false, "Session not found")
}

pub fn import_session_cancel_success_response() -> ImportV2RouteResponse {
    import_v2_message_response(200, true, "Session cleared")
}

pub fn import_v2_missing_session_id_response() -> ImportV2RouteResponse {
    import_v2_error_response(400, "Missing session_id")
}

pub fn import_v2_invalid_request_response() -> ImportV2RouteResponse {
    import_v2_error_response(400, "Invalid request")
}

pub fn expected_preview_state_is_valid(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Object(_)))
}

pub fn preview_state_conflict_response() -> ImportV2RouteResponse {
    import_v2_error_response(409, "Preview state changed, please refresh")
}

fn normalize_id_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) if text.is_empty() || text == "0" => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) if json_number_is_zero(number) => String::new(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Bool(true)) => "true".to_string(),
        Some(value) => value.to_string(),
    }
}

fn integer_lookup_key(value: Option<&Value>) -> Option<i64> {
    let text = normalize_id_text(value);
    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        text.parse::<i64>().ok()
    } else {
        None
    }
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a String>) -> Option<&'a str> {
    values
        .into_iter()
        .map(|value| value.as_str())
        .find(|value| !value.is_empty())
}

fn integer_field(value: &Value, key: &str) -> i64 {
    value
        .as_object()
        .map(|object| integer_field_from_map(object, key))
        .unwrap_or_default()
}

fn integer_field_from_map(map: &Map<String, Value>, key: &str) -> i64 {
    match map.get(key) {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1,
        _ => 0,
    }
}

fn float_field(value: &Value, key: &str) -> f64 {
    value
        .as_object()
        .map(|object| float_field_from_map(object, key))
        .unwrap_or_default()
}

fn float_field_from_map(map: &Map<String, Value>, key: &str) -> f64 {
    match map.get(key) {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .as_object()
        .map(|object| string_field_from_map(object, key))
        .unwrap_or_default()
}

fn string_field_from_map(map: &Map<String, Value>, key: &str) -> String {
    match map.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) if !json_number_is_zero(number) => number.to_string(),
        Some(Value::Bool(true)) => "true".to_string(),
        _ => String::new(),
    }
}

fn list_field_from_map(map: &Map<String, Value>, key: &str) -> Vec<Value> {
    match map.get(key) {
        Some(Value::Array(values)) => values.clone(),
        _ => Vec::new(),
    }
}

fn object_field_from_map<'a>(
    map: &'a Map<String, Value>,
    key: &str,
) -> Option<&'a Map<String, Value>> {
    object_field(map.get(key))
}

fn object_field(value: Option<&Value>) -> Option<&Map<String, Value>> {
    match value {
        Some(Value::Object(object)) => Some(object),
        _ => None,
    }
}

fn parse_dedup_source_ids(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::String(text)) => text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                if part.bytes().all(|byte| byte.is_ascii_digit()) {
                    part.parse::<i64>().map_or_else(
                        |_| Value::String(part.to_string()),
                        |number| Value::Number(Number::from(number)),
                    )
                } else {
                    Value::String(part.to_string())
                }
            })
            .collect(),
        Some(Value::Array(values)) => values.clone(),
        _ => Vec::new(),
    }
}

fn value_to_trimmed_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_to_trimmed_lowercase(value: &Value) -> String {
    value_to_trimmed_string(Some(value)).to_lowercase()
}

fn value_to_i64(value: &Value) -> i64 {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or_default(),
        Value::String(text) => text.trim().parse::<i64>().unwrap_or_default(),
        Value::Bool(true) => 1,
        _ => 0,
    }
}

fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => !json_number_is_zero(number),
        Value::String(text) => !text.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn json_number_is_zero(number: &Number) -> bool {
    number.as_i64() == Some(0) || number.as_u64() == Some(0) || number.as_f64() == Some(0.0)
}
