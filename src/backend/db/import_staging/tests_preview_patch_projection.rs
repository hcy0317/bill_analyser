#[test]
fn preview_patch_projection_applies_manual_identity_and_clears_decision_signals() {
    let mut row = preview_row(1);
    row.preview_matching_feedback = json!({
        "parser": {"parser_id": "alipay"},
        "transfer": {
            "review_status": "pending",
            "owned_fields": {
                "category_id": true,
                "source_account_id": true,
                "destination_account_id": true
            }
        },
        "learning": {"review_status": "pending"},
        "llm": {"review_status": "pending"}
    });
    let payload = json!({
        "preview_matching_feedback": row.preview_matching_feedback.clone()
    });
    let patch = ImportPreviewPatch::new(row.id)
        .with_changes([
            (
                ImportPreviewPatchField::ManualAnnotation,
                ImportPreviewPatchValue::Bool(true),
            ),
            (
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(42),
            ),
            (
                ImportPreviewPatchField::SourceAccountId,
                ImportPreviewPatchValue::Integer(12),
            ),
        ])
        .with_transfer_decision_cleared()
        .with_learning_decision_cleared()
        .with_llm_decision_cleared();
    let identity_maps = ImportIdentityMaps {
        active_accounts: BTreeSet::from([11, 12]),
        active_categories: BTreeMap::from([(42, Some(3))]),
    };

    let projection = project_preview_patch(row, payload, &patch, &identity_maps)
        .expect("shared preview patch projection");

    assert_eq!(projection.preview.category_id, Some(42));
    assert_eq!(projection.preview.preview_source_account_id, Some(12));
    assert_eq!(projection.amount_cents, 1000);
    assert_eq!(projection.direction, "expense");
    assert_eq!(
        projection
            .payload
            .pointer("/preview_matching_feedback/parser/parser_id"),
        Some(&json!("alipay"))
    );
    for family in ["transfer", "learning", "llm"] {
        assert_eq!(
            projection
                .payload
                .pointer(&format!("/preview_matching_feedback/{family}")),
            None,
            "cleared {family} decision must not remain observable"
        );
    }
    assert_eq!(
        projection
            .payload
            .pointer("/preview_matching_feedback/annotation/manual_fields/category_id"),
        Some(&json!(true))
    );
    assert_eq!(
        projection
            .payload
            .pointer("/preview_matching_feedback/annotation/manual_fields/source_account_id"),
        Some(&json!(true))
    );
    assert_eq!(
        projection
            .payload
            .pointer("/preview_matching_feedback/annotation/manual_fields/destination_account_id"),
        Some(&json!(false))
    );
    assert!(!projection.signal_projection.transfer);
    assert!(!projection.signal_projection.learning);
    assert!(!projection.signal_projection.llm);
}

#[test]
fn preview_patch_projection_rejects_invalid_field_value_pairs() {
    let row = preview_row(1);
    let patch = ImportPreviewPatch::new(row.id).with_change(
        ImportPreviewPatchField::Date,
        ImportPreviewPatchValue::Bool(false),
    );

    let error = project_preview_patch(row, json!({}), &patch, &ImportIdentityMaps::default())
        .expect_err("a date patch must not silently accept a boolean value");

    assert!(matches!(
        error,
        DbError::InvalidOperation(message)
            if message == "invalid preview patch value for Date"
    ));
}

#[test]
fn preview_patch_projection_accepts_all_declared_fields_with_canonical_values() {
    let cases = [
        (
            ImportPreviewPatchField::Date,
            ImportPreviewPatchValue::Text("2026-08-19 08:00:00".to_string()),
        ),
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text("收入".to_string()),
        ),
        (
            ImportPreviewPatchField::Amount,
            ImportPreviewPatchValue::Integer(-123),
        ),
        (
            ImportPreviewPatchField::DestinationAmount,
            ImportPreviewPatchValue::Integer(-456),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text("餐饮".to_string()),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text("午餐".to_string()),
        ),
        (
            ImportPreviewPatchField::CategoryId,
            ImportPreviewPatchValue::Integer(42),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            ImportPreviewPatchValue::Integer(11),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            ImportPreviewPatchValue::Null,
        ),
        (
            ImportPreviewPatchField::Counterparty,
            ImportPreviewPatchValue::Text("商户".to_string()),
        ),
        (
            ImportPreviewPatchField::PaymentMethod,
            ImportPreviewPatchValue::Text("银行卡".to_string()),
        ),
        (
            ImportPreviewPatchField::Description,
            ImportPreviewPatchValue::Text("说明".to_string()),
        ),
        (
            ImportPreviewPatchField::RecurringId,
            ImportPreviewPatchValue::Integer(7),
        ),
        (
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text("月付".to_string()),
        ),
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(2),
        ),
        (
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(0.75),
        ),
        (
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text("同商户".to_string()),
        ),
        (
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text("2026-08-01".to_string()),
        ),
        (
            ImportPreviewPatchField::Selected,
            ImportPreviewPatchValue::Bool(false),
        ),
        (
            ImportPreviewPatchField::ManualAnnotation,
            ImportPreviewPatchValue::Bool(true),
        ),
        (
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Json(json!({"parser": {"parser_id": "fixture"}})),
        ),
    ];
    assert_eq!(cases.len(), 21, "every patch field must have a canonical case");
    let identity_maps = ImportIdentityMaps {
        active_accounts: BTreeSet::from([11]),
        active_categories: BTreeMap::from([(42, Some(3))]),
    };

    for (field, value) in cases {
        let row = preview_row(1);
        let patch = ImportPreviewPatch::new(row.id).with_change(field, value);
        project_preview_patch(row, json!({}), &patch, &identity_maps)
            .unwrap_or_else(|error| panic!("{field:?} canonical value rejected: {error}"));
    }
}

#[test]
fn preview_patch_transaction_adapters_delegate_projection_to_one_kernel() {
    let generic_source = include_str!("patch_payload_helpers.rs");
    let confirm_source = include_str!("confirm/patches.rs");
    let learning_source = include_str!("preview_learning_lifecycle/decisions.rs");
    let projection_source = include_str!("preview_patch_projection.rs");

    for (adapter, source) in [
        ("generic", generic_source),
        ("confirm", confirm_source),
        ("learning", learning_source),
    ] {
        assert!(
            source.contains("project_preview_patch("),
            "{adapter} preview patch adapter must delegate to the shared projection kernel"
        );
    }
    assert!(
        !confirm_source.contains("apply_patch_value_to_preview(&mut preview"),
        "confirm must not retain a second patch projection loop"
    );
    assert!(
        !learning_source.contains("apply_patch_value_to_preview(&mut preview"),
        "learning must not retain a second patch projection loop"
    );
    assert!(projection_source.contains("fn apply_patch_value_to_preview("));
    assert!(!generic_source.contains("fn apply_patch_value_to_preview("));
    for forbidden in ["sqlx::", "QueryBuilder", ".execute(", ".await"] {
        assert!(
            !projection_source.contains(forbidden),
            "pure preview patch projection must not own {forbidden}"
        );
    }
}
