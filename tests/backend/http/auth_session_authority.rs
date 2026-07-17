use std::{env, error::Error, time::Instant};

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_postgres_token_session, invalidate_postgres_session_by_token_hash,
    CreateTokenSessionDraft, PostgresPool,
};
use bill_analyser_http::{build_router, HttpAppState, HttpShellConfig};
use chrono::{Duration as ChronoDuration, Utc};
use ring::hmac;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use tower::ServiceExt;

#[path = "../db/postgres_test_support.rs"]
mod postgres_test_support;

const JWT_SECRET: &str = "auth-session-authority-test-secret";

#[tokio::test]
async fn bearer_session_revoke_replay_fails_closed_and_index_is_covering(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("auth_session_revoke_replay").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "auth-session-revoke-replay").await?;
    let access_token = signed_access_token(user_id, ChronoDuration::minutes(10));
    let token_hash = sha256_hex(&access_token);
    create_session(pool, user_id, &token_hash).await?;
    let state = authority_state(&test_db.db_name)?;
    let app = build_router(state);

    assert_eq!(
        protected_request(&app, &access_token).await?.0,
        StatusCode::OK
    );
    assert!(invalidate_postgres_session_by_token_hash(pool, &token_hash).await?);
    assert_eq!(
        protected_request(&app, &access_token).await?.0,
        StatusCode::UNAUTHORIZED
    );

    let index_definition: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes WHERE schemaname='public' AND indexname='idx_token_sessions_authority_lookup'",
    )
    .fetch_one(pool)
    .await?;
    for column in ["token_hash", "user_id", "is_active", "expires_at"] {
        assert!(
            index_definition.contains(column),
            "authority index must cover {column}: {index_definition}"
        );
    }

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn bearer_session_pool_and_query_timeout_return_503_then_recover_without_restart(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("auth_session_timeout_recovery").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "auth-session-timeout-recovery").await?;
    let access_token = signed_access_token(user_id, ChronoDuration::minutes(10));
    create_session(pool, user_id, &sha256_hex(&access_token)).await?;
    let state = authority_state(&test_db.db_name)?;
    let shared_runtime = state.open_postgres_repository_runtime("auth-timeout-test")?;
    let app = build_router(state.clone());

    let mut held_connections = Vec::new();
    for _ in 0..5 {
        held_connections.push(shared_runtime.pool().acquire().await?);
    }
    let started = Instant::now();
    assert_eq!(
        protected_request(&app, &access_token).await?.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(started.elapsed() <= std::time::Duration::from_secs(1));
    drop(held_connections);
    assert_eq!(
        protected_request(&app, &access_token).await?.0,
        StatusCode::OK
    );

    let mut table_lock = pool.begin().await?;
    sqlx::query("LOCK TABLE token_sessions IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *table_lock)
        .await?;
    let started = Instant::now();
    assert_eq!(
        protected_request(&app, &access_token).await?.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(started.elapsed() <= std::time::Duration::from_secs(1));
    table_lock.rollback().await?;
    assert_eq!(
        protected_request(&app, &access_token).await?.0,
        StatusCode::OK
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_expired_and_unavailable_authority_never_fall_back_to_jwt_only(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("auth_session_fail_closed").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "auth-session-fail-closed").await?;
    let valid_without_session = signed_access_token(user_id, ChronoDuration::minutes(10));
    let expired = signed_access_token(user_id, ChronoDuration::minutes(-1));
    let app = build_router(authority_state(&test_db.db_name)?);

    assert_eq!(
        protected_request(&app, "not-a-jwt").await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        protected_request(&app, &expired).await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        protected_request(&app, &valid_without_session).await?.0,
        StatusCode::UNAUTHORIZED,
        "a valid JWT without an authoritative session must not reach bills"
    );

    let valid_with_expired_session = signed_access_token(user_id, ChronoDuration::minutes(10));
    let expired_session_hash = sha256_hex(&valid_with_expired_session);
    create_session(pool, user_id, &expired_session_hash).await?;
    sqlx::query(
        "UPDATE token_sessions SET expires_at = NOW() - INTERVAL '1 minute' WHERE token_hash = $1",
    )
    .bind(&expired_session_hash)
    .execute(pool)
    .await?;
    assert_eq!(
        protected_request(&app, &valid_with_expired_session)
            .await?
            .0,
        StatusCode::UNAUTHORIZED,
        "a valid JWT backed only by an expired database session must not reach bills"
    );

    let unavailable = build_router(HttpAppState::new(
        HttpShellConfig::default()
            .with_auth_jwt_secret(JWT_SECRET)
            .with_postgres_url("postgres://bill_analyser:invalid@127.0.0.1:9/unavailable")?,
    )?);
    let started = Instant::now();
    assert_eq!(
        protected_request(&unavailable, &valid_without_session)
            .await?
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(started.elapsed() <= std::time::Duration::from_secs(1));

    test_db.cleanup().await?;
    Ok(())
}

async fn protected_request(
    app: &axum::Router,
    access_token: &str,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body = serde_json::from_slice(&bytes)?;
    Ok((status, body))
}

fn authority_state(database: &str) -> Result<HttpAppState, Box<dyn Error>> {
    let mut postgres_url = url::Url::parse(&env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?)?;
    postgres_url.set_path(&format!("/{database}"));
    Ok(HttpAppState::new(
        HttpShellConfig::default()
            .with_auth_jwt_secret(JWT_SECRET)
            .with_postgres_url(postgres_url.to_string())?,
    )?)
}

async fn insert_user(pool: &PostgresPool, name: &str) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(name)
            .bind(format!("{name}@example.test"))
            .fetch_one(pool)
            .await?
            .try_get("id")?,
    )
}

async fn create_session(
    pool: &PostgresPool,
    user_id: i64,
    token_hash: &str,
) -> Result<(), Box<dyn Error>> {
    let now = Utc::now();
    create_postgres_token_session(
        pool,
        &CreateTokenSessionDraft {
            user_id: UserId::new(u64::try_from(user_id)?)?,
            token_hash: token_hash.to_string(),
            refresh_token_hash: None,
            expires_at: (now + ChronoDuration::minutes(10)).to_rfc3339(),
            refresh_expires_at: None,
            user_agent: "auth-session-authority-test".to_string(),
            ip_address: "127.0.0.1".to_string(),
            created_at: now.to_rfc3339(),
        },
    )
    .await?;
    Ok(())
}

fn signed_access_token(user_id: i64, expires_in: ChronoDuration) -> String {
    let now = Utc::now();
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "type": "access",
        "iat": now.timestamp(),
        "exp": (now + expires_in).timestamp(),
    });
    let encoded_header =
        general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header JSON"));
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("payload JSON"));
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac::HMAC_SHA256, JWT_SECRET.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    format!(
        "{signing_input}.{}",
        general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref())
    )
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
