#[test]
fn preview_patch_value_updates_category_identity_in_row_and_payload() {
    let mut row = preview_row(1);
    let mut payload = json!({});

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(42),
    );

    assert_eq!(row.category_id, Some(42));
    assert_eq!(payload["category_id"], json!(42));
    assert_eq!(payload["categoryId"], json!(42));

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Null,
    );

    assert_eq!(row.category_id, None);
    assert_eq!(payload["category_id"], Value::Null);
    assert_eq!(payload["categoryId"], Value::Null);

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::Amount,
        ImportPreviewPatchValue::Integer(-12345),
    );
    assert_eq!(row.preview_amount_cents, 12345);
    assert_eq!(payload["preview_amount_cents"], json!(12345));

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::DestinationAmount,
        ImportPreviewPatchValue::Integer(-54321),
    );
    assert_eq!(row.preview_destination_amount_cents, 54321);
    assert_eq!(payload["preview_destination_amount_cents"], json!(54321));
}

#[test]
fn preview_patch_value_marks_manual_annotation_without_replacing_feedback() {
    let mut row = preview_row(1);
    row.preview_matching_feedback = json!({
        "parser": {"parser_id": "alipay"}
    });
    let mut payload = json!({});

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::ManualAnnotation,
        ImportPreviewPatchValue::Bool(true),
    );

    assert_eq!(
        row.preview_matching_feedback.pointer("/parser/parser_id"),
        Some(&json!("alipay"))
    );
    assert_eq!(
        row.preview_matching_feedback
            .pointer("/annotation/is_manually_annotated"),
        Some(&json!(true))
    );
    assert_eq!(
        payload.pointer("/preview_matching_feedback/parser/parser_id"),
        Some(&json!("alipay"))
    );
    assert_eq!(
        payload.pointer("/preview_matching_feedback/annotation/is_manually_annotated"),
        Some(&json!(true))
    );
}

#[test]
fn preview_category_lookup_helpers_preserve_type_path_and_sql_contract() {
    let lookup =
        import_preview_category_lookup_from_values("理财收益", "理财/理财收益", Some("income"));

    assert_eq!(lookup.type_code, Some(2));
    assert_eq!(lookup.main_category, "理财");
    assert_eq!(lookup.sub_category, "理财收益");
    assert!(import_preview_category_lookup_sql().contains("FROM categories"));
    assert!(import_preview_category_lookup_sql().contains("is_active = true"));

    let fallback = import_preview_category_lookup_from_values("未分类", "", Some("unknown"));
    assert_eq!(fallback.type_code, None);
    assert_eq!(fallback.main_category, "未分类");
    assert!(fallback.sub_category.is_empty());

    let single = preview_category_names_from_path("理财", "理财");
    assert_eq!(single, ("理财".to_string(), String::new()));
}

#[test]
fn preview_filter_helpers_cover_none_account_and_nested_feedback_edges() {
    let mut row = preview_row(1);
    row.preview_source_account_id = None;
    row.preview_destination_account_id = None;
    row.preview_payment_method.clear();
    row.preview_matching_feedback = json!({
        "learning": {"review_status": "needs_review", "reason": "manual"},
        "debug": [12, true, null]
    });

    assert!(account_filter_matches(Some("__none__"), &row));
    assert!(signal_filter_matches(Some("learning:needs_review"), &row));
    assert!(!signal_filter_matches(Some("manual"), &row));
    assert!(!signal_filter_matches(Some("12"), &row));
    assert!(!signal_filter_matches(Some("true"), &row));
    assert!(!signal_filter_matches(Some("missing"), &row));
}

#[tokio::test]
async fn preview_pg_row_projection_reads_explicit_cents_payload_when_database_available(
) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    let row = sqlx::query(
        r#"
        SELECT
            7::BIGINT AS id,
            'session-1'::TEXT AS session_key,
            2::BIGINT AS user_id,
            now() AS occurred_at,
            'expense'::TEXT AS transaction_type,
            111::BIGINT AS amount_cents,
            9::BIGINT AS category_id,
            11::BIGINT AS account_id,
            12::BIGINT AS transfer_target_account_id,
            '商户'::TEXT AS merchant,
            '招商卡'::TEXT AS payment_method,
            '午餐'::TEXT AS description,
            true AS selected,
            ARRAY[1, 2]::BIGINT[] AS merged_source_ids,
            '{
                "preview_date": "2026-06-01 09:00:00",
                "preview_type": "支出",
                "preview_amount_cents": 12345,
                "preview_destination_amount_cents": 54321,
                "preview_main_category": "餐饮",
                "preview_sub_category": "午餐",
                "preview_matching_feedback": {"learning": "accepted"}
            }'::jsonb AS preview_payload,
            now() AS created_at
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let projected = preview_from_pg_row(&row).expect("preview row");

    assert_eq!(projected.preview_amount_cents, 12345);
    assert_eq!(projected.preview_destination_amount_cents, 54321);
    assert_eq!(projected.preview_main_category, "餐饮");
    assert_eq!(projected.dedup_source_ids, vec![1, 2]);
    Ok(())
}
