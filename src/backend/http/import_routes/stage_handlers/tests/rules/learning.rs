    fn learning_rule(
        id: i64,
        composite_hash: String,
        match_features: BTreeMap<String, String>,
    ) -> ImportIntelligenceLearningRule {
        ImportIntelligenceLearningRule {
            id,
            parser_id: "learning-test".to_string(),
            composite_hash,
            match_features,
            learned_type: Some("支出".to_string()),
            learned_category_id: None,
            learned_source_account_id: Some(81),
            learned_destination_account_id: Some(82),
        }
    }

    #[test]
    fn learning_projection_respects_manual_identity_and_transfer_account_authority() {
        let learned_category = category(90, 2, "规则收入", "规则分类");
        let rule = ImportIntelligenceLearningRule {
            id: 7,
            parser_id: "learning-test".to_string(),
            composite_hash: String::new(),
            match_features: BTreeMap::new(),
            learned_type: Some("收入".to_string()),
            learned_category_id: Some(90),
            learned_source_account_id: Some(81),
            learned_destination_account_id: Some(82),
        };
        let mut manual = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            category_id: Some(10),
            preview_main_category: "人工支出".to_string(),
            preview_sub_category: "人工分类".to_string(),
            preview_source_account_id: Some(11),
            preview_destination_account_id: Some(12),
            preview_matching_feedback: json!({
                "annotation": {
                    "manual_fields": {
                        "category_id": true,
                        "source_account_id": true,
                        "destination_account_id": true
                    }
                }
            }),
            ..ImportPreviewDraft::default()
        };

        apply_learning_rule_projection(
            &mut manual,
            Some("收入"),
            Some(&learned_category),
            &rule,
            false,
        );

        assert_eq!(manual.preview_type, "支出");
        assert_eq!(manual.category_id, Some(10));
        assert_eq!(manual.preview_main_category, "人工支出");
        assert_eq!(manual.preview_source_account_id, Some(11));
        assert_eq!(manual.preview_destination_account_id, Some(12));

        let transfer_category = category(91, 4, "资金往来", "内部转账");
        let mut transfer = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_source_account_id: Some(21),
            preview_destination_account_id: Some(22),
            ..ImportPreviewDraft::default()
        };
        apply_learning_rule_projection(
            &mut transfer,
            Some("转账"),
            Some(&transfer_category),
            &rule,
            true,
        );
        assert_eq!(transfer.category_id, Some(91));
        assert_eq!(transfer.preview_source_account_id, Some(21));
        assert_eq!(transfer.preview_destination_account_id, Some(22));

        let mut automatic = ImportPreviewDraft::default();
        apply_learning_rule_projection(
            &mut automatic,
            Some("收入"),
            Some(&learned_category),
            &rule,
            false,
        );
        assert_eq!(automatic.preview_type, "收入");
        assert_eq!(automatic.category_id, Some(90));
        assert_eq!(automatic.preview_source_account_id, Some(81));
        assert_eq!(automatic.preview_destination_account_id, Some(82));
    }

    #[test]
    fn learning_similarity_requires_counterparty_or_description_anchor() {
        assert!(learning_similarity_has_semantic_anchor(&json!({
            "matched_fields": ["payment_method", "counterparty"]
        })));
        assert!(!learning_similarity_has_semantic_anchor(&json!({
            "matched_fields": ["payment_method", 7]
        })));
        assert!(!learning_similarity_has_semantic_anchor(&json!({})));
    }

    #[test]
    fn learning_match_preparation_returns_errors_and_skips_invalid_recommendations() {
        let user_id = 1;
        let categories = BTreeMap::new();
        let mut blank = ImportPreviewDraft::default();
        assert!(prepare_learning_rule_match(
            0,
            user_id,
            &mut blank,
            &[],
            &categories,
        )
        .expect("blank features are not an error")
        .is_none());

        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_parser_id: "learning-test".to_string(),
            preview_counterparty: "相似商户".to_string(),
            preview_payment_method: "测试渠道".to_string(),
            preview_description: "相似描述".to_string(),
            preview_amount_cents: 1_200,
            ..ImportPreviewDraft::default()
        };
        let features = build_composite_match_features(
            &draft.preview_parser_id,
            &draft.preview_counterparty,
            &draft.preview_description,
            &draft.preview_payment_method,
        )
        .expect("learning features");
        let similar_rule = learning_rule(41, String::new(), features.clone());
        let prepared = prepare_learning_rule_match(
            0,
            user_id,
            &mut draft,
            &[similar_rule],
            &categories,
        )
        .expect("matching is prepared before lifecycle lookup")
        .expect("matching learning rule");
        assert_eq!(prepared.draft_index, 0);
        assert!(!prepared.recommendation_key.is_empty());

        let exact_rule = learning_rule(42, composite_hash_from_features(&features), features);
        for invalid_user_id in [-1, 0] {
            let error = prepare_learning_rule_match(
                0,
                invalid_user_id,
                &mut draft.clone(),
                std::slice::from_ref(&exact_rule),
                &categories,
            )
            .expect_err("invalid user id must propagate as a DB error");
            assert!(error.to_string().contains("invalid user id"));
        }

        let mut missing_category_rule = exact_rule.clone();
        missing_category_rule.learned_category_id = Some(999_999);
        let mut missing_category = draft.clone();
        assert!(prepare_learning_rule_match(
            0,
            user_id,
            &mut missing_category,
            &[missing_category_rule],
            &categories,
        )
        .expect("missing category is a skipped recommendation")
        .is_none());
        assert_eq!(
            missing_category
                .preview_matching_feedback
                .pointer("/learning/review_status"),
            Some(&json!("skipped"))
        );

        let incompatible_category = category(99, 4, "资金往来", "转账");
        let mut categories = BTreeMap::new();
        categories.insert(incompatible_category.id, incompatible_category);
        let mut incompatible_rule = exact_rule;
        incompatible_rule.learned_category_id = Some(99);
        let mut incompatible = draft;
        assert!(prepare_learning_rule_match(
            0,
            user_id,
            &mut incompatible,
            &[incompatible_rule],
            &categories,
        )
        .expect("incompatible category is a skipped recommendation")
        .is_none());
        assert_eq!(
            incompatible
                .preview_matching_feedback
                .pointer("/learning/review_status"),
            Some(&json!("skipped"))
        );
    }

    #[test]
    fn prepared_learning_match_respects_suppressed_pending_and_auto_apply_lifecycle() {
        let base = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_parser_id: "learning-test".to_string(),
            preview_counterparty: "批量商户".to_string(),
            preview_payment_method: "批量渠道".to_string(),
            preview_description: "批量描述".to_string(),
            preview_amount_cents: 2_400,
            ..ImportPreviewDraft::default()
        };
        let features = build_composite_match_features(
            &base.preview_parser_id,
            &base.preview_counterparty,
            &base.preview_description,
            &base.preview_payment_method,
        )
        .expect("learning features");
        let rule = learning_rule(51, composite_hash_from_features(&features), features);
        let prepare = |draft: &mut ImportPreviewDraft| {
            prepare_learning_rule_match(0, 1, draft, std::slice::from_ref(&rule), &BTreeMap::new())
                .expect("prepare learning match")
                .expect("matching rule")
        };

        let mut suppressed = base.clone();
        let suppressed_match = prepare(&mut suppressed);
        let suppressed_lifecycle = ImportLearningLifecycleView {
            recommendation_key: suppressed_match.recommendation_key.clone(),
            recommendation_type: "expense".to_string(),
            status: "auto_applied".to_string(),
            signal_state: "green".to_string(),
            accepted_count: 2,
            rejected_count: 1,
            auto_applied_count: 1,
            auto_apply_enabled: true,
            suppressed: true,
        };
        assert!(apply_prepared_learning_rule_match(
            &mut suppressed,
            suppressed_match,
            &suppressed_lifecycle,
            &[],
            &[],
        )
        .is_none());
        assert!(suppressed
            .preview_matching_feedback
            .get("learning")
            .is_none());

        let mut pending = base.clone();
        let pending_match = prepare(&mut pending);
        let pending_lifecycle = ImportLearningLifecycleView {
            recommendation_key: pending_match.recommendation_key.clone(),
            recommendation_type: "expense".to_string(),
            status: "pending".to_string(),
            signal_state: "yellow".to_string(),
            accepted_count: 0,
            rejected_count: 0,
            auto_applied_count: 0,
            auto_apply_enabled: false,
            suppressed: false,
        };
        let pending_result = apply_prepared_learning_rule_match(
            &mut pending,
            pending_match,
            &pending_lifecycle,
            &[],
            &[],
        )
        .expect("pending recommendation remains visible");
        assert!(!pending_result.auto_applied);
        assert_eq!(pending.preview_source_account_id, None);
        assert_eq!(
            pending
                .preview_matching_feedback
                .pointer("/learning/review_status"),
            Some(&json!("pending"))
        );

        let mut automatic = base;
        let automatic_match = prepare(&mut automatic);
        let automatic_lifecycle = ImportLearningLifecycleView {
            recommendation_key: automatic_match.recommendation_key.clone(),
            recommendation_type: "expense".to_string(),
            status: "green".to_string(),
            signal_state: "green".to_string(),
            accepted_count: 4,
            rejected_count: 0,
            auto_applied_count: 3,
            auto_apply_enabled: true,
            suppressed: false,
        };
        let automatic_result = apply_prepared_learning_rule_match(
            &mut automatic,
            automatic_match,
            &automatic_lifecycle,
            &[],
            &[],
        )
        .expect("green recommendation is applied");
        assert!(automatic_result.auto_applied);
        assert_eq!(automatic.preview_source_account_id, Some(81));
        assert_eq!(automatic.preview_destination_account_id, Some(82));
        assert_eq!(
            automatic
                .preview_matching_feedback
                .pointer("/learning/review_status"),
            Some(&json!("auto_applied"))
        );
    }

    #[test]
    fn stage2_category_matched_counts_auto_applied_learning_projection() {
        let mut draft = ImportPreviewDraft {
            preview_type: "收入".to_string(),
            preview_parser_id: "learning-test".to_string(),
            preview_counterparty: "统计商户".to_string(),
            preview_payment_method: "统计渠道".to_string(),
            preview_description: "统计描述".to_string(),
            preview_amount_cents: 3_600,
            ..ImportPreviewDraft::default()
        };
        let features = build_composite_match_features(
            &draft.preview_parser_id,
            &draft.preview_counterparty,
            &draft.preview_description,
            &draft.preview_payment_method,
        )
        .expect("learning features");
        let rule = learning_rule(61, composite_hash_from_features(&features), features);
        let context = ImportStage2ContextSnapshot {
            user_id: 1,
            categories: Vec::new(),
            categories_by_id: BTreeMap::new(),
            category_values: Vec::new(),
            category_rules: ImportIntelligenceRuleSet::default(),
            accounts: Vec::new(),
            account_values: Vec::new(),
            account_rules: Vec::new(),
            learning_rules: vec![rule],
            recurring_templates: Vec::new(),
            transfer_category: None,
        };
        let (mut stats, prepared, category_before_projection) =
            prepare_import_intelligence_snapshot(std::slice::from_mut(&mut draft), &context)
                .expect("prepare stage2");
        let key = prepared[0].recommendation_key.clone();
        let lifecycle_by_key = BTreeMap::from([(
            key.clone(),
            ImportLearningLifecycleView {
                recommendation_key: key,
                recommendation_type: "expense".to_string(),
                status: "green".to_string(),
                signal_state: "green".to_string(),
                accepted_count: 4,
                rejected_count: 0,
                auto_applied_count: 3,
                auto_apply_enabled: true,
                suppressed: false,
            },
        )]);

        finish_import_intelligence_snapshot(
            std::slice::from_mut(&mut draft),
            &context,
            prepared,
            category_before_projection,
            &lifecycle_by_key,
            &mut stats,
        );

        assert_eq!(draft.preview_type, "支出");
        assert_eq!(stats.category_matched, 1);
    }
