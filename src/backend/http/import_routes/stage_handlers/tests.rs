#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_rule_regex_flag_comes_from_rule_expression_json() {
        assert!(rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regex_enabled": true
        })));
        assert!(rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regexEnabled": "1"
        })));
        assert!(!rule_expression_regex_enabled(&json!({
            "expression": "商户"
        })));
        assert!(rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regex_enabled": 1
        })));
        assert!(!rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regex_enabled": null
        })));
    }

    #[test]
    fn import_db_error_response_keeps_public_message_generic() {
        let response = db_error_response("column category_rules.regex_enabled does not exist");

        assert_eq!(response.status_code, 500);
        assert_eq!(response.body["success"], false);
        assert_eq!(
            response.body["error"],
            "Rust import route runtime DB error"
        );
    }

    #[test]
    fn preview_index_lookup_maps_keep_valid_canonical_identities() {
        let categories_by_id = preview_index_category_lookup_by_id(vec![ImportIntelligenceCategory {
            id: 42,
            type_code: 3,
            main_category: "食品饮料".to_string(),
            sub_category: "外卖".to_string(),
        }]);
        let accounts_by_id = preview_index_account_lookup_by_id(vec![ImportIntelligenceAccount {
            id: 77,
            name: "支付宝".to_string(),
        }]);
        let preview = json!({
            "id": 9,
            "preview_date": "2026-06-01T00:00:00Z",
            "preview_type": "支出",
            "preview_amount_cents": -2800,
            "category_id": 42,
            "preview_main_category": "/",
            "preview_sub_category": "民生银行储蓄卡(6332)",
            "preview_source_account_id": 77,
            "preview_destination_account_id": null,
            "preview_payment_method": "民生银行储蓄卡(6332)"
        });
        let item = build_import_preview_filter_index_item(
            preview.as_object().expect("preview object"),
            &categories_by_id,
            &accounts_by_id,
        );

        assert_eq!(item.category_id, "42");
        assert_eq!(item.actual_category_name, "外卖");
        assert_eq!(item.source_account_id, "77");
        assert_eq!(item.actual_source_account_name, "支付宝");
    }

    #[test]
    fn preview_category_type_validation_prefers_canonical_category_id() {
        let categories = vec![
            ImportIntelligenceCategory {
                id: 42,
                type_code: 2,
                main_category: "理财".to_string(),
                sub_category: "理财收益".to_string(),
            },
            ImportIntelligenceCategory {
                id: 99,
                type_code: 3,
                main_category: "理财".to_string(),
                sub_category: "理财收益".to_string(),
            },
        ];
        let draft = ImportPreviewDraft {
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            ..ImportPreviewDraft::default()
        };

        assert!(preview_category_matches_type(&draft, &categories, 2));
        assert!(!preview_category_matches_type(&draft, &categories, 3));
    }

    #[test]
    fn import_preview_patch_from_draft_carries_canonical_category_id() {
        let draft = ImportPreviewDraft {
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            ..ImportPreviewDraft::default()
        };

        let patch = import_preview_patch_from_draft(7, &draft);

        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::CategoryId
                && *value == ImportPreviewPatchValue::Integer(42)
        }));
    }

    #[test]
    fn automatic_transfer_authority_requires_transfer_matching_feedback() {
        let raw_dedup_only = ImportPreviewDraft {
            dedup_type: Some("transfer_cross_batch".to_string()),
            preview_matching_feedback: json!({}),
            ..ImportPreviewDraft::default()
        };
        assert!(!is_transfer_protected_preview(&raw_dedup_only));

        let same_batch_transfer_match = ImportPreviewDraft {
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer",
                    "review_status": "pending"
                }
            }),
            ..ImportPreviewDraft::default()
        };
        assert!(is_transfer_protected_preview(&same_batch_transfer_match));

        let history_transfer_match = ImportPreviewDraft {
            dedup_type: Some("transfer_cross_batch".to_string()),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer_cross_batch",
                    "review_status": "pending"
                },
                "reconciliation": {
                    "candidate_type": "transfer"
                }
            }),
            ..ImportPreviewDraft::default()
        };
        assert!(is_transfer_protected_preview(&history_transfer_match));

        let rejected_transfer_match = ImportPreviewDraft {
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer",
                    "review_status": "rejected"
                }
            }),
            ..ImportPreviewDraft::default()
        };
        assert!(!is_transfer_protected_preview(&rejected_transfer_match));
    }

    #[test]
    fn learning_projection_cannot_create_transfer_without_authority() {
        assert_eq!(
            learning_projection_type_for_transfer_authority(Some("转账".to_string()), false),
            None
        );
        assert_eq!(
            learning_projection_type_for_transfer_authority(Some("支出".to_string()), false),
            Some("支出".to_string())
        );
        assert_eq!(
            learning_projection_type_for_transfer_authority(Some("转账".to_string()), true),
            Some("转账".to_string())
        );
        assert_eq!(
            learning_projection_type_for_transfer_authority(Some("收入".to_string()), true),
            None
        );
    }

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
    ) -> ImportIntelligenceRule {
        ImportIntelligenceRule {
            id,
            category_id,
            category_type,
            main_category: main.to_string(),
            sub_category: sub.to_string(),
            priority: 1,
            rule_expression: expression.to_string(),
            regex_enabled: false,
            compiled_expression: compile_rule_expression(expression, false),
        }
    }

    #[test]
    fn non_transfer_category_rules_can_reclassify_expense_to_investment() {
        let rule_set = ImportIntelligenceRuleSet::from_rules(vec![category_rule(
            10,
            55,
            5,
            "投资交易",
            "基金买入",
            "OR={定投扣款}",
        )]);
        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_description: "基金定投扣款".to_string(),
            preview_payment_method: "招商卡".to_string(),
            ..ImportPreviewDraft::default()
        };
        let preview_rule_text = import_preview_rule_text(&draft);

        assert!(apply_non_transfer_category_rule_match(
            &mut draft,
            &rule_set,
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

    #[test]
    fn reclassify_existing_bad_preview_row_emits_cleanup_patch() {
        let categories = vec![category(4, 4, "资金往来", "取款存款")];
        let row = ImportPreviewRow {
            id: 7,
            session_id: "session".to_string(),
            user_id: 1,
            preview_date: "2026-01-01 09:00:00".to_string(),
            preview_type: "转账".to_string(),
            preview_amount_cents: 12_300,
            preview_destination_amount_cents: 12_300,
            category_id: Some(4),
            preview_main_category: "资金往来".to_string(),
            preview_sub_category: "取款存款".to_string(),
            preview_source_account_id: Some(11),
            preview_destination_account_id: Some(22),
            preview_counterparty: "支付宝".to_string(),
            preview_payment_method: "支付宝".to_string(),
            preview_description: "parser-only transfer-like row".to_string(),
            preview_parser_id: "alipay".to_string(),
            preview_parser_tags: vec!["parser:alipay".to_string()],
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: true,
            dedup_type: "remaining".to_string(),
            dedup_source_ids: vec![99],
            preview_matching_feedback: json!({
                "parser": {"parser_id": "alipay"},
                "category_rule": {"category_id": 4, "review_status": "auto_applied"}
            }),
            created_at: "2026-01-01 09:00:00".to_string(),
        };
        let mut draft = import_preview_draft_from_row(&row);

        assert!(demote_unauthorized_transfer_preview(&mut draft, &categories));
        let patch = import_preview_patch_from_draft(row.id, &draft);

        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::Type
                && *value == ImportPreviewPatchValue::Text("支出".to_string())
        }));
        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::CategoryId && *value == ImportPreviewPatchValue::Null
        }));
        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::DestinationAccountId
                && *value == ImportPreviewPatchValue::Null
        }));
        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::DestinationAmount
                && *value == ImportPreviewPatchValue::Integer(0)
        }));
        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::MatchingFeedback
                && matches!(value, ImportPreviewPatchValue::Json(feedback) if feedback.get("category_rule").is_none())
        }));
    }

    #[test]
    fn preview_snapshots_and_row_draft_preserve_canonical_category_id() {
        let draft = ImportPreviewDraft {
            preview_type: "收入".to_string(),
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            preview_source_account_id: Some(7),
            preview_destination_account_id: Some(8),
            ..ImportPreviewDraft::default()
        };

        let stage2 = import_preview_stage2_snapshot(&draft);
        let transfer = import_preview_transfer_applied_snapshot(&draft);

        assert_eq!(stage2["category_id"], json!(42));
        assert_eq!(transfer["category_id"], json!(42));

        let row = ImportPreviewRow {
            id: 7,
            session_id: "session".to_string(),
            user_id: 1,
            preview_date: "2026-01-01 09:00:00".to_string(),
            preview_type: "收入".to_string(),
            preview_amount_cents: 123,
            preview_destination_amount_cents: 0,
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            preview_source_account_id: Some(7),
            preview_destination_account_id: Some(8),
            preview_counterparty: "基金平台".to_string(),
            preview_payment_method: "招商卡".to_string(),
            preview_description: "收益".to_string(),
            preview_parser_id: "fixture".to_string(),
            preview_parser_tags: vec!["tag".to_string()],
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: true,
            dedup_type: String::new(),
            dedup_source_ids: vec![1],
            preview_matching_feedback: json!({}),
            created_at: "2026-01-01 09:00:00".to_string(),
        };

        let row_draft = import_preview_draft_from_row(&row);

        assert_eq!(row_draft.category_id, Some(42));
        assert_eq!(row_draft.preview_main_category, "理财");
        assert_eq!(row_draft.preview_sub_category, "理财收益");
    }
}
