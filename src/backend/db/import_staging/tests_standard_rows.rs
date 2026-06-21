#[test]
fn standard_row_bulk_insert_value_preserves_parser_payload_and_amount() {
    let draft = ImportParserTemplateDraft {
        parser_date: "2026-01-01 09:00:00".to_string(),
        parser_amount: -19.88,
        parser_type: "支出".to_string(),
        parser_description: "午餐".to_string(),
        parser_id: "fixture".to_string(),
        parser_counterparty: "餐厅".to_string(),
        parser_payment_method: "招商卡".to_string(),
        parser_original_type: "消费".to_string(),
        parser_original_category: "餐饮".to_string(),
        parser_account_id: "11".to_string(),
        ..ImportParserTemplateDraft::default()
    };

    let value = standard_row_batch_value_from_parser_template(5, 3, &draft);
    let parser_payload: Value =
        serde_json::from_str(&value.parser_payload).expect("valid parser payload");

    assert_eq!(value.source_id, 5);
    assert_eq!(value.source_row_index, 3);
    assert_eq!(value.amount_cents, 1988);
    assert_eq!(value.direction, "expense");
    assert_eq!(parser_payload["parser_id"], json!("fixture"));

    let values = standard_row_batch_values_from_parser_templates(5, &[draft]);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].source_row_index, 0);
    assert_eq!(values[0].parser_payload, value.parser_payload);
}

#[test]
fn standard_row_batch_values_from_drafts_use_source_fallback_and_parser_payload() {
    let parser_draft = ImportParserTemplateDraft {
        parser_id: "wechat_pay".to_string(),
        parser_original_type: "交易".to_string(),
        parser_original_category: "餐饮".to_string(),
        parser_account_id: "card-1".to_string(),
        ..ImportParserTemplateDraft::default()
    };
    let standard_row = ImportStandardRowDraft {
        source_index: 99,
        source_row_index: 3,
        occurred_at: "2026-01-01 09:00:00".to_string(),
        amount_cents: 1234,
        direction: "expense".to_string(),
        transaction_type: "支出".to_string(),
        merchant: "餐厅".to_string(),
        payment_method: "招商卡".to_string(),
        description: "午餐".to_string(),
        parser_payload: json!({"raw": true}),
        standard_payload: json!({"normalized": true}),
    };
    let mut source_ids = std::collections::BTreeMap::new();
    source_ids.insert(0, 100);

    let values = standard_row_batch_values_from_standard_row_drafts(
        &[parser_draft],
        &[standard_row],
        &source_ids,
    )
    .expect("source fallback value");

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].source_id, 100);
    assert_eq!(values[0].source_row_index, 3);
    let parser_payload: Value =
        serde_json::from_str(&values[0].parser_payload).expect("parser payload json");
    let standard_payload: Value =
        serde_json::from_str(&values[0].standard_payload).expect("standard payload json");
    assert_eq!(parser_payload["parser_id"], json!("wechat_pay"));
    assert_eq!(parser_payload["parser_original_category"], json!("餐饮"));
    assert_eq!(standard_payload["normalized"], json!(true));

    let missing = standard_row_batch_values_from_standard_row_drafts(&[], &[], &source_ids);
    assert_eq!(missing.expect("empty standard rows").len(), 0);
    let missing_source = standard_row_batch_values_from_standard_row_drafts(
        &[],
        &[ImportStandardRowDraft {
            source_index: 1,
            source_row_index: 0,
            occurred_at: String::new(),
            amount_cents: 0,
            direction: String::new(),
            transaction_type: String::new(),
            merchant: String::new(),
            payment_method: String::new(),
            description: String::new(),
            parser_payload: json!({}),
            standard_payload: json!({}),
        }],
        &std::collections::BTreeMap::new(),
    );
    assert!(missing_source.is_err());
}
