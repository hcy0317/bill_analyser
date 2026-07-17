#[test]
fn llm_suggested_type_cannot_create_transfer_without_authority() {
    let row = preview_row(7);
    let suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "transfer".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };

    let (suggested_type, reason) = llm_suggested_type_for_preview(&row, &suggestion);

    assert_eq!(suggested_type, None);
    assert_eq!(reason, Some("unauthorized_transfer".to_string()));
}

#[test]
fn llm_suggested_type_allows_authorized_transfer_only_as_transfer() {
    let mut row = preview_row(7);
    row.preview_matching_feedback = json!({
        "transfer": {
            "candidate_type": "transfer",
            "review_status": "pending"
        }
    });
    let transfer = ImportPreviewLlmSuggestion {
        suggested_type: "transfer".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let expense = ImportPreviewLlmSuggestion {
        suggested_type: "expense".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };

    assert_eq!(
        llm_suggested_type_for_preview(&row, &transfer),
        (Some("转账".to_string()), None)
    );
    assert_eq!(
        llm_suggested_type_for_preview(&row, &expense),
        (None, Some("transfer_protected".to_string()))
    );
}

#[test]
fn llm_unresolved_category_text_is_record_only() {
    let suggestion = ImportPreviewLlmSuggestion {
        suggested_main_category: "不存在分类".to_string(),
        suggested_sub_category: "不存在子类".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let mut applied_fields = Vec::new();
    let patch = apply_resolved_llm_category_patch(
        ImportPreviewPatch::new(7),
        &mut applied_fields,
        None,
    );

    assert!(applied_fields.is_empty());
    assert!(patch.changes.is_empty());
    assert_eq!(
        category_ignored_reason_for_llm_suggestion(&suggestion, None),
        Some("unresolved_or_unauthorized_category")
    );
}

#[test]
fn llm_suggested_type_rejects_blank_unknown_and_unsupported_codes() {
    let row = preview_row(8);

    for (suggested_type, expected) in [
        ("   ", (None, None)),
        (
            "not-a-type",
            (None, Some("invalid_type".to_string())),
        ),
        ("other", (None, Some("invalid_type".to_string()))),
    ] {
        let suggestion = ImportPreviewLlmSuggestion {
            suggested_type: suggested_type.to_string(),
            ..ImportPreviewLlmSuggestion::default()
        };
        assert_eq!(llm_suggested_type_for_preview(&row, &suggestion), expected);
    }
}

#[test]
fn llm_transfer_authority_requires_non_rejected_transfer_candidate() {
    let mut row = preview_row(9);

    for (feedback, expected) in [
        (json!(null), false),
        (json!({"transfer": "invalid"}), false),
        (json!({"transfer": {"candidate_type": "expense"}}), false),
        (
            json!({"transfer": {"candidate_type": " transfer_cross_batch "}}),
            true,
        ),
        (
            json!({"transfer": {"candidate_type": "TRANSFER", "review_status": " ReJeCtEd "}}),
            false,
        ),
    ] {
        row.preview_matching_feedback = feedback;
        assert_eq!(preview_has_authorized_transfer_feedback(&row), expected);
    }
}

#[test]
fn llm_type_labels_cover_all_supported_codes() {
    assert_eq!(preview_type_label_from_code(2), Some("收入"));
    assert_eq!(preview_type_label_from_code(3), Some("支出"));
    assert_eq!(preview_type_label_from_code(4), Some("转账"));
    assert_eq!(preview_type_label_from_code(5), Some("投资"));
    assert_eq!(preview_type_label_from_code(0), None);
}

#[test]
fn llm_resolved_category_populates_identity_and_labels() {
    let suggestion = ImportPreviewLlmSuggestion {
        suggested_category_id: Some(42),
        ..ImportPreviewLlmSuggestion::default()
    };
    let category = ImportPreviewLlmCategoryMatch {
        id: 42,
        type_code: Some(3),
        main_category: "餐饮".to_string(),
        sub_category: "咖啡".to_string(),
    };
    let mut applied_fields = Vec::new();

    let patch = apply_resolved_llm_category_patch(
        ImportPreviewPatch::new(9),
        &mut applied_fields,
        Some(&category),
    );

    assert_eq!(
        applied_fields,
        vec!["category_id", "main_category", "sub_category"]
    );
    assert_eq!(patch.changes.len(), 3);
    assert_eq!(
        category_ignored_reason_for_llm_suggestion(&suggestion, Some(&category)),
        None
    );
    assert_eq!(
        category_ignored_reason_for_llm_suggestion(
            &ImportPreviewLlmSuggestion::default(),
            None,
        ),
        None
    );
}
