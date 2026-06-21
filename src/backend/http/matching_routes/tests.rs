mod tests {
    include!("../matching_routes_contract_tests.rs");

    #[test]
    fn preview_action_payload_parses_expected_state_learning_and_recurring_aliases() {
        let payload = json!({
            "responseMode": "preview-item",
            "reviewedType": "transfer",
            "type": "income",
            "mainCategory": "Food",
            "subCategory": "Coffee",
            "sourceAccountId": "10",
            "destinationAccountId": null,
            "ruleId": "8",
            "recurringId": "900",
            "candidateCount": "2",
            "targetCandidate": {
                "id": "900",
                "name": "Monthly Rent",
                "matchScore": "0.91",
                "matchReasons": "same_amount|monthly",
                "matchedDate": "2026-04-01"
            },
            "expectedState": {
                "sessionId": "session-http",
                "previewType": "expense",
                "mainCategory": "Food",
                "subCategory": "Coffee",
                "recurringId": null,
                "sourceAccountId": 10,
                "destinationAccountId": null,
                "matchingFeedback": {"transfer": {"review_status": "pending"}}
            }
        });
        let request = preview_action_request_from_payload(payload.as_object().expect("object"))
            .expect("parsed action request");

        assert!(request.response_mode_preview_item);
        assert_eq!(request.reviewed_type.as_deref(), Some("transfer"));
        assert_eq!(request.recurring_id, Some(900));
        assert_eq!(request.recurring_candidate_count, 2);
        let recurring = request.recurring_candidate.expect("recurring candidate");
        assert_eq!(recurring.id, 900);
        assert_eq!(recurring.name, "Monthly Rent");
        assert_eq!(recurring.match_score, 0.91);
        assert_eq!(recurring.match_reasons, vec!["same_amount", "monthly"]);
        assert_eq!(recurring.matched_occurrence_date, "2026-04-01");

        let expected = request.expected_state.expect("expected state");
        assert_eq!(expected.session_id.as_deref(), Some("session-http"));
        assert_eq!(expected.preview_type.as_deref(), Some("支出"));
        assert_eq!(expected.preview_main_category.as_deref(), Some("Food"));
        assert_eq!(expected.preview_sub_category.as_deref(), Some("Coffee"));
        assert_eq!(expected.preview_recurring_id, Some(None));
        assert_eq!(expected.preview_source_account_id, Some(Some(10)));
        assert_eq!(expected.preview_destination_account_id, Some(None));
        assert_eq!(
            expected.preview_matching_feedback,
            Some(json!({"transfer": {"review_status": "pending"}}))
        );

        let learning = request.learning_apply.expect("learning apply");
        assert_eq!(learning.preview_type.as_deref(), Some("收入"));
        assert_eq!(learning.preview_main_category.as_deref(), Some("Food"));
        assert_eq!(learning.preview_sub_category.as_deref(), Some("Coffee"));
        assert_eq!(learning.preview_source_account_id, Some(Some(10)));
        assert_eq!(learning.preview_destination_account_id, Some(None));
        assert_eq!(learning.rule_id, Some(8));
    }

    #[test]
    fn preview_action_payload_rejects_bad_expected_state_and_handles_empty_sections() {
        let invalid = json!({"expectedState": true});
        let error = preview_action_request_from_payload(invalid.as_object().expect("object"))
            .expect_err("invalid expected state");
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);

        let empty = json!({});
        let request = preview_action_request_from_payload(empty.as_object().expect("object"))
            .expect("empty payload parses");
        assert!(request.expected_state.is_none());
        assert!(request.learning_apply.is_none());
        assert_eq!(request.recurring_candidate_count, 0);
        assert!(request.recurring_candidate.is_none());

        let fallback_recurring = json!({"recurring_id": 12});
        let request =
            preview_action_request_from_payload(fallback_recurring.as_object().expect("object"))
                .expect("fallback recurring parses");
        assert_eq!(request.recurring_id, Some(12));
        assert_eq!(request.recurring_candidate_count, 1);
        assert_eq!(request.recurring_candidate.expect("candidate").id, 12);
    }

    #[test]
    fn matching_route_helper_parsers_cover_filters_and_history_edges() {
        let query = ReconciliationCandidatesQuery {
            candidate_type: Some("Duplicate".to_string()),
            status: Some("Pending".to_string()),
            session_id: Some("session-http".to_string()),
            preview_id: Some("101".to_string()),
            bill_id: Some("401".to_string()),
            limit: Some("900".to_string()),
        };
        let query_value = Value::Object(
            parse_reconciliation_candidates_query(&reconciliation_query_to_map(query))
                .expect("query parses")
                .as_object()
                .expect("query object")
                .clone(),
        );
        let filters = reconciliation_filters_from_value(&query_value);
        assert_eq!(filters.session_id.as_deref(), Some("session-http"));
        assert_eq!(filters.preview_id, Some(101));
        assert_eq!(filters.existing_bill_id, Some(401));
        assert_eq!(filters.candidate_type.as_deref(), Some("duplicate"));
        assert_eq!(filters.status.as_deref(), Some("pending"));
        assert_eq!(filters.limit, 500);

        let empty_filters = reconciliation_filters_from_value(&json!(null));
        assert_eq!(empty_filters.limit, 200);
        assert_eq!(
            parse_reconciliation_candidates_query(&Map::from_iter([(
                "candidateType".to_string(),
                json!("unknown"),
            )]))
            .expect_err("invalid type"),
            "Invalid candidateType"
        );

        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::new()).expect_err("missing bill ids"),
            "billIds is required"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([(
                "billIds".to_string(),
                json!("20"),
            )]))
            .expect_err("not list"),
            "billIds must be a non-empty list"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([
                ("billIds".to_string(), json!([]),)
            ]))
            .expect_err("empty list"),
            "billIds must be a non-empty list"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([(
                "billIds".to_string(),
                json!([20, "20", 0]),
            )]))
            .expect_err("invalid item"),
            "Invalid billIds"
        );
        assert_eq!(
            parse_reconcile_history_bill_ids(&Map::from_iter([(
                "billIds".to_string(),
                json!([20, "20", 21]),
            )]))
            .expect("deduped ids"),
            vec![20, 21]
        );
    }

    #[test]
    fn matching_route_value_helpers_cover_aliases_and_fallbacks() {
        assert!(pair_type_in_allowed_families(
            &json!({"pair_type": "investment"}),
            &["investment"]
        ));
        assert!(pair_type_in_allowed_families(&json!({}), &["transfer"]));
        assert!(!candidate_kind_in_allowed_families(
            &json!({"kind": "learning"}),
            &["transfer"]
        ));
        assert!(candidate_kind_in_allowed_families(
            &json!({"kind": "learning"}),
            &["learning"]
        ));

        assert_eq!(
            matching_history_pair_key(&json!({"id": 9})),
            Some("pair:9".to_string())
        );
        assert_eq!(
            matching_history_pair_key(&json!({"left_bill_id": 30, "rightBillId": 20})),
            Some("bills:20:30".to_string())
        );
        assert!(matching_history_pair_key(&json!({"leftBillId": 30})).is_none());

        assert_eq!(value_to_text(&json!(true)).as_deref(), Some("true"));
        assert_eq!(
            value_to_text(&json!({"a": 1})).as_deref(),
            Some("{\"a\":1}")
        );
        assert_eq!(value_to_i64(&json!("42")), Some(42));
        assert_eq!(value_to_i64(&json!(42_u64)), Some(42));
        assert!(value_to_i64(&json!("")).is_none());
        assert!(value_to_i64(&json!(false)).is_none());
        assert_eq!(value_to_f64(&json!("4.2")), Some(4.2));
        assert!(value_to_f64(&json!("")).is_none());
        assert!(value_to_f64(&json!({})).is_none());

        assert_eq!(normalize_preview_type_text("transfer"), "转账");
        assert_eq!(normalize_preview_type_text("investment"), "投资");
        assert_eq!(normalize_preview_type_text("custom"), "custom");
        assert!(response_mode_is_preview_item(
            json!({"response_mode": "preview-item"})
                .as_object()
                .expect("object")
        ));
        assert_eq!(
            match_reasons_from_value(&json!(["same", true, ""])),
            vec!["same", "true"]
        );
        assert_eq!(
            match_reasons_from_value(&json!("same|monthly,amount")),
            vec!["same", "monthly", "amount"]
        );

        assert_eq!(
            optional_id_field_from_object(
                json!({"sourceAccountId": "10"})
                    .as_object()
                    .expect("object"),
                &["sourceAccountId"]
            ),
            Some(Some(10))
        );
        assert_eq!(
            optional_id_field_from_object(
                json!({"sourceAccountId": null})
                    .as_object()
                    .expect("object"),
                &["sourceAccountId"]
            ),
            Some(None)
        );
        assert_eq!(
            optional_id_field_from_object(
                json!({"sourceAccountId": false})
                    .as_object()
                    .expect("object"),
                &["sourceAccountId"]
            ),
            Some(None)
        );

        assert_eq!(
            matching_error_response(MatchingRuntimeError::NotFound("missing".to_string())).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(status_or_internal(1000), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
