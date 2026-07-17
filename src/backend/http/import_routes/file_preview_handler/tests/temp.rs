use super::super::super::*;
use super::helpers::*;
use axum::{body::to_bytes, http::StatusCode};

#[test]
fn temp_file_helpers_enforce_session_path_and_bounded_reads() {
    let user_id = UserId::new(9210).unwrap();
    for invalid in ["", " ", "a/b", "a.b"] {
        assert!(
            import_temp_session_component(invalid).is_err(),
            "session={invalid:?}"
        );
    }
    assert!(import_temp_session_component(&"x".repeat(129)).is_err());
    assert_eq!(
        import_temp_session_component(" session_1-a ").unwrap(),
        "session_1-a"
    );

    let session = "preview-helper-a";
    let relative =
        save_unmatched_import_file(user_id, session, "statement 01.csv", b"a,b\n1,2").unwrap();
    let canonical = validate_import_temp_path(&relative, user_id, Some(session)).unwrap();
    assert_eq!(
        super::super::request::read_bounded_preview_file(&canonical, 64).unwrap(),
        b"a,b\n1,2"
    );
    assert_eq!(
        super::super::request::read_bounded_preview_file(&canonical, 2)
            .unwrap_err()
            .status_code,
        413
    );
    assert!(validate_import_temp_path("", user_id, Some(session)).is_err());
    assert!(
        validate_import_temp_path(canonical.to_str().unwrap(), user_id, Some(session)).is_err()
    );
    assert_eq!(
        validate_import_temp_path("missing/file.csv", user_id, None)
            .unwrap_err()
            .status_code,
        404
    );
    cleanup_temp_fixture(&relative);
    assert_eq!(
        super::super::request::read_bounded_preview_file(&canonical, 64)
            .unwrap_err()
            .status_code,
        404
    );
}

#[tokio::test]
async fn temp_preview_requires_session_and_preserves_error_envelope() {
    let user_id = UserId::new(9201).unwrap();
    let session_id = "preview-session-a";
    let temp_path = save_unmatched_import_file(
        user_id,
        session_id,
        "generic.csv",
        b"date,amount\n2026-01-15,-12.50\n",
    )
    .unwrap();
    let state = HttpAppState::new(
        HttpShellConfig::default().with_trusted_user_header_secret(PREVIEW_TEST_SECRET),
    )
    .unwrap();
    let response = preview_temp_request(state, &temp_path, None, 9201).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["success"], false);
    assert_eq!(
        payload["error"],
        "session_id is required for temp_path preview"
    );
    cleanup_temp_fixture(&temp_path);
}

#[test]
fn temp_preview_path_is_scoped_to_expected_user_and_session() {
    let user_id = UserId::new(9202).unwrap();
    let other_user_id = UserId::new(9203).unwrap();
    let session_id = "preview-session-scope-a";
    let other_session_id = "preview-session-scope-b";
    let temp_path = save_unmatched_import_file(
        user_id,
        session_id,
        "generic.csv",
        b"date,amount\n2026-01-15,-12.50\n",
    )
    .unwrap();
    let other_session_path = save_unmatched_import_file(
        user_id,
        other_session_id,
        "other-session.csv",
        b"date,amount\n2026-01-16,-3.20\n",
    )
    .unwrap();
    let other_user_path = save_unmatched_import_file(
        other_user_id,
        session_id,
        "other-user.csv",
        b"date,amount\n2026-01-17,-8.40\n",
    )
    .unwrap();

    assert!(validate_import_temp_path(&temp_path, user_id, Some(session_id)).is_ok());
    assert_eq!(
        validate_import_temp_path(&temp_path, user_id, Some(other_session_id))
            .unwrap_err()
            .status_code,
        400
    );
    assert_eq!(
        validate_import_temp_path(&other_user_path, user_id, Some(session_id))
            .unwrap_err()
            .status_code,
        400
    );
    assert_eq!(
        validate_import_temp_path("../outside.csv", user_id, Some(session_id))
            .unwrap_err()
            .status_code,
        400
    );
    cleanup_temp_fixture(&temp_path);
    cleanup_temp_fixture(&other_session_path);
    cleanup_temp_fixture(&other_user_path);
}

#[test]
fn temp_preview_path_rejects_cross_session_before_expected_directory_exists() {
    let user_id = UserId::new(9204).unwrap();
    let source_session_id = "preview-session-existing";
    let expected_session_id = "preview-session-not-created";
    let temp_path = save_unmatched_import_file(
        user_id,
        source_session_id,
        "generic.csv",
        b"date,amount\n2026-01-15,-12.50\n",
    )
    .unwrap();

    assert_eq!(
        validate_import_temp_path(&temp_path, user_id, Some(expected_session_id))
            .unwrap_err()
            .status_code,
        400
    );
    assert_eq!(
        validate_import_temp_path(
            &format!(
                "{}/{expected_session_id}/missing.csv",
                import_temp_user_component(user_id)
            ),
            user_id,
            Some(expected_session_id),
        )
        .unwrap_err()
        .status_code,
        404
    );

    cleanup_temp_fixture(&temp_path);
}

#[tokio::test(flavor = "multi_thread")]
async fn temp_preview_route_enforces_real_postgres_session_and_user_scope() {
    let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        eprintln!("skipping import preview PostgreSQL contract: BILL_ANALYSER_TEST_POSTGRES_URL is not set");
        return;
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&postgres_url)
        .await
        .expect("connect preview test PostgreSQL");
    bill_analyser_db::run_postgres_migrations(&pool)
        .await
        .expect("preview test migrations");
    let nonce = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .unsigned_abs();
    let owner_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("preview-owner-{nonce}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    let other_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("preview-other-{nonce}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    let owner = UserId::new(owner_id as u64).unwrap();
    let first_session = format!("preview-session-{nonce}-a");
    let second_session = format!("preview-session-{nonce}-b");
    for session_id in [&first_session, &second_session] {
        bill_analyser_db::create_import_session(
            &pool,
            &ImportSessionDraft {
                session_id: session_id.clone(),
                user_id: owner,
                file_count: 1,
            },
        )
        .unwrap();
    }
    let temp_path = save_unmatched_import_file(
        owner,
        &first_session,
        "generic.csv",
        b"date,amount\n2026-01-15,-12.50\n",
    )
    .unwrap();
    let state = HttpAppState::new(
        HttpShellConfig::default()
            .with_postgres_url(postgres_url)
            .unwrap()
            .with_trusted_user_header_secret(PREVIEW_TEST_SECRET),
    )
    .unwrap();

    assert_eq!(
        preview_temp_request(state.clone(), &temp_path, Some(&first_session), owner_id)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        preview_temp_request(state.clone(), &temp_path, Some(&first_session), other_id)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        preview_temp_request(state, &temp_path, Some(&second_session), owner_id)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );

    cleanup_temp_fixture(&temp_path);
    sqlx::query("DELETE FROM users WHERE id = ANY($1)")
        .bind(vec![owner_id, other_id])
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}
