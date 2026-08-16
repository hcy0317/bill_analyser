    #[tokio::test(flavor = "multi_thread")]
    async fn legacy_transfer_accept_clear_returns_session_and_canonical_preview_items() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let preview_id =
            insert_legacy_same_batch_transfer_fixture(&state, user_id, &session_id).await;
        let headers = import_test_headers(user_id);

        let (accept_status, accept_body) = import_test_response(
            preview_transfer_decision_runtime_handler(
                State(state.clone()),
                Path(preview_id),
                headers.clone(),
                Json(json!({
                    "decision": "accept",
                    "expectedState": {
                        "sessionId": session_id,
                        "type": "转账",
                        "mainCategory": "",
                        "subCategory": ""
                    }
                })),
            )
            .await,
        )
        .await;
        assert_eq!(accept_status, StatusCode::OK, "body: {accept_body}");
        assert_eq!(accept_body["data"]["sessionId"], session_id);
        assert_eq!(accept_body["data"]["removed"], json!([]));
        assert_eq!(
            accept_body["data"]["removedPreviewIds"],
            json!([preview_id])
        );
        assert_eq!(
            accept_body["data"]["upsertedPreviewItems"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            accept_body["data"]["upsertedPreviewItems"][0]["id"],
            preview_id
        );
        assert_eq!(
            accept_body["data"]["upsertedPreviewItems"][0]["session_id"],
            session_id
        );
        assert_eq!(
            accept_body["data"]["upsertedPreviewItems"][0]["preview_matching_feedback"]["transfer"]
                ["review_status"],
            "accepted"
        );

        for attempt in 0..2 {
            let (clear_status, clear_body) = import_test_response(
                preview_transfer_decision_runtime_handler(
                    State(state.clone()),
                    Path(preview_id),
                    headers.clone(),
                    Json(json!({
                        "decision": "clear",
                        "expectedState": {
                            "sessionId": session_id,
                            "type": "转账",
                            "mainCategory": "",
                            "subCategory": ""
                        }
                    })),
                )
                .await,
            )
            .await;
            assert_eq!(
                clear_status,
                StatusCode::OK,
                "attempt {attempt}, body: {clear_body}"
            );
            assert_eq!(clear_body["data"]["sessionId"], session_id);
            assert_eq!(clear_body["data"]["removed"], json!([]));
            assert_eq!(clear_body["data"]["removedPreviewIds"], json!([preview_id]));
            assert_eq!(
                clear_body["data"]["upsertedPreviewItems"]
                    .as_array()
                    .map(Vec::len),
                Some(1)
            );
            assert_eq!(
                clear_body["data"]["upsertedPreviewItems"][0]["id"],
                preview_id
            );
            assert_eq!(
                clear_body["data"]["upsertedPreviewItems"][0]["preview_matching_feedback"]
                    ["transfer"]["review_status"],
                "pending"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stale_transfer_row_version_returns_typed_conflict_without_advancing_group() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let preview_id =
            insert_legacy_same_batch_transfer_fixture(&state, user_id, &session_id).await;
        let runtime = state
            .open_postgres_repository_runtime("stale-transfer-row-version")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");
        let stale_preview = get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("stale preview lookup")
            .expect("stale preview row");

        let concurrent_patch = ImportPreviewPatch::new(preview_id).with_change(
            ImportPreviewPatchField::Description,
            ImportPreviewPatchValue::Text("newer concurrent transfer description".to_string()),
        );
        assert!(
            update_preview_bill(pool, &session_id, canonical_user, &concurrent_patch)
                .expect("concurrent preview update")
        );
        let latest_preview = get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("latest preview lookup")
            .expect("latest preview row");
        assert_eq!(latest_preview.version, stale_preview.version + 1);
        let groups_before = get_import_decision_groups_by_session(
            pool,
            &session_id,
            canonical_user,
        )
        .expect("decision groups before stale request");
        let command_versions = decision_group_preview_versions(
            groups_before.first().expect("transfer decision group"),
            Some((preview_id, stale_preview.version)),
        );
        assert_eq!(
            command_versions
                .iter()
                .find(|item| item.preview_row_id == preview_id)
                .map(|item| item.version),
            Some(stale_preview.version),
            "the client anchor token must reach the repository CAS command"
        );

        let (status, body) = import_test_response(
            preview_transfer_decision_runtime_handler(
                State(state),
                Path(preview_id),
                import_test_headers(user_id),
                Json(json!({
                    "decision": "reject",
                    "expectedState": {
                        "sessionId": session_id,
                        "rowVersion": stale_preview.version,
                        "type": "转账",
                        "mainCategory": "",
                        "subCategory": ""
                    }
                })),
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
        assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
        assert_eq!(body["data"]["expected_row_version"], stale_preview.version);
        assert_eq!(body["data"]["actual_row_version"], latest_preview.version);
        assert_eq!(body["data"]["previewItem"]["id"], preview_id);
        assert_eq!(
            body["data"]["previewItem"]["preview_description"],
            "newer concurrent transfer description"
        );

        let preview_after = get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("preview lookup after stale request")
            .expect("preview row after stale request");
        assert_eq!(preview_after, latest_preview);
        let groups_after = get_import_decision_groups_by_session(
            pool,
            &session_id,
            canonical_user,
        )
        .expect("decision groups after stale request");
        assert_eq!(groups_after, groups_before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn same_batch_transfer_reject_reclassification_preserves_manual_identity_fields() {
        let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
            return;
        };
        let preview_id =
            insert_legacy_same_batch_transfer_fixture(&state, user_id, &session_id).await;
        let runtime = state
            .open_postgres_repository_runtime("manual-reclassification-fixture")
            .expect("postgres runtime");
        let pool = runtime.pool();
        let canonical_user = UserId::new(user_id as u64).expect("positive user id");

        let manual_category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories(user_id,name,category_type,path) VALUES($1,'人工分类','4','人工转账/人工分类') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("manual category");
        let rule_category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories(user_id,name,category_type,path) VALUES($1,'规则分类','3','规则支出/规则分类') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("rule category");
        let incoming_category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories(user_id,name,category_type,path) VALUES($1,'入账规则','2','规则收入/入账规则') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("incoming rule category");
        let learning_category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories(user_id,name,category_type,path) VALUES($1,'学习分类','3','学习支出/学习分类') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("learning category");
        let mut account_ids = Vec::new();
        for name in [
            "人工转出账户",
            "人工转入账户",
            "学习转出账户",
            "学习目标账户",
        ] {
            account_ids.push(
                sqlx::query_scalar(
                    "INSERT INTO accounts(user_id,name,account_type) VALUES($1,$2,'asset') RETURNING id",
                )
                .bind(user_id)
                .bind(name)
                .fetch_one(pool)
                .await
                .expect("fixture account"),
            );
        }
        let [manual_source_id, manual_destination_id, learning_source_id, learning_destination_id] =
            account_ids.as_slice()
        else {
            panic!("four fixture accounts");
        };

        let session_db_id: i64 = sqlx::query_scalar(
            "SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2",
        )
        .bind(&session_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("session database id");
        for (direction, marker) in [
            ("expense", "manual-protected-outgoing"),
            ("income", "auto-rematch-incoming"),
        ] {
            sqlx::query(
                "UPDATE import_standard_rows SET merchant=$1,payment_method=$1,description=$1 WHERE session_id=$2 AND user_id=$3 AND direction=$4",
            )
            .bind(marker)
            .bind(session_db_id)
            .bind(user_id)
            .bind(direction)
            .execute(pool)
            .await
            .expect("specialize standard row features");
        }

        let manual_patch = ImportPreviewPatch::new(preview_id).with_changes([
            (
                ImportPreviewPatchField::ManualAnnotation,
                ImportPreviewPatchValue::Bool(true),
            ),
            (
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(manual_category_id),
            ),
            (
                ImportPreviewPatchField::MainCategory,
                ImportPreviewPatchValue::Text("人工转账".to_string()),
            ),
            (
                ImportPreviewPatchField::SubCategory,
                ImportPreviewPatchValue::Text("人工分类".to_string()),
            ),
            (
                ImportPreviewPatchField::SourceAccountId,
                ImportPreviewPatchValue::Integer(*manual_source_id),
            ),
            (
                ImportPreviewPatchField::DestinationAccountId,
                ImportPreviewPatchValue::Integer(*manual_destination_id),
            ),
        ]);
        assert!(
            update_preview_bill(pool, &session_id, canonical_user, &manual_patch,)
                .expect("manual preview patch")
        );
        let manually_patched = get_preview_bill_by_id(pool, preview_id, canonical_user)
            .expect("manual preview lookup")
            .expect("manually patched preview");
        assert_eq!(manually_patched.category_id, Some(manual_category_id));
        assert_eq!(
            manually_patched
                .preview_matching_feedback
                .pointer("/annotation/manual_fields/category_id"),
            Some(&json!(true))
        );

        for (category_id, name, expression, priority) in [
            (
                rule_category_id,
                "outgoing overwrite threat",
                "OR={manual-protected-outgoing}",
                1,
            ),
            (
                incoming_category_id,
                "incoming rematch",
                "OR={auto-rematch-incoming}",
                2,
            ),
        ] {
            sqlx::query(
                "INSERT INTO category_rules(user_id,category_id,name,rule_expression,priority,enabled) VALUES($1,$2,$3,jsonb_build_object('expression',$4::text,'regex_enabled',false),$5,true)",
            )
            .bind(user_id)
            .bind(category_id)
            .bind(name)
            .bind(expression)
            .bind(priority)
            .execute(pool)
            .await
            .expect("category rule");
        }

        let learning_features = build_composite_match_features(
            "legacy-transfer-fixture",
            "manual-protected-outgoing",
            "manual-protected-outgoing",
            "manual-protected-outgoing",
        )
        .expect("learning features");
        let recommendation_key =
            build_import_learning_recommendation_key(&ImportLearningRecommendationKeyInput {
                user_id,
                recommended_type: "支出".to_string(),
                recommended_category_id: Some(learning_category_id),
                recommended_source_account_id: Some(*learning_source_id),
                recommended_destination_account_id: Some(*learning_destination_id),
                transaction_type_scope: "支出".to_string(),
                parser_bucket: "legacy-transfer-fixture".to_string(),
                counterparty_bucket: "manual-protected-outgoing".to_string(),
                payment_bucket: "manual-protected-outgoing".to_string(),
                description_bucket: "manual-protected-outgoing".to_string(),
                amount_bucket: Some(amount_cents_bucket(Some(&json!(5_000))).to_string()),
                ..ImportLearningRecommendationKeyInput::default()
            });
        sqlx::query(
            r#"INSERT INTO import_learning_lifecycle(
                   user_id,recommendation_key,recommendation_type,status,accepted_count,
                   auto_apply_enabled,metadata
               ) VALUES($1,$2,'支出','green',3,true,$3)"#,
        )
        .bind(user_id)
        .bind(&recommendation_key)
        .bind(json!({
            "parser_id": "legacy-transfer-fixture",
            "composite_hash": composite_hash_from_features(&learning_features),
            "match_features": learning_features,
            "learned_type": "支出",
            "learned_category_id": learning_category_id,
            "learned_source_account_id": learning_source_id,
            "learned_destination_account_id": learning_destination_id
        }))
        .execute(pool)
        .await
        .expect("auto-apply learning rule");

        let (status, body) = import_test_response(
            preview_transfer_decision_runtime_handler(
                State(state.clone()),
                Path(preview_id),
                import_test_headers(user_id),
                Json(json!({
                    "decision": "reject",
                    "expectedState": {
                        "sessionId": session_id,
                        "type": "转账",
                        "mainCategory": "人工转账",
                        "subCategory": "人工分类"
                    }
                })),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "body: {body}");
        let items = body["data"]["upsertedPreviewItems"]
            .as_array()
            .expect("canonical response items");
        assert_eq!(items.len(), 2, "body: {body}");
        let outgoing = items
            .iter()
            .find(|item| item["preview_type"] == "支出")
            .expect("outgoing response item");
        assert_eq!(outgoing["category_id"], manual_category_id);
        assert_eq!(outgoing["preview_main_category"], "人工转账");
        assert_eq!(outgoing["preview_sub_category"], "人工分类");
        assert_eq!(outgoing["preview_source_account_id"], *manual_source_id);
        assert_eq!(
            outgoing["preview_destination_account_id"],
            Value::Null,
            "non-transfer destination remains invalid after learning projection"
        );
        assert_eq!(
            outgoing.pointer("/preview_matching_feedback/annotation/manual_fields"),
            Some(&json!({
                "category_id": true,
                "source_account_id": true,
                "destination_account_id": false
            }))
        );
        assert_eq!(
            outgoing.pointer("/preview_matching_feedback/learning/review_status"),
            Some(&json!("auto_applied"))
        );

        let incoming = items
            .iter()
            .find(|item| item["preview_type"] == "收入")
            .expect("incoming response item");
        assert_eq!(incoming["category_id"], incoming_category_id);
        assert_eq!(incoming["preview_main_category"], "规则收入");
        assert_eq!(incoming["preview_sub_category"], "入账规则");
        assert_eq!(
            incoming["preview_source_account_id"],
            *manual_destination_id
        );
        assert_eq!(
            incoming.pointer("/preview_matching_feedback/annotation/manual_fields"),
            Some(&json!({
                "category_id": false,
                "source_account_id": true,
                "destination_account_id": false
            }))
        );

        for item in items {
            let preview_id = item["id"].as_i64().expect("response preview id");
            let canonical = get_preview_bill_by_id(pool, preview_id, canonical_user)
                .expect("canonical lookup")
                .expect("canonical preview row");
            assert_eq!(preview_row_to_value(canonical), *item);
        }
    }
