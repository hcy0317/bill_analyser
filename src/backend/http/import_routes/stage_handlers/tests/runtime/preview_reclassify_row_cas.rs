    #[test]
    fn optional_row_version_attaches_only_the_supplied_token() {
        let legacy_patch = attach_optional_expected_row_version(ImportPreviewPatch::new(11), None);
        assert_eq!(legacy_patch.expected_row_version, None);

        let versioned_patch =
            attach_optional_expected_row_version(ImportPreviewPatch::new(12), Some(7));
        assert_eq!(versioned_patch.expected_row_version, Some(7));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stale_reclassify_preview_update_rolls_back_the_entire_patch_batch() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("stale-reclassify-preview-update")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let first_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 08:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_000,
                preview_description: "first original".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert first reclassify preview");
        let second_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 09:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_000,
                preview_description: "second original".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert second reclassify preview");
        let first_before = bill_analyser_db::get_preview_bill_by_id(
            pool,
            first_preview_id,
            canonical_user,
        )
        .expect("load first preview")
        .expect("first preview exists");
        let second_stale = bill_analyser_db::get_preview_bill_by_id(
            pool,
            second_preview_id,
            canonical_user,
        )
        .expect("load second stale preview")
        .expect("second preview exists");
        assert!(bill_analyser_db::update_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewPatch::new(second_preview_id).with_change(
                ImportPreviewPatchField::Description,
                ImportPreviewPatchValue::Text("second concurrent writer".into()),
            ),
        )
        .expect("apply concurrent second-row update"));
        let second_latest = bill_analyser_db::get_preview_bill_by_id(
            pool,
            second_preview_id,
            canonical_user,
        )
        .expect("load second latest preview")
        .expect("second latest preview exists");
        assert_eq!(second_latest.version, second_stale.version + 1);

        let (status, body) = import_test_response(
            import_reclassify_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "preview_updates": [
                        {
                            "id": first_preview_id,
                            "description": "first stale batch writer",
                            "expected_row_version": first_before.version
                        },
                        {
                            "id": second_preview_id,
                            "description": "second stale batch writer",
                            "expected_row_version": second_stale.version
                        }
                    ]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["expected_row_version"], second_stale.version);
        assert_eq!(body["data"]["actual_row_version"], second_latest.version);
        assert_eq!(body["data"]["previewItem"]["id"], second_preview_id);
        assert_eq!(
            body["data"]["previewItem"]["preview_description"],
            "second concurrent writer"
        );

        let first_after = bill_analyser_db::get_preview_bill_by_id(
            pool,
            first_preview_id,
            canonical_user,
        )
        .expect("reload first preview")
        .expect("first preview remains");
        let second_after = bill_analyser_db::get_preview_bill_by_id(
            pool,
            second_preview_id,
            canonical_user,
        )
        .expect("reload second preview")
        .expect("second preview remains");
        assert_eq!(first_after, first_before, "first patch must roll back");
        assert_eq!(second_after, second_latest, "stale patch must not write");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn current_reclassify_preview_version_succeeds_and_returns_new_token() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("current-reclassify-preview-version")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 10:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_000,
                preview_description: "current original".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert current-version reclassify preview");
        let before = bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("load current-version preview")
            .expect("current-version preview exists");

        let (status, body) = import_test_response(
            import_reclassify_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "preview_updates": [{
                        "id": preview_id,
                        "description": "current reclassify writer",
                        "expected_row_version": before.version
                    }]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["updated"], 1);
        assert_eq!(body["data"]["preview"][0]["id"], preview_id);
        assert_eq!(
            body["data"]["preview"][0]["preview_description"],
            "current reclassify writer"
        );
        assert!(
            body["data"]["preview"][0]["row_version"]
                .as_i64()
                .is_some_and(|version| version > before.version),
            "successful reclassify must return a newer row token: {body}"
        );
    }
