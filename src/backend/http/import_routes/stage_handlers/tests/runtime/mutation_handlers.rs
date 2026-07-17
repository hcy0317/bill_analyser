    #[tokio::test(flavor = "multi_thread")]
    async fn preview_mutation_handlers_cover_auth_payload_lookup_and_empty_reclassify() {
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
            (json!({"previewId": 9_999_999_999_i64}), StatusCode::NOT_FOUND),
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
        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["data"]["updated"], 0);
        assert_eq!(body["data"]["total"], 0);
        assert_eq!(body["data"]["preview"], json!([]));
    }

    async fn insert_reclassification_group(
        pool: &PostgresPool,
        session_id: &str,
        user_id: i64,
        status: &str,
        operation_id: &str,
    ) -> i64 {
        let session_db_id: i64 = sqlx::query_scalar(
            "SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2",
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("session database id");
        let group_id: i64 = sqlx::query_scalar(
            "INSERT INTO import_decision_groups(session_id,user_id,group_type,group_key,signal_payload) VALUES($1,$2,'same_batch_transfer',$3,jsonb_build_object('reclassification',jsonb_build_object('status',$4::text,'lease_expires_at',clock_timestamp()+interval '5 minutes'))) RETURNING id",
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(format!("reclass-{operation_id}"))
        .bind(status)
        .fetch_one(pool)
        .await
        .expect("insert reclassification group");
        sqlx::query("INSERT INTO import_confirm_operations(session_id,user_id,operation_kind,operation_id,status,payload) VALUES($1,$2,'decision_group',$3,'completed',jsonb_build_object('group_id',$4,'result',jsonb_build_object('upserted_preview_items',jsonb_build_array())))")
            .bind(session_db_id)
            .bind(user_id)
            .bind(operation_id)
            .bind(group_id)
            .execute(pool)
            .await
            .expect("insert operation ledger");
        group_id
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dematerialized_reclassification_covers_claim_finish_replay_and_state_errors() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let mut runtime = open_runtime(&state).expect("import runtime");
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            runtime.connection(),
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-07-11 12:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: 2_800,
                preview_destination_amount_cents: 2_800,
                preview_counterparty: "覆盖率商户".into(),
                preview_payment_method: "测试渠道".into(),
                preview_description: "撤销转账后重新分类".into(),
                preview_parser_id: "coverage-parser".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        let operation_id = format!("reclass-success-{preview_id}");
        let group_id = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "pending",
            &operation_id,
        )
        .await;

        for (operation_id, decision) in [("invalid-decision", "unexpected"), ("", "accept")] {
            let result = apply_import_decision_group_command(
                runtime.connection(),
                canonical_user,
                &ImportDecisionGroupCommand {
                    operation_id: operation_id.into(),
                    session_id: session_id.clone(),
                    group_id,
                    decision: decision.into(),
                    expected_group_version: 1,
                    expected_preview_versions: vec![],
                },
            )
            .expect("invalid command returns a conflict result");
            assert_eq!(result, ImportDecisionGroupCommandResult::Conflict);
        }

        let items = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            group_id,
            &operation_id,
            &[preview_id],
        )
        .await
        .expect("claimed reclassification succeeds");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], preview_id);

        let replay = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            group_id,
            &operation_id,
            &[preview_id],
        )
        .await
        .expect("completed operation replays items");
        assert_eq!(replay, items);

        let completed_empty_operation = format!("reclass-empty-{preview_id}");
        let completed_empty_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "completed",
            &completed_empty_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            completed_empty_group,
            &completed_empty_operation,
            &[],
        )
        .await
        .expect_err("completed without items is invalid");
        assert!(error.to_string().contains("completed without preview items"));

        let running_operation = format!("reclass-running-{preview_id}");
        let running_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "running",
            &running_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            running_group,
            &running_operation,
            &[],
        )
        .await
        .expect_err("live running lease remains pending");
        assert!(error.to_string().contains("reclassification pending"));

        let unknown_operation = format!("reclass-unknown-{preview_id}");
        let unknown_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "unknown",
            &unknown_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            unknown_group,
            &unknown_operation,
            &[],
        )
        .await
        .expect_err("unknown state is rejected");
        assert!(error.to_string().contains("state missing"));

        let failed_operation = format!("reclass-failed-{preview_id}");
        let failed_group = insert_reclassification_group(
            runtime.connection(),
            &session_id,
            user_id,
            "failed",
            &failed_operation,
        )
        .await;
        let error = reclassify_dematerialized_preview_items(
            &mut runtime,
            &session_id,
            canonical_user,
            failed_group,
            &failed_operation,
            &[],
        )
        .await
        .expect_err("retry with no preview rows fails and records failure");
        assert!(error.to_string().contains("has no preview rows"));
        assert_eq!(
            get_import_group_reclassification_state(
                runtime.connection(),
                canonical_user,
                failed_group,
                &failed_operation,
            )
            .expect("failed state persisted")
            .0,
            "failed"
        );
    }
