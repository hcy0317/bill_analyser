    async fn import_test_response(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        (status, serde_json::from_slice(&body).expect("JSON response"))
    }

    fn import_test_headers(user_id: i64) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            TRUSTED_USER_SECRET_HEADER,
            HeaderValue::from_static("import-test-secret"),
        );
        headers.insert(
            "x-user-id",
            HeaderValue::from_str(&user_id.to_string()).expect("user header"),
        );
        headers
    }

    async fn import_postgres_test_state() -> Option<(HttpAppState, i64, String)> {
        let postgres_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL").unwrap_or_else(|_| {
            "postgres://bill_analyser:bill_analyser_dev@127.0.0.1:55432/bill_analyser".into()
        });
        let state = HttpAppState::new(
            HttpShellConfig::default()
                .with_postgres_url(postgres_url)
                .expect("test postgres URL")
                .with_trusted_user_header_secret("import-test-secret"),
        )
        .expect("test state");
        let runtime = state
            .open_postgres_repository_runtime("import-test")
            .expect("postgres runtime");
        bill_analyser_db::run_postgres_migrations(runtime.pool())
            .await
            .expect("postgres migrations");
        let nonce = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos(),
            IMPORT_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let username = format!("http-import-{nonce}");
        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
        )
        .bind(&username)
        .bind(format!("{username}@example.test"))
        .fetch_one(runtime.pool())
        .await
        .expect("insert test user");
        let session_id = format!("http-import-session-{nonce}");
        bill_analyser_db::create_import_session(
            runtime.pool(),
            &ImportSessionDraft {
                session_id: session_id.clone(),
                user_id: UserId::new(user_id as u64).expect("positive user id"),
                file_count: 1,
            },
        )
        .expect("create import session");
        Some((state, user_id, session_id))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn import_postgres_test_state_applies_the_canonical_migration_set() {
        let Some((state, _, _)) = import_postgres_test_state().await else {
            return;
        };
        let runtime = state
            .open_postgres_repository_runtime("import-schema-contract")
            .expect("postgres runtime");
        let latest_applied_version: i64 =
            sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success")
                .fetch_one(runtime.pool())
                .await
                .expect("canonical migration metadata");
        let latest_manifest_version = bill_analyser_db::postgres_migration_manifest()
            .last()
            .expect("postgres migration manifest")
            .version;

        assert_eq!(latest_applied_version, latest_manifest_version);
    }

    async fn insert_legacy_same_batch_transfer_fixture(
        state: &HttpAppState,
        user_id: i64,
        session_id: &str,
    ) -> i64 {
        let runtime = state
            .open_postgres_repository_runtime("legacy-transfer-fixture")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let session_db_id: i64 = sqlx::query_scalar(
            "SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2",
        )
        .bind(session_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("session database id");
        let source_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO import_sources
               (session_id,user_id,source_index,parser_id,parser_name,feature_signature)
               VALUES($1,$2,0,'legacy-transfer-fixture','legacy-transfer-fixture','legacy-transfer-fixture')
               RETURNING id"#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("insert import source");
        let mut standard_row_ids = Vec::new();
        for (source_row_index, direction, amount_cents) in
            [(0, "expense", -5_000_i64), (1, "income", 5_000_i64)]
        {
            standard_row_ids.push(
                sqlx::query_scalar(
                    r#"INSERT INTO import_standard_rows
                       (session_id,source_id,user_id,source_row_index,occurred_at,amount_cents,
                        direction,transaction_type,merchant,payment_method,description,parser_payload)
                       VALUES($1,$2,$3,$4,'2026-07-13 09:00:00+08',$5,$6,$6,
                              'legacy-fixture','legacy-fixture','legacy-fixture',
                              '{"parser_id":"legacy-transfer-fixture"}'::jsonb)
                       RETURNING id"#,
                )
                .bind(session_db_id)
                .bind(source_id)
                .bind(user_id)
                .bind(source_row_index)
                .bind(amount_cents)
                .bind(direction)
                .fetch_one(pool)
                .await
                .expect("insert standard row"),
            );
        }
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let preview_id = bill_analyser_db::insert_preview_bill(
            pool,
            session_id,
            canonical_user,
            &ImportPreviewDraft {
                preview_date: "2026-07-13 09:00:00".into(),
                preview_type: "转账".into(),
                preview_amount_cents: 5_000,
                preview_destination_amount_cents: 5_000,
                preview_counterparty: "legacy-fixture".into(),
                preview_payment_method: "legacy-fixture".into(),
                preview_description: "legacy canonical response".into(),
                preview_parser_id: "legacy-transfer-fixture".into(),
                preview_selected: true,
                preview_matching_feedback: json!({
                    "transfer": {"state": "pending", "review_status": "pending"}
                }),
                ..ImportPreviewDraft::default()
            },
        )
        .expect("insert transfer preview");
        bill_analyser_db::insert_import_decision_groups_batch(
            pool,
            session_id,
            canonical_user,
            &[ImportDecisionGroupDraft {
                group_type: "same_batch_transfer".into(),
                group_key: format!("legacy-transfer-{preview_id}"),
                decision_status: "pending".into(),
                base_preview_row_id: Some(preview_id),
                signal_payload: json!({"candidate_type": "transfer"}),
                members: vec![
                    ImportDecisionGroupMemberDraft {
                        preview_row_id: Some(preview_id),
                        standard_row_id: Some(standard_row_ids[0]),
                        history_bill_id: None,
                        member_role: "outgoing".into(),
                        parser_name: "legacy-transfer-fixture".into(),
                        metadata: json!({}),
                    },
                    ImportDecisionGroupMemberDraft {
                        preview_row_id: Some(preview_id),
                        standard_row_id: Some(standard_row_ids[1]),
                        history_bill_id: None,
                        member_role: "incoming".into(),
                        parser_name: "legacy-transfer-fixture".into(),
                        metadata: json!({}),
                    },
                ],
            }],
        )
        .expect("insert transfer decision group");
        preview_id
    }
