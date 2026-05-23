// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use std::borrow::Cow;

use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use bill_analyser_core::{normalize_bill_date_text, Money};

mod dedicated;

pub use dedicated::{parse_dedicated_import_bytes, DedicatedParseResult};

const DESCRIPTION_FIELDS: &[&str] = &[
    "description",
    "counterparty",
    "goods",
    "product",
    "remark",
    "note",
    "memo",
    "abstract",
    "summary",
    "payment_method",
    "original_category",
    "merchant",
    "shop",
    "transaction_type",
    "opponent_account",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
/// Parser registry row exposed to HTTP/UI code so users can see which provider
/// recognized a file and which extensions are supported.
pub struct ParserInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub source_label: &'static str,
    pub class_name: &'static str,
    pub channel_tag: &'static str,
    pub supported_extensions: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Provider-local bill row before normalization. Dedicated parsers should keep
/// source wording here instead of guessing categories, accounts, or staging
/// decisions that belong to the import runtime.
pub struct RawBill {
    pub date: String,
    pub trade_time: String,
    pub amount: String,
    pub transaction_type: String,
    pub description: String,
    pub counterparty: String,
    pub goods: String,
    pub product: String,
    pub remark: String,
    pub note: String,
    pub memo: String,
    pub abstract_text: String,
    pub summary: String,
    pub payment_method: String,
    pub channel: String,
    pub original_category: String,
    pub merchant: String,
    pub shop: String,
    pub opponent_account: String,
    pub parser_tags: Vec<String>,
    pub transaction_id: String,
    pub order_id: String,
    pub merchant_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Import-runtime bill draft after parser normalization. Amounts serialize as
/// yuan numbers for parser/API parity, while `Money` still preserves cents
/// internally for later amount-boundary checks.
pub struct StandardBill {
    pub date: String,
    #[serde(
        serialize_with = "serialize_money_as_yuan_number",
        deserialize_with = "deserialize_money_from_yuan_value"
    )]
    pub amount: Money,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub description: String,
    pub source_account_id: String,
    pub counterparty: String,
    pub payment_method: String,
    pub parser_tags: Vec<String>,
    pub original_type: String,
    pub original_category: String,
    pub transaction_id: String,
    pub merchant_id: String,
    pub status: String,
    pub main_category: String,
    pub sub_category: String,
}

impl StandardBill {
    pub fn from_json_value(data: &Value) -> Self {
        let parser_id = string_field(data, "source_account_id");
        let payment_method = string_field(data, "payment_method");
        Self {
            date: string_field(data, "date"),
            amount: number_field(data, "amount"),
            transaction_type: normalized_json_type(data),
            description: string_field(data, "description"),
            source_account_id: parser_id.clone(),
            counterparty: string_field(data, "counterparty"),
            payment_method: payment_method.clone(),
            parser_tags: resolve_parser_tags(
                data.get("parser_tags"),
                &parser_id,
                &payment_method,
                "",
            ),
            original_type: string_field(data, "original_type"),
            original_category: string_field(data, "original_category"),
            transaction_id: string_field(data, "transaction_id"),
            merchant_id: string_field(data, "merchant_id"),
            status: string_field(data, "status"),
            main_category: string_field(data, "main_category"),
            sub_category: string_field(data, "sub_category"),
        }
    }
}

/// Stable provider list used by import route discovery and parser selection.
pub fn parser_registry() -> &'static [ParserInfo] {
    &[
        ParserInfo {
            id: "wechat",
            name: "微信支付",
            source_label: "微信",
            class_name: "WeChatParser",
            channel_tag: "wallet",
            supported_extensions: &[".csv", ".xlsx"],
        },
        ParserInfo {
            id: "alipay",
            name: "支付宝",
            source_label: "支付宝",
            class_name: "AlipayParser",
            channel_tag: "wallet",
            supported_extensions: &[".csv"],
        },
        ParserInfo {
            id: "icbc",
            name: "工商银行",
            source_label: "工商银行",
            class_name: "ICBCParser",
            channel_tag: "bank",
            supported_extensions: &[".csv", ".xlsx", ".xls"],
        },
        ParserInfo {
            id: "cmbc",
            name: "民生银行",
            source_label: "民生银行",
            class_name: "CMBCParser",
            channel_tag: "bank",
            supported_extensions: &[".csv", ".xlsx", ".xls"],
        },
        ParserInfo {
            id: "abc",
            name: "农业银行",
            source_label: "农业银行",
            class_name: "ABCParser",
            channel_tag: "bank",
            supported_extensions: &[".csv", ".xlsx", ".xls"],
        },
        ParserInfo {
            id: "ccb",
            name: "建设银行",
            source_label: "建设银行",
            class_name: "CCBParser",
            channel_tag: "bank",
            supported_extensions: &[".xlsx", ".xls"],
        },
    ]
}

pub fn parser_source_label(parser_id: &str) -> Cow<'static, str> {
    let normalized_parser_id = parser_id.trim().to_lowercase();
    match normalized_parser_id.as_str() {
        "" => Cow::Borrowed(""),
        "wechat" => Cow::Borrowed("微信"),
        "alipay" => Cow::Borrowed("支付宝"),
        "abc" => Cow::Borrowed("农业银行"),
        "ccb" => Cow::Borrowed("建设银行"),
        "cmbc" => Cow::Borrowed("民生银行"),
        "icbc" => Cow::Borrowed("工商银行"),
        "generic" => Cow::Borrowed("通用来源"),
        _ => Cow::Owned(normalized_parser_id),
    }
}

pub fn normalize_parser_tags(raw_tags: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut normalized = Vec::new();
    for raw_tag in raw_tags {
        let tag = raw_tag.as_ref().trim().to_lowercase();
        if !tag.is_empty() && !normalized.contains(&tag) {
            normalized.push(tag);
        }
    }
    normalized
}

pub fn normalize_parser_tags_value(raw_tags: Option<&Value>) -> Vec<String> {
    match raw_tags {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(text)) => normalize_parser_tags_text(text),
        Some(Value::Array(items)) => normalize_parser_tags(items.iter().map(normalize_json_tag)),
        _ => Vec::new(),
    }
}

pub fn normalize_parser_tags_text(raw_tags: &str) -> Vec<String> {
    let text = raw_tags.trim();
    if text.is_empty() {
        return Vec::new();
    }

    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return normalize_parser_tags_value(Some(&value));
    }

    normalize_parser_tags(text.split(','))
}

pub fn build_parser_tags(parser_id: &str, payment_method: &str, channel: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let normalized_parser_id = parser_id.trim().to_lowercase();
    if !normalized_parser_id.is_empty() {
        tags.push(format!("parser:{normalized_parser_id}"));
    }

    if let Some(channel_tag) = detect_channel_tag(parser_id, payment_method, channel) {
        tags.push(format!("channel:{channel_tag}"));
    }

    normalize_parser_tags(tags)
}

pub fn resolve_parser_tags(
    raw_tags: Option<&Value>,
    parser_id: &str,
    payment_method: &str,
    channel: &str,
) -> Vec<String> {
    let normalized = normalize_parser_tags_value(raw_tags);
    if normalized.is_empty() {
        build_parser_tags(parser_id, payment_method, channel)
    } else {
        normalized
    }
}

pub fn serialize_parser_tags(
    raw_tags: Option<&Value>,
    parser_id: &str,
    payment_method: &str,
    channel: &str,
) -> String {
    serde_json::to_string(&resolve_parser_tags(
        raw_tags,
        parser_id,
        payment_method,
        channel,
    ))
    .unwrap_or_else(|_| "[]".to_string())
}

pub fn normalize_transaction_type(raw_type: &str) -> String {
    let text = raw_type.trim();
    for (keyword, normalized) in [
        ("收入", "收入"),
        ("收款", "收入"),
        ("入账", "收入"),
        ("支出", "支出"),
        ("支付", "支出"),
        ("消费", "支出"),
        ("付款", "支出"),
        ("转账", "转账"),
        ("转出", "转账"),
        ("转入", "转账"),
        ("退款", "退款"),
        ("退钱", "退款"),
        ("不计收支", "转账"),
        ("投资理财", "投资"),
        ("投资", "投资"),
        ("理财", "投资"),
    ] {
        if text.contains(keyword) {
            return normalized.to_string();
        }
    }
    "支出".to_string()
}

pub fn normalize_amount_text(raw_amount: &str) -> Money {
    let cleaned = clean_amount_text(raw_amount);
    Money::from_yuan_str(&cleaned).unwrap_or(Money::ZERO)
}

pub fn aggregate_description(raw_bill: &RawBill) -> String {
    let values = [
        raw_bill.description.as_str(),
        raw_bill.counterparty.as_str(),
        raw_bill.goods.as_str(),
        raw_bill.product.as_str(),
        raw_bill.remark.as_str(),
        raw_bill.note.as_str(),
        raw_bill.memo.as_str(),
        raw_bill.abstract_text.as_str(),
        raw_bill.summary.as_str(),
        raw_bill.payment_method.as_str(),
        raw_bill.original_category.as_str(),
        raw_bill.merchant.as_str(),
        raw_bill.shop.as_str(),
        raw_bill.transaction_type.as_str(),
        raw_bill.opponent_account.as_str(),
    ];

    let mut parts = Vec::new();
    for value in values {
        let cleaned = value.trim();
        if cleaned.is_empty()
            || matches!(cleaned, "/" | "-" | "无" | "空" | "nan" | "None")
            || parts.iter().any(|part| part == cleaned)
        {
            continue;
        }
        parts.push(cleaned.to_string());
    }

    parts.join(" | ")
}

/// Converts provider-local rows into import drafts without touching DB staging.
/// This is the last parser-layer boundary before dedup, category, account,
/// transfer and learning decisions take over in the import pipeline.
pub fn post_process_raw_bills(parser_id: &str, raw_bills: &[RawBill]) -> Vec<StandardBill> {
    let mut processed = Vec::new();

    for raw_bill in raw_bills {
        let raw_date = if raw_bill.date.trim().is_empty() {
            raw_bill.trade_time.as_str()
        } else {
            raw_bill.date.as_str()
        };
        if raw_date.trim().is_empty() {
            continue;
        }

        let normalized_date = normalize_bill_date_text(raw_date);
        let original_type = raw_bill.transaction_type.clone();
        let original_category = raw_bill.original_category.clone();
        let amount = normalize_amount_text(&raw_bill.amount);
        let mut transaction_type = if raw_bill.transaction_type.trim().is_empty() {
            "支出".to_string()
        } else {
            let normalized = normalize_transaction_type(&raw_bill.transaction_type);
            if normalized == "转账" && raw_bill.transaction_type.trim().contains("不计收支") {
                normalized
            } else if matches!(normalized.as_str(), "投资" | "转账") {
                if amount.is_negative() {
                    "支出".to_string()
                } else {
                    "收入".to_string()
                }
            } else {
                normalized
            }
        };

        let signed_amount = match transaction_type.as_str() {
            "支出" => amount
                .checked_abs()
                .and_then(Money::checked_negated)
                .unwrap_or(Money::ZERO),
            "收入" | "退款" => amount.checked_abs().unwrap_or(Money::ZERO),
            _ => amount,
        };

        if normalized_date.is_empty() || signed_amount == Money::ZERO {
            continue;
        }

        if transaction_type.is_empty() {
            transaction_type = "支出".to_string();
        }

        processed.push(StandardBill {
            date: normalized_date,
            amount: signed_amount,
            transaction_type,
            description: aggregate_description(raw_bill),
            source_account_id: parser_id.to_string(),
            counterparty: raw_bill.counterparty.clone(),
            payment_method: if raw_bill.payment_method.is_empty() {
                raw_bill.channel.clone()
            } else {
                raw_bill.payment_method.clone()
            },
            parser_tags: if raw_bill.parser_tags.is_empty() {
                build_parser_tags(parser_id, &raw_bill.payment_method, &raw_bill.channel)
            } else {
                normalize_parser_tags(&raw_bill.parser_tags)
            },
            original_type,
            original_category,
            transaction_id: if raw_bill.transaction_id.is_empty() {
                raw_bill.order_id.clone()
            } else {
                raw_bill.transaction_id.clone()
            },
            merchant_id: raw_bill.merchant_id.clone(),
            status: raw_bill.status.clone(),
            main_category: String::new(),
            sub_category: String::new(),
        });
    }

    processed
}

fn detect_channel_tag(
    parser_id: &str,
    payment_method: &str,
    channel: &str,
) -> Option<&'static str> {
    match parser_id.trim().to_lowercase().as_str() {
        "wechat" | "alipay" => return Some("wallet"),
        "abc" | "ccb" | "cmbc" | "icbc" => return Some("bank"),
        _ => {}
    }

    let combined = format!("{payment_method} {channel}").to_lowercase();
    if ["wechat", "微信", "alipay", "支付宝", "零钱", "wallet"]
        .iter()
        .any(|keyword| combined.contains(keyword))
    {
        return Some("wallet");
    }
    if ["信用卡", "credit"]
        .iter()
        .any(|keyword| combined.contains(keyword))
    {
        return Some("credit_card");
    }
    if ["银行", "bank", "借记卡", "储蓄卡"]
        .iter()
        .any(|keyword| combined.contains(keyword))
    {
        return Some("bank");
    }
    None
}

fn normalize_json_tag(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(false) => String::new(),
        Value::Bool(true) => "true".to_string(),
        Value::Number(number) if is_json_zero_number(number) => String::new(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn is_json_zero_number(number: &serde_json::Number) -> bool {
    number.as_i64() == Some(0) || number.as_u64() == Some(0) || number.as_f64() == Some(0.0)
}

fn string_field(data: &Value, key: &str) -> String {
    match data.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn number_field(data: &Value, key: &str) -> Money {
    match data.get(key) {
        Some(Value::Number(number)) => {
            Money::from_yuan_str(&number.to_string()).unwrap_or(Money::ZERO)
        }
        Some(Value::String(text)) => normalize_amount_text(text),
        _ => Money::ZERO,
    }
}

fn normalized_json_type(data: &Value) -> String {
    let transaction_type = string_field(data, "type");
    if transaction_type.trim().is_empty() {
        "支出".to_string()
    } else {
        transaction_type
    }
}

fn serialize_money_as_yuan_number<S>(amount: &Money, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // The Python parser contract exposes StandardBill.amount as a JSON number
    // in yuan. Rust keeps cents in Money and converts only at this serde edge.
    let yuan_value = amount
        .to_yuan_string()
        .parse::<f64>()
        .map_err(S::Error::custom)?;
    serializer.serialize_f64(yuan_value)
}

fn deserialize_money_from_yuan_value<'de, D>(deserializer: D) -> Result<Money, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(number) => {
            Money::from_yuan_str(&number.to_string()).map_err(D::Error::custom)
        }
        Value::String(text) => {
            Money::from_yuan_str(&clean_amount_text(&text)).map_err(D::Error::custom)
        }
        Value::Null => Ok(Money::ZERO),
        _ => Err(D::Error::custom("invalid yuan amount")),
    }
}

fn clean_amount_text(raw_amount: &str) -> String {
    raw_amount
        .replace(['¥', '$', ',', '，'], "")
        .trim()
        .to_string()
}

#[allow(dead_code)]
fn _description_fields() -> &'static [&'static str] {
    DESCRIPTION_FIELDS
}
