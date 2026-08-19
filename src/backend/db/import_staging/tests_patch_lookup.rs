#[test]
fn preview_patch_value_updates_category_identity_in_row_and_payload() {
    let mut row = preview_row(1);
    let mut payload = json!({});

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(42),
    )
    .expect("category id patch");

    assert_eq!(row.category_id, Some(42));
    assert_eq!(payload["category_id"], json!(42));
    assert_eq!(payload["categoryId"], json!(42));

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Null,
    )
    .expect("category clear patch");

    assert_eq!(row.category_id, None);
    assert_eq!(payload["category_id"], Value::Null);
    assert_eq!(payload["categoryId"], Value::Null);

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::Amount,
        ImportPreviewPatchValue::Integer(-12345),
    )
    .expect("amount patch");
    assert_eq!(row.preview_amount_cents, 12345);
    assert_eq!(payload["preview_amount_cents"], json!(12345));

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::DestinationAmount,
        ImportPreviewPatchValue::Integer(-54321),
    )
    .expect("destination amount patch");
    assert_eq!(row.preview_destination_amount_cents, 54321);
    assert_eq!(payload["preview_destination_amount_cents"], json!(54321));
}

#[test]
fn preview_patch_rejects_i64_min_money_without_panicking() {
    let mut row = preview_row(1);
    let mut payload = json!({});

    let error = apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::Amount,
        ImportPreviewPatchValue::Integer(i64::MIN),
    )
    .expect_err("i64::MIN cannot become a positive preview amount");

    assert!(matches!(
        error,
        DbError::InvalidOperation(message) if message == "invalid preview amount_cents"
    ));
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
    )
    .expect("manual annotation patch");

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

    let single = category_names_from_postgres_path(Some("理财"), "理财");
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

#[test]
fn transfer_learning_feedback_matches_learning_filter_but_plain_transfer_does_not() {
    let mut row = preview_row(1);
    row.preview_matching_feedback = json!({
        "transfer": {
            "review_status": "pending",
            "candidate_type": "cross_account",
            "learning_level": "green"
        }
    });
    assert!(signal_filter_matches(Some("transfer"), &row));
    assert!(signal_filter_matches(Some("learning"), &row));
    assert!(!signal_filter_matches(Some("learning:pending"), &row));

    let mut transfer_only = preview_row(4);
    transfer_only.preview_matching_feedback = json!({
        "transfer": {
            "review_status": "pending",
            "candidate_type": "cross_account"
        }
    });
    assert!(signal_filter_matches(Some("transfer"), &transfer_only));
    assert!(!signal_filter_matches(Some("learning"), &transfer_only));

    let mut suppressed = preview_row(3);
    suppressed.preview_matching_feedback = json!({
        "transfer": {
            "review_status": "pending",
            "candidate_type": "cross_account",
            "learning_level": "yellow",
            "suppressed": true
        }
    });
    assert!(!signal_filter_matches(Some("learning"), &suppressed));

    let mut plain_transfer = preview_row(2);
    plain_transfer.preview_type = "转账".to_string();
    plain_transfer.preview_matching_feedback = json!({});
    assert!(!signal_filter_matches(Some("transfer"), &plain_transfer));
    assert!(!signal_filter_matches(Some("learning"), &plain_transfer));
}

#[test]
fn manual_markers_distinguish_category_and_account_ownership() {
    let cases = [
        (ImportPreviewPatchField::CategoryId, "category_id"),
        (
            ImportPreviewPatchField::SourceAccountId,
            "source_account_id",
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            "destination_account_id",
        ),
    ];
    let ownership_fields = ["category_id", "source_account_id", "destination_account_id"];

    for (edited_field, expected_owned_field) in cases {
        let mut row = preview_row(1);
        row.preview_matching_feedback = json!({
            "transfer": {"owned_fields": ["category_id", "source_account_id", "destination_account_id"]}
        });
        let mut payload = json!({});
        // Mirrors the HTTP category resolver: the base builder emits the manual marker,
        // then canonical category resolution appends the identity change.
        let changes = vec![
            (
                ImportPreviewPatchField::ManualAnnotation,
                ImportPreviewPatchValue::Bool(true),
            ),
            (edited_field, ImportPreviewPatchValue::Integer(42)),
        ];
        apply_patch_changes_to_preview(&mut row, &mut payload, &changes)
            .expect("identity patch changes");

        for ownership_field in ownership_fields {
            let expected = ownership_field == expected_owned_field;
            assert_eq!(
                payload.pointer(&format!(
                    "/preview_matching_feedback/annotation/manual_fields/{ownership_field}"
                )),
                Some(&json!(expected)),
                "editing {expected_owned_field} must not mark {ownership_field}"
            );
        }
        assert_eq!(
            payload.pointer(&format!(
                "/preview_matching_feedback/transfer/owned_fields/{expected_owned_field}"
            )),
            Some(&json!(false)),
            "manual ownership must replace transfer ownership for the edited field"
        );
        assert_eq!(
            payload.pointer("/preview_matching_feedback/annotation/manual_fields/amount_cents"),
            None,
            "unrelated fields must not be marked"
        );
    }
}

#[test]
fn consecutive_manual_identity_patches_accumulate_field_ownership() {
    let mut row = preview_row(1);
    row.preview_matching_feedback = json!({
        "transfer": {
            "owned_fields": {
                "category_id": true,
                "source_account_id": true,
                "destination_account_id": true,
                "amount_cents": true
            }
        },
        "annotation": {
            "manual_fields": {"amount_cents": true}
        }
    });
    let mut payload = json!({});

    for field in [
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchField::SourceAccountId,
        ImportPreviewPatchField::DestinationAccountId,
    ] {
        // Mirrors separate real HTTP updates: base manual marker precedes a resolved
        // canonical identity change, and ownership must accumulate across requests.
        apply_patch_changes_to_preview(
            &mut row,
            &mut payload,
            &[
                (
                    ImportPreviewPatchField::ManualAnnotation,
                    ImportPreviewPatchValue::Bool(true),
                ),
                (field, ImportPreviewPatchValue::Integer(42)),
            ],
        )
        .expect("manual ownership patch changes");
    }

    for field in ["category_id", "source_account_id", "destination_account_id"] {
        assert_eq!(
            row.preview_matching_feedback.pointer(&format!(
                "/annotation/manual_fields/{field}"
            )),
            Some(&json!(true))
        );
        assert_eq!(
            row.preview_matching_feedback.pointer(&format!(
                "/transfer/owned_fields/{field}"
            )),
            Some(&json!(false))
        );
    }
    assert_eq!(
        row.preview_matching_feedback
            .pointer("/annotation/manual_fields/amount_cents"),
        Some(&json!(true))
    );
    assert_eq!(
        row.preview_matching_feedback
            .pointer("/transfer/owned_fields/amount_cents"),
        Some(&json!(true))
    );
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
            3::BIGINT AS version,
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

    assert_eq!(projected.version, 3);
    assert_eq!(projected.preview_amount_cents, 12345);
    assert_eq!(projected.preview_destination_amount_cents, 54321);
    assert_eq!(projected.preview_main_category, "餐饮");
    assert_eq!(projected.dedup_source_ids, vec![1, 2]);
    Ok(())
}

#[tokio::test]
async fn preview_pg_row_projection_treats_typed_identity_columns_as_authoritative(
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
        SELECT 7::BIGINT AS id, 4::BIGINT AS version,
            'session-1'::TEXT AS session_key, 2::BIGINT AS user_id,
            now() AS occurred_at, 'expense'::TEXT AS transaction_type, 111::BIGINT AS amount_cents,
            42::BIGINT AS category_id, 11::BIGINT AS account_id,
            12::BIGINT AS transfer_target_account_id, '商户'::TEXT AS merchant,
            '招商卡'::TEXT AS payment_method, '午餐'::TEXT AS description, true AS selected,
            ARRAY[]::BIGINT[] AS merged_source_ids,
            '{"category_id": 9, "categoryId": 9, "preview_source_account_id": 99,
              "preview_destination_account_id": 98}'::jsonb AS preview_payload,
            now() AS created_at
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let projected = preview_from_pg_row(&row)?;
    assert_eq!(projected.version, 4);
    assert_eq!(projected.category_id, Some(42));
    assert_eq!(projected.preview_source_account_id, Some(11));
    assert_eq!(projected.preview_destination_account_id, Some(12));
    Ok(())
}

#[tokio::test]
async fn preview_pg_row_projection_does_not_revive_payload_identity_when_typed_columns_are_null(
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
        SELECT 8::BIGINT AS id, 5::BIGINT AS version,
            'session-1'::TEXT AS session_key, 2::BIGINT AS user_id,
            now() AS occurred_at, 'expense'::TEXT AS transaction_type, 111::BIGINT AS amount_cents,
            NULL::BIGINT AS category_id, NULL::BIGINT AS account_id,
            NULL::BIGINT AS transfer_target_account_id, NULL::TEXT AS merchant,
            NULL::TEXT AS payment_method, NULL::TEXT AS description, false AS selected,
            ARRAY[]::BIGINT[] AS merged_source_ids,
            '{"category_id": 9, "categoryId": 9, "preview_source_account_id": 99,
              "preview_destination_account_id": 98}'::jsonb AS preview_payload,
            now() AS created_at
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let projected = preview_from_pg_row(&row)?;
    assert_eq!(projected.version, 5);
    assert_eq!(projected.category_id, None);
    assert_eq!(projected.preview_source_account_id, None);
    assert_eq!(projected.preview_destination_account_id, None);
    Ok(())
}
