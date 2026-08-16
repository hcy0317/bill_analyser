#[cfg(test)]
mod preview_mutation_helper_tests {
    use super::*;

    fn legacy_group(id: i64, preview_id: i64) -> bill_analyser_db::ImportDecisionGroupRow {
        bill_analyser_db::ImportDecisionGroupRow {
            id, session_id: "session".into(), user_id: 1, group_type: "same_batch_transfer".into(),
            group_key: format!("group-{id}"), decision_status: "pending".into(),
            base_preview_row_id: Some(preview_id), signal_payload: json!({}), version: 1,
            members: vec![bill_analyser_db::ImportDecisionGroupMemberRow {
                id, group_id: id, preview_row_id: Some(preview_id), standard_row_id: None,
                history_bill_id: None, member_role: "outgoing".into(), parser_name: String::new(),
                metadata: json!({}), version: 1, created_at: String::new(),
            }], created_at: String::new(), updated_at: String::new(),
        }
    }

    #[test]
    fn legacy_transfer_group_resolution_requires_exactly_one_group() {
        assert_eq!(unique_legacy_transfer_group(&[], 7).unwrap_err(), "Decision group materialization is pending");
        let one = vec![legacy_group(1, 7)];
        assert_eq!(unique_legacy_transfer_group(&one, 7).unwrap().id, 1);
        let many = vec![legacy_group(1, 7), legacy_group(2, 7)];
        assert_eq!(unique_legacy_transfer_group(&many, 7).unwrap_err(), "Preview belongs to multiple decision groups");
    }

    fn change_value(
        patch: &ImportPreviewPatch,
        target: ImportPreviewPatchField,
    ) -> Option<&ImportPreviewPatchValue> {
        patch
            .changes
            .iter()
            .rev()
            .find_map(|(field, value)| (*field == target).then_some(value))
    }

    #[test]
    fn category_id_patch_clear_removes_identity_and_category_names() {
        let mut patch = ImportPreviewPatch::new(7)
            .with_change(
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(99),
            )
            .with_change(
                ImportPreviewPatchField::MainCategory,
                ImportPreviewPatchValue::Text("旧主类".to_string()),
            )
            .with_change(
                ImportPreviewPatchField::SubCategory,
                ImportPreviewPatchValue::Text("旧子类".to_string()),
            );

        clear_category_id_on_preview_patch(&mut patch);

        assert!(matches!(
            change_value(&patch, ImportPreviewPatchField::CategoryId),
            Some(ImportPreviewPatchValue::Null)
        ));
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MainCategory),
            Some(&ImportPreviewPatchValue::Text(String::new()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::SubCategory),
            Some(&ImportPreviewPatchValue::Text(String::new()))
        );
        assert_eq!(
            patch.changes
                .iter()
                .filter(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
                .count(),
            1
        );
    }

    #[test]
    fn category_id_patch_apply_uses_canonical_category_identity() {
        let mut patch = ImportPreviewPatch::new(7)
            .with_change(
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(1),
            )
            .with_change(
                ImportPreviewPatchField::Type,
                ImportPreviewPatchValue::Text("支出".to_string()),
            );
        let category = PreviewPayloadCategory {
            type_code: Some(2),
            main_category: "理财".to_string(),
            sub_category: "理财收益".to_string(),
        };

        apply_loaded_category_to_preview_patch(&mut patch, 42, category);

        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::CategoryId),
            Some(&ImportPreviewPatchValue::Integer(42))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::Type),
            Some(&ImportPreviewPatchValue::Text("收入".to_string()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MainCategory),
            Some(&ImportPreviewPatchValue::Text("理财".to_string()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::SubCategory),
            Some(&ImportPreviewPatchValue::Text("理财收益".to_string()))
        );
        assert_eq!(
            patch.changes
                .iter()
                .filter(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
                .count(),
            1
        );
    }

    #[test]
    fn preview_payload_category_maps_from_db_lookup() {
        let category = PreviewPayloadCategory::from(ImportPreviewCategoryLookup {
            type_code: Some(5),
            main_category: "投资".to_string(),
            sub_category: "理财收益".to_string(),
        });

        assert_eq!(category.type_code, Some(5));
        assert_eq!(category.main_category, "投资");
        assert_eq!(category.sub_category, "理财收益");
    }

    #[test]
    fn preview_patch_payload_accepts_explicit_cents_aliases() {
        let object = Map::from_iter([
            ("amountCents".to_string(), json!(12345)),
            ("destinationAmountCents".to_string(), json!("54321")),
        ]);

        let patch = build_preview_patch_from_payload(7, &object)
            .expect("explicit cents aliases are valid");

        assert_eq!(patch.preview_id, 7);
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::Amount),
            Some(&ImportPreviewPatchValue::Integer(12345))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::DestinationAmount),
            Some(&ImportPreviewPatchValue::Integer(54321))
        );
    }

    #[test]
    fn preview_patch_payload_rejects_non_integer_cents_values() {
        let object = Map::from_iter([
            ("amountCents".to_string(), json!(true)),
            ("destinationAmountCents".to_string(), json!(12.34)),
            ("recurringCandidateCount".to_string(), json!(2)),
        ]);

        let patch = build_preview_patch_from_payload(7, &object)
            .expect("non-integer cents values preserve the existing ignore contract");

        assert_eq!(change_value(&patch, ImportPreviewPatchField::Amount), None);
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::DestinationAmount),
            None
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::RecurringCandidateCount),
            Some(&ImportPreviewPatchValue::Integer(2))
        );
    }

    #[test]
    fn preview_patch_payload_ignores_client_supplied_matching_feedback() {
        let object = Map::from_iter([
            (
                "matchingFeedback".to_string(),
                json!({
                    "transfer": {
                        "candidate_type": "transfer",
                        "review_status": "pending"
                    }
                }),
            ),
            ("preview_type".to_string(), json!("转账")),
        ]);

        let patch = build_preview_patch_from_payload(7, &object)
            .expect("matching feedback payload is otherwise valid");

        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::Type),
            Some(&ImportPreviewPatchValue::Text("转账".to_string()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MatchingFeedback),
            None
        );
    }

    #[test]
    fn preview_patch_payload_marks_manual_annotation_without_matching_feedback_authority() {
        let object = Map::from_iter([
            ("is_manually_annotated".to_string(), json!(true)),
            ("preview_type".to_string(), json!("转账")),
            (
                "previewMatchingFeedback".to_string(),
                json!({
                    "transfer": {
                        "candidate_type": "transfer",
                        "review_status": "pending"
                    }
                }),
            ),
        ]);

        let patch = build_preview_patch_from_payload(7, &object)
            .expect("manual annotation payload is valid");

        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::ManualAnnotation),
            Some(&ImportPreviewPatchValue::Bool(true))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::Type),
            Some(&ImportPreviewPatchValue::Text("转账".to_string()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MatchingFeedback),
            None
        );
    }

    #[test]
    fn resolved_category_is_appended_after_base_manual_marker() {
        let object = Map::from_iter([
            ("is_manually_annotated".to_string(), json!(true)),
            ("mainCategory".to_string(), json!("餐饮")),
        ]);
        let mut patch = build_preview_patch_from_payload(7, &object)
            .expect("category payload is valid");
        apply_loaded_category_to_preview_patch(
            &mut patch,
            42,
            PreviewPayloadCategory {
                type_code: Some(3),
                main_category: "餐饮".to_string(),
                sub_category: "午餐".to_string(),
            },
        );

        let manual_index = patch
            .changes
            .iter()
            .position(|(field, _)| *field == ImportPreviewPatchField::ManualAnnotation)
            .expect("manual marker");
        let category_index = patch
            .changes
            .iter()
            .position(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
            .expect("resolved category");
        assert!(manual_index < category_index);
    }

    #[test]
    fn preview_patch_payload_ignores_standalone_manual_annotation_marker() {
        let object = Map::from_iter([("is_manually_annotated".to_string(), json!(true))]);

        let patch = build_preview_patch_from_payload(7, &object)
            .expect("standalone manual marker payload is valid");

        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::ManualAnnotation),
            None
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MatchingFeedback),
            None
        );
    }

    #[test]
    fn preview_patch_payload_rejects_i64_min_money_aliases() {
        for key in [
            "amountCents",
            "preview_amount_cents",
            "sourceAmountCents",
            "destinationAmountCents",
            "preview_destination_amount_cents",
        ] {
            let object = Map::from_iter([(key.to_string(), json!(i64::MIN))]);
            let error = build_preview_patch_from_payload(7, &object)
                .expect_err("i64::MIN must be rejected at the HTTP patch boundary");

            assert_eq!(error.status_code, 400, "{key}");
        }
    }

    #[tokio::test]
    async fn category_id_patch_ignores_missing_value_and_clears_invalid_value() {
        let connection =
            PostgresPool::connect_lazy("postgres://localhost/bill_analyser_test").expect("lazy pool");
        let mut patch = ImportPreviewPatch::new(7);
        let object = Map::new();

        apply_category_id_to_preview_patch(
            &connection,
            UserId::new(1).expect("user id"),
            &object,
            &mut patch,
        )
        .expect("missing category id is ignored");
        assert!(patch.changes.is_empty());

        let mut object = Map::new();
        object.insert("categoryId".to_string(), json!(0));
        apply_category_id_to_preview_patch(
            &connection,
            UserId::new(1).expect("user id"),
            &object,
            &mut patch,
        )
        .expect("invalid category id clears category");
        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::CategoryId
                && *value == ImportPreviewPatchValue::Null
        }));
    }

    #[test]
    fn llm_suggestion_parser_normalizes_suggested_type() {
        let suggestion = llm_suggestion_from_value(&json!({
            "suggestedType": "investment",
            "suggested_category_id": 55,
            "suggested_main_category": "投资交易",
            "suggested_sub_category": "基金买入",
            "confidence": 0.82
        }))
        .expect("llm suggestion");

        assert_eq!(suggestion.suggested_type, "投资");
        assert_eq!(suggestion.suggested_category_id, Some(55));
        assert_eq!(suggestion.suggested_main_category, "投资交易");
        assert_eq!(suggestion.suggested_sub_category, "基金买入");
        assert_eq!(suggestion.confidence, 0.82);
    }

    #[test]
    fn expected_row_version_parser_keeps_legacy_absence_and_rejects_invalid_tokens() {
        assert_eq!(
            expected_row_version_from_payload(&Map::new()).expect("legacy payload"),
            None
        );
        assert_eq!(
            expected_row_version_from_payload(&Map::from_iter([(
                "expected_row_version".to_string(),
                json!(7),
            )]))
            .expect("snake case token"),
            Some(7)
        );
        assert_eq!(
            expected_row_version_from_payload(&Map::from_iter([(
                "expectedRowVersion".to_string(),
                json!("8"),
            )]))
            .expect("camel case token"),
            Some(8)
        );
        for value in [json!(0), json!(-1), json!(true), json!(1.5)] {
            let error = expected_row_version_from_payload(&Map::from_iter([(
                "expected_row_version".to_string(),
                value,
            )]))
            .expect_err("invalid row version token");
            assert_eq!(error.status_code, 400);
        }
    }

    #[test]
    fn llm_suggestion_parser_accepts_minimal_identity_output() {
        let suggestion = llm_suggestion_from_value(&json!({
            "preview_id": 9,
            "type": "expense",
            "category_id": 55,
            "source_account_id": 7,
            "destination_account_id": null,
            "confidence": 0.91,
            "reason": "merchant keyword"
        }))
        .expect("minimal llm suggestion");

        assert_eq!(suggestion.suggested_type, "支出");
        assert_eq!(suggestion.suggested_category_id, Some(55));
        assert_eq!(suggestion.resolved_source_account_id, Some(7));
        assert_eq!(suggestion.resolved_destination_account_id, None);
        assert_eq!(suggestion.reason, "merchant keyword");
    }

    #[test]
    fn llm_review_expected_state_parser_keeps_desktop_status_and_category_aliases() {
        let payload = json!({
            "expected_state": {
                "session_id": "session-llm",
                "row_version": 9,
                "review_status": "Pending",
                "category_id": "42"
            }
        });

        let expected = expected_state_from_payload(payload.as_object().expect("payload object"))
            .expect("expected state");

        assert_eq!(expected.session_id.as_deref(), Some("session-llm"));
        assert_eq!(expected.expected_row_version, Some(9));
        assert_eq!(expected.review_status.as_deref(), Some("pending"));
        assert_eq!(expected.preview_category_id, Some(Some(42)));
    }

    #[test]
    fn llm_review_expected_state_parser_rejects_invalid_row_versions() {
        for value in [json!(0), json!(-1), json!(true), json!(1.5)] {
            let payload = json!({"expectedState": {"rowVersion": value}});
            let error = expected_state_from_payload(payload.as_object().expect("payload object"))
                .expect_err("invalid LLM row version");
            assert_eq!(error.status_code, 400);
        }
    }
}
