    #[tokio::test(flavor = "multi_thread")]
    async fn stale_confirm_session_version_returns_latest_snapshot_without_writing() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("confirm-session-version-cas")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            &session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-08-17 08:00:00".into(),
                preview_type: "支出".into(),
                preview_amount_cents: -1_200,
                preview_description: "must survive stale confirm".into(),
                preview_selected: true,
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert preview");
        let session_before = get_import_session(pool, &session_id, canonical_user)
            .expect("session lookup")
            .expect("session row");
        let state_before: (String, i64, Value, i64, i64) = sqlx::query_as(
            r#"
            SELECT status,
                   version,
                   metadata,
                   (SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE user_id=$1 AND session_id=import_sessions.id),
                   (SELECT COUNT(*)::BIGINT FROM bills WHERE user_id=$1)
            FROM import_sessions
            WHERE session_key=$2 AND user_id=$1
            "#,
        )
        .bind(user_id)
        .bind(&session_id)
        .fetch_one(pool)
        .await
        .expect("confirm state before stale request");

        let (session_status, session_body) = import_test_response(
            import_session_runtime_handler(
                State(state.clone()),
                Path(session_id.clone()),
                import_test_headers(user_id),
            )
            .await,
        )
        .await;
        assert_eq!(session_status, StatusCode::OK, "body: {session_body}");
        assert_eq!(
            session_body["data"]["session_version"],
            session_before.version
        );

        let stale_version = session_before.version + 1;
        let (status, body) = import_test_response(
            import_confirm_runtime_handler(
                State(state),
                import_test_headers(user_id),
                Json(json!({
                    "session_id": session_id,
                    "expected_session_version": stale_version,
                    "preview_updates": [{
                        "id": preview_id,
                        "description": "must not be written"
                    }]
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "IMPORT_SESSION_VERSION_CONFLICT");
        assert_eq!(body["data"]["expected_session_version"], stale_version);
        assert_eq!(
            body["data"]["actual_session_version"],
            session_before.version
        );
        assert_eq!(
            body["data"]["session"]["session_version"],
            session_before.version
        );

        let state_after: (String, i64, Value, i64, i64) = sqlx::query_as(
            r#"
            SELECT status,
                   version,
                   metadata,
                   (SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE user_id=$1 AND session_id=import_sessions.id),
                   (SELECT COUNT(*)::BIGINT FROM bills WHERE user_id=$1)
            FROM import_sessions
            WHERE session_key=$2 AND user_id=$1
            "#,
        )
        .bind(user_id)
        .bind(&session_id)
        .fetch_one(pool)
        .await
        .expect("confirm state after stale request");
        assert_eq!(state_after, state_before, "stale confirm left partial state");
        let preview_after = bill_analyser_db::get_preview_bill_by_id(
            pool,
            preview_id,
            canonical_user,
        )
        .expect("preview lookup after stale confirm")
        .expect("preview must remain staged");
        assert_eq!(
            preview_after.preview_description,
            "must survive stale confirm"
        );
        assert!(state_after.2.get("confirm_receipt").is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_requires_session_version_but_terminal_replay_remains_compatible() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("confirm-session-version-required")
            .expect("postgres runtime");
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let before = get_import_session(runtime.pool(), &session_id, canonical_user)
            .expect("session lookup")
            .expect("session row");

        let (missing_status, missing_body) = import_test_response(
            import_confirm_runtime_handler(
                State(state.clone()),
                import_test_headers(user_id),
                Json(json!({"session_id": session_id.clone()})),
            )
            .await,
        )
        .await;
        assert_eq!(missing_status, StatusCode::PRECONDITION_REQUIRED);
        assert_eq!(missing_body["code"], "IMPORT_SESSION_VERSION_REQUIRED");
        let after_missing = get_import_session(runtime.pool(), &session_id, canonical_user)
            .expect("session lookup after missing token")
            .expect("session row after missing token");
        assert_eq!(after_missing.status, before.status);
        assert_eq!(after_missing.version, before.version);

        let (status, body) = import_test_response(
            import_confirm_runtime_handler(
                State(state.clone()),
                import_test_headers(user_id),
                Json(json!({
                    "session_id": session_id.clone(),
                    "expected_session_version": before.version
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["imported_count"], 0);

        let (replay_status, replay_body) = import_test_response(
            import_confirm_runtime_handler(
                State(state),
                import_test_headers(user_id),
                Json(json!({"session_id": session_id})),
            )
            .await,
        )
        .await;
        assert_eq!(replay_status, StatusCode::OK, "body: {replay_body}");
        assert_eq!(replay_body["success"], true);
    }
