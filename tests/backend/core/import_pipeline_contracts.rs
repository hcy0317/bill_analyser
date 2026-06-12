use std::collections::BTreeMap;

use bill_analyser_core::{
    build_import_history_rewrite_ack_token, build_import_history_rewrite_operation_id,
    build_import_preview_filter_index_item, build_import_preview_matching_payload,
    coerce_preview_selected_value, expected_preview_state_is_valid, import_preview_index_success,
    import_preview_page_success, import_session_cancel_missing_response,
    import_session_cancel_success_response, import_session_not_found_response,
    import_session_success, import_stage_confirm_success, import_stage_dedup_success,
    import_stage_parse_success, import_v2_invalid_request_response,
    import_v2_missing_session_id_response, map_import_preview_type_to_frontend_value,
    normalize_import_preview_page_query, normalize_import_preview_page_sort_direction,
    normalize_import_preview_page_sort_key, normalize_page, normalize_page_size,
    normalize_preview_ids, preview_state_conflict_response, preview_update_is_selected,
    sort_import_preview_page_items, AccountLookup, CategoryLookup, ExpectedPreviewState,
    ImportPreviewIndexData, ImportPreviewMatchingPayload, ImportPreviewPageData,
    ImportPreviewSortDirection, ImportSessionSummary, ImportStageConfirmData, ImportStageDedupData,
    ImportStageParseData, BILLS_PREVIEW_CONTRACT_FIELDS, HISTORY_REWRITE_NOTICE,
    IMPORT_STAGING_TABLES, IMPORT_V2_PIPELINE_STEPS,
};
use serde_json::{json, Map};

#[test]
fn preview_page_query_normalization_matches_current_v2_contract() {
    let query = normalize_import_preview_page_query(
        Some(0),
        Some(999),
        Some(" sourceAmountCents "),
        Some("DESC"),
        &[7, 0, 7, -3, 5],
    );

    assert_eq!(query.page, 1);
    assert_eq!(query.page_size, 200);
    assert_eq!(query.sort_by, "sourceAmountCents");
    assert_eq!(query.sort_direction, ImportPreviewSortDirection::Desc);
    assert_eq!(query.preview_ids, vec![7, 5]);

    assert_eq!(normalize_page(None), 1);
    assert_eq!(normalize_page_size(Some(0)), 1);
    assert_eq!(normalize_page_size(None), 50);
    assert_eq!(normalize_import_preview_page_sort_key(Some("amount")), "");
    assert_eq!(
        normalize_import_preview_page_sort_direction(Some("ascending")),
        ImportPreviewSortDirection::Asc
    );
    assert_eq!(normalize_preview_ids(&[1, 2, 1, 3]), vec![1, 2, 3]);
}

#[test]
fn preview_sort_is_stabilized_by_id_and_uses_current_field_mapping() {
    let items = vec![
        json!({"id": 3, "preview_amount_cents": "850", "preview_counterparty": "beta"}),
        json!({"id": 1, "preview_amount_cents": "bad", "preview_counterparty": "alpha"}),
        json!({"id": 2, "preview_amount_cents": 850, "preview_counterparty": "Alpha"}),
    ];

    let invalid = sort_import_preview_page_items(&items, Some("not-allowed"), Some("desc"));
    assert_eq!(invalid, items);

    let amount_asc = sort_import_preview_page_items(&items, Some("sourceAmountCents"), Some("asc"));
    assert_eq!(
        amount_asc
            .iter()
            .map(|item| item["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    let counterparty_desc =
        sort_import_preview_page_items(&items, Some("counterparty"), Some("desc"));
    assert_eq!(
        counterparty_desc
            .iter()
            .map(|item| item["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![3, 1, 2]
    );
}

#[test]
fn preview_selection_keys_preserve_confirm_update_contract() {
    assert!(coerce_preview_selected_value(None, true));
    assert!(!coerce_preview_selected_value(Some(&json!("0")), true));
    assert!(!coerce_preview_selected_value(Some(&json!("false")), true));
    assert!(coerce_preview_selected_value(Some(&json!("yes")), false));
    assert!(coerce_preview_selected_value(Some(&json!("")), true));

    let mut update = Map::new();
    update.insert("preview_selected".to_string(), json!(true));
    update.insert("isSelected".to_string(), json!(false));
    assert!(!preview_update_is_selected(&update, true));

    let empty = Map::new();
    assert!(preview_update_is_selected(&empty, true));
}

#[test]
fn preview_matching_payload_normalizes_empty_feedback_from_flat_fields() {
    let preview = json!({
        "preview_matching_feedback": {},
        "preview_parser_id": "wechat",
        "preview_parser_tags": ["parser:wechat", "channel:wallet"],
        "dedup_type": "remaining",
        "dedup_source_ids": [7, 8],
        "preview_recurring_id": 12,
        "preview_recurring_name": "月度午餐",
        "preview_recurring_candidate_count": 2,
        "preview_recurring_match_score": 0.91,
        "preview_recurring_match_reasons": "amount|date",
        "preview_recurring_matched_date": "2026-05-01",
        "preview_is_manually_annotated": 1
    });
    let matching = build_import_preview_matching_payload(preview.as_object().unwrap());

    for key in [
        "transfer",
        "investment",
        "learning",
        "llm",
        "recurring",
        "dedup",
        "parser",
        "annotation",
        "reconciliation",
    ] {
        assert!(matching.get(key).is_some(), "missing section {key}");
    }
    assert_eq!(matching["transfer"]["candidate_type"], "");
    assert_eq!(matching["parser"]["id"], "wechat");
    assert_eq!(matching["parser"]["parser_id"], "wechat");
    assert_eq!(matching["parser"]["tags"][0], "parser:wechat");
    assert_eq!(matching["parser"]["parser_tags"][1], "channel:wallet");
    assert_eq!(matching["dedup"]["type"], "remaining");
    assert_eq!(matching["dedup"]["source_count"], 2);
    assert_eq!(matching["recurring"]["id"], 12);
    assert_eq!(matching["recurring"]["match_score"], 0.91);
    assert_eq!(matching["annotation"]["is_manually_annotated"], true);
}

#[test]
fn preview_matching_payload_preserves_sparse_feedback_extras_and_parser_aliases() {
    let preview = json!({
        "preview_parser_id": "flat-parser",
        "preview_parser_tags": ["flat"],
        "dedup_type": "remaining",
        "dedup_source_ids": [1],
        "preview_matching_feedback": {
            "parser": {
                "parser_id": "alipay",
                "parser_tags": ["parser:alipay"],
                "payment_method": "支付宝"
            },
            "dedup": {
                "type": "duplicate",
                "source_ids": [99]
            },
            "learning": {
                "mode": "exact",
                "auto_apply": true,
                "model_version": "v2"
            },
            "reconciliation": {
                "candidate_type": "duplicate",
                "candidate_id": "rc-1"
            }
        }
    });
    let matching = build_import_preview_matching_payload(preview.as_object().unwrap());

    assert_eq!(matching["transfer"]["candidate_type"], "");
    assert_eq!(matching["parser"]["id"], "alipay");
    assert_eq!(matching["parser"]["parser_id"], "alipay");
    assert_eq!(matching["parser"]["tags"][0], "parser:alipay");
    assert_eq!(matching["parser"]["parser_tags"][0], "parser:alipay");
    assert_eq!(matching["parser"]["payment_method"], "支付宝");
    assert_eq!(matching["dedup"]["type"], "duplicate");
    assert_eq!(matching["dedup"]["source_ids"][0], 99);
    assert_eq!(matching["dedup"]["source_count"], 1);
    assert_eq!(matching["learning"]["mode"], "exact");
    assert_eq!(matching["learning"]["auto_apply"], true);
    assert_eq!(matching["learning"]["model_version"], "v2");
    assert_eq!(matching["reconciliation"]["candidate_type"], "duplicate");
}

#[test]
fn preview_matching_payload_preserves_stage2_baseline_snapshot() {
    let preview = json!({
        "preview_type": "支出",
        "preview_main_category": "餐饮",
        "preview_sub_category": "午餐",
        "preview_source_account_id": 42,
        "preview_matching_feedback": {
            "stage2_baseline": {
                "preview_type": "支出",
                "preview_main_category": "餐饮",
                "preview_sub_category": "午餐",
                "preview_source_account_id": 42,
                "preview_destination_account_id": null
            }
        }
    });
    let matching = build_import_preview_matching_payload(preview.as_object().unwrap());

    assert_eq!(matching["stage2_baseline"]["preview_type"], "支出");
    assert_eq!(matching["stage2_baseline"]["preview_main_category"], "餐饮");
    assert_eq!(matching["stage2_baseline"]["preview_source_account_id"], 42);
    assert!(matching["stage2_baseline"]["preview_destination_account_id"].is_null());
}

#[test]
fn history_rewrite_matching_payload_exposes_operation_ack_evidence() {
    let operation_id =
        build_import_history_rewrite_operation_id("update_history", 9001, 3, "hist:9001");
    let expected_token = build_import_history_rewrite_ack_token(
        "session-history",
        &operation_id,
        "update_history",
        9001,
        3,
    );
    let preview = json!({
        "id": 77,
        "session_id": "session-history",
        "matching": {
            "reconciliation": {
                "planned_operation": "update_history",
                "history_bill_id": 9001,
                "history_bill_version": 3,
                "group_key": "hist:9001",
                "notice": HISTORY_REWRITE_NOTICE,
            },
            "annotation": {
                "type": "history_rewrite_pending",
                "suppressed": true,
            }
        }
    });
    let matching = build_import_preview_matching_payload(preview.as_object().unwrap());

    assert_eq!(matching["reconciliation"]["operation_id"], operation_id);
    assert_eq!(
        matching["reconciliation"]["acknowledgement_token"],
        expected_token
    );
    assert_eq!(matching["reconciliation"]["destructive_ack_required"], true);
    assert_eq!(
        matching["annotation"]["history_rewrite_notice"],
        HISTORY_REWRITE_NOTICE
    );
}

#[test]
fn preview_filter_index_item_preserves_lightweight_index_shape() {
    let preview = json!({
        "id": 42,
        "preview_date": "2026-05-01 08:00:00",
        "preview_type": "支出",
        "preview_amount_cents": -1860,
        "category_id": 9,
        "preview_main_category": "餐饮",
        "preview_sub_category": "早餐",
        "preview_source_account_id": "7",
        "preview_destination_account_id": "0",
        "preview_description": "豆浆",
        "preview_counterparty": "早餐店",
        "preview_payment_method": "微信",
        "preview_selected": "false",
        "preview_is_manually_annotated": 1,
        "preview_parser_id": "wechat",
        "preview_parser_tags": ["parser:wechat", "account:wallet"],
        "dedup_type": "similar",
        "dedup_source_ids": "10, abc, 11",
        "matching": {
            "transfer": {"suppressed": false},
            "learning": {"review_status": "accepted"}
        },
        "suggested_preview_type": "transfer",
        "transfer_suggestion_score": 0.8,
        "transfer_suggestion_reason": "same amount",
        "learning_recommendation_reason": "manual overlay",
        "learning_recommendation_summary": "matched corpus",
        "learning_recommendation_mode": "exact",
        "preview_recurring_id": 15,
        "preview_recurring_candidate_count": 2,
        "preview_recurring_match_reasons": "monthly",
        "preview_recurring_matched_date": "2026-05-01"
    });
    let preview = preview.as_object().unwrap();

    let mut categories = BTreeMap::new();
    categories.insert(
        9,
        CategoryLookup {
            name: "早餐分类".to_string(),
            ..CategoryLookup::default()
        },
    );
    let mut accounts = BTreeMap::new();
    accounts.insert(
        7,
        AccountLookup {
            name: "微信钱包".to_string(),
        },
    );

    let item = build_import_preview_filter_index_item(preview, &categories, &accounts);

    assert_eq!(item.id, 42);
    assert_eq!(item.frontend_type, 3);
    assert_eq!(item.source_amount_cents, -1860);
    assert_eq!(item.category_id, "9");
    assert_eq!(item.actual_category_name, "早餐分类");
    assert_eq!(item.source_account_id, "7");
    assert_eq!(item.destination_account_id, "");
    assert_eq!(item.actual_source_account_name, "微信钱包");
    assert_eq!(item.actual_destination_account_name, "");
    assert!(item.selected);
    assert!(item.is_manually_annotated);
    assert_eq!(item.parser_source, "wechat");
    assert_eq!(
        item.dedup_source_ids,
        vec![json!(10), json!("abc"), json!(11)]
    );
    assert_eq!(item.transfer_status.as_deref(), Some("pending"));
    assert_eq!(item.learning_status.as_deref(), Some("accepted"));
    assert_eq!(item.recurring_template_id, "15");
}

#[test]
fn preview_filter_index_item_reads_signals_from_matching_feedback_payload() {
    let preview = json!({
        "id": 77,
        "preview_date": "2026-05-02 08:00:00",
        "preview_type": "支出",
        "preview_amount_cents": 2100,
        "preview_description": "咖啡",
        "preview_counterparty": "咖啡店",
        "preview_payment_method": "支付宝",
        "preview_parser_id": "alipay",
        "preview_parser_tags": ["parser:alipay", "channel:wallet"],
        "dedup_type": "transfer",
        "preview_matching_feedback": {
            "transfer": {
                "candidate_type": "cash_transfer",
                "score": 0.91,
                "reason": "same amount",
                "review_status": "pending"
            },
            "learning": {
                "rule_id": 9,
                "score": 1.0,
                "reason": "composite exact",
                "summary": "餐饮/咖啡 | 支付宝",
                "mode": "exact",
                "review_status": "pending"
            }
        }
    });
    let preview = preview.as_object().unwrap();

    let item = build_import_preview_filter_index_item(
        preview,
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );

    assert_eq!(item.transfer_status.as_deref(), Some("pending"));
    assert_eq!(item.transfer_title, "same amount");
    assert_eq!(item.learning_status.as_deref(), Some("pending"));
    assert_eq!(item.learning_title, "composite exact");
    assert_eq!(item.learning_summary, "餐饮/咖啡 | 支付宝");
    assert_eq!(item.learning_mode, "exact");
}

#[test]
fn preview_filter_index_item_treats_auto_applied_learning_as_accepted() {
    for review_status in ["auto_applied", "auto-applied"] {
        let preview = json!({
            "id": 78,
            "preview_type": "收入",
            "preview_amount_cents": 22,
            "preview_main_category": "投资收入",
            "preview_sub_category": "理财收益",
            "preview_matching_feedback": {
                "learning": {
                    "rule_id": 9,
                    "score": 1.0,
                    "reason": "composite exact",
                    "summary": "收入 | 投资收入/理财收益 | 支付宝",
                    "mode": "exact",
                    "review_status": review_status,
                    "auto_apply": true
                }
            }
        });
        let preview = preview.as_object().unwrap();

        let item = build_import_preview_filter_index_item(
            preview,
            &BTreeMap::<i64, CategoryLookup>::new(),
            &BTreeMap::<i64, AccountLookup>::new(),
        );

        assert_eq!(item.learning_status.as_deref(), Some("accepted"));
        assert_eq!(item.learning_title, "composite exact");
        assert_eq!(item.learning_summary, "收入 | 投资收入/理财收益 | 支付宝");
        assert_eq!(item.learning_mode, "exact");
    }
}

#[test]
fn route_envelopes_and_expected_state_match_v2_error_surface() {
    assert_eq!(
        map_import_preview_type_to_frontend_value(Some(&json!("income"))),
        2
    );
    assert_eq!(
        map_import_preview_type_to_frontend_value(Some(&json!("unknown"))),
        1
    );
    assert!(expected_preview_state_is_valid(Some(&json!({
        "sessionId": "session-1",
        "reviewStatus": "pending",
        "previewType": "支出"
    }))));
    let expected: ExpectedPreviewState = serde_json::from_value(json!({
        "sessionId": "session-1",
        "reviewStatus": "pending",
        "previewType": "支出",
        "categoryId": 10,
        "recurringId": null
    }))
    .unwrap();
    assert_eq!(expected.session_id.as_deref(), Some("session-1"));
    assert_eq!(expected.category_id, Some(json!(10)));
    assert!(!expected_preview_state_is_valid(Some(&json!(null))));

    assert_eq!(
        import_v2_missing_session_id_response().body,
        json!({"success": false, "error": "Missing session_id"})
    );
    assert_eq!(
        import_v2_invalid_request_response().body,
        json!({"success": false, "error": "Invalid request"})
    );
    assert_eq!(
        preview_state_conflict_response().body,
        json!({"success": false, "error": "Preview state changed, please refresh"})
    );
    assert_eq!(preview_state_conflict_response().status_code, 409);
    assert_eq!(import_session_not_found_response().status_code, 404);
    assert_eq!(
        import_session_cancel_missing_response().body,
        json!({"success": false, "message": "Session not found"})
    );
    assert_eq!(
        import_session_cancel_success_response().body,
        json!({"success": true, "message": "Session cleared"})
    );
}

#[test]
fn stage_envelopes_and_matching_payload_pin_pipeline_wire_contracts() {
    assert_eq!(
        IMPORT_V2_PIPELINE_STEPS,
        &[
            "parse",
            "validation",
            "smart_dedup",
            "category_match",
            "account_match",
            "learning_replay",
            "recurring_projection",
            "preview",
            "confirm"
        ]
    );
    assert!(IMPORT_STAGING_TABLES.contains(&"import_sessions"));
    assert!(IMPORT_STAGING_TABLES.contains(&"import_sources"));
    assert!(IMPORT_STAGING_TABLES.contains(&"import_standard_rows"));
    assert!(IMPORT_STAGING_TABLES.contains(&"import_decision_groups"));
    assert!(IMPORT_STAGING_TABLES.contains(&"import_decision_group_members"));
    assert!(IMPORT_STAGING_TABLES.contains(&"import_history_materializations"));
    assert!(IMPORT_STAGING_TABLES.contains(&"import_confirm_operations"));
    assert!(IMPORT_STAGING_TABLES.contains(&"bills_parser_template"));
    assert!(IMPORT_STAGING_TABLES.contains(&"bills_preview"));
    assert!(BILLS_PREVIEW_CONTRACT_FIELDS.contains(&"preview_parser_tags_json"));
    assert!(!BILLS_PREVIEW_CONTRACT_FIELDS.contains(&"preview_parser_tags"));
    assert!(BILLS_PREVIEW_CONTRACT_FIELDS.contains(&"preview_recurring_name"));
    assert!(BILLS_PREVIEW_CONTRACT_FIELDS.contains(&"preview_recurring_match_score"));
    assert!(BILLS_PREVIEW_CONTRACT_FIELDS.contains(&"preview_matching_feedback_json"));

    let parse = import_stage_parse_success(ImportStageParseData {
        session_id: "sess-parse".to_string(),
        parsed_count: 2,
        files: vec![json!({"file": "a.csv", "success": true})],
        unmatched_files: vec![json!({"file": "unknown.csv"})],
        errors: vec![],
    });
    assert_eq!(parse.body["data"]["parsed_count"], 2);
    assert_eq!(
        parse.body["data"]["unmatched_files"][0]["file"],
        "unknown.csv"
    );

    let dedup = import_stage_dedup_success(ImportStageDedupData {
        session_id: "sess-preview".to_string(),
        preview: vec![json!({"id": 1})],
        preview_included: true,
        total: 2,
        after_dedup: 1,
        dedup_stats: json!({"removed": 1}),
        match_stats: json!({"transfer": 0}),
    });
    assert_eq!(dedup.body["data"]["preview_included"], true);
    assert_eq!(dedup.body["data"]["dedup_stats"]["removed"], 1);

    let confirm = import_stage_confirm_success(ImportStageConfirmData {
        imported_count: 1,
        skipped_count: 1,
        errors: vec!["duplicate".to_string()],
    });
    assert_eq!(confirm.body["data"]["imported_count"], 1);
    assert_eq!(confirm.body["data"]["errors"][0], "duplicate");

    let matching: ImportPreviewMatchingPayload = serde_json::from_value(json!({
        "transfer": {
            "candidate_type": "cash_transfer",
            "review_status": "pending",
            "source_chain": [{"source": "wechat"}]
        },
        "llm": {
            "suggested_main_category": "餐饮",
            "suggested_sub_category": "早餐",
            "review_status": "pending"
        },
        "recurring": {
            "id": 9,
            "name": "早餐月付",
            "match_score": 0.92,
            "match_reasons": "monthly"
        },
        "reconciliation": {
            "candidate_id": "rc-1",
            "candidate_type": "duplicate",
            "status": "pending",
            "source_chain": [{"source": "db"}]
        }
    }))
    .unwrap();
    let matching_value = serde_json::to_value(matching).unwrap();
    for key in [
        "transfer",
        "investment",
        "learning",
        "llm",
        "recurring",
        "dedup",
        "parser",
        "annotation",
        "reconciliation",
    ] {
        assert!(matching_value.get(key).is_some());
    }
    assert_eq!(
        matching_value["transfer"]["candidate_type"],
        "cash_transfer"
    );
    assert_eq!(matching_value["transfer"]["review_status"], "pending");
    assert_eq!(
        matching_value["transfer"]["source_chain"][0]["source"],
        "wechat"
    );
    assert_eq!(matching_value["llm"]["suggested_main_category"], "餐饮");
    assert_eq!(matching_value["recurring"]["match_score"], 0.92);
    assert_eq!(
        matching_value["reconciliation"]["candidate_type"],
        "duplicate"
    );
}

#[test]
fn success_envelopes_keep_session_and_preview_page_data_keys() {
    let session = ImportSessionSummary {
        session_id: "sess-1".to_string(),
        status: "previewing".to_string(),
        created_at: "2026-05-01 08:00:00".to_string(),
        parsed_count: 3,
        preview_count: 2,
        file_paths: json!("a.csv"),
    };
    let response = import_session_success(session);
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"]["session_id"], "sess-1");
    assert_eq!(response.body["data"]["preview_count"], 2);
    assert_eq!(response.body["data"]["file_paths"], "a.csv");

    let page = import_preview_page_success(ImportPreviewPageData {
        preview: vec![json!({"id": 1})],
        total: 1,
        page: 1,
        page_size: 50,
        query: Some(json!({"page": 1, "page_size": 50})),
        metadata: Some(json!({"counts": {"total": 1}})),
    });
    assert_eq!(
        page.body,
        json!({
            "success": true,
            "data": {
                "preview": [{"id": 1}],
                "total": 1,
                "page": 1,
                "page_size": 50,
                "query": {"page": 1, "page_size": 50},
                "metadata": {"counts": {"total": 1}}
            }
        })
    );

    let preview_value = json!({"id": 1, "preview_type": "expense"});
    let index_item = build_import_preview_filter_index_item(
        preview_value.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    let index = import_preview_index_success(ImportPreviewIndexData {
        items: vec![index_item],
        total: 1,
    });
    assert_eq!(index.body["data"]["items"][0]["id"], 1);
    assert_eq!(index.body["data"]["total"], 1);
}
