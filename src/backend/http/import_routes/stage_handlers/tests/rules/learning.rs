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

    #[tokio::test(flavor = "multi_thread")]
    async fn learning_match_returns_explicit_errors_and_skips_invalid_recommendations() {
        let Some((state, user_id, _)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = open_runtime(&state).expect("import runtime");
        let categories = BTreeMap::new();
        let mut blank = ImportPreviewDraft::default();
        assert!(apply_learning_rule_match(
            runtime.connection(),
            user_id,
            &mut blank,
            &[],
            &categories,
            &[],
            &[],
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
        assert!(apply_learning_rule_match(
            runtime.connection(),
            user_id,
            &mut draft,
            &[similar_rule],
            &categories,
            &[],
            &[],
        )
        .expect("missing lifecycle skips recommendation")
        .is_none());

        let exact_rule = learning_rule(42, composite_hash_from_features(&features), features);
        for invalid_user_id in [-1, 0] {
            let error = apply_learning_rule_match(
                runtime.connection(),
                invalid_user_id,
                &mut draft.clone(),
                std::slice::from_ref(&exact_rule),
                &categories,
                &[],
                &[],
            )
            .expect_err("invalid user id must propagate as a DB error");
            assert!(error.to_string().contains("invalid user id"));
        }

        let mut missing_category_rule = exact_rule.clone();
        missing_category_rule.learned_category_id = Some(999_999);
        let mut missing_category = draft.clone();
        assert!(apply_learning_rule_match(
            runtime.connection(),
            user_id,
            &mut missing_category,
            &[missing_category_rule],
            &categories,
            &[],
            &[],
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
        assert!(apply_learning_rule_match(
            runtime.connection(),
            user_id,
            &mut incompatible,
            &[incompatible_rule],
            &categories,
            &[],
            &[],
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
