    #[tokio::test(flavor = "multi_thread")]
    async fn selection_patch_rejects_a_stale_selection_hash_without_writing() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("selection-patch-stale-hash")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let first_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-16 10:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_001,
                preview_counterparty: "selection A".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert first preview");
        let second_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-16 10:01:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_002,
                preview_counterparty: "selection B".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert second preview");
        bill_analyser_db::update_preview_selection(
            pool,
            &[first_preview_id],
            true,
            canonical_user,
        )
        .expect("establish selection baseline");
        let stale_hash = bill_analyser_db::preview_id_snapshot_hash(&[first_preview_id]);

        bill_analyser_db::update_preview_selection(
            pool,
            &[second_preview_id],
            true,
            canonical_user,
        )
        .expect("concurrent selection writer");

        let (status, body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "expected_selection_hash": stale_hash,
                    "selectedIds": [],
                    "deselectedIds": [first_preview_id]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_SELECTION_CONFLICT");
        assert_eq!(body["data"]["expected_selection_hash"], stale_hash);
        assert_eq!(
            body["data"]["actual_selection_hash"],
            body["data"]["metadata"]["selection_hash"]
        );
        assert_eq!(body["data"]["metadata"]["counts"]["selected"], 2);

        let first = bill_analyser_db::get_preview_bill_by_id(
            pool,
            first_preview_id,
            canonical_user,
        )
        .expect("load first preview")
        .expect("first preview exists");
        let second = bill_analyser_db::get_preview_bill_by_id(
            pool,
            second_preview_id,
            canonical_user,
        )
        .expect("load second preview")
        .expect("second preview exists");
        assert!(first.preview_selected);
        assert!(second.preview_selected);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn selection_patch_applies_both_sets_atomically_and_rejects_overlap() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("selection-patch-atomic")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let selected_target = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-16 11:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_101,
                preview_counterparty: "select target".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert selected target");
        let deselected_target = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-16 11:01:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_102,
                preview_counterparty: "deselect target".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert deselected target");
        bill_analyser_db::update_preview_selection(
            pool,
            &[deselected_target],
            true,
            canonical_user,
        )
        .expect("establish selected target");
        let selection_hash = bill_analyser_db::preview_id_snapshot_hash(&[deselected_target]);
        let trigger_name = format!("fail_selection_patch_{deselected_target}");
        let function_name = format!("fail_selection_patch_fn_{deselected_target}");
        sqlx::query(&format!(
            "CREATE FUNCTION {function_name}() RETURNS trigger AS $$ BEGIN IF NEW.id = {deselected_target} AND NOT NEW.selected THEN RAISE EXCEPTION 'forced patch failure'; END IF; RETURN NEW; END; $$ LANGUAGE plpgsql"
        ))
        .execute(pool)
        .await
        .expect("create failure function");
        sqlx::query(&format!(
            "CREATE TRIGGER {trigger_name} BEFORE UPDATE OF selected ON import_preview_rows FOR EACH ROW EXECUTE FUNCTION {function_name}()"
        ))
        .execute(pool)
        .await
        .expect("create failure trigger");

        let (status, _) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "expected_selection_hash": selection_hash,
                    "selectedIds": [selected_target],
                    "deselectedIds": [deselected_target]
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        sqlx::query(&format!(
            "DROP TRIGGER {trigger_name} ON import_preview_rows"
        ))
        .execute(pool)
        .await
        .expect("drop failure trigger");
        sqlx::query(&format!("DROP FUNCTION {function_name}()"))
            .execute(pool)
            .await
            .expect("drop failure function");

        let selected_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            selected_target,
            canonical_user,
        )
        .expect("load selected target")
        .expect("selected target exists");
        let deselected_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            deselected_target,
            canonical_user,
        )
        .expect("load deselected target")
        .expect("deselected target exists");
        assert!(!selected_row.preview_selected);
        assert!(deselected_row.preview_selected);

        let current_hash = bill_analyser_db::preview_id_snapshot_hash(&[deselected_target]);
        let (success_status, success_body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "expected_selection_hash": current_hash,
                    "selectedIds": [selected_target],
                    "deselectedIds": [deselected_target]
                })),
            )
            .await,
        )
        .await;
        assert_eq!(success_status, StatusCode::OK, "body: {success_body}");
        assert_eq!(success_body["data"]["updated"], 2);
        assert_eq!(success_body["data"]["metadata"]["counts"]["selected"], 1);
        let selected_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            selected_target,
            canonical_user,
        )
        .expect("reload selected target")
        .expect("selected target exists");
        let deselected_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            deselected_target,
            canonical_user,
        )
        .expect("reload deselected target")
        .expect("deselected target exists");
        assert!(selected_row.preview_selected);
        assert!(!deselected_row.preview_selected);

        let (overlap_status, overlap_body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state),
                Path(session_id),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "selectedIds": [selected_target],
                    "deselectedIds": [selected_target]
                })),
            )
            .await,
        )
        .await;
        assert_eq!(overlap_status, StatusCode::BAD_REQUEST, "body: {overlap_body}");
        assert_eq!(overlap_body["success"], false);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn selection_patch_validates_hash_scope_and_preserves_legacy_callers() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("selection-patch-validation")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-16 12:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_201,
                preview_counterparty: "legacy selection target".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");

        let (invalid_hash_status, invalid_hash_body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "expectedSelectionHash": "not-a-selection-hash",
                    "selectedIds": [preview_id],
                    "deselectedIds": []
                })),
            )
            .await,
        )
        .await;
        assert_eq!(
            invalid_hash_status,
            StatusCode::BAD_REQUEST,
            "body: {invalid_hash_body}"
        );
        assert!(!bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("load preview after invalid hash")
            .expect("preview exists")
            .preview_selected);

        let foreign_session_id = format!("{session_id}-foreign");
        bill_analyser_db::create_import_session(
            pool,
            &ImportSessionDraft {
                session_id: foreign_session_id.clone(),
                user_id: canonical_user,
                file_count: 1,
            },
        )
        .expect("create foreign session");
        let foreign_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &foreign_session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-16 12:01:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_202,
                preview_counterparty: "foreign selection target".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert foreign preview");
        let (foreign_status, foreign_body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "selectedIds": [foreign_preview_id],
                    "deselectedIds": []
                })),
            )
            .await,
        )
        .await;
        assert_eq!(foreign_status, StatusCode::BAD_REQUEST, "body: {foreign_body}");
        assert!(!bill_analyser_db::get_preview_bill_by_id(
            pool,
            foreign_preview_id,
            canonical_user
        )
        .expect("load foreign preview")
        .expect("foreign preview exists")
        .preview_selected);

        let (legacy_status, legacy_body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state),
                Path(session_id),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "patch",
                    "selectedIds": [preview_id],
                    "deselectedIds": []
                })),
            )
            .await,
        )
        .await;
        assert_eq!(legacy_status, StatusCode::OK, "body: {legacy_body}");
        assert_eq!(legacy_body["data"]["updated"], 1);
        assert!(bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("load legacy-updated preview")
            .expect("preview exists")
            .preview_selected);
    }
