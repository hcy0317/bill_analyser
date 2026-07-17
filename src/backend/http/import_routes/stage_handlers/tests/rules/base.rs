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
