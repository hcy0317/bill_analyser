    #[tokio::test(flavor = "multi_thread")]
    async fn conditional_selection_rejects_a_stale_selection_hash_before_preview_patches() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("conditional-selection-stale-hash")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'组合 CAS 账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'组合 CAS 分类','3','支出/组合 CAS 分类',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert category");
        let patch_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_100,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "组合 CAS 草稿".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert patch preview");
        let concurrent_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:01:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_200,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "并发选择".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert concurrent preview");
        let patch_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            patch_preview_id,
            canonical_user,
        )
        .expect("load patch preview")
        .expect("patch preview exists");
        let patch_row_version = patch_row.version;
        let stale_selection_hash = bill_analyser_db::preview_id_snapshot_hash(&[]);

        bill_analyser_db::update_preview_selection(
            pool,
            &[concurrent_preview_id],
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
                    "selectionAction": "select_none",
                    "expected_selection_hash": stale_selection_hash,
                    "preview_updates": [{
                        "id": patch_preview_id,
                        "expected_row_version": patch_row_version,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 3_100,
                        "preview_destination_amount_cents": 0,
                        "is_manually_annotated": true,
                        "selected": false
                    }]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_SELECTION_CONFLICT");
        assert_eq!(
            body["data"]["actual_selection_hash"],
            body["data"]["metadata"]["selection_hash"]
        );
        assert_eq!(body["data"]["previewItems"][0]["id"], patch_preview_id);

        let patch_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            patch_preview_id,
            canonical_user,
        )
        .expect("reload patch preview")
        .expect("patch preview exists");
        let concurrent_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            concurrent_preview_id,
            canonical_user,
        )
        .expect("reload concurrent preview")
        .expect("concurrent preview exists");
        assert_eq!(patch_row.category_id, None);
        assert_eq!(patch_row.version, patch_row_version);
        assert!(concurrent_row.preview_selected);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conditional_selection_rejects_a_stale_row_version_without_selection_writes() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("conditional-selection-stale-row")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'行 CAS 账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'行 CAS 分类','3','支出/行 CAS 分类',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert category");
        let patch_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:10:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_300,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "过期行草稿".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert patch preview");
        let untouched_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:11:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_400,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "不得被选择".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert untouched preview");
        let patch_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            patch_preview_id,
            canonical_user,
        )
        .expect("load patch preview")
        .expect("patch preview exists");
        let selection_hash = bill_analyser_db::preview_id_snapshot_hash(&[]);

        let (status, body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "select_all",
                    "expected_selection_hash": selection_hash,
                    "preview_updates": [{
                        "id": patch_preview_id,
                        "expected_row_version": patch_row.version + 1,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 3_300,
                        "preview_destination_amount_cents": 0,
                        "is_manually_annotated": true,
                        "selected": false
                    }]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["previewItem"]["id"], patch_preview_id);
        assert_eq!(
            body["data"]["previewItem"]["row_version"],
            patch_row.version
        );

        let patch_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            patch_preview_id,
            canonical_user,
        )
        .expect("reload patch preview")
        .expect("patch preview exists");
        let untouched_row = bill_analyser_db::get_preview_bill_by_id(
            pool,
            untouched_preview_id,
            canonical_user,
        )
        .expect("reload untouched preview")
        .expect("untouched preview exists");
        assert_eq!(patch_row.category_id, None);
        assert_eq!(
            Some(patch_row.version),
            body["data"]["actual_row_version"].as_i64()
        );
        assert!(!patch_row.preview_selected);
        assert!(!untouched_row.preview_selected);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn selection_only_writers_do_not_advance_preview_row_versions() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("selection-only-row-version")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let first_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:20:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_500,
                preview_counterparty: "选择 token A".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert first preview");
        let second_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:21:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_600,
                preview_counterparty: "选择 token B".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert second preview");
        let baseline = bill_analyser_db::get_preview_by_ids(
            pool,
            &session_id,
            &[first_preview_id, second_preview_id],
            canonical_user,
        )
        .expect("load baseline rows");
        let versions = baseline
            .iter()
            .map(|row| (row.id, row.version))
            .collect::<BTreeMap<_, _>>();

        bill_analyser_db::update_preview_selection(
            pool,
            &[first_preview_id],
            true,
            canonical_user,
        )
        .expect("select by ids");
        let current_hash = bill_analyser_db::preview_id_snapshot_hash(&[first_preview_id]);
        bill_analyser_db::patch_preview_selection(
            pool,
            &session_id,
            &[second_preview_id],
            &[first_preview_id],
            Some(&current_hash),
            canonical_user,
        )
        .expect("patch selection");
        bill_analyser_db::update_session_preview_selection_by_query(
            pool,
            &session_id,
            canonical_user,
            bill_analyser_db::ImportPreviewSelectionMode::Select,
            bill_analyser_db::ImportPreviewSelectionTarget::All,
            &bill_analyser_db::ImportPreviewPageRequest::default(),
        )
        .expect("select by query");
        bill_analyser_db::reset_session_preview_selection(pool, &session_id, canonical_user)
            .expect("reset selection");

        let rows = bill_analyser_db::get_preview_by_ids(
            pool,
            &session_id,
            &[first_preview_id, second_preview_id],
            canonical_user,
        )
        .expect("reload rows");
        assert_eq!(
            rows.iter()
                .map(|row| (row.id, row.version))
                .collect::<BTreeMap<_, _>>(),
            versions
        );
        assert!(rows.iter().all(|row| !row.preview_selected));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conditional_selection_returns_authoritative_rows_after_a_combined_success() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("conditional-selection-success")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'组合成功账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'组合成功分类','3','支出/组合成功分类',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert category");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:30:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 3_700,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "组合成功草稿".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        let baseline = bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("load baseline preview")
            .expect("baseline preview exists");
        let selection_hash = bill_analyser_db::preview_id_snapshot_hash(&[]);

        let (status, body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "select_valid",
                    "expected_selection_hash": selection_hash,
                    "filters": {},
                    "preview_updates": [{
                        "id": preview_id,
                        "expected_row_version": baseline.version,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 3_700,
                        "preview_destination_amount_cents": 0,
                        "is_manually_annotated": true,
                        "selected": false
                    }]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["applied_preview_updates"], 1);
        assert_eq!(body["data"]["previewItems"][0]["id"], preview_id);
        assert_eq!(
            body["data"]["previewItems"][0]["row_version"],
            baseline.version + 1
        );
        assert_eq!(
            body["data"]["previewItems"][0]["category_id"],
            category_id
        );
        assert_eq!(
            body["data"]["previewItems"][0]["preview_selected"],
            true
        );

        let current = bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("reload preview")
            .expect("preview exists");
        assert_eq!(current.version, baseline.version + 1);
        assert_eq!(current.category_id, Some(category_id));
        assert!(current.preview_selected);
    }
