#[test]
fn preview_learning_first_nonempty_status_authority() {
    let j1 =
        json!({"review_status": "pending", "status": "accepted", "score": 0.9, "reason": "test"});
    let j2 = json!({"review_status": "accepted", "status": "none", "score": 0.9, "reason": "test"});
    let j3 = json!({"lifecycle_status": "skipped", "score": 0.9, "reason": "test"});
    let j4 = json!({"review_status": "", "status": "", "score": 0.0, "reason": ""});
    let j5 = json!({"review_status": "pending"});
    let j6 = json!({"status": "accepted"});
    let j7 = json!({"signal_state": "rejected"});
    let test_cases: Vec<(&str, &Value, bool)> = vec![
        // review_status=pending, status=accepted → resolved to pending, with evidence visible
        ("learning:review_pending_status_accepted", &j1, true),
        // review_status=accepted, status=none → resolved to accepted
        ("learning:review_accepted_status_none", &j2, true),
        // review_status=empty, lifecycle_status=skipped → resolved to skipped
        ("learning:lifecycle_skipped", &j3, true),
        // all empty → not meaningful
        ("learning:all_empty", &j4, false),
        // transfer: review_status=pending → visible (existing domain rule)
        ("transfer:review_pending", &j5, true),
        // accepted transfer evidence remains discoverable even though it is no longer actionable
        ("transfer:status_accepted", &j6, true),
        ("transfer:signal_state_rejected", &j7, false),
    ];

    for (description, payload, expect_visible) in test_cases {
        let family = if description.starts_with("transfer") {
            "transfer"
        } else {
            "learning"
        };
        let mut row = preview_row(400);
        row.preview_matching_feedback =
            Value::Object(Map::from_iter([(family.to_string(), (*payload).clone())]));
        let visible = apply_preview_filters(
            vec![row],
            &ImportPreviewQueryFilters {
                signal: Some(family.to_string()),
                ..ImportPreviewQueryFilters::default()
            },
        )
        .into_iter()
        .next()
        .is_some();
        assert_eq!(
            visible, expect_visible,
            "{description}: expected visible={expect_visible}"
        );
    }
}

#[test]
fn preview_signal_filters_fail_closed_for_unknown_nonempty_review_statuses() {
    for (family, evidence) in [
        ("transfer", json!({"candidate_type": "transfer", "score": 0.9})),
        ("learning", json!({"score": 0.9, "summary": "actionable"})),
        ("llm", json!({"confidence": 0.9, "reason": "actionable"})),
    ] {
        for review_status in ["__invalid_status__", "  FUTURE_UNKNOWN  "] {
            let mut section = evidence.as_object().cloned().expect("object evidence");
            section.insert("review_status".to_string(), json!(review_status));
            let mut row = preview_row(450);
            row.preview_matching_feedback =
                Value::Object(Map::from_iter([(family.to_string(), Value::Object(section))]));

            assert!(
                apply_preview_filters(
                    vec![row],
                    &ImportPreviewQueryFilters {
                        signal: Some(family.to_string()),
                        ..ImportPreviewQueryFilters::default()
                    },
                )
                .is_empty(),
                "{family} must fail closed for non-canonical review_status={review_status:?}"
            );
        }
    }
}

#[test]
fn preview_decimal_lexical_positive_never_overflows() {
    let mut positive = preview_row(410);
    positive.preview_matching_feedback = json!({"learning": {"score": "2", "summary": "positive"}});
    let mut zero = preview_row(411);
    zero.preview_matching_feedback = json!({"learning": {"score": "0", "summary": ""}});
    let mut negative = preview_row(412);
    negative.preview_matching_feedback = json!({"learning": {"score": "-1", "summary": ""}});
    let mut four_hundred_digit = preview_row(413);
    let big = "1".repeat(400);
    four_hundred_digit.preview_matching_feedback =
        json!({"learning": {"score": &big, "summary": "400-digit"}});

    let positive_hit = apply_preview_filters(
        vec![positive],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(positive_hit.len(), 1, "positive score must be visible");

    let zero_hit = apply_preview_filters(
        vec![zero],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(zero_hit.len(), 0, "zero score must not be visible");

    let negative_hit = apply_preview_filters(
        vec![negative],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(negative_hit.len(), 0, "negative score must not be visible");

    let big_hit = apply_preview_filters(
        vec![four_hundred_digit],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(
        big_hit.len(),
        1,
        "400-digit positive score must be visible without overflow"
    );
}

#[test]
fn preview_json_string_type_required_for_text_evidence() {
    let mut text_string = preview_row(420);
    text_string.preview_matching_feedback =
        json!({"learning": {"summary": "nice coffee", "reason": "merchant says"}});
    let mut number_text = preview_row(421);
    number_text.preview_matching_feedback =
        json!({"learning": {"summary": 1, "reason": "merchant says"}});
    let mut bool_text = preview_row(422);
    bool_text.preview_matching_feedback =
        json!({"learning": {"summary": false, "reason": "merchant says"}});

    let string_visible = apply_preview_filters(
        vec![text_string],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(string_visible.len(), 1, "JSON string summary is visible");

    let number_visible = apply_preview_filters(
        vec![number_text],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(
        number_visible.len(),
        1,
        "JSON number summary still allows reason evidence"
    );

    let mut number_only = preview_row(423);
    number_only.preview_matching_feedback = json!({"learning": {"summary": 1}});
    let number_only_visible = apply_preview_filters(
        vec![number_only],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(
        number_only_visible.len(),
        0,
        "JSON number summary alone must not create phantom text evidence"
    );

    let mut bool_only = preview_row(424);
    bool_only.preview_matching_feedback = json!({"learning": {"summary": false}});
    let bool_visible = apply_preview_filters(
        vec![bool_only],
        &ImportPreviewQueryFilters {
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    assert_eq!(
        bool_visible.len(),
        0,
        "JSON bool summary alone must not create phantom text evidence"
    );
}

#[test]
fn preview_sort_uses_id_tie_break() {
    let mut a1 = preview_row(1);
    a1.preview_date = "2026-01-01 10:00:00".to_string();
    a1.preview_amount_cents = 100;
    a1.preview_counterparty = "商户".to_string();
    let mut a2 = preview_row(2);
    a2.preview_date = "2026-01-01 10:00:00".to_string();
    a2.preview_amount_cents = 100;
    a2.preview_counterparty = "商户".to_string();
    let mut later = preview_row(3);
    later.preview_date = "2026-01-02 10:00:00".to_string();
    later.preview_amount_cents = 200;
    later.preview_counterparty = "其他".to_string();

    let mut rows = vec![later.clone(), a1.clone(), a2.clone()];
    sort_preview_rows(&mut rows, "time", "asc");
    assert_eq!(
        rows.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "asc time sort must tie-break by id ascending"
    );

    let mut rows = vec![a2.clone(), a1.clone(), later.clone()];
    sort_preview_rows(&mut rows, "time", "desc");
    assert_eq!(
        rows.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "desc time sort must tie-break by id descending"
    );

    let mut rows = vec![later.clone(), a2.clone(), a1.clone()];
    sort_preview_rows(&mut rows, "amount_cents", "asc");
    assert_eq!(
        rows.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "asc amount_cents sort must tie-break by id ascending"
    );

    let mut rows = vec![a1.clone(), a2.clone(), later.clone()];
    sort_preview_rows(&mut rows, "counterparty", "desc");
    assert_eq!(
        rows.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![2, 1, 3],
        "desc counterparty sort must tie-break by id descending"
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
#[test]
fn preview_transfer_filter_uses_shared_parity_corpus() {
    for (index, case) in TRANSFER_SIGNAL_PARITY_CORPUS.iter().enumerate() {
        let mut row = preview_row(10_000 + index as i64);
        row.preview_type = case.preview_type.to_string();
        row.preview_matching_feedback = json!({
            "transfer": serde_json::from_str::<Value>(case.transfer_json).unwrap()
        });
        let visible = apply_preview_filters(
            vec![row],
            &ImportPreviewQueryFilters {
                signal: Some("transfer".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
        );
        assert_eq!(
            !visible.is_empty(),
            case.expected_filter_visible,
            "{}",
            case.name
        );
    }
}

#[test]
fn preview_filter_index_history_ack_token_preserves_session_identity() {
    let mut full_row = preview_row(20_001);
    full_row.session_id = "session-history-a".to_string();
    full_row.preview_matching_feedback = json!({
        "reconciliation": {
            "planned_operation": "update_current_bill",
            "history_bill_id": 9_001,
            "history_bill_version": 3,
            "group_key": "hist:9001",
            "notice": "将改写/合并历史账单"
        }
    });

    let mut full_preview = serde_json::to_value(&full_row)
        .expect("serialize full preview row")
        .as_object()
        .expect("full preview object")
        .clone();
    bill_analyser_core::attach_import_preview_matching_payload(&mut full_preview);
    let full_token = full_preview["matching"]["reconciliation"]["acknowledgement_token"]
        .as_str()
        .expect("full preview acknowledgement token")
        .to_string();

    let filter_row = ImportPreviewFilterIndexRow::from(full_row.clone());
    let filter_preview = serde_json::to_value(filter_row)
        .expect("serialize filter index row")
        .as_object()
        .expect("filter index object")
        .clone();
    let filter_item = bill_analyser_core::build_import_preview_filter_index_item(
        &filter_preview,
        &BTreeMap::new(),
        &BTreeMap::new(),
    );

    assert_eq!(
        filter_item.history_acknowledgement_token, full_token,
        "lightweight index must generate the same acknowledgement token as full preview"
    );

    let mut other_session_row = full_row;
    other_session_row.session_id = "session-history-b".to_string();
    let other_filter_row = ImportPreviewFilterIndexRow::from(other_session_row);
    let other_filter_preview = serde_json::to_value(other_filter_row)
        .expect("serialize other-session filter index row")
        .as_object()
        .expect("other-session filter index object")
        .clone();
    let other_filter_item = bill_analyser_core::build_import_preview_filter_index_item(
        &other_filter_preview,
        &BTreeMap::new(),
        &BTreeMap::new(),
    );

    assert_ne!(
        other_filter_item.history_acknowledgement_token, full_token,
        "acknowledgement token must remain bound to the import session"
    );
}
