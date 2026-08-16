    #[test]
    fn reclassify_existing_bad_preview_row_emits_cleanup_patch() {
        let categories = vec![category(4, 4, "资金往来", "取款存款")];
        let row = ImportPreviewRow {
            id: 7,
            version: 1,
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
            version: 1,
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
