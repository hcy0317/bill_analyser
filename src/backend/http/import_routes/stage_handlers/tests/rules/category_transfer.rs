    fn category(id: i64, type_code: i64, main: &str, sub: &str) -> ImportIntelligenceCategory {
        ImportIntelligenceCategory {
            id,
            type_code,
            main_category: main.to_string(),
            sub_category: sub.to_string(),
        }
    }

    fn category_rule(
        id: i64,
        category_id: i64,
        category_type: i64,
        main: &str,
        sub: &str,
        expression: &str,
    ) -> CategoryRuleCandidate {
        CategoryRuleCandidate::compile(CategoryRuleCandidateDraft {
            id,
            category_id,
            category_type: i32::try_from(category_type).expect("category type"),
            main_category: main.to_string(),
            sub_category: sub.to_string(),
            priority: 1,
            rule_expression: expression.to_string(),
            regex_enabled: false,
        })
        .expect("category rule candidate")
    }

    #[test]
    fn non_transfer_category_rules_can_reclassify_expense_to_investment() {
        let rules = vec![category_rule(
            10,
            55,
            5,
            "投资交易",
            "基金买入",
            "OR={定投扣款}",
        )];
        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_description: "基金定投扣款".to_string(),
            preview_payment_method: "招商卡".to_string(),
            ..ImportPreviewDraft::default()
        };
        let preview_rule_text = import_preview_rule_text(&draft);

        assert!(apply_non_transfer_category_rule_match(
            &mut draft,
            &rules,
            &preview_rule_text
        ));

        assert_eq!(draft.preview_type, "投资");
        assert_eq!(draft.category_id, Some(55));
        assert_eq!(draft.preview_main_category, "投资交易");
        assert_eq!(draft.preview_sub_category, "基金买入");
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/category_rule/category_id"),
            Some(&json!(55))
        );
    }

    #[test]
    fn non_transfer_category_rules_apply_main_only_category_target() {
        let rules = vec![category_rule(
            11,
            56,
            3,
            "日常消费",
            "",
            "OR={便利店}",
        )];
        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_description: "便利店购物".to_string(),
            ..ImportPreviewDraft::default()
        };
        let preview_rule_text = import_preview_rule_text(&draft);

        assert!(apply_non_transfer_category_rule_match(
            &mut draft,
            &rules,
            &preview_rule_text
        ));
        assert_eq!(draft.category_id, Some(56));
        assert_eq!(draft.preview_main_category, "日常消费");
        assert!(draft.preview_sub_category.is_empty());
    }

    #[test]
    fn non_transfer_learning_projection_allows_income_expense_investment_mutual_category() {
        assert!(category_type_matches_learning_projection(5, "支出"));
        assert!(category_type_matches_learning_projection(5, "收入"));
        assert!(category_type_matches_learning_projection(2, "投资"));
        assert!(category_type_matches_learning_projection(3, "投资"));
        assert!(!category_type_matches_learning_projection(4, "支出"));
        assert!(!category_type_matches_learning_projection(5, "转账"));
        assert!(!category_type_matches_learning_projection(4, "坏类型"));
        assert!(category_type_matches_learning_projection(5, "坏类型"));
    }

    #[test]
    fn transfer_category_rules_require_structured_transfer_authority() {
        let rules = vec![category_rule(9, 4, 4, "资金往来", "取款存款", "OR={还款}")];
        let mut parser_only = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_description: "自动还款".to_string(),
            preview_matching_feedback: json!({"parser": {"parser_id": "alipay"}}),
            ..ImportPreviewDraft::default()
        };

        assert!(!apply_transfer_category_rule_match(
            &mut parser_only,
            &rules,
            "自动还款"
        ));
        assert_eq!(parser_only.preview_type, "转账");
        assert_eq!(parser_only.category_id, None);
        assert!(parser_only
            .preview_matching_feedback
            .get("category_rule")
            .is_none());

        let mut authorized = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_description: "自动还款".to_string(),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer",
                    "review_status": "pending"
                }
            }),
            ..ImportPreviewDraft::default()
        };

        assert!(apply_transfer_category_rule_match(
            &mut authorized,
            &rules,
            "自动还款"
        ));
        assert_eq!(authorized.preview_type, "转账");
        assert_eq!(authorized.category_id, Some(4));
        assert_eq!(authorized.preview_main_category, "资金往来");
        assert_eq!(authorized.preview_sub_category, "取款存款");
    }

    #[test]
    fn unauthorized_generated_transfer_preview_is_sanitized_before_stage2_matching() {
        let categories = vec![category(4, 4, "资金往来", "取款存款")];
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_amount_cents: 17_930,
            preview_destination_amount_cents: 17_930,
            category_id: Some(4),
            preview_main_category: "资金往来".to_string(),
            preview_sub_category: "取款存款".to_string(),
            preview_destination_account_id: Some(88),
            preview_matching_feedback: json!({
                "parser": {"parser_id": "alipay"},
                "transfer": {
                    "account_resolution": "account_rules",
                    "resolved_source_account_id": 11,
                    "resolved_destination_account_id": 88
                },
                "category_rule": {
                    "category_id": 4,
                    "review_status": "auto_applied"
                },
                "account_rule": {
                    "source": {
                        "account_id": 11,
                        "transaction_type": "transfer",
                        "review_status": "auto_applied"
                    },
                    "destination": {
                        "account_id": 88,
                        "review_status": "auto_applied"
                    }
                },
                "account": {
                    "destination_account_id": 88,
                    "destination_account_name": "支付宝"
                },
                "stage2_baseline": {
                    "preview_type": "转账"
                },
                "identity_validation": {
                    "review_status": "requires_identity_review"
                }
            }),
            ..ImportPreviewDraft::default()
        };

        assert!(demote_unauthorized_transfer_preview(&mut draft, &categories));

        assert_eq!(draft.preview_type, "支出");
        assert_eq!(draft.preview_destination_amount_cents, 0);
        assert_eq!(draft.preview_destination_account_id, None);
        assert_eq!(draft.category_id, None);
        assert!(draft.preview_main_category.is_empty());
        assert!(draft.preview_sub_category.is_empty());
        assert!(draft
            .preview_matching_feedback
            .get("category_rule")
            .is_none());
        assert!(draft.preview_matching_feedback.get("transfer").is_none());
        assert!(draft
            .preview_matching_feedback
            .get("stage2_baseline")
            .is_none());
        assert!(draft
            .preview_matching_feedback
            .get("identity_validation")
            .is_none());
        assert!(draft
            .preview_matching_feedback
            .pointer("/account_rule/destination")
            .is_none());
        assert!(draft
            .preview_matching_feedback
            .pointer("/account_rule/source/transaction_type")
            .is_none());
        assert!(draft
            .preview_matching_feedback
            .pointer("/account/destination_account_id")
            .is_none());
        assert!(draft.preview_matching_feedback.get("parser").is_some());
    }

    #[test]
    fn manual_transfer_preview_is_not_demoted_but_still_has_no_rule_authority() {
        let categories = vec![category(4, 4, "资金往来", "取款存款")];
        let rules = vec![category_rule(9, 4, 4, "资金往来", "取款存款", "OR={还款}")];
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_description: "自动还款".to_string(),
            category_id: Some(4),
            preview_main_category: "资金往来".to_string(),
            preview_sub_category: "取款存款".to_string(),
            preview_matching_feedback: json!({
                "annotation": {"is_manually_annotated": true},
                "transfer": {
                    "account_resolution": "account_rules",
                    "resolved_source_account_id": 11,
                    "resolved_destination_account_id": 88
                },
                "category_rule": {
                    "category_id": 4,
                    "review_status": "auto_applied"
                }
            }),
            ..ImportPreviewDraft::default()
        };

        assert!(demote_unauthorized_transfer_preview(&mut draft, &categories));
        assert_eq!(draft.preview_type, "转账");
        assert_eq!(draft.category_id, None);
        assert!(draft.preview_main_category.is_empty());
        assert!(draft.preview_sub_category.is_empty());
        assert!(draft.preview_matching_feedback.get("transfer").is_none());
        assert!(draft
            .preview_matching_feedback
            .get("category_rule")
            .is_none());
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/annotation/is_manually_annotated"),
            Some(&json!(true))
        );
        assert!(!apply_transfer_category_rule_match(
            &mut draft,
            &rules,
            "自动还款"
        ));
        assert_eq!(draft.category_id, None);
    }
