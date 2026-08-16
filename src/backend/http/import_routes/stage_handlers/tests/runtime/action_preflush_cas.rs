    #[tokio::test(flavor = "multi_thread")]
    async fn learning_suggestions_rejects_a_stale_preflush_row_without_writing() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("learning-preflush-stale-row")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'学习预写账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'学习预写分类','3','支出/学习预写分类',true) RETURNING id",
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
                preview_date: "2026-08-17 03:40:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 4_100,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "学习预写草稿".into(),
                preview_selected: true,
                preview_matching_feedback: json!({
                    "learning": {
                        "rule_id": 7,
                        "score": 0.9,
                        "review_status": "pending"
                    }
                }),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        bill_analyser_db::update_preview_selection(
            pool,
            &[preview_id],
            true,
            canonical_user,
        )
        .expect("select preview");
        let baseline = bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("load baseline preview")
            .expect("baseline preview exists");
        assert!(baseline.preview_selected);
        let selected_rows = bill_analyser_db::get_preview_by_session(
            pool,
            &session_id,
            canonical_user,
            true,
        )
        .expect("load selected preview rows");
        let selection_hash = bill_analyser_db::preview_id_snapshot_hash(
            &selected_rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        );

        let (status, body) = import_test_response(
            import_learning_suggestions_post_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "action_scope": {
                        "kind": "selected",
                        "selection_hash": selection_hash
                    },
                    "preview_updates": [{
                        "id": preview_id,
                        "expected_row_version": baseline.version + 1,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 4_100,
                        "preview_destination_amount_cents": 0,
                        "is_manually_annotated": true,
                        "selected": true
                    }]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["previewItem"]["id"], preview_id);
        assert_eq!(
            body["data"]["previewItem"]["row_version"],
            baseline.version
        );

        let current = bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("reload preview")
            .expect("preview exists");
        assert_eq!(current.category_id, None);
        assert_eq!(current.version, baseline.version);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn learning_suggestions_rejects_a_stale_preflush_selection_without_writing() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("learning-preflush-stale-selection")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'集合预写账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'集合预写分类','3','支出/集合预写分类',true) RETURNING id",
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
                preview_date: "2026-08-17 03:50:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 4_200,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "集合过期草稿".into(),
                preview_matching_feedback: json!({
                    "learning": {"rule_id": 8, "score": 0.8, "review_status": "pending"}
                }),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert patch preview");
        let concurrent_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 03:51:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 4_300,
                preview_counterparty: "并发集合成员".into(),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert concurrent preview");
        bill_analyser_db::update_preview_selection(
            pool,
            &[patch_preview_id],
            true,
            canonical_user,
        )
        .expect("select patch preview");
        let baseline = bill_analyser_db::get_preview_bill_by_id(
            pool,
            patch_preview_id,
            canonical_user,
        )
        .expect("load baseline preview")
        .expect("baseline preview exists");
        let stale_selection_hash = bill_analyser_db::preview_id_snapshot_hash(&[patch_preview_id]);
        bill_analyser_db::update_preview_selection(
            pool,
            &[concurrent_preview_id],
            true,
            canonical_user,
        )
        .expect("change selection concurrently");

        let (status, body) = import_test_response(
            import_learning_suggestions_post_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "action_scope": {
                        "kind": "selected",
                        "selection_hash": stale_selection_hash
                    },
                    "preview_updates": [{
                        "id": patch_preview_id,
                        "expected_row_version": baseline.version,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 4_200,
                        "preview_destination_amount_cents": 0,
                        "is_manually_annotated": true,
                        "selected": true
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
        assert_eq!(
            body["data"]["previewItems"][0]["id"],
            patch_preview_id
        );

        let current = bill_analyser_db::get_preview_bill_by_id(
            pool,
            patch_preview_id,
            canonical_user,
        )
        .expect("reload preview")
        .expect("preview exists");
        assert_eq!(current.category_id, None);
        assert_eq!(current.version, baseline.version);
    }

    struct ActionPreflushFixture {
        state: HttpAppState,
        user_id: i64,
        session_id: String,
        preview_id: i64,
        account_id: i64,
        category_id: i64,
        row_version: i64,
        selection_hash: String,
    }

    async fn action_preflush_fixture(label: &'static str) -> Option<ActionPreflushFixture> {
        let (state, user_id, session_id) = import_postgres_test_state().await?;
        let runtime = state
            .open_postgres_repository_runtime(label)
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,$2,'asset',true) RETURNING id",
        )
        .bind(user_id)
        .bind(format!("{label}-account"))
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,$2,'3',$3,true) RETURNING id",
        )
        .bind(user_id)
        .bind(format!("{label}-category"))
        .bind(format!("支出/{label}-category"))
        .fetch_one(pool)
        .await
        .expect("insert category");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 04:10:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 4_400,
                preview_source_account_id: Some(account_id),
                preview_counterparty: label.to_string(),
                preview_matching_feedback: json!({
                    "learning": {"rule_id": 9, "score": 0.88, "review_status": "pending"}
                }),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        bill_analyser_db::update_preview_selection(
            pool,
            &[preview_id],
            true,
            canonical_user,
        )
        .expect("select preview");
        let row_version = bill_analyser_db::get_preview_bill_by_id(
            pool,
            preview_id,
            canonical_user,
        )
        .expect("load preview")
        .expect("preview exists")
        .version;
        let selection_hash = bill_analyser_db::preview_id_snapshot_hash(&[preview_id]);
        Some(ActionPreflushFixture {
            state,
            user_id,
            session_id,
            preview_id,
            account_id,
            category_id,
            row_version,
            selection_hash,
        })
    }

    fn action_preflush_payload(fixture: &ActionPreflushFixture, row_version: i64) -> Value {
        json!({
            "session_id": fixture.session_id,
            "action_scope": {
                "kind": "selected",
                "selection_hash": fixture.selection_hash
            },
            "preview_updates": [{
                "id": fixture.preview_id,
                "expected_row_version": row_version,
                "preview_type": "支出",
                "category_id": fixture.category_id,
                "preview_source_account_id": fixture.account_id,
                "preview_amount_cents": 4_400,
                "preview_destination_amount_cents": 0,
                "is_manually_annotated": true,
                "selected": true
            }]
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn learning_suggestions_commits_current_preflush_and_returns_post_patch_rows() {
        let Some(fixture) = action_preflush_fixture("learning-preflush-success").await else {
            return;
        };
        let payload = action_preflush_payload(&fixture, fixture.row_version);
        let (status, body) = import_test_response(
            import_learning_suggestions_post_runtime_handler(
                State(fixture.state.clone()),
                Path(fixture.session_id.clone()),
                import_test_headers(fixture.user_id),
                Json(payload),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["applied_preview_updates"], 1);
        assert_eq!(body["data"]["count"], 1);
        let runtime = fixture
            .state
            .open_postgres_repository_runtime("learning-preflush-success-check")
            .expect("postgres runtime");
        let current = bill_analyser_db::get_preview_bill_by_id(
            runtime.pool(),
            fixture.preview_id,
            UserId::new(fixture.user_id as u64).expect("positive user id"),
        )
        .expect("reload preview")
        .expect("preview exists");
        assert_eq!(current.category_id, Some(fixture.category_id));
        assert_eq!(current.version, fixture.row_version + 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn llm_preview_recommend_rejects_a_stale_preflush_row_before_provider_lookup() {
        let Some(fixture) = action_preflush_fixture("llm-preview-preflush-stale").await else {
            return;
        };
        let payload = action_preflush_payload(&fixture, fixture.row_version + 1);
        let (status, body) = import_test_response(
            llm_preview_recommend_runtime_handler(
                State(fixture.state),
                import_test_headers(fixture.user_id),
                Bytes::from(serde_json::to_vec(&payload).expect("serialize payload")),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["previewItem"]["id"], fixture.preview_id);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn llm_analyze_rejects_a_stale_preflush_row_before_provider_lookup() {
        let Some(fixture) = action_preflush_fixture("llm-analyze-preflush-stale").await else {
            return;
        };
        let payload = action_preflush_payload(&fixture, fixture.row_version + 1);
        let (status, body) = import_test_response(
            llm_analyze_transactions_runtime_handler(
                State(fixture.state),
                import_test_headers(fixture.user_id),
                Bytes::from(serde_json::to_vec(&payload).expect("serialize payload")),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["previewItem"]["id"], fixture.preview_id);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn selected_action_rejects_a_preflush_patch_outside_the_selection() {
        let Some(fixture) = action_preflush_fixture("selected-preflush-scope").await else {
            return;
        };
        let runtime = fixture
            .state
            .open_postgres_repository_runtime("selected-preflush-scope-extra")
            .expect("postgres runtime");
        let canonical_user = UserId::new(fixture.user_id as u64).expect("positive user id");
        let unselected_preview_id = bill_analyser_db::insert_preview_bill(
            runtime.pool(),
            &fixture.session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 04:20:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 4_500,
                preview_source_account_id: Some(fixture.account_id),
                preview_counterparty: "outside selected action scope".into(),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert unselected preview");
        let baseline = bill_analyser_db::get_preview_bill_by_id(
            runtime.pool(),
            unselected_preview_id,
            canonical_user,
        )
        .expect("load unselected preview")
        .expect("unselected preview exists");
        let payload = json!({
            "session_id": fixture.session_id,
            "action_scope": {
                "kind": "selected",
                "selection_hash": fixture.selection_hash
            },
            "preview_updates": [{
                "id": unselected_preview_id,
                "expected_row_version": baseline.version,
                "preview_type": "支出",
                "category_id": fixture.category_id,
                "preview_source_account_id": fixture.account_id,
                "preview_amount_cents": 4_500,
                "preview_destination_amount_cents": 0,
                "is_manually_annotated": true,
                "selected": false
            }]
        });

        let (status, body) = import_test_response(
            import_learning_suggestions_post_runtime_handler(
                State(fixture.state),
                Path(fixture.session_id),
                import_test_headers(fixture.user_id),
                Json(payload),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
        assert_eq!(body["code"], "INVALID_ACTION_SCOPE");
        let current = bill_analyser_db::get_preview_bill_by_id(
            runtime.pool(),
            unselected_preview_id,
            canonical_user,
        )
        .expect("reload unselected preview")
        .expect("unselected preview exists");
        assert_eq!(current.category_id, None);
        assert_eq!(current.version, baseline.version);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn all_matching_action_loads_authoritative_rows_without_preview_patches() {
        let Some(fixture) = action_preflush_fixture("all-matching-preflush").await else {
            return;
        };
        let filters = ImportPreviewQueryFilters::default();
        let filter_hash = action_scope_filter_hash(&filters).expect("hash filters");
        let (status, body) = import_test_response(
            import_learning_suggestions_post_runtime_handler(
                State(fixture.state),
                Path(fixture.session_id.clone()),
                import_test_headers(fixture.user_id),
                Json(json!({
                    "session_id": fixture.session_id,
                    "action_scope": {
                        "kind": "all_matching",
                        "filters": filters,
                        "filter_hash": filter_hash
                    },
                    "preview_updates": []
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["applied_preview_updates"], 0);
        assert_eq!(body["data"]["count"], 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn explicit_selected_action_applies_current_preview_patches() {
        let Some(fixture) = action_preflush_fixture("explicit-selected-preflush").await else {
            return;
        };
        let mut payload = action_preflush_payload(&fixture, fixture.row_version);
        payload["action_scope"] = json!({
            "kind": "explicit_selected",
            "preview_ids": [fixture.preview_id]
        });
        let (status, body) = import_test_response(
            import_learning_suggestions_post_runtime_handler(
                State(fixture.state.clone()),
                Path(fixture.session_id.clone()),
                import_test_headers(fixture.user_id),
                Json(payload),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["applied_preview_updates"], 1);
        assert_eq!(body["data"]["count"], 1);
        let runtime = fixture
            .state
            .open_postgres_repository_runtime("explicit-selected-preflush-check")
            .expect("postgres runtime");
        let current = bill_analyser_db::get_preview_bill_by_id(
            runtime.pool(),
            fixture.preview_id,
            UserId::new(fixture.user_id as u64).expect("positive user id"),
        )
        .expect("reload preview")
        .expect("preview exists");
        assert_eq!(current.category_id, Some(fixture.category_id));
        assert_eq!(current.version, fixture.row_version + 1);
    }
