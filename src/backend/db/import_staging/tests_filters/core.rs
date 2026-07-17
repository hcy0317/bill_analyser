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
fn learning_filter_rejects_non_actionable_payloads_and_leaves_them_parser_visible() {
    let cases = [
        json!({}),
        json!({"review_status": "none"}),
        json!({"score": 0.92, "summary": "actionable", "suppressed": true}),
        json!({"summary": " \t\n\u{3000}"}),
        json!({"score": -0.5}),
        json!({"score": " -0.5 "}),
    ];
    let rows = cases
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            let mut row = preview_row(100 + index as i64);
            row.preview_matching_feedback =
                Value::Object(Map::from_iter([("learning".to_string(), payload.clone())]));
            row
        })
        .collect::<Vec<_>>();

    let learning_ids = apply_preview_filters(
        rows.clone(),
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    )
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();
    assert!(
        learning_ids.is_empty(),
        "empty/none/suppressed/whitespace/negative learning payloads are not visible: {learning_ids:?}"
    );

    let parser_ids = apply_preview_filters(
        rows.clone(),
        &ImportPreviewQueryFilters {
            signal: Some("parser".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    )
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();
    assert_eq!(
        parser_ids,
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        "a non-visible learning payload must not suppress the parser family"
    );
}

#[test]
fn llm_filter_rejects_non_actionable_payloads_and_leaves_them_parser_visible() {
    let cases = [
        json!({}),
        json!({"review_status": "none"}),
        json!({"confidence": 0.87, "suggested_main_category": "餐饮", "suppressed": true}),
        json!({"suggested_main_category": " \t\n\u{3000}"}),
        json!({"confidence": -0.75}),
        json!({"confidence": " -0.75 "}),
    ];
    let rows = cases
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            let mut row = preview_row(200 + index as i64);
            row.preview_matching_feedback =
                Value::Object(Map::from_iter([("llm".to_string(), payload.clone())]));
            row
        })
        .collect::<Vec<_>>();

    let llm_ids = apply_preview_filters(
        rows.clone(),
        &ImportPreviewQueryFilters {
            signal: Some("llm".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    )
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();
    assert!(
        llm_ids.is_empty(),
        "empty/none/suppressed/whitespace/negative LLM payloads are not visible: {llm_ids:?}"
    );

    let parser_ids = apply_preview_filters(
        rows.clone(),
        &ImportPreviewQueryFilters {
            signal: Some("parser".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    )
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();
    assert_eq!(
        parser_ids,
        rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
        "a non-visible LLM payload must not suppress the parser family"
    );
}

#[test]
fn preview_metadata_counts_always_contains_the_six_visible_families() {
    let parser = preview_row(301);
    let mut platform_duplicate = preview_row(302);
    platform_duplicate.dedup_type = "platform_bank".to_string();
    let mut transfer = preview_row(303);
    transfer.preview_matching_feedback = json!({"transfer": {"review_status": "pending"}});
    let mut history = preview_row(304);
    history.preview_matching_feedback =
        json!({"reconciliation": {"planned_operation": "update_history"}});
    let mut learning = preview_row(305);
    learning.preview_matching_feedback = json!({"learning": {"score": 0.81}});
    let mut llm = preview_row(306);
    llm.preview_matching_feedback = json!({"llm": {"confidence": 0.76}});
    let mut auxiliary_only = preview_row(307);
    auxiliary_only.preview_parser_id.clear();
    auxiliary_only.preview_parser_tags.clear();
    auxiliary_only.preview_matching_feedback = json!({
        "recurring": {"review_status": "pending"},
        "identity_validation": {"issues": [{"field": "category_id"}]}
    });

    let result = build_preview_page_result_from_rows(
        vec![
            parser,
            platform_duplicate,
            transfer,
            history,
            learning,
            llm,
            auxiliary_only,
        ],
        &ImportPreviewPageRequest {
            page_size: 50,
            ..ImportPreviewPageRequest::default()
        },
    );

    assert_eq!(
        result.metadata.counts.signals,
        BTreeMap::from([
            ("history".to_string(), 1),
            ("learning".to_string(), 1),
            ("llm".to_string(), 1),
            ("parser".to_string(), 1),
            ("platform_duplicate".to_string(), 1),
            ("transfer".to_string(), 1),
        ])
    );
}

#[test]
fn preview_metadata_counts_includes_all_six_visible_families_when_empty() {
    let result = build_preview_page_result_from_rows(
        Vec::new(),
        &ImportPreviewPageRequest {
            page_size: 50,
            ..ImportPreviewPageRequest::default()
        },
    );

    assert_eq!(
        result.metadata.counts.signals,
        BTreeMap::from([
            ("history".to_string(), 0),
            ("learning".to_string(), 0),
            ("llm".to_string(), 0),
            ("parser".to_string(), 0),
            ("platform_duplicate".to_string(), 0),
            ("transfer".to_string(), 0),
        ]),
        "empty results still expose the canonical six signal-count keys"
    );
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
