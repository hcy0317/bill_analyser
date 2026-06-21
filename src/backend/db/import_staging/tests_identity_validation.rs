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
