#[test]
fn preview_bulk_insert_value_preserves_amount_and_category_identity() {
    let draft = ImportPreviewDraft {
        preview_date: "2026-01-01 09:00:00".to_string(),
        preview_type: "收入".to_string(),
        preview_amount_cents: 1234,
        category_id: Some(42),
        preview_main_category: "理财".to_string(),
        preview_sub_category: "理财收益".to_string(),
        preview_counterparty: "基金平台".to_string(),
        preview_payment_method: "招商卡".to_string(),
        preview_description: "收益".to_string(),
        preview_selected: true,
        dedup_source_ids: vec![7, 8],
        ..ImportPreviewDraft::default()
    };

    let value = preview_row_batch_value_from_draft(&draft);
    let payload: Value = serde_json::from_str(&value.preview_payload).expect("valid payload");

    assert_eq!(value.amount_cents, 1234);
    assert_eq!(value.direction, "income");
    assert_eq!(value.category_id, Some(42));
    assert_eq!(value.merged_source_ids, vec![7, 8]);
    assert_eq!(payload["category_id"], json!(42));
    assert_eq!(payload["categoryId"], json!(42));
}
