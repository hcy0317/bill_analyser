async fn insert_recurring_decision_preview_fixture(
    state: &HttpAppState,
    user_id: i64,
    session_id: &str,
) -> i64 {
    let runtime = state
        .open_postgres_repository_runtime("recurring-decision-row-cas-fixture")
        .expect("postgres runtime");
    bill_analyser_db::insert_preview_bill(
        runtime.pool(),
        session_id,
        UserId::new(user_id as u64).expect("positive user id"),
        &ImportPreviewDraft {
            preview_date: "2026-08-17 00:00:00".into(),
            preview_type: "支出".into(),
            preview_amount_cents: -2_500,
            preview_counterparty: "recurring-row-cas".into(),
            preview_payment_method: "card".into(),
            preview_description: "recurring row CAS fixture".into(),
            preview_parser_id: "fixture".into(),
            preview_selected: true,
            ..ImportPreviewDraft::default()
        },
    )
    .expect("insert recurring decision preview")
}

#[tokio::test(flavor = "multi_thread")]
async fn recurring_decision_current_versions_increment_and_missing_version_writes_nothing() {
    let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
        return;
    };
    let preview_id =
        insert_recurring_decision_preview_fixture(&state, user_id, &session_id).await;
    let runtime = state
        .open_postgres_repository_runtime("recurring-decision-current-version")
        .expect("postgres runtime");
    let pool = runtime.pool();
    let canonical_user = UserId::new(user_id as u64).expect("positive user id");
    let before = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("preview lookup")
        .expect("preview row");

    let (put_status, put_body) = import_test_response(
        preview_recurring_match_put_runtime_handler(
            State(state.clone()),
            Path(preview_id),
            import_test_headers(user_id),
            Json(json!({
                "recurringId": 17,
                "candidate": {
                    "id": 17,
                    "name": "Monthly coffee",
                    "matchScore": 0.92,
                    "matchReasons": ["amount", "date"],
                    "matchedOccurrenceDate": "2026-08-17"
                },
                "expectedState": {
                    "sessionId": session_id,
                    "rowVersion": before.version
                },
                "responseMode": "preview-item"
            })),
        )
        .await,
    )
    .await;
    assert_eq!(put_status, StatusCode::OK, "body: {put_body}");
    assert_eq!(put_body["data"]["previewItem"]["id"], preview_id);
    assert_eq!(put_body["data"]["previewItem"]["row_version"], before.version + 1);
    assert_eq!(put_body["data"]["previewItem"]["preview_recurring_id"], 17);
    assert_eq!(
        put_body["data"]["previewItem"]["preview_recurring_name"],
        "Monthly coffee"
    );

    let after_put = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("preview lookup after put")
        .expect("preview row after put");
    assert_eq!(after_put.version, before.version + 1);
    assert_eq!(after_put.preview_recurring_id, Some(17));

    let (delete_status, delete_body) = import_test_response(
        preview_recurring_match_delete_runtime_handler(
            State(state.clone()),
            Path(preview_id),
            import_test_headers(user_id),
            Json(json!({
                "expectedState": {
                    "sessionId": session_id,
                    "rowVersion": after_put.version
                },
                "responseMode": "preview-item"
            })),
        )
        .await,
    )
    .await;
    assert_eq!(delete_status, StatusCode::OK, "body: {delete_body}");
    assert_eq!(
        delete_body["data"]["previewItem"]["row_version"],
        after_put.version + 1
    );
    assert!(delete_body["data"]["previewItem"]["preview_recurring_id"].is_null());

    let after_delete = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("preview lookup after delete")
        .expect("preview row after delete");
    let (missing_status, missing_body) = import_test_response(
        preview_recurring_match_put_runtime_handler(
            State(state),
            Path(preview_id),
            import_test_headers(user_id),
            Json(json!({
                "recurringId": 23,
                "expectedState": { "sessionId": session_id },
                "responseMode": "preview-item"
            })),
        )
        .await,
    )
    .await;
    assert_eq!(
        missing_status,
        StatusCode::PRECONDITION_REQUIRED,
        "body: {missing_body}"
    );
    assert_eq!(missing_body["code"], "PREVIEW_ROW_VERSION_REQUIRED");
    let after_missing = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("preview lookup after missing token")
        .expect("preview row after missing token");
    assert_eq!(after_missing.version, after_delete.version);
    assert_eq!(after_missing.preview_recurring_id, after_delete.preview_recurring_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_recurring_decision_returns_latest_row_and_writes_nothing() {
    let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
        return;
    };
    let preview_id =
        insert_recurring_decision_preview_fixture(&state, user_id, &session_id).await;
    let runtime = state
        .open_postgres_repository_runtime("recurring-decision-stale-version")
        .expect("postgres runtime");
    let pool = runtime.pool();
    let canonical_user = UserId::new(user_id as u64).expect("positive user id");
    let stale = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("stale preview lookup")
        .expect("stale preview row");
    let concurrent_patch = ImportPreviewPatch::new(preview_id).with_change(
        ImportPreviewPatchField::Description,
        ImportPreviewPatchValue::Text("newer recurring description".into()),
    );
    assert!(
        update_preview_bill(pool, &session_id, canonical_user, &concurrent_patch)
            .expect("concurrent preview update")
    );
    let latest = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("latest preview lookup")
        .expect("latest preview row");

    let (status, body) = import_test_response(
        preview_recurring_match_put_runtime_handler(
            State(state),
            Path(preview_id),
            import_test_headers(user_id),
            Json(json!({
                "recurringId": 17,
                "expectedState": {
                    "sessionId": session_id,
                    "rowVersion": stale.version
                },
                "responseMode": "preview-item"
            })),
        )
        .await,
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
    assert_eq!(body["code"], "PREVIEW_ROW_VERSION_CONFLICT");
    assert_eq!(body["data"]["expected_row_version"], stale.version);
    assert_eq!(body["data"]["actual_row_version"], latest.version);
    assert_eq!(body["data"]["previewItem"]["id"], preview_id);
    assert_eq!(
        body["data"]["previewItem"]["preview_description"],
        "newer recurring description"
    );

    let after = get_preview_bill_by_id(pool, preview_id, canonical_user)
        .expect("preview lookup after stale request")
        .expect("preview row after stale request");
    assert_eq!(after, latest);
    assert_eq!(after.preview_recurring_id, None);
}
