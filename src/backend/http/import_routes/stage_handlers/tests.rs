#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::HeaderValue;

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
    fn reclassify_invalidates_semantic_recommendations_and_history_plan_only() {
        let mut draft = ImportPreviewDraft {
            preview_matching_feedback: json!({
                "parser": {"parser_id": "alipay"},
                "transfer": {"candidate_type": "transfer", "review_status": "pending"},
                "recurring": {"candidate_id": 7},
                "learning": {"review_status": "accepted", "score": 0.98},
                "llm": {"review_status": "rejected", "confidence": 0.92},
                "history": {"candidate_id": "history:8"},
                "reconciliation": {
                    "planned_operation": "update_current_bill",
                    "history_bill_id": 8,
                    "destructive_ack_required": true
                }
            }),
            ..ImportPreviewDraft::default()
        };

        invalidate_reclassification_dependent_signals(&mut draft);

        let feedback = draft
            .preview_matching_feedback
            .as_object()
            .expect("matching feedback object");
        assert!(feedback.get("learning").is_none());
        assert!(feedback.get("llm").is_none());
        assert!(feedback.get("history").is_none());
        assert!(feedback.get("reconciliation").is_none());
        assert!(feedback.get("parser").is_some());
        assert!(feedback.get("transfer").is_some());
        assert!(feedback.get("recurring").is_none());
        assert_eq!(draft.preview_recurring_id, None);
        assert!(draft.preview_recurring_name.is_empty());
        assert_eq!(draft.preview_recurring_candidate_count, 0);
        assert_eq!(draft.preview_recurring_match_score, 0.0);
        assert!(draft.preview_recurring_match_reasons.is_empty());
        assert!(draft.preview_recurring_matched_date.is_empty());
    }

    #[test]
    fn reclassify_preserves_non_history_reconciliation_evidence() {
        let mut draft = ImportPreviewDraft {
            preview_matching_feedback: json!({
                "reconciliation": {
                    "candidate_type": "transfer",
                    "review_status": "pending"
                }
            }),
            ..ImportPreviewDraft::default()
        };

        invalidate_reclassification_dependent_signals(&mut draft);

        assert!(draft
            .preview_matching_feedback
            .get("reconciliation")
            .is_some());
    }

    #[test]
    fn reclassify_removes_recurring_projection_when_candidate_disappears() {
        let mut draft = ImportPreviewDraft {
            preview_recurring_id: Some(7),
            preview_recurring_name: "Old monthly candidate".to_string(),
            preview_recurring_candidate_count: 1,
            preview_recurring_match_score: 0.95,
            preview_recurring_match_reasons: "amount|schedule".to_string(),
            preview_recurring_matched_date: "2026-07-01".to_string(),
            preview_matching_feedback: json!({
                "parser": {"parser_id": "fixture"},
                "transfer": {"candidate_type": "transfer", "review_status": "pending"},
                "recurring": {"id": 7, "review_status": "pending"}
            }),
            ..ImportPreviewDraft::default()
        };

        invalidate_reclassification_dependent_signals(&mut draft);

        assert!(best_recurring_candidate_for_draft(&draft, &[]).is_none());
        assert_eq!(draft.preview_recurring_id, None);
        assert_eq!(draft.preview_recurring_candidate_count, 0);
        assert!(draft.preview_matching_feedback.get("recurring").is_none());
        assert!(draft.preview_matching_feedback.get("parser").is_some());
        assert!(draft.preview_matching_feedback.get("transfer").is_some());
    }

    #[test]
    fn reclassify_recreates_only_newly_valid_recurring_candidate() {
        let mut draft = ImportPreviewDraft {
            preview_date: "2026-07-10 09:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 8_800,
            preview_recurring_id: Some(7),
            preview_recurring_name: "Old candidate".to_string(),
            preview_matching_feedback: json!({
                "parser": {"parser_id": "fixture"},
                "recurring": {"id": 7, "review_status": "pending"}
            }),
            ..ImportPreviewDraft::default()
        };
        let templates = vec![ImportIntelligenceRecurringTemplate {
            id: 19,
            name: "New candidate".to_string(),
            bill_type: "支出".to_string(),
            amount_cents: 8_800,
            account: String::new(),
            counterparty: String::new(),
            next_date: "2026-07-10".to_string(),
            start_date: "2026-06-10".to_string(),
        }];

        invalidate_reclassification_dependent_signals(&mut draft);
        let candidate = best_recurring_candidate_for_draft(&draft, &templates)
            .expect("new candidate remains valid");
        apply_recurring_candidate(&mut draft, candidate);

        assert_eq!(draft.preview_recurring_id, Some(19));
        assert_eq!(draft.preview_recurring_name, "New candidate");
        assert_eq!(
            draft.preview_matching_feedback.pointer("/recurring/id"),
            Some(&json!(19))
        );
        assert!(draft.preview_matching_feedback.get("parser").is_some());
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

    fn transfer_group_fixture(
        id: i64,
        preview_id: i64,
    ) -> bill_analyser_db::ImportDecisionGroupRow {
        bill_analyser_db::ImportDecisionGroupRow {
            id,
            session_id: "session".into(),
            user_id: 1,
            group_type: "same_batch_transfer".into(),
            group_key: format!("group-{id}"),
            decision_status: "pending".into(),
            base_preview_row_id: Some(preview_id),
            signal_payload: json!({}),
            version: 1,
            members: vec![bill_analyser_db::ImportDecisionGroupMemberRow {
                id: id * 10,
                group_id: id,
                preview_row_id: Some(preview_id),
                standard_row_id: None,
                history_bill_id: None,
                member_role: "outgoing".into(),
                parser_name: "test".into(),
                metadata: json!({}),
                version: 1,
                created_at: "2026-01-01T00:00:00Z".into(),
            }],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn legacy_transfer_group_resolution_requires_exactly_one_matching_group() {
        assert_eq!(
            unique_legacy_transfer_group(&[], 7).unwrap_err(),
            "Decision group materialization is pending"
        );

        let unrelated = bill_analyser_db::ImportDecisionGroupRow {
            group_type: "history_duplicate".into(),
            ..transfer_group_fixture(1, 7)
        };
        assert_eq!(
            unique_legacy_transfer_group(&[unrelated], 7).unwrap_err(),
            "Decision group materialization is pending"
        );

        let one = transfer_group_fixture(2, 7);
        assert_eq!(unique_legacy_transfer_group(std::slice::from_ref(&one), 7).unwrap().id, 2);

        let two = transfer_group_fixture(3, 7);
        assert_eq!(
            unique_legacy_transfer_group(&[one, two], 7).unwrap_err(),
            "Preview belongs to multiple decision groups"
        );
    }

    #[test]
    fn decision_group_command_parser_validates_complete_cas_token() {
        let error = decision_group_command_from_object(
            json!({}).as_object().unwrap(), "s".into(), 1,
        ).unwrap_err();
        assert_eq!(error.status_code, 400);

        for payload in [
            json!({"operationId":"op"}),
            json!({"operationId":"op","expectedGroupVersion":1}),
            json!({"operationId":"op","expectedGroupVersion":1,"expectedPreviewVersions":[null]}),
            json!({"operationId":"op","expectedGroupVersion":1,"expectedPreviewVersions":[{}]}),
            json!({"operationId":"op","expectedGroupVersion":1,"expectedPreviewVersions":[{"previewRowId":0,"version":1}]}),
            json!({"operationId":"op","expectedGroupVersion":1,"expectedPreviewVersions":[{"previewRowId":2,"version":1},{"previewRowId":2,"version":1}]}),
        ] {
            assert_eq!(decision_group_command_from_object(payload.as_object().unwrap(), "s".into(), 1).unwrap_err().status_code, 400);
        }

        let payload = json!({
            "decision":"accept",
            "operation_id":"op-1",
            "expected_group_version":3,
            "expected_preview_versions":[
                {"preview_row_id":11,"version":4},
                {"previewRowId":12,"version":5}
            ]
        });
        let (command, ids) = decision_group_command_from_object(payload.as_object().unwrap(), "session".into(), 9).unwrap();
        assert_eq!(command.operation_id, "op-1");
        assert_eq!(command.session_id, "session");
        assert_eq!(command.group_id, 9);
        assert_eq!(command.expected_preview_versions.len(), 2);
        assert_eq!(ids, std::collections::HashSet::from([11, 12]));
    }

    async fn import_test_response(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        (status, serde_json::from_slice(&body).expect("JSON response"))
    }

    fn import_test_headers(user_id: i64) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            TRUSTED_USER_SECRET_HEADER,
            HeaderValue::from_static("import-test-secret"),
        );
        headers.insert(
            "x-user-id",
            HeaderValue::from_str(&user_id.to_string()).expect("user header"),
        );
        headers
    }

    async fn import_postgres_test_state() -> Option<(HttpAppState, i64, String)> {
        let postgres_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL").unwrap_or_else(|_| {
            "postgres://bill_analyser:bill_analyser_dev@127.0.0.1:55432/bill_analyser".into()
        });
        let state = HttpAppState::new(
            HttpShellConfig::default()
                .with_postgres_url(postgres_url)
                .expect("test postgres URL")
                .with_trusted_user_header_secret("import-test-secret"),
        )
        .expect("test state");
        let runtime = state
            .open_postgres_repository_runtime("import-test")
            .expect("postgres runtime");
        bill_analyser_db::init_import_staging_schema(runtime.pool()).expect("import schema");
        sqlx::query(
            "ALTER TABLE import_confirm_operations ADD COLUMN IF NOT EXISTS operation_id TEXT",
        )
        .execute(runtime.pool())
        .await
        .expect("decision operation migration");
        let nonce = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos(),
            IMPORT_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let username = format!("http-import-{nonce}");
        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
        )
        .bind(&username)
        .bind(format!("{username}@example.test"))
        .fetch_one(runtime.pool())
        .await
        .expect("insert test user");
        let session_id = format!("http-import-session-{nonce}");
        bill_analyser_db::create_import_session(
            runtime.pool(),
            &ImportSessionDraft {
                session_id: session_id.clone(),
                user_id: UserId::new(user_id as u64).expect("positive user id"),
                file_count: 1,
            },
        )
        .expect("create import session");
        Some((state, user_id, session_id))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn preview_mutation_handlers_cover_auth_payload_lookup_and_empty_reclassify() {
        let unauthenticated = import_preview_update_runtime_handler(
            State(HttpAppState::new(HttpShellConfig::default()).expect("default state")),
            Path("missing".into()),
            HeaderMap::new(),
            Json(json!({})),
        )
        .await;
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let headers = import_test_headers(user_id);

        for (payload, expected_status) in [
            (json!([]), StatusCode::BAD_REQUEST),
            (json!({}), StatusCode::BAD_REQUEST),
            (json!({"previewId": "bad"}), StatusCode::BAD_REQUEST),
            (json!({"previewId": 9_999_999_999_i64}), StatusCode::NOT_FOUND),
        ] {
            let (status, body) = import_test_response(
                import_preview_update_runtime_handler(
                    State(state.clone()),
                    Path(session_id.clone()),
                    headers.clone(),
                    Json(payload),
                )
                .await,
            )
            .await;
            assert_eq!(status, expected_status, "body: {body}");
            assert_eq!(body["success"], false);
        }

        let (status, body) = import_test_response(
            import_reclassify_runtime_handler(
                State(state),
                Path(session_id),
                headers,
                Json(json!({"updates": []})),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["updated"], 0);
        assert_eq!(body["data"]["total"], 0);
        assert_eq!(body["data"]["preview"], json!([]));
    }

    async fn insert_reclassification_group(
        pool: &PostgresPool,
        session_id: &str,
        user_id: i64,
        status: &str,
        operation_id: &str,
    ) -> i64 {
        let session_db_id: i64 = sqlx::query_scalar(
            "SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2",
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("session database id");
        let group_id: i64 = sqlx::query_scalar(
            "INSERT INTO import_decision_groups(session_id,user_id,group_type,group_key,signal_payload) VALUES($1,$2,'same_batch_transfer',$3,jsonb_build_object('reclassification',jsonb_build_object('status',$4::text,'lease_expires_at',clock_timestamp()+interval '5 minutes'))) RETURNING id",
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(format!("reclass-{operation_id}"))
        .bind(status)
        .fetch_one(pool)
        .await
        .expect("insert reclassification group");
        sqlx::query("INSERT INTO import_confirm_operations(session_id,user_id,operation_kind,operation_id,status,payload) VALUES($1,$2,'decision_group',$3,'completed',jsonb_build_object('group_id',$4,'result',jsonb_build_object('upserted_preview_items',jsonb_build_array())))")
            .bind(session_db_id)
            .bind(user_id)
            .bind(operation_id)
            .bind(group_id)
            .execute(pool)
            .await
            .expect("insert operation ledger");
        group_id
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dematerialized_reclassification_covers_claim_finish_replay_and_state_errors() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let mut runtime = open_runtime(&state).expect("import runtime");
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            runtime.connection(),
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-07-11 12:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_800,
                preview_destination_amount_cents: 2_800,
                preview_counterparty: "覆盖率商户".into(),
                preview_payment_method: "测试渠道".into(),
                preview_description: "撤销转账后重新分类".into(),
                preview_parser_id: "coverage-parser".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        let operation_id = format!("reclass-success-{preview_id}");
        let group_id = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "pending",
            &operation_id,
        )
        .await;

        for (operation_id, decision) in [("invalid-decision", "unexpected"), ("", "accept")] {
            let result = apply_import_decision_group_command(
                runtime.connection(),
                canonical_user,
                &ImportDecisionGroupCommand {
                    operation_id: operation_id.into(),
                    session_id: session_id.clone(),
                    group_id,
                    decision: decision.into(),
                    expected_group_version: 1,
                    expected_preview_versions: vec![],
                },
            )
            .expect("invalid command returns a conflict result");
            assert_eq!(result, ImportDecisionGroupCommandResult::Conflict);
        }

        let items = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            group_id,
            &operation_id,
            &[preview_id],
        )
        .await
        .expect("claimed reclassification succeeds");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], preview_id);

        let replay = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            group_id,
            &operation_id,
            &[preview_id],
        )
        .await
        .expect("completed operation replays items");
        assert_eq!(replay, items);

        let completed_empty_operation = format!("reclass-empty-{preview_id}");
        let completed_empty_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "completed",
            &completed_empty_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            completed_empty_group,
            &completed_empty_operation,
            &[],
        )
        .await
        .expect_err("completed without items is invalid");
        assert!(error.to_string().contains("completed without preview items"));

        let running_operation = format!("reclass-running-{preview_id}");
        let running_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "running",
            &running_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            running_group,
            &running_operation,
            &[],
        )
        .await
        .expect_err("live running lease remains pending");
        assert!(error.to_string().contains("reclassification pending"));

        let unknown_operation = format!("reclass-unknown-{preview_id}");
        let unknown_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "unknown",
            &unknown_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            unknown_group,
            &unknown_operation,
            &[],
        )
        .await
        .expect_err("unknown state is rejected");
        assert!(error.to_string().contains("state missing"));

        let failed_operation = format!("reclass-failed-{preview_id}");
        let failed_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "failed",
            &failed_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            failed_group,
            &failed_operation,
            &[],
        )
        .await
        .expect_err("retry with no preview rows fails and records failure");
        assert!(error.to_string().contains("has no preview rows"));
        assert_eq!(
            get_import_group_reclassification_state(
                runtime.connection(),
                canonical_user,
                failed_group,
                &failed_operation,
            )
            .expect("failed state persisted")
            .0,
            "failed"
        );
    }
}
