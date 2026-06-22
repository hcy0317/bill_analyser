use serde::{Deserialize, Serialize};
use serde_json::Value;

use bill_analyser_core::Money;

use crate::normalization::normalize_amount_text;
use crate::tags::resolve_parser_tags;

/// parser 注册表行，供 HTTP/UI 展示来源名称、来源标签和支持的扩展名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ParserInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub source_label: &'static str,
    pub class_name: &'static str,
    pub channel_tag: &'static str,
    pub supported_extensions: &'static [&'static str],
}

/// 来源局部账单行，dedicated parser 只在这里保留原始字段，不推断分类、账户或 staging 决策。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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

/// parser 标准化后的导入草稿，JSON 兼容边界仍按元序列化，Rust 内部通过 `Money` 保留分单位。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardBill {
    pub date: String,
    #[serde(
        serialize_with = "crate::money_serde::serialize_money_as_yuan_number",
        deserialize_with = "crate::money_serde::deserialize_money_from_yuan_value"
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
    /// 从兼容 JSON row 恢复标准账单，统一补齐默认类型、金额和 parser tag 合同。
    #[tracing::instrument(level = "debug", skip_all)]
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
