use bill_analyser_core::{normalize_bill_date_text, Money};

use crate::normalization::{normalize_amount_text, normalize_transaction_type};
use crate::tags::{build_parser_tags, normalize_parser_tags};
use crate::{RawBill, StandardBill};

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

/// 聚合来源描述字段，去掉空值和占位符后形成可复核的导入描述文本。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 将来源局部账单行转换为导入草稿，仍不触碰 DB staging、分类、账户和 learning 决策。
#[tracing::instrument(level = "debug", skip_all)]
pub fn post_process_raw_bills(parser_id: &str, raw_bills: &[RawBill]) -> Vec<StandardBill> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "post_process_raw_bills",
        "business operation entered"
    );
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
            if matches!(normalized.as_str(), "投资" | "转账") {
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

#[allow(dead_code)]
fn _description_fields() -> &'static [&'static str] {
    DESCRIPTION_FIELDS
}
