#[test]
fn identity_validation_feedback_makes_preview_require_review() {
    let mut row = preview_row(1);
    row.category_id = Some(42);
    row.preview_matching_feedback = json!({
        "identity_validation": {
            "review_status": "requires_identity_review",
            "issues": [
                {"field": "category_id", "reason": "not_active_or_not_found", "value": 42}
            ]
        }
    });

    assert!(preview_requires_review(&row));

    let filters = ImportPreviewQueryFilters {
        category: Some("__invalid__".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(vec![row], &filters);
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn identity_validation_clears_invalid_draft_ids_and_records_issues() {
    let mut draft = ImportPreviewDraft {
        preview_type: "转账".to_string(),
        category_id: Some(42),
        preview_source_account_id: Some(11),
        preview_destination_account_id: Some(11),
        preview_selected: true,
        ..ImportPreviewDraft::default()
    };
    let mut maps = ImportIdentityMaps::default();
    maps.active_categories.insert(42, Some(3));
    maps.active_accounts.insert(11);

    apply_identity_validation_to_draft(&mut draft, &maps);

    assert_eq!(draft.category_id, None);
    assert_eq!(draft.preview_destination_account_id, None);
    assert!(!draft.preview_selected);
    let issues = draft
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .expect("identity issues");
    assert!(issues.iter().any(|issue| {
        issue["field"] == json!("category_id") && issue["reason"] == json!("type_mismatch")
    }));
    assert!(issues.iter().any(|issue| {
        issue["field"] == json!("destination_account_id")
            && issue["reason"] == json!("same_as_source_account")
    }));
}

#[test]
fn identity_validation_preserves_only_active_manually_owned_category_mismatches() {
    let mut maps = ImportIdentityMaps::default();
    maps.active_categories.insert(42, Some(4));
    maps.active_accounts.insert(11);
    let mut manually_owned = ImportPreviewDraft {
        preview_type: "支出".to_string(),
        category_id: Some(42),
        preview_source_account_id: Some(11),
        preview_selected: true,
        preview_matching_feedback: json!({
            "annotation": {
                "is_manually_annotated": true,
                "manual_fields": {"category_id": true}
            }
        }),
        ..ImportPreviewDraft::default()
    };

    apply_identity_validation_to_draft(&mut manually_owned, &maps);

    assert_eq!(manually_owned.category_id, Some(42));
    assert!(manually_owned
        .preview_matching_feedback
        .get("identity_validation")
        .is_none());
    assert!(manually_owned.preview_selected);

    manually_owned.category_id = Some(99);
    apply_identity_validation_to_draft(&mut manually_owned, &maps);
    assert_eq!(manually_owned.category_id, None);
    assert_eq!(
        manually_owned
            .preview_matching_feedback
            .pointer("/identity_validation/issues/0/reason"),
        Some(&json!("not_active_or_not_found"))
    );
    assert!(!manually_owned.preview_selected);
}

#[test]
fn identity_validation_records_non_positive_and_missing_identity_edges() {
    let maps = ImportIdentityMaps::default();

    let mut expense = ImportPreviewDraft {
        preview_type: "支出".to_string(),
        category_id: Some(-1),
        preview_source_account_id: Some(0),
        preview_selected: true,
        ..ImportPreviewDraft::default()
    };
    apply_identity_validation_to_draft(&mut expense, &maps);
    assert_eq!(expense.category_id, None);
    assert_eq!(expense.preview_source_account_id, None);
    assert!(!expense.preview_selected);
    let expense_issues = expense
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .expect("expense issues");
    assert!(expense_issues.iter().any(|issue| {
        issue["field"] == json!("category_id") && issue["reason"] == json!("non_positive")
    }));
    assert!(expense_issues.iter().any(|issue| {
        issue["field"] == json!("source_account_id") && issue["reason"] == json!("non_positive")
    }));

    let mut income = ImportPreviewDraft {
        preview_type: "收入".to_string(),
        category_id: Some(0),
        preview_source_account_id: None,
        preview_selected: true,
        ..ImportPreviewDraft::default()
    };
    apply_identity_validation_to_draft(&mut income, &maps);
    let income_issues = income
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .expect("income issues");
    assert!(income_issues.iter().any(|issue| {
        issue["field"] == json!("category_id") && issue["reason"] == json!("non_positive")
    }));
    assert!(income_issues.iter().any(|issue| {
        issue["field"] == json!("source_account_id") && issue["reason"] == json!("missing")
    }));

    let mut transfer = ImportPreviewDraft {
        preview_type: "转账".to_string(),
        category_id: None,
        preview_source_account_id: None,
        preview_destination_account_id: Some(-2),
        preview_selected: true,
        ..ImportPreviewDraft::default()
    };
    apply_identity_validation_to_draft(&mut transfer, &maps);
    let transfer_issues = transfer
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .expect("transfer issues");
    assert!(transfer_issues.iter().any(|issue| {
        issue["field"] == json!("category_id") && issue["reason"] == json!("missing")
    }));
    assert!(transfer_issues.iter().any(|issue| {
        issue["field"] == json!("destination_account_id")
            && issue["reason"] == json!("non_positive")
    }));

    let mut expense_with_destination = ImportPreviewDraft {
        preview_type: "支出".to_string(),
        category_id: Some(42),
        preview_source_account_id: Some(11),
        preview_destination_account_id: Some(12),
        preview_selected: true,
        ..ImportPreviewDraft::default()
    };
    let mut valid_maps = ImportIdentityMaps::default();
    valid_maps.active_categories.insert(42, Some(3));
    valid_maps.active_accounts.extend([11, 12]);
    apply_identity_validation_to_draft(&mut expense_with_destination, &valid_maps);
    assert_eq!(
        expense_with_destination.preview_destination_account_id,
        None
    );
    assert!(!expense_with_destination.preview_selected);
    let destination_issues = expense_with_destination
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .expect("destination issues");
    assert!(destination_issues.iter().any(|issue| {
        issue["field"] == json!("destination_account_id")
            && issue["reason"] == json!("not_allowed_for_type")
    }));
}

#[test]
fn manual_identity_patch_recomputes_feedback_and_replaces_stale_annotation() {
    let mut row = preview_row(1);
    row.preview_type = "支出".to_string();
    row.category_id = None;
    row.preview_source_account_id = None;
    row.preview_matching_feedback = json!({
        "annotation": {"status": "missing_category", "is_manually_annotated": true}
    });
    let mut payload = json!({});
    let mut maps = ImportIdentityMaps::default();
    maps.active_categories.insert(42, Some(3));
    maps.active_accounts.insert(11);

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(42),
    )
    .expect("category identity patch");
    apply_identity_validation_to_preview(&mut row, &mut payload, &maps);
    assert_eq!(
        row.preview_matching_feedback.pointer("/annotation/status"),
        Some(&json!("missing_source_account"))
    );
    assert_eq!(
        row.preview_matching_feedback
            .pointer("/identity_validation/issues/0/field"),
        Some(&json!("source_account_id"))
    );

    apply_patch_value_to_preview(
        &mut row,
        &mut payload,
        ImportPreviewPatchField::SourceAccountId,
        ImportPreviewPatchValue::Integer(11),
    )
    .expect("source account identity patch");
    apply_identity_validation_to_preview(&mut row, &mut payload, &maps);
    assert!(!preview_requires_review(&row));
    assert!(row
        .preview_matching_feedback
        .get("identity_validation")
        .is_none());
    assert!(row
        .preview_matching_feedback
        .pointer("/annotation/status")
        .is_none());
    assert_eq!(
        row.preview_matching_feedback
            .pointer("/annotation/is_manually_annotated"),
        Some(&json!(true))
    );
}

#[test]
fn identity_annotation_sync_preserves_non_identity_and_maps_all_identity_fields() {
    let mut non_object = serde_json::Map::from_iter([("annotation".into(), json!("legacy"))]);
    synchronize_identity_annotation(&mut non_object, &[]);
    assert_eq!(non_object["annotation"], json!("legacy"));

    let mut business = serde_json::Map::from_iter([(
        "annotation".into(),
        json!({"status":"manual_review","note":"keep"}),
    )]);
    synchronize_identity_annotation(&mut business, &[json!({"field":"category_id"})]);
    assert_eq!(business["annotation"]["status"], "manual_review");

    let mut destination =
        serde_json::Map::from_iter([("annotation".into(), json!({"reason":"missing_account"}))]);
    synchronize_identity_annotation(
        &mut destination,
        &[json!({"field":"destination_account_id"})],
    );
    assert_eq!(
        destination["annotation"]["status"],
        "missing_destination_account"
    );

    let mut cleared = serde_json::Map::from_iter([(
        "annotation".into(),
        json!({"review_status":"requires_identity_review"}),
    )]);
    synchronize_identity_annotation(&mut cleared, &[]);
    assert!(!cleared.contains_key("annotation"));

    assert!(category_type_matches_preview_type(None, "支出"));
    assert!(category_type_matches_preview_type(Some(0), "支出"));
    assert!(category_type_matches_preview_type(Some(3), "支出"));
    assert!(!category_type_matches_preview_type(Some(2), "支出"));
    assert!(category_type_matches_preview_type(Some(99), "未知"));
}

#[test]
fn import_identity_maps_have_one_transactional_loader_for_every_runtime_path() {
    let identity_persistence_source = include_str!("identity_persistence.rs");
    let identity_validation_source = include_str!("identity_validation.rs");
    let runtime_sources = [
        include_str!("preview_write.rs"),
        include_str!("preview_selection.rs"),
        include_str!("confirm/orchestration.rs"),
        include_str!("confirm/patches.rs"),
        include_str!("preview_learning_lifecycle/decisions.rs"),
    ];
    let retired_owner_sources = [
        include_str!("confirm/persistence.rs"),
        include_str!("preview_learning_lifecycle/persistence.rs"),
    ];

    assert_eq!(
        identity_persistence_source
            .matches("async fn load_import_identity_maps_on_tx(")
            .count(),
        1,
        "identity persistence must own the single transactional identity-map loader"
    );
    assert_eq!(
        runtime_sources
            .iter()
            .map(|source| source.matches("load_import_identity_maps_on_tx(").count())
            .sum::<usize>(),
        6,
        "all six runtime reads must use the shared loader"
    );
    assert!(identity_persistence_source.contains(
        "SELECT id FROM accounts WHERE user_id = $1 AND is_active = true"
    ));
    assert!(identity_persistence_source.contains(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true"
    ));
    assert!(!identity_persistence_source.contains(".begin()"));
    assert!(!identity_persistence_source.contains(".commit()"));
    assert!(!identity_validation_source.contains("sqlx::"));

    let retired_sources = retired_owner_sources.join("\n");
    assert!(!retired_sources.contains("load_import_identity_maps_for_confirm"));
    assert!(!retired_sources.contains("load_import_identity_maps_in_transaction"));
    assert!(!retired_sources.contains("SELECT id FROM accounts"));
    assert!(!retired_sources.contains("SELECT id, category_type FROM categories"));
}
