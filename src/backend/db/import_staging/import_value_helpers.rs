fn standard_bill_parser_tags_value(bill: &StandardBill) -> Option<Value> {
    if bill.parser_tags.is_empty() {
        None
    } else {
        serde_json::to_value(&bill.parser_tags).ok()
    }
}

fn dedup_bill_parser_tags_value(bill: &DedupBill) -> Option<Value> {
    let mut tags = bill.parser_tags.clone();
    if !bill.destination_parser_id.trim().is_empty() {
        tags.push(format!("parser:{}", bill.destination_parser_id.trim()));
    }
    if tags.is_empty() {
        None
    } else {
        serde_json::to_value(tags).ok()
    }
}

fn parser_template_type(transaction_type: &str, amount: Money) -> String {
    let transaction_type = transaction_type.trim();
    if !transaction_type.is_empty() {
        return transaction_type.to_string();
    }

    if amount.is_positive() {
        "收入".to_string()
    } else if amount.is_negative() {
        "支出".to_string()
    } else {
        "其他".to_string()
    }
}

fn preview_destination_amount_for_bill(bill: &DedupBill, amount: f64) -> f64 {
    if is_investment_type(&bill.transaction_type) {
        amount.abs()
    } else {
        0.0
    }
}

fn is_investment_type(bill_type: &str) -> bool {
    matches!(
        bill_type.trim().to_ascii_lowercase().as_str(),
        "投资" | "investment" | "5"
    )
}

fn money_to_yuan_f64(amount: Money) -> f64 {
    amount.to_yuan_string().parse::<f64>().unwrap_or_default()
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn first_non_empty(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    values
        .into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

pub fn calculate_import_bill_hash(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
) -> String {
    let source = format!(
        "{}|{}|{}|{}|{}",
        date,
        bill_type,
        python_float_text(amount),
        counterparty,
        description
    );
    format!("{:x}", md5::compute(source.as_bytes()))
}

fn confirm_amount_for_type(bill_type: &str, preview_amount: f64) -> f64 {
    let amount = preview_amount.abs();
    if matches!(bill_type, "支出" | "expense") {
        -amount
    } else {
        amount
    }
}

fn normalize_confirm_bill_type(bill_type: &str) -> String {
    let trimmed = bill_type.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "3" | "expense" => "支出".to_string(),
        "2" | "income" => "收入".to_string(),
        "4" | "transfer" => "转账".to_string(),
        "5" | "investment" => "投资".to_string(),
        _ => trimmed.to_string(),
    }
}

fn python_float_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

#[cfg(test)]
mod import_value_helper_tests {
    use super::*;

    fn standard_bill(
        transaction_type: &str,
        amount: Money,
        parser_tags: Vec<String>,
    ) -> StandardBill {
        StandardBill {
            date: "2026-05-01".to_string(),
            amount,
            transaction_type: transaction_type.to_string(),
            description: "coffee".to_string(),
            source_account_id: "12".to_string(),
            counterparty: "cafe".to_string(),
            payment_method: "card".to_string(),
            parser_tags,
            original_type: String::new(),
            original_category: String::new(),
            transaction_id: String::new(),
            merchant_id: String::new(),
            status: String::new(),
            main_category: String::new(),
            sub_category: String::new(),
        }
    }

    #[test]
    fn parser_template_and_preview_value_helpers_cover_empty_edges() {
        let income =
            parser_template_draft_from_standard_bill(&standard_bill("", Money::from_cents(1234), Vec::new()), "wechat");
        assert_eq!(income.parser_type, "收入");
        assert!(income.parser_tags.is_none());

        let expense = parser_template_draft_from_standard_bill(
            &standard_bill("", Money::from_cents(-1234), vec!["wechat".to_string()]),
            "wechat",
        );
        assert_eq!(expense.parser_type, "支出");
        assert_eq!(expense.parser_tags, Some(serde_json::json!(["wechat"])));

        let zero =
            parser_template_draft_from_standard_bill(&standard_bill("", Money::ZERO, Vec::new()), "wechat");
        assert_eq!(zero.parser_type, "其他");

        let explicit = parser_template_draft_from_standard_bill(
            &standard_bill("转账", Money::ZERO, Vec::new()),
            "wechat",
        );
        assert_eq!(explicit.parser_type, "转账");

        let investment = preview_draft_from_dedup_bill(&DedupBill {
            amount: Money::from_cents(-2500),
            transaction_type: "investment".to_string(),
            parser_id: "icbc".to_string(),
            destination_parser_id: "alipay".to_string(),
            source_account_id: " ".to_string(),
            destination_account_id: Some("7".to_string()),
            ..DedupBill::default()
        });
        assert_eq!(investment.preview_destination_amount, 25.0);
        assert_eq!(investment.preview_payment_method, "icbc");
        assert_eq!(investment.preview_source_account_id, None);
        assert_eq!(investment.preview_destination_account_id, Some(7));
        assert_eq!(
            investment.preview_parser_tags,
            Some(serde_json::json!(["parser:alipay"]))
        );

        let no_tag_preview = preview_draft_from_dedup_bill(&DedupBill::default());
        assert!(no_tag_preview.preview_parser_tags.is_none());
        assert_eq!(parse_positive_i64(" "), None);
        assert_eq!(confirm_amount_for_type("income", -12.5), 12.5);
    }
}
