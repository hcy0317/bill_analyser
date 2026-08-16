use std::collections::BTreeMap;

use bill_analyser_core::{
    attach_import_preview_matching_payload, build_import_history_rewrite_ack_token,
    build_import_history_rewrite_operation_id, build_import_preview_filter_index_item,
    build_import_preview_matching_payload, coerce_preview_selected_value,
    expected_preview_state_is_valid, import_preview_index_success,
    import_preview_matching_feedback_has_unknown_signal_status, import_preview_page_success,
    import_preview_signal_value_is_truthy, import_session_cancel_missing_response,
    import_session_cancel_success_response, import_session_not_found_response,
    import_session_success, import_stage_confirm_success, import_stage_dedup_success,
    import_stage_parse_success, import_v2_invalid_request_response,
    import_v2_missing_session_id_response, map_import_preview_type_to_frontend_value,
    normalize_history_operation, normalize_import_preview_page_query,
    normalize_import_preview_page_sort_direction, normalize_import_preview_page_sort_key,
    normalize_page, normalize_page_size, normalize_preview_ids,
    parse_import_preview_decimal_number, preview_state_conflict_response,
    preview_update_is_selected, resolve_first_nonempty_status,
    resolve_import_preview_learning_signal_status, resolve_import_preview_llm_signal_status,
    resolve_import_preview_transfer_signal_status, sort_import_preview_page_items,
    strict_decimal_has_non_zero, strict_decimal_is_positive, AccountLookup, CategoryLookup,
    ExpectedPreviewState, ImportHistoryRewriteOperation, ImportPreviewIndexData,
    ImportPreviewMatchingPayload, ImportPreviewPageData, ImportPreviewSortDirection,
    ImportSessionSummary, ImportStageConfirmData, ImportStageDedupData, ImportStageParseData,
    PreviewFamilyEvidence, PreviewIdentityInput, PreviewLearningLevel, PreviewSignalStatus,
    PreviewStateInput, PreviewStateKernel, ReviewIssueCode, BILLS_PREVIEW_CONTRACT_FIELDS,
    HISTORY_REWRITE_NOTICE, IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES, IMPORT_STAGING_TABLES,
    IMPORT_V2_PIPELINE_STEPS,
};
use serde_json::{json, Map, Value};

include!("transfer_signal_parity_corpus.rs");

#[derive(serde::Deserialize)]
struct PreviewStateFixtureCase {
    name: String,
    input: PreviewStateInput,
    legacy_payload: Value,
    expected_signals: Vec<String>,
    expected_issues: Vec<ReviewIssueCode>,
    expected_confirmable: bool,
}

#[test]
fn preview_state_kernel_matches_canonical_v1_fixture() {
    let cases: Vec<PreviewStateFixtureCase> = serde_json::from_str(include_str!(
        "../../fixtures/import_preview_state_kernel_v1.json"
    ))
    .expect("preview state fixture");

    for case in cases {
        let expected_decisions = [
            case.input.transfer,
            case.input.history,
            case.input.learning,
            case.input.llm,
        ];
        let snapshot = PreviewStateKernel::derive(case.input);
        assert_eq!(
            snapshot.signals.names(),
            case.expected_signals,
            "{} signals",
            case.name
        );
        assert_eq!(
            snapshot.issues.codes(),
            case.expected_issues,
            "{} issues",
            case.name
        );
        assert_eq!(
            snapshot.is_confirmable(),
            case.expected_confirmable,
            "{} confirmable",
            case.name
        );
        assert_eq!(
            [
                snapshot.decisions.transfer,
                snapshot.decisions.history,
                snapshot.decisions.learning,
                snapshot.decisions.llm,
            ],
            expected_decisions,
            "{} decision states",
            case.name
        );

        let compatibility_item = build_import_preview_filter_index_item(
            case.legacy_payload
                .as_object()
                .expect("legacy preview payload"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert_eq!(
            compatibility_item.preview_state.signals.names(),
            case.expected_signals,
            "{} legacy adapter signal parity",
            case.name
        );
    }
}

#[test]
fn preview_state_kernel_derives_dual_membership_and_fails_closed_on_unknown_status() {
    let snapshot = PreviewStateKernel::derive(PreviewStateInput {
        parser_present: true,
        transfer: PreviewFamilyEvidence {
            status: PreviewSignalStatus::Pending,
            has_evidence: true,
        },
        transfer_learning_level: Some(PreviewLearningLevel::Green),
        llm: PreviewFamilyEvidence {
            status: PreviewSignalStatus::Unknown,
            has_evidence: true,
        },
        identity: PreviewIdentityInput {
            category_required: true,
            ..PreviewIdentityInput::default()
        },
        ..PreviewStateInput::default()
    });

    assert_eq!(
        snapshot.signals.names(),
        vec!["transfer", "learning"],
        "likely transfer must own both memberships while parser-only and unknown LLM stay hidden"
    );
    assert_eq!(
        snapshot.issues.codes(),
        vec![
            ReviewIssueCode::MissingCategory,
            ReviewIssueCode::UnknownLlmState
        ],
        "identity gaps and unknown signal states must fail closed as typed review issues"
    );
}

#[test]
fn filter_index_projects_legacy_row_through_preview_state_kernel() {
    let preview = json!({
        "id": 812,
        "preview_type": "支出",
        "preview_parser_id": "wechat",
        "preview_matching_feedback": {
            "transfer": {
                "review_status": "pending",
                "candidate_type": "transfer",
                "learning_level": "green"
            },
            "learning": {
                "review_status": "skipped",
                "reason": "transfer preview is protected from learning type/category overrides"
            },
            "llm": {
                "review_status": "__invalid_status__",
                "confidence": 0.95
            }
        }
    });

    let item = build_import_preview_filter_index_item(
        preview.as_object().expect("preview object"),
        &BTreeMap::new(),
        &BTreeMap::new(),
    );

    assert_eq!(
        item.preview_state.signals.names(),
        vec!["transfer", "learning"]
    );
    assert!(item
        .preview_state
        .issues
        .contains(ReviewIssueCode::UnknownLlmState));
    assert_eq!(
        serde_json::to_value(&item.preview_state).unwrap()["signals"],
        json!(["transfer", "learning"])
    );
}

#[test]
fn strict_decimal_and_text_evidence_follow_sql_grammar() {
    for (name, value, has_non_zero, decimal_positive, evidence_visible) in [
        ("trailing_dot", "1.", false, false, true),
        ("zero", "0", false, false, false),
        ("negative", "-1", true, false, false),
        ("negative_zero", "-0", false, false, false),
        ("malformed_text", "abc1", false, false, true),
        ("positive_integer", "1", true, true, true),
        ("positive_fraction", ".5", true, true, true),
    ] {
        assert_eq!(
            strict_decimal_is_positive(value),
            decimal_positive,
            "{name}"
        );
        assert_eq!(strict_decimal_has_non_zero(value), has_non_zero, "{name}");
        let preview = json!({
            "preview_type": "支出",
            "preview_matching_feedback": {"learning": {"summary": value}}
        });
        let index = build_import_preview_filter_index_item(
            preview.as_object().unwrap(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert_eq!(index.learning_status.is_some(), evidence_visible, "{name}");
    }
}

#[test]
fn transfer_index_projection_uses_shared_parity_corpus() {
    for case in TRANSFER_SIGNAL_PARITY_CORPUS {
        let transfer: Value = serde_json::from_str(case.transfer_json).unwrap();
        let preview = json!({
            "preview_type": case.preview_type,
            "preview_matching_feedback": {"transfer": transfer}
        });
        assert_eq!(
            resolve_import_preview_transfer_signal_status(preview.as_object().unwrap()).as_deref(),
            case.expected_index_status,
            "{}",
            case.name
        );
    }
}

#[test]
fn signal_index_projection_fails_closed_for_unknown_nonempty_statuses() {
    for (family, resolver) in [
        (
            "learning",
            resolve_import_preview_learning_signal_status
                as fn(&Map<String, Value>) -> Option<String>,
        ),
        ("llm", resolve_import_preview_llm_signal_status),
        ("transfer", resolve_import_preview_transfer_signal_status),
    ] {
        for status_field in [
            "review_status",
            "status",
            "lifecycle_status",
            "signal_state",
        ] {
            let preview = json!({
                "preview_type": "支出",
                "preview_matching_feedback": {
                    (family): {
                        (status_field): "__invalid_status__",
                        "score": 0.91,
                        "confidence": 0.91,
                        "candidate_type": "transfer",
                        "summary": "actionable"
                    }
                }
            });
            assert_eq!(
                resolver(preview.as_object().unwrap()),
                None,
                "{family}.{status_field}"
            );
            assert!(
                import_preview_matching_feedback_has_unknown_signal_status(
                    &preview["preview_matching_feedback"]
                ),
                "{family}.{status_field}"
            );
        }
    }

    assert!(!import_preview_matching_feedback_has_unknown_signal_status(
        &json!({"transfer": {"review_status": "pending", "status": "__ignored__"}})
    ));

    let history = json!({
        "id": 91,
        "preview_type": "支出",
        "preview_matching_feedback": {
            "reconciliation": {
                "status": "__invalid_status__",
                "planned_operation": "update_history",
                "history_bill_id": 9
            }
        }
    });
    let history_index = build_import_preview_filter_index_item(
        history.as_object().unwrap(),
        &BTreeMap::new(),
        &BTreeMap::new(),
    );
    assert_eq!(history_index.history_status, None);
    assert!(import_preview_matching_feedback_has_unknown_signal_status(
        &history["preview_matching_feedback"]
    ));
}

#[test]
fn needs_review_is_learning_specific_and_does_not_leak_to_other_signal_families() {
    for (family, resolver, expected) in [
        (
            "learning",
            resolve_import_preview_learning_signal_status
                as fn(&Map<String, Value>) -> Option<String>,
            Some("pending"),
        ),
        ("llm", resolve_import_preview_llm_signal_status, None),
        (
            "transfer",
            resolve_import_preview_transfer_signal_status,
            None,
        ),
    ] {
        let preview = json!({
            "preview_type": "支出",
            "preview_matching_feedback": {
                (family): {
                    "review_status": "needs_review",
                    "score": 0.91,
                    "confidence": 0.91,
                    "candidate_type": "transfer",
                    "summary": "actionable"
                }
            }
        });
        assert_eq!(
            resolver(preview.as_object().unwrap()).as_deref(),
            expected,
            "{family}"
        );
        assert_eq!(
            import_preview_matching_feedback_has_unknown_signal_status(
                &preview["preview_matching_feedback"]
            ),
            family != "learning",
            "{family}"
        );
    }
}

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
        vec![3, 2, 1],
        "desc counterparty: beta > alpha, id tie-break desc: 2 before 1"
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
    let operation_id = build_import_history_rewrite_operation_id(
        ImportHistoryRewriteOperation::UpdateHistory,
        9001,
        3,
        "hist:9001",
    );
    let expected_token = build_import_history_rewrite_ack_token(
        "session-history",
        &operation_id,
        ImportHistoryRewriteOperation::UpdateHistory,
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
fn visible_signal_family_order_is_rust_enum_authoritative() {
    assert_eq!(
        IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES,
        &[
            "parser",
            "platform_duplicate",
            "transfer",
            "history",
            "learning",
            "llm",
        ]
    );

    assert_eq!(
        normalize_history_operation("update_current_bill"),
        Some(ImportHistoryRewriteOperation::UpdateHistory)
    );
    assert_eq!(
        normalize_history_operation("merge_current_bill_transfer"),
        Some(ImportHistoryRewriteOperation::MergeTransferHistory)
    );
    assert_eq!(
        normalize_history_operation("merge_transfer"),
        Some(ImportHistoryRewriteOperation::MergeTransferHistory)
    );
    assert_eq!(normalize_history_operation("recurring"), None);
    assert_eq!(normalize_history_operation("UPDATE_HISTORY"), None);
}

#[test]
fn unknown_history_operation_fails_before_ack_projection() {
    let preview = json!({
        "id": 77,
        "session_id": "session-history",
        "preview_matching_feedback": {
            "reconciliation": {
                "planned_operation": "delete_history",
                "history_bill_id": 9001,
                "history_bill_version": 3,
                "group_key": "hist:9001"
            }
        }
    });
    let matching =
        build_import_preview_matching_payload(preview.as_object().expect("preview object"));

    assert_eq!(
        matching["reconciliation"]["planned_operation"],
        "delete_history"
    );
    assert!(matching["reconciliation"].get("operation_id").is_none());
    assert!(matching["reconciliation"]
        .get("acknowledgement_token")
        .is_none());
    assert!(matching["reconciliation"]
        .get("destructive_ack_required")
        .is_none());
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
fn full_preview_preserves_matching_llm_and_index_exposes_flattened_llm_signal_fields() {
    let preview = json!({
        "id": 88,
        "preview_type": "支出",
        "preview_amount_cents": -1999,
        "preview_parser_id": "alipay",
        "preview_parser_tags": ["parser:alipay"],
        "preview_matching_feedback": {
            "llm": {
                "review_status": "pending",
                "confidence": 0.83,
                "suggested_type": "expense",
                "suggested_category_id": 42,
                "suggested_main_category": "Meals",
                "suggested_sub_category": "Coffee",
                "suggested_source_account": "Checking",
                "suggested_destination_account": "Wallet",
                "reason": "merchant evidence"
            }
        }
    });
    let mut full_preview = preview.as_object().expect("preview object").clone();
    attach_import_preview_matching_payload(&mut full_preview);
    let expected_llm = full_preview["matching"]["llm"].clone();
    assert_eq!(expected_llm["review_status"], "pending");
    assert_eq!(expected_llm["confidence"], 0.83);
    assert_eq!(expected_llm["suggested_category_id"], 42);

    let index = build_import_preview_filter_index_item(
        preview.as_object().expect("preview object"),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    let index = serde_json::to_value(index).expect("serialize filter index");
    assert_eq!(index["llm_status"], "pending");
    assert_eq!(index["llm_title"], "merchant evidence");
    assert_eq!(index["llm_confidence"], 0.83);
    assert_eq!(index["llm_category_path"], "Meals/Coffee");
    assert_eq!(index["llm_source_account"], "Checking");
    assert_eq!(index["llm_destination_account"], "Wallet");
}

#[test]
fn lightweight_index_exposes_history_rewrite_fields_from_canonical_projection() {
    let preview = json!({
        "id": 89,
        "session_id": "session-history",
        "preview_type": "支出",
        "preview_amount_cents": -1888,
        "preview_parser_id": "alipay",
        "preview_parser_tags": ["parser:alipay"],
        "preview_matching_feedback": {
            "reconciliation": {
                "planned_operation": "update_current_bill",
                "history_bill_id": 9001,
                "history_bill_version": 3,
                "group_key": "hist:9001",
                "operation_id": "history:legacy-raw-operation-id",
                "notice": HISTORY_REWRITE_NOTICE,
                "history_summary": {
                    "bill_id": 9001,
                    "date_time": "2026-07-01 09:30:00",
                    "amount_cents": -1880,
                    "currency": "CNY",
                    "category_name": "餐饮 / 早餐",
                    "category_status": "known",
                    "source_account_name": "工资卡",
                    "source_account_status": "known",
                    "destination_account_name": null,
                    "destination_account_status": "unknown",
                    "identity_source": "runtime_current",
                    "counterparty": "早餐店",
                    "description": "工作日早餐"
                }
            }
        }
    });
    let mut full_preview = preview.as_object().expect("preview object").clone();
    attach_import_preview_matching_payload(&mut full_preview);
    let canonical_operation =
        normalize_history_operation("update_current_bill").expect("known operation");
    let expected_operation_id =
        build_import_history_rewrite_operation_id(canonical_operation, 9001, 3, "hist:9001");
    let expected_token = build_import_history_rewrite_ack_token(
        "session-history",
        &expected_operation_id,
        canonical_operation,
        9001,
        3,
    );
    assert_eq!(
        full_preview["matching"]["reconciliation"]["planned_operation"],
        "update_history"
    );
    assert_eq!(
        full_preview["matching"]["reconciliation"]["operation_id"],
        expected_operation_id
    );

    let index = build_import_preview_filter_index_item(
        preview.as_object().expect("preview object"),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    let index = serde_json::to_value(index).expect("serialize filter index");
    assert_eq!(index["history_status"], "pending");
    assert_eq!(index["history_title"], HISTORY_REWRITE_NOTICE);
    assert_eq!(index["history_planned_operation"], "update_history");
    assert_eq!(index["history_bill_id"], 9001);
    assert_eq!(index["history_bill_version"], 3);
    assert_eq!(index["history_operation_id"], expected_operation_id);
    assert_eq!(index["history_acknowledgement_token"], expected_token);
    assert_eq!(index["history_destructive_ack_required"], true);
    assert_eq!(index["history_summary"]["category_name"], "餐饮 / 早餐");
    assert_eq!(index["history_summary"]["source_account_name"], "工资卡");
    assert_eq!(index["history_summary"]["amount_cents"], -1880);
}

#[test]
fn planned_history_operation_aliases_are_canonical_before_ack_projection() {
    for (raw, canonical) in [
        ("update_history", "update_history"),
        ("merge_transfer_history", "merge_transfer_history"),
        ("update_current_bill", "update_history"),
        ("merge_current_bill_transfer", "merge_transfer_history"),
        ("merge_transfer", "merge_transfer_history"),
    ] {
        let preview = json!({
            "id": 77,
            "session_id": "session-history",
            "preview_matching_feedback": {
                "reconciliation": {
                    "planned_operation": raw,
                    "history_bill_id": 9001,
                    "history_bill_version": 3,
                    "group_key": "hist:9001",
                    "operation_id": "history:legacy-raw-operation-id"
                }
            }
        });
        let matching =
            build_import_preview_matching_payload(preview.as_object().expect("preview object"));
        let canonical_operation =
            normalize_history_operation(canonical).expect("canonical operation");
        let expected_operation_id =
            build_import_history_rewrite_operation_id(canonical_operation, 9001, 3, "hist:9001");
        let expected_token = build_import_history_rewrite_ack_token(
            "session-history",
            &expected_operation_id,
            canonical_operation,
            9001,
            3,
        );
        assert_eq!(
            matching["reconciliation"]["planned_operation"], canonical,
            "raw operation {raw} must normalize at the read boundary"
        );
        assert_eq!(
            matching["reconciliation"]["operation_id"], expected_operation_id,
            "raw operation {raw} must receive the canonical operation id"
        );
        assert_eq!(
            matching["reconciliation"]["acknowledgement_token"], expected_token,
            "raw operation {raw} must receive the canonical acknowledgement token"
        );
    }
}

#[test]
fn preview_filter_index_item_keeps_raw_invalid_identity_out_of_canonical_columns() {
    let preview = json!({
        "id": 79,
        "preview_date": "2026-05-03 08:00:00",
        "preview_type": "支出",
        "preview_amount_cents": 2100,
        "category_id": "/",
        "preview_main_category": "/",
        "preview_sub_category": "民生银行储蓄卡(6332)",
        "preview_source_account_id": "民生银行储蓄卡(6332)",
        "preview_destination_account_id": "/",
        "preview_payment_method": "民生银行储蓄卡(6332)",
        "preview_description": "raw import evidence"
    });
    let preview = preview.as_object().unwrap();

    let item = build_import_preview_filter_index_item(
        preview,
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );

    assert_eq!(item.category_id, "");
    assert_eq!(item.actual_category_name, "");
    assert_eq!(item.source_account_id, "");
    assert_eq!(item.destination_account_id, "");
    assert_eq!(item.actual_source_account_name, "");
    assert_eq!(item.actual_destination_account_name, "");
    assert_eq!(item.payment_method, "民生银行储蓄卡(6332)");
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

/// AC-002 回归语料：紧凑表驱动，锁定真值对齐、状态别名投影、Unicode 空白裁剪和组合多信号行。
#[test]
fn ac002_truthy_corpus_parity_and_signal_projection() {
    // --- Truthy parity: numeric values ---
    // JSON number 2 → truthy (finite, non-zero)
    assert!(import_preview_signal_value_is_truthy(&json!(2)));
    assert!(import_preview_signal_value_is_truthy(&json!(0.5)));
    assert!(import_preview_signal_value_is_truthy(&json!(-3)));
    assert!(import_preview_signal_value_is_truthy(&json!(1)));
    // zero and negative zero → falsey
    assert!(!import_preview_signal_value_is_truthy(&json!(0)));
    assert!(!import_preview_signal_value_is_truthy(&json!(-0.0)));
    // NaN and Infinity → falsey
    assert!(!import_preview_signal_value_is_truthy(&json!(f64::NAN)));
    assert!(!import_preview_signal_value_is_truthy(&json!(
        f64::INFINITY
    )));

    // --- Truthy parity: string values ---
    assert!(import_preview_signal_value_is_truthy(&json!("true")));
    assert!(import_preview_signal_value_is_truthy(&json!("yes")));
    assert!(import_preview_signal_value_is_truthy(&json!("y")));
    assert!(import_preview_signal_value_is_truthy(&json!("1")));
    assert!(import_preview_signal_value_is_truthy(&json!("2")));
    assert!(import_preview_signal_value_is_truthy(&json!("3.5")));
    assert!(!import_preview_signal_value_is_truthy(&json!("0")));
    assert!(!import_preview_signal_value_is_truthy(&json!("false")));
    assert!(!import_preview_signal_value_is_truthy(&json!("no")));
    assert!(!import_preview_signal_value_is_truthy(&json!("none")));
    assert!(!import_preview_signal_value_is_truthy(&json!("suppressed")));
    assert!(!import_preview_signal_value_is_truthy(&json!("null")));
    assert!(!import_preview_signal_value_is_truthy(&json!("")));
    assert!(!import_preview_signal_value_is_truthy(&json!("  ")));
    // negative zero string → falsey
    assert!(!import_preview_signal_value_is_truthy(&json!("-0")));
    assert!(!import_preview_signal_value_is_truthy(&json!("-0.0")));
    // malformed numeric text → falsey
    assert!(!import_preview_signal_value_is_truthy(&json!("1.2.3")));
    assert!(!import_preview_signal_value_is_truthy(&json!("abc")));

    // --- parse_import_preview_decimal_number parity ---
    assert_eq!(parse_import_preview_decimal_number("2"), Some(2.0));
    assert_eq!(parse_import_preview_decimal_number("3.5"), Some(3.5));
    assert_eq!(parse_import_preview_decimal_number("-0.0"), Some(-0.0));
    assert_eq!(parse_import_preview_decimal_number("1.2.3"), None);
    assert_eq!(parse_import_preview_decimal_number("abc"), None);
    // Unicode whitespace NBSP trim
    assert_eq!(
        parse_import_preview_decimal_number("\u{00A0}42\u{00A0}"),
        Some(42.0)
    );

    // --- Unicode whitespace trim (NBSP, ideographic space, etc.) ---
    let nbsp_value = json!(format!("\u{00A0}true\u{00A0}"));
    assert!(import_preview_signal_value_is_truthy(&nbsp_value));
    let ideographic = json!(format!("\u{3000}yes\u{3000}"));
    assert!(import_preview_signal_value_is_truthy(&ideographic));

    // --- Status alias projection: first-nonempty priority ---
    // LLM with status field (not review_status) should project correctly
    let llm_status_alias_preview = json!({
        "id": 201,
        "preview_type": "支出",
        "preview_amount_cents": -500,
        "preview_matching_feedback": {
            "llm": {
                "status": "rejected",
                "confidence": 0.9,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        llm_status_alias_preview.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.llm_status.as_deref(),
        Some("rejected"),
        "LLM must resolve status from 'status' field when 'review_status' is absent"
    );

    // LLM with lifecycle_status
    let llm_lifecycle_preview = json!({
        "id": 202,
        "preview_type": "支出",
        "preview_amount_cents": -600,
        "preview_matching_feedback": {
            "llm": {
                "lifecycle_status": "skipped",
                "confidence": 0.7,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        llm_lifecycle_preview.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.llm_status.as_deref(),
        Some("skipped"),
        "LLM must resolve status from 'lifecycle_status' when prior fields absent"
    );

    // LLM with signal_state
    let llm_signal_state_preview = json!({
        "id": 203,
        "preview_type": "支出",
        "preview_amount_cents": -700,
        "preview_matching_feedback": {
            "llm": {
                "signal_state": "accepted",
                "confidence": 0.8,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        llm_signal_state_preview.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.llm_status.as_deref(),
        Some("accepted"),
        "LLM must resolve status from 'signal_state' when all prior fields absent"
    );

    // Learning with status field alias
    let learning_status_alias_preview = json!({
        "id": 204,
        "preview_type": "支出",
        "preview_amount_cents": -800,
        "preview_matching_feedback": {
            "learning": {
                "status": "accepted",
                "score": 0.9,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        learning_status_alias_preview.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.learning_status.as_deref(),
        Some("accepted"),
        "Learning must resolve status from 'status' field when 'review_status' absent"
    );

    // --- Terminal states: auto_applied → accepted ---
    for auto_status in ["auto_applied", "auto-applied"] {
        let llm_auto = json!({
            "id": 210,
            "preview_type": "支出",
            "preview_amount_cents": -900,
            "preview_matching_feedback": {
                "llm": {
                    "review_status": auto_status,
                    "confidence": 0.85,
                    "reason": "auto test"
                }
            }
        });
        let index = build_import_preview_filter_index_item(
            llm_auto.as_object().unwrap(),
            &BTreeMap::<i64, CategoryLookup>::new(),
            &BTreeMap::<i64, AccountLookup>::new(),
        );
        assert_eq!(
            index.llm_status.as_deref(),
            Some("accepted"),
            "LLM auto_applied must canonicalize to accepted, input={auto_status}"
        );
    }

    // --- Trimmed transfer/history identifiers (NBSP around operation name) ---
    let nbsp_operation = json!({
        "id": 220,
        "session_id": "nbsp-test",
        "preview_type": "支出",
        "preview_amount_cents": -1000,
        "preview_matching_feedback": {
            "reconciliation": {
                "planned_operation": format!("\u{00A0}update_history\u{00A0}"),
                "history_bill_id": 5001,
                "history_bill_version": 2,
                "group_key": "hist:5001"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        nbsp_operation.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.history_status.as_deref(),
        Some("pending"),
        "NBSP-padded planned_operation must still resolve to pending"
    );
    assert_eq!(
        index.history_planned_operation, "update_history",
        "NBSP must be trimmed from planned_operation"
    );

    // --- Suppressed/none status → invisible ---
    for suppressed_status in ["none", "suppressed"] {
        let suppressed_preview = json!({
            "id": 230,
            "preview_type": "支出",
            "preview_amount_cents": -1100,
            "preview_matching_feedback": {
                "learning": {
                    "review_status": suppressed_status,
                    "score": 0.95,
                    "reason": "suppressed test"
                }
            }
        });
        let index = build_import_preview_filter_index_item(
            suppressed_preview.as_object().unwrap(),
            &BTreeMap::<i64, CategoryLookup>::new(),
            &BTreeMap::<i64, AccountLookup>::new(),
        );
        assert!(
            index.learning_status.is_none(),
            "suppressed/none status must be invisible, input={suppressed_status}"
        );
    }

    // --- Conflict status precedence: review_status=pending (first field) wins over status=accepted ---
    // With score=0.9 as evidence, resolved pending is visible and projects "pending"
    let conflict_review_pending_status_accepted = json!({
        "id": 250,
        "preview_type": "支出",
        "preview_amount_cents": -1200,
        "preview_matching_feedback": {
            "learning": {
                "review_status": "pending",
                "status": "accepted",
                "score": 0.9,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        conflict_review_pending_status_accepted.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.learning_status.as_deref(),
        Some("pending"),
        "review_status=pending,status=accepted → first-nonempty resolves to pending, with evidence visible"
    );

    // --- Conflict with no evidence: review_status=pending alone, no evidence → None ---
    let conflict_pending_no_evidence = json!({
        "id": 255,
        "preview_type": "支出",
        "preview_amount_cents": -1250,
        "preview_matching_feedback": {
            "learning": {
                "review_status": "pending",
                "status": "accepted",
                "score": 0.0,
                "reason": ""
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        conflict_pending_no_evidence.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.learning_status.as_deref(),
        None,
        "review_status=pending,no evidence → invisible"
    );

    // --- Conflict: review_status=accepted, status=none → first-nonempty wins (accepted) ---
    let conflict_accepted_status_none = json!({
        "id": 251,
        "preview_type": "支出",
        "preview_amount_cents": -1300,
        "preview_matching_feedback": {
            "learning": {
                "review_status": "accepted",
                "status": "none",
                "score": 0.8,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        conflict_accepted_status_none.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.learning_status.as_deref(),
        Some("accepted"),
        "review_status=accepted,status=none → first-nonempty accepted wins"
    );

    // --- Conflict: empty review_status, lifecycle_status=skipped → resolved to skipped ---
    let lifecycle_skipped = json!({
        "id": 252,
        "preview_type": "支出",
        "preview_amount_cents": -1400,
        "preview_matching_feedback": {
            "learning": {
                "review_status": "",
                "lifecycle_status": "skipped",
                "score": 0.7,
                "reason": "test"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        lifecycle_skipped.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.learning_status.as_deref(),
        Some("skipped"),
        "empty review_status + lifecycle_status=skipped → resolved to skipped"
    );

    // --- Transfer: first-nonempty for accepted/rejected ---
    let transfer_status_accepted = json!({
        "id": 253,
        "preview_type": "支出",
        "preview_amount_cents": -1500,
        "preview_matching_feedback": {
            "transfer": {
                "status": "accepted"
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        transfer_status_accepted.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.transfer_status.as_deref(),
        Some("accepted"),
        "transfer must resolve accepted from 'status' field via first-nonempty"
    );

    // --- 400-digit safe decimal never overflows truthy ---
    let big = "1".repeat(400);
    assert!(
        strict_decimal_has_non_zero(&big),
        "400-digit all-1 string must be lexically truthy"
    );
    assert!(
        strict_decimal_is_positive(&big),
        "400-digit all-1 string must be lexically positive"
    );
    assert!(
        !strict_decimal_has_non_zero("0"),
        "single zero must not be lexically truthy"
    );
    assert!(
        !strict_decimal_has_non_zero("-0.0"),
        "negative zero must not be lexically truthy"
    );
    assert!(
        !strict_decimal_has_non_zero("abc"),
        "non-decimal must not be lexically truthy"
    );
    assert!(
        !strict_decimal_is_positive("-2"),
        "negative must not be lexically positive"
    );

    // --- JSON scalar type parity: summary:1 (number) must not create text phantom signal ---
    let summary_number = json!({
        "id": 254,
        "preview_type": "支出",
        "preview_amount_cents": -1600,
        "preview_matching_feedback": {
            "learning": {
                "summary": 1
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        summary_number.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert!(
        index.learning_status.is_none(),
        "JSON number summary=1 must not create phantom text evidence"
    );

    // --- Lexical grammar validation: reject malformed with non-zero digits ---
    assert!(
        !strict_decimal_has_non_zero("abc1"),
        "abc1 has alpha chars, must be rejected by grammar"
    );
    assert!(
        !strict_decimal_has_non_zero("1e2"),
        "1e2 has exponent, must be rejected by grammar"
    );
    assert!(
        !strict_decimal_has_non_zero("1.2.3"),
        "1.2.3 has two dots, must be rejected by grammar"
    );
    assert!(
        !strict_decimal_is_positive("abc1"),
        "abc1 must not be lexically positive"
    );
    assert!(
        strict_decimal_has_non_zero(".5"),
        ".5 (dot-five) must be lexically truthy"
    );
    assert!(
        strict_decimal_is_positive(".5"),
        ".5 (dot-five) must be lexically positive"
    );
    assert!(
        !strict_decimal_is_positive("-0.001"),
        "-0.001 is negative, must not be lexically positive"
    );

    // --- Status-scoped filter: resolved status, not any-field OR ---
    // review_status=pending,status=accepted must NOT match "learning:accepted" filter
    // because resolved first-nonempty is "pending", not "accepted"
    let pending_accepted_learning = json!({
        "id": 260,
        "preview_type": "支出",
        "preview_amount_cents": -1700,
        "preview_matching_feedback": {
            "learning": {
                "review_status": "pending",
                "status": "accepted",
                "score": 0.9,
                "reason": "test"
            }
        }
    });
    let matching =
        build_import_preview_matching_payload(pending_accepted_learning.as_object().unwrap());
    let feedback = &matching;
    let resolved =
        resolve_first_nonempty_status(feedback.get("learning").and_then(Value::as_object).unwrap());
    assert_eq!(
        resolved, "pending",
        "review_status=pending,status=accepted → first-nonempty resolves to 'pending'"
    );
    // memory filter: preview_feedback_family_contains_status checks resolved status
    // This is tested via import_staging tests. At core level, just assert resolved is correct.

    // --- Stable tie sort: same date, different ids ---
    let items = vec![
        json!({"id": 3, "preview_date": "2026-06-01", "preview_amount_cents": 100, "preview_counterparty": "A"}),
        json!({"id": 1, "preview_date": "2026-06-01", "preview_amount_cents": 100, "preview_counterparty": "A"}),
        json!({"id": 2, "preview_date": "2026-06-01", "preview_amount_cents": 100, "preview_counterparty": "A"}),
    ];
    let sorted = sort_import_preview_page_items(&items, Some("time"), Some("asc"));
    assert_eq!(
        sorted
            .iter()
            .map(|i| i["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3],
        "stable tie: asc time must order by id ascending"
    );
    let sorted_desc = sort_import_preview_page_items(&items, Some("time"), Some("desc"));
    assert_eq!(
        sorted_desc
            .iter()
            .map(|i| i["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![3, 2, 1],
        "stable tie: desc time must order by id descending"
    );

    // --- Combined multi-family row: all signals present ---
    let combined_preview = json!({
        "id": 300,
        "preview_type": "支出",
        "preview_amount_cents": -2000,
        "preview_parser_id": "alipay",
        "preview_parser_tags": ["parser:alipay"],
        "dedup_type": "platform_bank",
        "preview_matching_feedback": {
            "transfer": {
                "candidate_type": "cash_transfer",
                "score": 0.88,
                "reason": "same amount",
                "review_status": "pending"
            },
            "learning": {
                "rule_id": 5,
                "score": 0.92,
                "summary": "餐饮",
                "review_status": "accepted"
            },
            "llm": {
                "confidence": 0.76,
                "reason": "merchant",
                "review_status": "pending"
            },
            "reconciliation": {
                "planned_operation": "merge_transfer_history",
                "history_bill_id": 7001,
                "history_bill_version": 1,
                "group_key": "hist:7001",
                "destructive_ack_required": true
            }
        }
    });
    let index = build_import_preview_filter_index_item(
        combined_preview.as_object().unwrap(),
        &BTreeMap::<i64, CategoryLookup>::new(),
        &BTreeMap::<i64, AccountLookup>::new(),
    );
    assert_eq!(
        index.transfer_status.as_deref(),
        Some("pending"),
        "combined row: transfer pending"
    );
    assert_eq!(
        index.learning_status.as_deref(),
        Some("accepted"),
        "combined row: learning accepted"
    );
    assert_eq!(
        index.llm_status.as_deref(),
        Some("pending"),
        "combined row: llm pending"
    );
    assert_eq!(
        index.history_status.as_deref(),
        Some("pending"),
        "combined row: history pending from destructive_ack_required"
    );
    assert_eq!(
        index.history_planned_operation, "merge_transfer_history",
        "combined row: operation canonicalized"
    );
    assert!(
        index.history_destructive_ack_required,
        "combined row: destructive ack required"
    );
}
