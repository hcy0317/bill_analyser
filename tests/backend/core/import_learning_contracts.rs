use std::collections::BTreeMap;

use bill_analyser_core::{
    amount_cents_bucket, build_composite_match_features, build_composite_match_hash,
    build_dataset_snapshot_payload, build_feature_payload,
    build_import_learning_recommendation_key, build_label_confirmation_counts,
    build_llm_preview_apply_plan, build_model_registry_payload, build_route_label,
    build_semantic_label, build_semantic_label_counts, evaluate_learning_policy,
    import_learning_model_version, iter_feature_tokens, learning_batch_accept_response,
    learning_center_page_response, learning_lifecycle_is_auto_eligible,
    learning_lifecycle_signal_state, learning_rules_page_response, llm_error_response,
    llm_memory_events_success, normalize_import_learning_suggestion_id,
    normalize_import_learning_text, normalize_learning_text, normalize_llm_preview_review_decision,
    parse_composite_match_value, parse_learning_suggestion_ids, parse_preview_ids,
    parse_route_label, parse_semantic_label, prepare_training_samples,
    should_revert_llm_preview_application, transition_import_learning_lifecycle,
    ImportLearningLifecycleState, ImportLearningPrediction, ImportLearningRecommendationKeyInput,
    LlmMemoryEventContract, LlmPreviewSnapshot, LlmPreviewSuggestion,
    BLUE_ACCEPT_CONFIRMATION_THRESHOLD, BLUE_CONFIDENCE_THRESHOLD, BLUE_MARGIN_THRESHOLD,
    DEFAULT_FEATURE_DIMENSION, FEATURE_SCHEMA_VERSION, GREEN_CONFIDENCE_THRESHOLD,
    GREEN_MARGIN_THRESHOLD, HIDDEN_DIMENSION, LEARNING_LIFECYCLE_ACCEPTS_TO_GREEN,
    LEARNING_LIFECYCLE_GREEN_REJECTS_TO_DOWNGRADE, LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED,
    LEARNING_LIFECYCLE_STATUS_DOWNGRADED, LEARNING_LIFECYCLE_STATUS_GREEN,
    LEARNING_LIFECYCLE_STATUS_SUPPRESSED, LEARNING_LIFECYCLE_STATUS_YELLOW,
    LEARNING_LIFECYCLE_YELLOW_REJECTS_TO_SUPPRESS, MIN_TRAINING_SAMPLES, MODEL_FAMILY, MODEL_KEY,
    POLICY_VERSION, RECOMMENDATION_KEY_SCHEMA_VERSION,
};
use serde_json::json;

#[test]
fn composite_match_hash_and_parser_preserve_learning_rule_contract() {
    assert_eq!(
        normalize_import_learning_text(Some(&json!("  A | b || c  "))),
        "a | b | c"
    );
    assert_eq!(
        build_composite_match_hash("WeChat", " 早餐店 ", "豆浆 | 包子", ""),
        Some("c=早餐店|d=豆浆 | 包子|p=wechat".to_string())
    );
    assert!(build_composite_match_hash("wechat", "", "", "").is_none());

    let parsed = parse_composite_match_value(Some(&json!(
        "parser=wechat|counterparty=早餐店|payment=零钱"
    )))
    .unwrap();
    assert_eq!(parsed.get("parser_id").map(String::as_str), Some("wechat"));
    assert_eq!(
        parsed.get("payment_method").map(String::as_str),
        Some("零钱")
    );

    let features = build_composite_match_features("wechat", "早餐店", "", "零钱").unwrap();
    assert_eq!(
        features.keys().cloned().collect::<Vec<_>>(),
        vec![
            "counterparty".to_string(),
            "parser_id".to_string(),
            "payment_method".to_string()
        ]
    );
    assert_eq!(
        normalize_import_learning_suggestion_id(Some(&json!("9"))),
        Some(9)
    );
    assert_eq!(
        normalize_import_learning_suggestion_id(Some(&json!(0))),
        None
    );
}

#[test]
fn recommendation_key_is_stable_for_similar_features_and_invalidates_on_schema_or_tuple() {
    assert_eq!(
        RECOMMENDATION_KEY_SCHEMA_VERSION,
        "import-learning-recommendation-key-v1"
    );
    let base = ImportLearningRecommendationKeyInput {
        user_id: 42,
        recommendation_type: "import_preview".to_string(),
        recommended_type: "支出".to_string(),
        recommended_category_id: Some(12),
        recommended_source_account_id: Some(3),
        recommended_destination_account_id: None,
        transaction_type_scope: "支出".to_string(),
        parser_bucket: " WeChat ".to_string(),
        counterparty_bucket: " 早餐  店 ".to_string(),
        payment_bucket: "零钱".to_string(),
        description_bucket: "豆浆 | 包子".to_string(),
        amount_bucket: Some("lt20".to_string()),
        suppression_scope: "default".to_string(),
        transfer_protected: false,
        ..ImportLearningRecommendationKeyInput::default()
    };
    let same = ImportLearningRecommendationKeyInput {
        parser_bucket: "wechat".to_string(),
        counterparty_bucket: "早餐 店".to_string(),
        payment_bucket: " 零钱 ".to_string(),
        description_bucket: "豆浆|包子".to_string(),
        ..base.clone()
    };
    assert_eq!(
        build_import_learning_recommendation_key(&base),
        build_import_learning_recommendation_key(&same)
    );

    let changed_tuple = ImportLearningRecommendationKeyInput {
        recommended_category_id: Some(13),
        ..base.clone()
    };
    assert_ne!(
        build_import_learning_recommendation_key(&base),
        build_import_learning_recommendation_key(&changed_tuple)
    );

    let current_schema = ImportLearningRecommendationKeyInput {
        feature_schema_version: "import-learning-recommendation-key-v2".to_string(),
        ..base.clone()
    };
    assert_ne!(
        build_import_learning_recommendation_key(&base),
        build_import_learning_recommendation_key(&current_schema)
    );
}

#[test]
fn learning_lifecycle_thresholds_have_no_off_by_one_and_transfer_signal_states() {
    assert_eq!(LEARNING_LIFECYCLE_ACCEPTS_TO_GREEN, 3);
    assert_eq!(LEARNING_LIFECYCLE_GREEN_REJECTS_TO_DOWNGRADE, 2);
    assert_eq!(LEARNING_LIFECYCLE_YELLOW_REJECTS_TO_SUPPRESS, 3);

    let yellow_two_accepts = ImportLearningLifecycleState {
        accepted_count: 2,
        rejected_count: 2,
        ..ImportLearningLifecycleState::default()
    };
    let third_accept = transition_import_learning_lifecycle(&yellow_two_accepts, "accept");
    assert_eq!(
        third_accept.previous_status,
        LEARNING_LIFECYCLE_STATUS_YELLOW
    );
    assert_eq!(third_accept.next_status, LEARNING_LIFECYCLE_STATUS_GREEN);
    assert_eq!(third_accept.signal_state, "green");
    assert_eq!(third_accept.rejected_count, 0);
    assert!(third_accept.auto_apply_enabled);

    let first_green_reject = transition_import_learning_lifecycle(
        &ImportLearningLifecycleState {
            status: LEARNING_LIFECYCLE_STATUS_GREEN.to_string(),
            accepted_count: third_accept.accepted_count,
            rejected_count: third_accept.rejected_count,
            auto_applied_count: 0,
        },
        "reject",
    );
    assert_eq!(
        first_green_reject.next_status,
        LEARNING_LIFECYCLE_STATUS_GREEN
    );
    assert_eq!(first_green_reject.accepted_count, 3);
    assert_eq!(first_green_reject.rejected_count, 1);

    let green_one_reject = ImportLearningLifecycleState {
        status: LEARNING_LIFECYCLE_STATUS_GREEN.to_string(),
        accepted_count: 3,
        rejected_count: 1,
        auto_applied_count: 0,
    };
    let second_green_reject = transition_import_learning_lifecycle(&green_one_reject, "reject");
    assert_eq!(
        second_green_reject.next_status,
        LEARNING_LIFECYCLE_STATUS_DOWNGRADED
    );
    assert_eq!(second_green_reject.event_type, "downgrade");
    assert_eq!(second_green_reject.signal_state, "yellow");
    assert_eq!(second_green_reject.accepted_count, 0);
    assert_eq!(second_green_reject.rejected_count, 0);
    assert!(!second_green_reject.auto_apply_enabled);

    let yellow_two_rejects = ImportLearningLifecycleState {
        rejected_count: 2,
        ..ImportLearningLifecycleState::default()
    };
    let third_yellow_reject = transition_import_learning_lifecycle(&yellow_two_rejects, "reject");
    assert_eq!(
        third_yellow_reject.next_status,
        LEARNING_LIFECYCLE_STATUS_SUPPRESSED
    );
    assert_eq!(third_yellow_reject.event_type, "suppress");
    assert_eq!(third_yellow_reject.signal_state, "suppressed");
    assert!(third_yellow_reject.suppressed);

    let auto_applied = transition_import_learning_lifecycle(
        &ImportLearningLifecycleState {
            status: LEARNING_LIFECYCLE_STATUS_GREEN.to_string(),
            accepted_count: 3,
            rejected_count: 0,
            auto_applied_count: 0,
        },
        "auto_apply",
    );
    assert_eq!(
        auto_applied.next_status,
        LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED
    );
    assert_eq!(auto_applied.auto_applied_count, 1);
    assert!(learning_lifecycle_is_auto_eligible(
        &auto_applied.next_status
    ));
    assert_eq!(
        learning_lifecycle_signal_state(LEARNING_LIFECYCLE_STATUS_DOWNGRADED),
        "yellow"
    );
}

#[test]
fn feature_labels_samples_and_tokens_match_dual_head_training_contract() {
    assert_eq!(FEATURE_SCHEMA_VERSION, "import-learning-features-v1");
    assert_eq!(DEFAULT_FEATURE_DIMENSION, 96);
    assert_eq!(MODEL_KEY, "import-learning-dual-head");
    assert_eq!(MODEL_FAMILY, "shared-hidden-dual-softmax-v1");
    assert_eq!(MIN_TRAINING_SAMPLES, 3);
    assert_eq!(HIDDEN_DIMENSION, 16);

    let row = json!({
        "id": 7,
        "parser_id": "WeChat",
        "counterparty": " 早餐 店 ",
        "description": "豆浆-包子",
        "payment_method": "零钱",
        "annotated_type": "支出",
        "annotated_category_id": "12",
        "annotated_source_account_id": 3,
        "annotated_destination_account_id": null,
        "source_snapshot_json": {"preview_amount_cents": -1860, "preview_type": "expense"}
    });
    let row_map = row.as_object().unwrap();

    assert_eq!(
        normalize_learning_text(Some(&json!("  WeChat   Pay  "))),
        "wechat pay"
    );
    assert_eq!(amount_cents_bucket(Some(&json!(0))), "zero");
    assert_eq!(amount_cents_bucket(Some(&json!(1999))), "lt20");
    assert_eq!(amount_cents_bucket(Some(&json!(9999))), "lt100");
    assert_eq!(amount_cents_bucket(Some(&json!(49999))), "lt500");
    assert_eq!(amount_cents_bucket(Some(&json!(50000))), "gte500");
    assert_eq!(build_semantic_label(row_map), "type=支出|category=12");
    assert_eq!(build_route_label(row_map), "source=3|destination=0");
    assert_eq!(
        parse_semantic_label("type=支出|category=12").category_id,
        Some(12)
    );
    assert_eq!(
        parse_route_label("source=3|destination=0").source_account_id,
        Some(3)
    );
    assert_eq!(
        parse_route_label("source=3|destination=0").destination_account_id,
        None
    );

    let features = build_feature_payload(row_map);
    assert_eq!(features["parser_id"], "wechat");
    assert_eq!(features["amount_bucket"], "lt20");
    assert_eq!(features["preview_type"], "expense");
    let row_with_blank_snapshot_type = json!({
        "id": 8,
        "parser_id": "wechat",
        "counterparty": "早餐店",
        "description": "豆浆",
        "payment_method": "零钱",
        "annotated_type": "收入",
        "annotated_category_id": 12,
        "source_snapshot_json": {"preview_amount_cents": 500, "preview_type": "   "}
    });
    assert_eq!(
        build_feature_payload(row_with_blank_snapshot_type.as_object().unwrap())["preview_type"],
        "收入"
    );
    let tokens = iter_feature_tokens(&features);
    assert!(tokens.contains(&"parser_id=wechat".to_string()));
    assert!(tokens.contains(&"description:tok=豆浆".to_string()));
    assert!(tokens.contains(&"description:bi=豆浆".to_string()));

    let samples = prepare_training_samples(&[row]);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].sample_id, 7);
    let counts = build_label_confirmation_counts(&samples);
    assert_eq!(
        counts.get("type=支出|category=12||source=3|destination=0"),
        Some(&1)
    );
}

#[test]
fn green_blue_policy_thresholds_and_model_metadata_match_current_contract() {
    assert_eq!(POLICY_VERSION, "learning-green-blue-policy-v1");
    assert_eq!(GREEN_CONFIDENCE_THRESHOLD, 0.52);
    assert_eq!(GREEN_MARGIN_THRESHOLD, 0.02);
    assert_eq!(BLUE_CONFIDENCE_THRESHOLD, 0.70);
    assert_eq!(BLUE_MARGIN_THRESHOLD, 0.05);
    assert_eq!(BLUE_ACCEPT_CONFIRMATION_THRESHOLD, 2);

    let weak = evaluate_learning_policy(
        &ImportLearningPrediction {
            semantic_label: "type=支出|category=12".to_string(),
            route_label: "source=3|destination=0".to_string(),
            semantic_confidence: 0.51,
            route_confidence: 0.8,
            semantic_margin: 0.2,
            route_margin: 0.2,
        },
        9,
        &[],
    );
    assert_eq!(weak.mode, "none");
    assert_eq!(weak.rejection_reasons, vec!["green_confidence"]);

    let green = evaluate_learning_policy(
        &ImportLearningPrediction {
            semantic_label: "type=支出|category=12".to_string(),
            route_label: "source=3|destination=0".to_string(),
            semantic_confidence: 0.74,
            route_confidence: 0.71,
            semantic_margin: 0.07,
            route_margin: 0.06,
        },
        2,
        &["category_conflict".to_string()],
    );
    assert_eq!(green.mode, "green");
    assert!(!green.auto_apply);
    assert_eq!(
        green.rejection_reasons,
        vec!["confirmations", "conflict:category_conflict"]
    );

    let blue = evaluate_learning_policy(
        &ImportLearningPrediction {
            semantic_label: "type=支出|category=12".to_string(),
            route_label: "source=3|destination=0".to_string(),
            semantic_confidence: 0.9,
            route_confidence: 0.88,
            semantic_margin: 0.11,
            route_margin: 0.09,
        },
        3,
        &[],
    );
    assert_eq!(blue.mode, "blue");
    assert!(blue.auto_apply);
    assert_eq!(blue.level, "high");

    let sample = bill_analyser_core::ImportLearningTrainingSample {
        sample_id: 7,
        features: BTreeMap::new(),
        semantic_label: "type=支出|category=12".to_string(),
        route_label: "source=3|destination=0".to_string(),
    };
    let snapshot = build_dataset_snapshot_payload(std::slice::from_ref(&sample));
    assert_eq!(snapshot.feature_schema_version, FEATURE_SCHEMA_VERSION);
    assert_eq!(snapshot.policy_version, POLICY_VERSION);
    assert_eq!(snapshot.sample_ids, vec![7]);
    assert_eq!(snapshot.semantic_label_counts["type=支出|category=12"], 1);
    assert_eq!(
        snapshot.joint_label_confirmation_counts["type=支出|category=12||source=3|destination=0"],
        1
    );
    assert_eq!(import_learning_model_version(15), "v15");
    assert_eq!(
        build_semantic_label_counts(std::slice::from_ref(&sample))["type=支出|category=12"],
        1
    );

    let model_payload = build_model_registry_payload(
        json!({"model_family": MODEL_FAMILY}),
        json!({"sample_count": 3}),
        build_label_confirmation_counts(&[sample]),
    );
    assert_eq!(model_payload.parameter_ref, "metrics_json.model_parameters");
    assert_eq!(
        model_payload.joint_label_confirmation_counts
            ["type=支出|category=12||source=3|destination=0"],
        1
    );
}

#[test]
fn llm_preview_memory_only_fills_blank_fields_and_reject_revert_is_guarded() {
    let current = LlmPreviewSnapshot {
        preview_main_category: "   ".to_string(),
        preview_sub_category: " ".to_string(),
        preview_source_account_id: None,
        preview_destination_account_id: Some(8),
    };
    let suggestion = LlmPreviewSuggestion {
        suggested_main_category: "餐饮".to_string(),
        suggested_sub_category: "早餐".to_string(),
        suggested_source_account: "微信钱包".to_string(),
        suggested_destination_account: "银行卡".to_string(),
        confidence: 0.81,
        reason: "merchant history".to_string(),
    };
    let plan = build_llm_preview_apply_plan(current.clone(), &suggestion, Some(3), Some(9));
    assert_eq!(plan.previous_preview_snapshot, current);
    assert_eq!(plan.applied_preview_snapshot.preview_main_category, "餐饮");
    assert_eq!(
        plan.applied_preview_snapshot.preview_source_account_id,
        Some(3)
    );
    assert_eq!(
        plan.applied_preview_snapshot.preview_destination_account_id,
        Some(8)
    );
    assert_eq!(
        plan.applied_fields,
        vec![
            "preview_main_category",
            "preview_sub_category",
            "preview_source_account_id"
        ]
    );

    assert_eq!(
        normalize_llm_preview_review_decision(" Accept "),
        Some("accept")
    );
    assert_eq!(
        normalize_llm_preview_review_decision("reject"),
        Some("reject")
    );
    assert_eq!(normalize_llm_preview_review_decision("other"), None);
    assert!(should_revert_llm_preview_application(
        "reject",
        Some(&plan.previous_preview_snapshot),
        Some(&plan.applied_preview_snapshot),
        &plan.applied_preview_snapshot,
    ));
    assert!(!should_revert_llm_preview_application(
        "reject",
        Some(&plan.previous_preview_snapshot),
        Some(&plan.applied_preview_snapshot),
        &LlmPreviewSnapshot::default(),
    ));

    let event = LlmMemoryEventContract {
        id: Some(99),
        user_id: 1,
        session_id: Some("sess-llm".to_string()),
        preview_id: Some(42),
        event_type: "recommendation".to_string(),
        decision: None,
        prompt_text: Some("prompt".to_string()),
        llm_response_raw: Some("response".to_string()),
        llm_provider: Some("openai".to_string()),
        llm_model: Some("gpt".to_string()),
        suggested_main_category: Some("餐饮".to_string()),
        suggested_sub_category: Some("早餐".to_string()),
        suggested_source_account: Some("微信钱包".to_string()),
        suggested_destination_account: None,
        confidence: 0.81,
        user_correction_category: None,
        user_correction_account: None,
        snapshot_before: Some(serde_json::to_string(&plan.previous_preview_snapshot).unwrap()),
        snapshot_after: Some(serde_json::to_string(&plan.applied_preview_snapshot).unwrap()),
        metadata: Some(json!({"applied_fields": plan.applied_fields}).to_string()),
        created_at: Some("2026-05-01T00:00:00Z".to_string()),
    };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["session_id"], "sess-llm");
    assert_eq!(value["event_type"], "recommendation");
    assert_eq!(value["llm_response_raw"], "response");
    assert!(value["metadata"]
        .as_str()
        .unwrap()
        .contains("applied_fields"));
    let memory_response = llm_memory_events_success(vec![event], 1);
    assert_eq!(memory_response.body["data"][0]["id"], 99);
    assert!(memory_response.body["data"][0]["metadata"].is_string());
    assert_eq!(memory_response.body["total"], 1);
}

#[test]
fn learning_and_llm_route_envelopes_preserve_error_and_paging_shapes() {
    assert_eq!(parse_preview_ids(None).unwrap(), None);
    assert_eq!(
        parse_preview_ids(Some(&json!([3, 4]))).unwrap(),
        Some(vec![3, 4])
    );
    assert_eq!(
        parse_preview_ids(Some(&json!([]))).unwrap(),
        Some(Vec::<i64>::new())
    );
    assert_eq!(
        parse_preview_ids(Some(&json!("bad"))).unwrap_err(),
        "previewIds must be an array"
    );
    assert_eq!(
        parse_preview_ids(Some(&json!(["4"]))).unwrap_err(),
        "previewIds must contain positive integers"
    );
    assert_eq!(
        parse_preview_ids(Some(&json!([true]))).unwrap_err(),
        "previewIds must contain positive integers"
    );

    let suggestions = learning_center_page_response(vec![json!({"id": 1})], 1, 2000, -3);
    assert_eq!(suggestions.status_code, 200);
    assert_eq!(suggestions.body["data"]["items"][0]["id"], 1);
    assert_eq!(suggestions.body["data"]["limit"], 1000);
    assert_eq!(suggestions.body["data"]["offset"], 0);
    let all_suggestions = learning_center_page_response(Vec::new(), 0, -1, -3);
    assert_eq!(all_suggestions.body["data"]["limit"], -1);
    assert_eq!(all_suggestions.body["data"]["offset"], 0);

    assert_eq!(
        parse_learning_suggestion_ids(Some(&json!([3, "4", 3, true, false]))).unwrap(),
        vec![3, 4, 1, 0]
    );
    assert_eq!(
        parse_learning_suggestion_ids(Some(&json!([]))).unwrap_err(),
        "suggestionIds must be a non-empty array"
    );
    assert_eq!(
        parse_learning_suggestion_ids(Some(&json!(null))).unwrap_err(),
        "suggestionIds must be a non-empty array"
    );
    assert_eq!(
        parse_learning_suggestion_ids(Some(&json!((1..=101).collect::<Vec<i32>>()))).unwrap_err(),
        "batch size must not exceed 100"
    );
    assert!(parse_learning_suggestion_ids(Some(&json!(["bad"])))
        .unwrap_err()
        .starts_with("Invalid suggestionIds:"));
    let batch = learning_batch_accept_response(
        vec![json!({"id": 3, "ruleId": 9})],
        vec![json!({"id": 404, "error": "suggestion_not_found"})],
    );
    assert_eq!(batch.body["data"]["acceptedCount"], 1);
    assert_eq!(batch.body["data"]["failedCount"], 1);

    let rules = learning_rules_page_response(vec![json!({"id": 2})], 11, 2, 5);
    assert_eq!(rules.body["result"][0]["id"], 2);
    assert_eq!(rules.body["totalCount"], 11);
    assert_eq!(rules.body["page"], 2);
    assert_eq!(rules.body["pageSize"], 5);
    assert_eq!(rules.body["totalPages"], 3);
    let empty_rules = learning_rules_page_response(Vec::new(), 0, 5, 0);
    assert_eq!(empty_rules.body["page"], 1);
    assert_eq!(empty_rules.body["pageSize"], 100);
    assert_eq!(empty_rules.body["totalPages"], 1);
    let all_rules = learning_rules_page_response(Vec::new(), 0, 1, -1);
    assert_eq!(all_rules.body["pageSize"], -1);
    assert_eq!(all_rules.body["totalPages"], 1);

    assert_eq!(
        llm_error_response(404, "Import session not found", "IMPORT_SESSION_NOT_FOUND").body,
        json!({
            "success": false,
            "error": "Import session not found",
            "code": "IMPORT_SESSION_NOT_FOUND",
            "error_code": "IMPORT_SESSION_NOT_FOUND"
        })
    );
}
