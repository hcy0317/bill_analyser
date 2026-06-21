#[cfg(test)]
mod preview_mutation_helper_tests {
    use super::*;

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

        let patch = build_preview_patch_from_payload(7, &object);

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

        let patch = build_preview_patch_from_payload(7, &object);

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

        let patch = build_preview_patch_from_payload(7, &object);

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

        let patch = build_preview_patch_from_payload(7, &object);

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
    fn preview_patch_payload_ignores_standalone_manual_annotation_marker() {
        let object = Map::from_iter([("is_manually_annotated".to_string(), json!(true))]);

        let patch = build_preview_patch_from_payload(7, &object);

        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::ManualAnnotation),
            None
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MatchingFeedback),
            None
        );
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
}
