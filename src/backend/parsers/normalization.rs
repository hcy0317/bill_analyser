use bill_analyser_core::Money;

use crate::money_serde::clean_amount_text;

/// 将来源侧交易类型文本归一为导入管线使用的收入、支出、转账、退款或投资。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 清洗来源侧金额文本并按元单位解析为内部 `Money`，无效金额回退为零。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_amount_text(raw_amount: &str) -> Money {
    let cleaned = clean_amount_text(raw_amount);
    Money::from_yuan_str(&cleaned).unwrap_or(Money::ZERO)
}
