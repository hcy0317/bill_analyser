#[test]
fn preview_filters_cover_server_paged_contract_fields() {
    let mut first = preview_row(1);
    first.preview_date = "2026-01-10 10:00:00".to_string();
    first.preview_description = "含 手续费".to_string();
    first.preview_parser_tags = vec!["银行".to_string(), "工资".to_string()];
    first.preview_matching_feedback = json!({
        "annotation": {"status": "missing_category"},
        "learning": {"review_status": "needs_review"}
    });

    let mut second = preview_row(2);
    second.preview_date = "2026-02-01 10:00:00".to_string();
    second.preview_selected = false;
    second.preview_parser_tags = vec!["微信".to_string()];
    second.preview_matching_feedback = json!({
        "annotation": {"status": "ok"},
        "learning": {"review_status": "none"}
    });

    let filters = ImportPreviewQueryFilters {
        min_datetime: Some("2026-01-01 00:00:00".to_string()),
        max_datetime: Some("2026-01-31 23:59:59".to_string()),
        tag: Some("工资".to_string()),
        signal: Some("learning:needs_review".to_string()),
        annotation: Some("missing_category".to_string()),
        description: Some("手续费".to_string()),
        selected_only: true,
        ..ImportPreviewQueryFilters::default()
    };

    let rows = apply_preview_filters(vec![first, second], &filters);
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn preview_signal_filters_follow_visible_family_contract() {
    let parser_only = preview_row(1);

    let mut platform_duplicate = preview_row(2);
    platform_duplicate.dedup_type = "platform_bank".to_string();
    platform_duplicate.preview_matching_feedback = json!({
        "parser": {"parser_id": "alipay"},
        "dedup": {"type": "platform_bank", "source_count": 2}
    });

    let mut transfer = preview_row(3);
    transfer.dedup_type = "transfer".to_string();
    transfer.preview_matching_feedback = json!({
        "transfer": {"review_status": "pending"}
    });

    let mut cross_batch_transfer = preview_row(4);
    cross_batch_transfer.dedup_type = "transfer_cross_batch".to_string();

    let mut transfer_type_without_signal = preview_row(8);
    transfer_type_without_signal.preview_type = "转账".to_string();
    transfer_type_without_signal.dedup_type = "transfer".to_string();

    let mut transfer_feedback_without_visible_signal = preview_row(9);
    transfer_feedback_without_visible_signal.preview_type = "转账".to_string();
    transfer_feedback_without_visible_signal.preview_matching_feedback = json!({
        "transfer": {"candidate_type": "cash_transfer", "reason": "parser inferred transfer"}
    });

    let mut history = preview_row(5);
    history.preview_matching_feedback = json!({
        "reconciliation": {
            "planned_operation": "update_history",
            "history_bill_id": 88,
            "destructive_ack_required": true
        }
    });

    let mut learning = preview_row(6);
    learning.preview_matching_feedback = json!({
        "learning": {"review_status": "needs_review", "reason": "manual"}
    });

    let mut llm = preview_row(7);
    llm.preview_matching_feedback = json!({
        "llm": {"review_status": "pending", "reason": "model recommendation"}
    });

    let rows = vec![
        parser_only,
        platform_duplicate,
        transfer,
        cross_batch_transfer,
        history,
        learning,
        llm,
        transfer_type_without_signal,
        transfer_feedback_without_visible_signal,
    ];
    let filtered_ids = |signal: &str| {
        apply_preview_filters(
            rows.clone(),
            &ImportPreviewQueryFilters {
                signal: Some(signal.to_string()),
                ..ImportPreviewQueryFilters::default()
            },
        )
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>()
    };

    assert_eq!(filtered_ids("parser"), vec![1, 4, 8, 9]);
    assert_eq!(filtered_ids("parser_12"), vec![1, 4, 8, 9]);
    assert_eq!(filtered_ids("platform_duplicate"), vec![2]);
    assert_eq!(filtered_ids("transfer"), vec![3]);
    assert_eq!(filtered_ids("history"), vec![5]);
    assert_eq!(filtered_ids("learning"), vec![6]);
    assert_eq!(filtered_ids("learning:needs_review"), vec![6]);
    assert!(filtered_ids("learning:pending").is_empty());
    assert_eq!(filtered_ids("learning_1"), vec![6]);
    assert_eq!(filtered_ids("learning-1"), vec![6]);
    assert_eq!(filtered_ids("llm"), vec![7]);
    assert!(filtered_ids("manual").is_empty());
    assert!(filtered_ids("12").is_empty());
    assert!(filtered_ids("true").is_empty());
    assert!(filtered_ids("missing").is_empty());
}

#[test]
fn preview_category_filter_uses_persisted_category_identity() {
    let mut matched = preview_row(1);
    matched.category_id = Some(42);
    matched.preview_main_category = "理财".to_string();
    matched.preview_sub_category = "理财收益".to_string();

    let mut same_name_wrong_identity = preview_row(2);
    same_name_wrong_identity.category_id = Some(99);
    same_name_wrong_identity.preview_main_category = "理财".to_string();
    same_name_wrong_identity.preview_sub_category = "理财收益".to_string();

    let mut missing = preview_row(3);
    missing.category_id = None;
    missing.preview_main_category.clear();
    missing.preview_sub_category.clear();

    let id_filters = ImportPreviewQueryFilters {
        category: Some("42".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(
        vec![
            matched.clone(),
            same_name_wrong_identity.clone(),
            missing.clone(),
        ],
        &id_filters,
    );
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );

    let label_filters = ImportPreviewQueryFilters {
        category: Some("理财".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(
        vec![matched.clone(), same_name_wrong_identity.clone()],
        &label_filters,
    );
    assert!(
        rows.is_empty(),
        "category filter must not match preview labels without canonical category id"
    );

    let missing_filters = ImportPreviewQueryFilters {
        category: Some("__none__".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(
        vec![matched, same_name_wrong_identity, missing],
        &missing_filters,
    );
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![3]
    );

    let mut named_without_identity = preview_row(4);
    named_without_identity.category_id = None;
    named_without_identity.preview_main_category = "理财".to_string();
    named_without_identity.preview_sub_category = "理财收益".to_string();
    let invalid_filters = ImportPreviewQueryFilters {
        category: Some("__invalid__".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(vec![named_without_identity], &invalid_filters);
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![4]
    );
}

#[test]
fn preview_filters_preserve_account_tag_and_annotation_sentinels() {
    let mut valid = preview_row(1);
    valid.category_id = Some(42);
    valid.preview_source_account_id = Some(11);
    valid.preview_parser_tags = vec!["工资".to_string()];

    let mut invalid = preview_row(2);
    invalid.preview_source_account_id = None;
    invalid.preview_parser_tags.clear();

    let invalid_filters = ImportPreviewQueryFilters {
        account: Some("__invalid__".to_string()),
        tag: Some("__invalid__".to_string()),
        annotation: Some("needs-review".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(vec![valid.clone(), invalid.clone()], &invalid_filters);
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2]
    );

    let no_issue_filters = ImportPreviewQueryFilters {
        annotation: Some("no-issues".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let rows = apply_preview_filters(vec![valid, invalid], &no_issue_filters);
    assert_eq!(
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );
}
