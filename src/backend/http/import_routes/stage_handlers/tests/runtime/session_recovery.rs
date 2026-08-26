#[tokio::test(flavor = "multi_thread")]
async fn recoverable_sessions_route_returns_only_current_users_preview_sessions() {
    let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
        return;
    };
    let runtime = state
        .open_postgres_repository_runtime("session-recovery-contract")
        .expect("postgres runtime");
    let canonical_user = UserId::new(user_id as u64).expect("positive user id");
    bill_analyser_db::insert_preview_bill(
        runtime.pool(),
        &session_id,
        canonical_user,
        &ImportPreviewDraft {
            preview_date: "2026-08-26 08:00:00".into(),
            preview_type: "支出".into(),
            preview_amount_cents: -1_200,
            preview_description: "recoverable preview".into(),
            preview_selected: true,
            ..ImportPreviewDraft::default()
        },
    )
    .expect("insert recoverable preview");
    update_import_session_status(
        runtime.pool(),
        &ImportSessionStatusUpdate {
            session_id: session_id.clone(),
            user_id: canonical_user,
            status: "preview".into(),
            total_parsed: Some(1),
            total_preview: Some(1),
            total_confirmed: None,
        },
    )
    .expect("mark session preview");

    let (status, body) = import_test_response(
        import_sessions_recovery_runtime_handler(
            State(state.clone()),
            import_test_headers(user_id),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["data"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["data"][0]["session_id"], session_id);
    assert_eq!(body["data"][0]["preview_count"], 1);

    clear_session_data(runtime.pool(), &session_id, canonical_user)
        .expect("recoverable session cleanup");
    let (_, empty_body) = import_test_response(
        import_sessions_recovery_runtime_handler(State(state), import_test_headers(user_id)).await,
    )
    .await;
    assert_eq!(empty_body["data"].as_array().map(Vec::len), Some(0));
}
