#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportParserTemplateDraft {
    pub parser_date: String,
    pub parser_amount: f64,
    pub parser_type: String,
    pub parser_description: String,
    pub parser_id: String,
    pub parser_tags: Option<Value>,
    pub parser_counterparty: String,
    pub parser_payment_method: String,
    pub parser_original_type: String,
    pub parser_original_category: String,
    pub parser_account_id: String,
}

impl Default for ImportParserTemplateDraft {
    fn default() -> Self {
        Self {
            parser_date: String::new(),
            parser_amount: 0.0,
            parser_type: String::new(),
            parser_description: String::new(),
            parser_id: String::new(),
            parser_tags: None,
            parser_counterparty: String::new(),
            parser_payment_method: String::new(),
            parser_original_type: String::new(),
            parser_original_category: String::new(),
            parser_account_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportParserTemplateRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub parser_date: String,
    pub parser_amount: f64,
    pub parser_type: String,
    pub parser_description: String,
    pub parser_id: String,
    pub parser_tags: Vec<String>,
    pub parser_counterparty: String,
    pub parser_payment_method: String,
    pub parser_original_type: String,
    pub parser_original_category: String,
    pub parser_account_id: String,
    pub parser_is_processed: bool,
    pub created_at: String,
}

/// 把 parser 标准账单投影为 staging template，保持 parser 元单位边界到 DB 写入前才归一。
pub fn parser_template_draft_from_standard_bill(
    bill: &StandardBill,
    parser_id: &str,
) -> ImportParserTemplateDraft {
    ImportParserTemplateDraft {
        parser_date: bill.date.clone(),
        parser_amount: bill
            .amount
            .to_yuan_string()
            .parse::<f64>()
            .unwrap_or_default(),
        parser_type: parser_template_type(&bill.transaction_type, bill.amount),
        parser_description: bill.description.clone(),
        parser_id: parser_id.to_string(),
        parser_tags: standard_bill_parser_tags_value(bill),
        parser_counterparty: bill.counterparty.clone(),
        parser_payment_method: bill.payment_method.clone(),
        parser_original_type: bill.original_type.clone(),
        parser_original_category: bill.original_category.clone(),
        parser_account_id: bill.source_account_id.clone(),
    }
}

pub fn parser_template_drafts_from_standard_bills(
    bills: &[StandardBill],
    parser_id: &str,
) -> Vec<ImportParserTemplateDraft> {
    bills
        .iter()
        .map(|bill| parser_template_draft_from_standard_bill(bill, parser_id))
        .collect()
}

/// 把 staging template 还原成 dedup 输入，必须保留 parser_tags、原始类型和金额精度。
pub fn dedup_bill_from_parser_template(template: &ImportParserTemplateRow) -> DedupBill {
    let payment_method = first_non_empty([
        template.parser_payment_method.as_str(),
        template.parser_id.as_str(),
    ]);
    DedupBill {
        date: template.parser_date.clone(),
        amount: Money::from_yuan_str(&finite_float_text(template.parser_amount))
            .unwrap_or(Money::ZERO),
        transaction_type: template.parser_type.clone(),
        source_account_id: template.parser_account_id.clone(),
        parser_id: template.parser_id.clone(),
        source: template.parser_id.clone(),
        counterparty: template.parser_counterparty.clone(),
        payment_method,
        description: template.parser_description.clone(),
        original_type: template.parser_original_type.clone(),
        original_category: template.parser_original_category.clone(),
        template_id: Some(template.id.to_string()),
        parser_tags: template.parser_tags.clone(),
        session_id: Some(template.session_id.clone()),
        ..DedupBill::default()
    }
}

pub fn dedup_bills_from_parser_templates(templates: &[ImportParserTemplateRow]) -> Vec<DedupBill> {
    templates
        .iter()
        .map(dedup_bill_from_parser_template)
        .collect()
}
