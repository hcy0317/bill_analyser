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
