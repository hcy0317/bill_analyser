    #[tokio::test(flavor = "multi_thread")]
    async fn preview_mutation_handlers_reject_empty_reclassify_without_writing() {
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
            (
                json!({
                    "previewId": 9_999_999_999_i64,
                    "expected_row_version": 1
                }),
                StatusCode::NOT_FOUND,
            ),
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

        let runtime = state
            .open_postgres_repository_runtime("empty-update-full-session-reclassify")
            .expect("postgres runtime");
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            runtime.pool(),
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 08:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 1_000,
                preview_counterparty: "全量重新分类".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview for empty update reclassify");

        let (status, body) = import_test_response(
            import_preview_update_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                headers.clone(),
                Json(json!({
                    "previewId": preview_id,
                    "description": "first HTTP writer",
                    "expected_row_version": 1
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["updated"], true);
        assert_eq!(body["data"]["previewItem"]["row_version"], 2);
        assert_eq!(
            body["data"]["previewItem"]["preview_description"],
            "first HTTP writer"
        );

        let (status, body) = import_test_response(
            import_preview_update_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                headers.clone(),
                Json(json!({
                    "previewId": preview_id,
                    "description": "stale HTTP writer",
                    "expected_row_version": 1
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["success"], false);
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["expected_row_version"], 1);
        assert_eq!(body["data"]["actual_row_version"], 2);
        assert_eq!(body["data"]["previewItem"]["row_version"], 2);
        assert_eq!(
            body["data"]["previewItem"]["preview_description"],
            "first HTTP writer"
        );

        let version_before: i64 = sqlx::query_scalar(
            "SELECT version FROM import_preview_rows WHERE id=$1 AND user_id=$2",
        )
        .bind(preview_id)
        .bind(user_id)
        .fetch_one(runtime.pool())
        .await
        .expect("preview version before empty update reclassify");

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
        assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "body: {body}");
        assert_eq!(body["success"], false);
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_REQUIRED");
        let version_after: i64 = sqlx::query_scalar(
            "SELECT version FROM import_preview_rows WHERE id=$1 AND user_id=$2",
        )
        .bind(preview_id)
        .bind(user_id)
        .fetch_one(runtime.pool())
        .await
        .expect("preview version after empty update reclassify");
        assert_eq!(version_after, version_before, "rejected empty reclassify must not write");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reclassify_preserves_manually_owned_category_on_pending_transfer_preview() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("manual-transfer-category-reclassify")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let source_account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'人工分类来源账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert source account");
        let destination_account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'人工分类目标账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert destination account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'人工转账分类','4','转账/人工转账分类',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert active category");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 09:00:00".into(),
                preview_type: "转账".into(),
                preview_amount_cents: 5_000,
                preview_destination_amount_cents: 5_000,
                preview_source_account_id: Some(source_account_id),
                preview_destination_account_id: Some(destination_account_id),
                preview_counterparty: "人工分类转账".into(),
                preview_description: "pending transfer with manual category".into(),
                preview_selected: true,
                preview_matching_feedback: json!({
                    "transfer": {"state": "pending", "review_status": "pending"}
                }),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert pending transfer preview");

        let patch = ImportPreviewPatch::new(preview_id)
            .with_change(
                ImportPreviewPatchField::ManualAnnotation,
                ImportPreviewPatchValue::Bool(true),
            )
            .with_change(
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(category_id),
            )
            .with_change(
                ImportPreviewPatchField::MainCategory,
                ImportPreviewPatchValue::Text("转账".into()),
            )
            .with_change(
                ImportPreviewPatchField::SubCategory,
                ImportPreviewPatchValue::Text("人工转账分类".into()),
            )
            .with_change(
                ImportPreviewPatchField::Selected,
                ImportPreviewPatchValue::Bool(true),
            );
        assert!(bill_analyser_db::update_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &patch,
        )
        .expect("apply manual category patch"));
        let manually_patched = bill_analyser_db::get_preview_bill_by_id(
            pool,
            preview_id,
            canonical_user,
        )
        .expect("load manually patched preview")
        .expect("manually patched preview exists");
        assert_eq!(manually_patched.preview_type, "转账");
        assert_eq!(manually_patched.category_id, Some(category_id));
        assert_eq!(
            manually_patched
                .preview_matching_feedback
                .pointer("/annotation/manual_fields/category_id"),
            Some(&json!(true))
        );
        assert!(manually_patched
            .preview_matching_feedback
            .get("identity_validation")
            .is_none());

        let unrelated_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 10:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_000,
                category_id: Some(category_id),
                preview_main_category: "转账".into(),
                preview_sub_category: "人工转账分类".into(),
                preview_source_account_id: Some(source_account_id),
                preview_counterparty: "无关预览行".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert unrelated preview");
        let unrelated_version_before: i64 = sqlx::query_scalar(
            "SELECT version FROM import_preview_rows WHERE id=$1 AND user_id=$2",
        )
        .bind(unrelated_preview_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("unrelated preview version before reclassify");

        let (status, body) = import_test_response(
            import_reclassify_runtime_handler(
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "preview_updates": [{
                        "id": preview_id,
                        "expected_row_version": manually_patched.version,
                        "preview_type": "转账",
                        "category_id": category_id,
                        "preview_source_account_id": source_account_id,
                        "preview_destination_account_id": destination_account_id,
                        "preview_amount_cents": 5_000,
                        "preview_destination_amount_cents": 5_000,
                        "is_manually_annotated": true,
                        "selected": true
                    }]
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["total"], 1);
        assert_eq!(
            body["data"]["preview"].as_array().map(Vec::len),
            Some(1),
            "non-empty preview_updates must scope the reclassification response"
        );
        let response_preview = body["data"]["preview"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"] == preview_id))
            .expect("reclassify response preview");
        assert_eq!(response_preview["category_id"], category_id);
        assert_eq!(response_preview["preview_type"], "转账");
        assert_eq!(
            response_preview.pointer("/preview_matching_feedback/annotation/manual_fields/category_id"),
            Some(&json!(true))
        );
        assert!(response_preview
            .pointer("/preview_matching_feedback/identity_validation")
            .is_none());
        let unrelated_version_after: i64 = sqlx::query_scalar(
            "SELECT version FROM import_preview_rows WHERE id=$1 AND user_id=$2",
        )
        .bind(unrelated_preview_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("unrelated preview version after reclassify");
        assert_eq!(
            unrelated_version_after, unrelated_version_before,
            "non-empty preview_updates must not rewrite unrelated preview rows"
        );

        let page = bill_analyser_db::query_preview_page_by_session(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewPageRequest {
                preview_ids: vec![preview_id],
                ..ImportPreviewPageRequest::default()
            },
        )
        .expect("reload preview page");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].category_id, Some(category_id));
        assert_eq!(page.rows[0].preview_type, "转账");
        assert!(page.rows[0]
            .preview_matching_feedback
            .get("identity_validation")
            .is_none());
        assert_ne!(
            page.rows[0]
                .preview_matching_feedback
                .pointer("/annotation/status"),
            Some(&json!("missing_category"))
        );
        assert!(page.rows[0].preview_selected);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conditional_selection_applies_preview_updates_before_querying_valid_rows() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("conditional-selection-preview-update")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,is_active) VALUES($1,'条件选择账户','asset',true) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert account");
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path,is_active) VALUES($1,'条件选择分类','3','支出/条件选择分类',true) RETURNING id",
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
                preview_date: "2026-08-13 11:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_600,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "条件选择草稿".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        let preview_version = bill_analyser_db::get_preview_bill_by_id(
            pool,
            preview_id,
            canonical_user,
        )
        .expect("load conditional selection preview")
        .expect("conditional selection preview exists")
        .version;

        let (status, body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "select_valid",
                    "filters": {},
                    "preview_updates": [{
                        "id": preview_id,
                        "expected_row_version": preview_version,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 2_600,
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
        assert_eq!(body["data"]["metadata"]["counts"]["selected"], 1);

        let row = bill_analyser_db::get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("load selected preview")
            .expect("selected preview exists");
        assert_eq!(row.category_id, Some(category_id));
        assert!(row.preview_selected);
        assert!(row
            .preview_matching_feedback
            .get("identity_validation")
            .is_none());

        let oversized_updates = (0..=500)
            .map(|offset| json!({"id": preview_id + 1 + offset}))
            .collect::<Vec<_>>();
        let (status, body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "select_valid",
                    "preview_updates": oversized_updates
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_SELECTION_TOO_LARGE");

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
                preview_date: "2026-08-13 11:30:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_650,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "其他会话草稿".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert foreign preview");
        let foreign_preview_version = bill_analyser_db::get_preview_bill_by_id(
            pool,
            foreign_preview_id,
            canonical_user,
        )
        .expect("load foreign preview version")
        .expect("foreign preview exists")
        .version;
        let (status, body) = import_test_response(
            import_preview_selection_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "select_valid",
                    "preview_updates": [{
                        "id": foreign_preview_id,
                        "expected_row_version": foreign_preview_version,
                        "category_id": category_id,
                        "selected": false
                    }]
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
        let foreign_row =
            bill_analyser_db::get_preview_bill_by_id(pool, foreign_preview_id, canonical_user)
                .expect("load foreign preview")
                .expect("foreign preview exists");
        assert_eq!(foreign_row.category_id, None);
        assert!(!foreign_row.preview_selected);

        let rollback_preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-13 12:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_700,
                preview_source_account_id: Some(account_id),
                preview_counterparty: "条件选择回滚".into(),
                preview_selected: false,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert rollback preview");
        let rollback_preview_version = bill_analyser_db::get_preview_bill_by_id(
            pool,
            rollback_preview_id,
            canonical_user,
        )
        .expect("load rollback preview version")
        .expect("rollback preview exists")
        .version;
        let trigger_name = format!("fail_conditional_selection_{rollback_preview_id}");
        let function_name = format!("fail_conditional_selection_fn_{rollback_preview_id}");
        sqlx::query(&format!(
            "CREATE FUNCTION {function_name}() RETURNS trigger AS $$ BEGIN IF NEW.id = {rollback_preview_id} AND NEW.selected THEN RAISE EXCEPTION 'forced selection failure'; END IF; RETURN NEW; END; $$ LANGUAGE plpgsql"
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
                State(state),
                Path(session_id.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "selectionAction": "select_valid",
                    "preview_updates": [{
                        "id": rollback_preview_id,
                        "expected_row_version": rollback_preview_version,
                        "preview_type": "支出",
                        "category_id": category_id,
                        "preview_source_account_id": account_id,
                        "preview_amount_cents": 2_700,
                        "preview_destination_amount_cents": 0,
                        "is_manually_annotated": true,
                        "selected": false
                    }]
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
        let rolled_back = bill_analyser_db::get_preview_bill_by_id(
            pool,
            rollback_preview_id,
            canonical_user,
        )
        .expect("load rollback preview")
        .expect("rollback preview exists");
        assert_eq!(rolled_back.category_id, None);
        assert!(!rolled_back.preview_selected);
    }
