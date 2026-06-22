// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::collections::{HashMap, HashSet};

use chrono::NaiveDateTime;
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::primitives::{parse_bill_datetime, Money};

const PLATFORM_SOURCES: &[&str] = &["wechat", "alipay"];
const BANK_SOURCES: &[&str] = &["icbc", "cmbc", "abc", "ccb"];
const TIME_TOLERANCE_SECONDS: i64 = 30;
const DATABASE_TIME_TOLERANCE_SECONDS: i64 = 300;
const AMOUNT_TOLERANCE_CENTS: i128 = 1;
const SIMILARITY_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationType {
    Exact,
    SameBatch,
    Transfer,
    PlatformBank,
    Similar,
    SplitMerge,
    DatabaseDuplicate,
}

impl DeduplicationType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::SameBatch => "same_batch",
            Self::Transfer => "transfer",
            Self::PlatformBank => "platform_bank",
            Self::Similar => "similar",
            Self::SplitMerge => "split_merge",
            Self::DatabaseDuplicate => "database_duplicate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationCandidateType {
    Duplicate,
    Transfer,
}

impl ReconciliationCandidateType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Transfer => "transfer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MergedBillSource {
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub date: String,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub amount: Option<String>,
    #[serde(
        default,
        rename = "_template_id",
        alias = "template_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TransferSourceSnapshot {
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub role: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub original_type: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub parser_id: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub payment_method: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub counterparty: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub source_account_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub account_name: String,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub bill_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub template_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_vec")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DedupBill {
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub date: String,
    #[serde(
        default = "default_money",
        deserialize_with = "deserialize_yuan_money",
        serialize_with = "serialize_yuan_money"
    )]
    pub amount: Money,
    #[serde(default, rename = "type", deserialize_with = "deserialize_stringish")]
    pub transaction_type: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source_account_id: String,
    #[serde(
        default,
        rename = "_parser_id",
        alias = "parser_id",
        deserialize_with = "deserialize_stringish"
    )]
    pub parser_id: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub source: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub counterparty: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub payment_method: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub original_type: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub original_category: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub main_category: String,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub sub_category: String,
    #[serde(
        default,
        rename = "_template_id",
        alias = "template_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub template_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_stringish")]
    pub account_name: String,
    #[serde(
        default,
        rename = "_parser_tags",
        alias = "parser_tags",
        deserialize_with = "deserialize_string_vec"
    )]
    pub parser_tags: Vec<String>,
    #[serde(
        default,
        rename = "_dedup_type",
        alias = "dedup_type",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub dedup_type: Option<String>,
    #[serde(
        default,
        rename = "_removed",
        alias = "removed",
        deserialize_with = "deserialize_boolish"
    )]
    pub removed: bool,
    #[serde(
        default,
        rename = "_merged_template_ids",
        alias = "merged_template_ids",
        deserialize_with = "deserialize_string_vec"
    )]
    pub merged_template_ids: Vec<String>,
    #[serde(default, rename = "_merged_from")]
    pub merged_from: Vec<MergedBillSource>,
    #[serde(
        default,
        rename = "_merged_into",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub merged_into: Option<String>,
    #[serde(
        default,
        rename = "_duplicate_of_db_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub duplicate_of_db_id: Option<String>,
    #[serde(
        default,
        rename = "_cross_batch_db_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub cross_batch_db_id: Option<String>,
    #[serde(
        default,
        rename = "_transfer_pair_order",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub transfer_pair_order: Option<String>,
    #[serde(default, rename = "_transfer_pair_sources")]
    pub transfer_pair_sources: Vec<TransferSourceSnapshot>,
    #[serde(
        default,
        rename = "_destination_parser_id",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_parser_id: String,
    #[serde(
        default,
        rename = "_destination_payment_method",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_payment_method: String,
    #[serde(
        default,
        rename = "_destination_counterparty",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_counterparty: String,
    #[serde(
        default,
        rename = "_destination_account_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub destination_account_id: Option<String>,
    #[serde(
        default,
        rename = "_destination_account_name",
        deserialize_with = "deserialize_stringish"
    )]
    pub destination_account_name: String,
    #[serde(
        default,
        rename = "_session_id",
        alias = "session_id",
        alias = "import_session_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub session_id: Option<String>,
    #[serde(
        default,
        rename = "_preview_id",
        alias = "preview_id",
        deserialize_with = "deserialize_optional_stringish"
    )]
    pub preview_id: Option<String>,
    #[serde(
        default,
        rename = "_skip_reconciliation_candidate",
        deserialize_with = "deserialize_boolish"
    )]
    pub skip_reconciliation_candidate: bool,
}

impl Default for DedupBill {
    fn default() -> Self {
        Self {
            id: None,
            date: String::new(),
            amount: Money::ZERO,
            transaction_type: String::new(),
            source_account_id: String::new(),
            parser_id: String::new(),
            source: String::new(),
            counterparty: String::new(),
            payment_method: String::new(),
            description: String::new(),
            original_type: String::new(),
            original_category: String::new(),
            main_category: String::new(),
            sub_category: String::new(),
            template_id: None,
            account_name: String::new(),
            parser_tags: Vec::new(),
            dedup_type: None,
            removed: false,
            merged_template_ids: Vec::new(),
            merged_from: Vec::new(),
            merged_into: None,
            duplicate_of_db_id: None,
            cross_batch_db_id: None,
            transfer_pair_order: None,
            transfer_pair_sources: Vec::new(),
            destination_parser_id: String::new(),
            destination_payment_method: String::new(),
            destination_counterparty: String::new(),
            destination_account_id: None,
            destination_account_name: String::new(),
            session_id: None,
            preview_id: None,
            skip_reconciliation_candidate: false,
        }
    }
}

impl DedupBill {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn source_type(&self) -> String {
        let parser_id = normalized_source(&self.parser_id);
        if !parser_id.is_empty() {
            return parser_id;
        }

        let source = normalized_source(&self.source);
        if !source.is_empty() {
            return source;
        }

        let source_account_id = normalized_source(&self.source_account_id);
        if is_platform_source(&source_account_id) || is_bank_source(&source_account_id) {
            source_account_id
        } else {
            String::new()
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn source_identifier(&self) -> String {
        let parser_id = normalized_source(&self.parser_id);
        if parser_id.is_empty() {
            normalized_source(&self.source_account_id)
        } else {
            parser_id
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn dedup_source_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        append_optional_id(&mut ids, &self.template_id);
        append_ids(&mut ids, &self.merged_template_ids);
        ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub dedup_type: DeduplicationType,
    pub bill_indices: Vec<usize>,
    pub keep_index: usize,
    pub remove_indices: Vec<usize>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferPair {
    pub outgoing_index: usize,
    pub incoming_index: usize,
    pub cross_batch_db_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitGroup {
    pub total_index: usize,
    pub split_indices: Vec<usize>,
    #[serde(
        default = "default_money",
        deserialize_with = "deserialize_yuan_money",
        serialize_with = "serialize_yuan_money"
    )]
    pub total_amount: Money,
    pub source_total: String,
    pub source_splits: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeduplicationResult {
    pub original_count: usize,
    pub kept_bills: Vec<DedupBill>,
    pub removed_count: usize,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub transfer_pairs: Vec<TransferPair>,
    pub split_groups: Vec<SplitGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseDuplicateMatch {
    pub imported_index: usize,
    pub existing_index: usize,
    pub existing_bill_id: Option<String>,
    pub is_platform_bank_pair: bool,
    pub dedup_type: DeduplicationType,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBatchTransferMatch {
    pub imported_index: usize,
    pub existing_index: usize,
    pub existing_bill_id: Option<String>,
    pub dedup_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReconciliationCandidate {
    pub candidate_id: String,
    pub candidate_type: ReconciliationCandidateType,
    pub import_bill_key: String,
    pub existing_bill_id: Option<String>,
    pub group_key: String,
    #[serde(
        default = "default_money",
        deserialize_with = "deserialize_yuan_money",
        serialize_with = "serialize_yuan_money"
    )]
    pub amount_abs: Money,
    pub time_diff_seconds: i64,
    pub score_percent: u8,
    pub level: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SmartDeduplicationEngine;

include!("smart_dedup/engine.rs");
include!("smart_dedup/exact.rs");
include!("smart_dedup/same_batch.rs");
include!("smart_dedup/platform_bank.rs");
include!("smart_dedup/transfer_pairs.rs");
include!("smart_dedup/similar_duplicates.rs");
include!("smart_dedup/split_groups.rs");
include!("smart_dedup/merge_helpers.rs");
include!("smart_dedup/matching_helpers.rs");
include!("smart_dedup/reconciliation_helpers.rs");

fn default_money() -> Money {
    Money::ZERO
}

// 中文说明：把导入/去重 JSON 中的元单位金额转换为核心 Money，兼容数字、字符串和空值。
fn deserialize_yuan_money<'de, D>(deserializer: D) -> Result<Money, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    let text = match value {
        Value::Null => return Ok(Money::ZERO),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text,
        other => {
            return Err(D::Error::custom(format!(
                "expected yuan amount string or number, got {other}"
            )))
        }
    };
    if text.trim().is_empty() {
        return Ok(Money::ZERO);
    }
    Money::from_yuan_str(&text).map_err(D::Error::custom)
}

fn serialize_yuan_money<S>(amount: &Money, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(amount.to_cents() as f64 / 100.0)
}

// 中文说明：把任意 JSON 标量归一为字符串，供导入去重 DTO 兼容数字、布尔和对象旧值。
fn deserialize_stringish<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value_to_string(&value).unwrap_or_default())
}

// 中文说明：把任意 JSON 标量归一为可空字符串，空白值保持 None 以避免制造伪来源字段。
fn deserialize_optional_stringish<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value_to_string(&value).filter(|value| !value.trim().is_empty()))
}

// 中文说明：把来源 id、parser tags 等字段兼容解析为字符串数组，支持 JSON 数组、逗号文本和单值。
fn deserialize_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Null => Vec::new(),
        Value::Array(values) => values
            .iter()
            .filter_map(value_to_string)
            .filter(|value| !value.trim().is_empty())
            .collect(),
        Value::String(text) => parse_string_vec_text(&text),
        other => value_to_string(&other)
            .filter(|value| !value.trim().is_empty())
            .into_iter()
            .collect(),
    })
}

// 中文说明：兼容导入数据中的 bool、数字和字符串布尔值，保持旧 payload 的 suppressed/selected 语义。
fn deserialize_boolish<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Bool(value) => value,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => matches!(
            text.trim().to_lowercase().as_str(),
            "true" | "1" | "yes" | "y"
        ),
        _ => false,
    })
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：解析字符串数组字段，优先兼容 JSON 数组文本，再回退到逗号分隔形式。
fn parse_string_vec_text(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('[') {
        if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(text) {
            return values
                .iter()
                .filter_map(value_to_string)
                .filter(|value| !value.trim().is_empty())
                .collect();
        }
    }
    text.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}
