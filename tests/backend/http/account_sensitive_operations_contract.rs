use std::{error::Error, process::Command, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use bill_analyser_db::PostgresPool;
use bill_analyser_http::{taxonomy_runtime_router, HttpAppState, HttpShellConfig};
use serde_json::{json, Value};
use sqlx::Row;
use tower::ServiceExt;

#[path = "../db/postgres_test_support.rs"]
mod postgres_test_support;

const TRUST_SECRET: &str = "account-sensitive-operation-contract-secret";
const OPERATION_PASSWORD_ENV: &str = "BILL_ANALYSER_OPERATION_PASSWORD";

#[tokio::test]
async fn wrong_current_password_without_operation_password_fails_closed(
) -> Result<(), Box<dyn Error>> {
    if rerun_without_operation_password_env_if_configured(
        "wrong_current_password_without_operation_password_fails_closed",
    )? {
        return Ok(());
    }
    let test_db = required_isolated_postgres("account_sensitive_fail_closed").await?;
    let user_id = insert_user_with_password(
        &test_db.pool,
        "account-sensitive-fail-closed",
        "correct-current-password",
    )
    .await?;
    let source_account_id = insert_account(&test_db.pool, user_id, "来源账户").await?;
    let target_account_id = insert_account(&test_db.pool, user_id, "目标账户").await?;
    let bill_id = insert_bill(&test_db.pool, user_id, source_account_id).await?;
    let app = taxonomy_runtime_router().with_state(state_for_database(&test_db.db_name)?);

    let (clear_status, clear_body) = request_json(
        app.clone(),
        format!("/api/accounts/{source_account_id}/transactions/clear"),
        user_id,
        json!({"password": "wrong-current-password"}),
    )
    .await?;
    assert_eq!(clear_status, StatusCode::UNAUTHORIZED);
    assert_eq!(clear_body["error"], "Invalid password");

    let (move_status, move_body) = request_json(
        app,
        format!("/api/accounts/{source_account_id}/transactions/move"),
        user_id,
        json!({
            "password": "wrong-current-password",
            "toAccountId": target_account_id
        }),
    )
    .await?;
    assert_eq!(move_status, StatusCode::UNAUTHORIZED);
    assert_eq!(move_body["error"], "Invalid password");

    let bill = sqlx::query("SELECT account_id, is_deleted FROM bills WHERE id = $1")
        .bind(bill_id)
        .fetch_one(&test_db.pool)
        .await?;
    assert_eq!(bill.try_get::<i64, _>("account_id")?, source_account_id);
    assert!(!bill.try_get::<bool, _>("is_deleted")?);

    let failed_audits: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'account'
          AND action IN ('delete_transactions', 'move_transactions')
          AND metadata->>'status' = 'failed'
          AND metadata->>'error_message' = 'Invalid password'
        "#,
    )
    .bind(user_id)
    .fetch_one(&test_db.pool)
    .await?;
    assert_eq!(failed_audits, 2);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn legacy_object_operation_password_still_authorizes_the_account_operation(
) -> Result<(), Box<dyn Error>> {
    if rerun_without_operation_password_env_if_configured(
        "legacy_object_operation_password_still_authorizes_the_account_operation",
    )? {
        return Ok(());
    }
    let test_db = required_isolated_postgres("account_sensitive_legacy_password").await?;
    let user_id = insert_user_with_password(
        &test_db.pool,
        "account-sensitive-legacy-password",
        "correct-current-password",
    )
    .await?;
    let account_id = insert_account(&test_db.pool, user_id, "旧操作密码账户").await?;
    sqlx::query("INSERT INTO settings (user_id, key, value) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("operation_password")
        .bind(json!({"value": "legacy-operation-password"}))
        .execute(&test_db.pool)
        .await?;
    let app = taxonomy_runtime_router().with_state(state_for_database(&test_db.db_name)?);

    let (status, body) = request_json(
        app,
        format!("/api/accounts/{account_id}/transactions/clear"),
        user_id,
        json!({"password": "legacy-operation-password"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["result"], true);

    let successful_audit: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'account'
          AND action = 'delete_transactions'
          AND metadata->>'status' = 'success'
        "#,
    )
    .bind(user_id)
    .fetch_one(&test_db.pool)
    .await?;
    assert_eq!(successful_audit, 1);

    test_db.cleanup().await?;
    Ok(())
}

async fn request_json(
    app: axum::Router,
    path: String,
    user_id: i64,
    payload: Value,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let request = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TRUST_SECRET)
        .header("user-agent", "account-sensitive-operation-contract")
        .body(Body::from(serde_json::to_vec(&payload)?))?;
    let response = app.oneshot(request).await?;
    let status = response.status();
    let body = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    Ok((status, body))
}

async fn required_isolated_postgres(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for account sensitive operation tests"
                .into()
        })
}

fn rerun_without_operation_password_env_if_configured(
    test_name: &str,
) -> Result<bool, Box<dyn Error>> {
    if std::env::var(OPERATION_PASSWORD_ENV)
        .ok()
        .is_none_or(|value| value.is_empty())
    {
        return Ok(false);
    }

    let status = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env_remove(OPERATION_PASSWORD_ENV)
        .status()?;
    if !status.success() {
        return Err(format!(
            "account operation contract child process failed for {test_name}: {status}"
        )
        .into());
    }
    Ok(true)
}

fn state_for_database(database: &str) -> Result<HttpAppState, Box<dyn Error>> {
    let base_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
    let mut url = url::Url::parse(&base_url)?;
    url.set_path(&format!("/{database}"));
    let config = HttpShellConfig::new("", Duration::from_secs(2), 1024 * 1024)?
        .with_postgres_url(url.to_string())?
        .with_trusted_user_header_secret(TRUST_SECRET);
    Ok(HttpAppState::new(config)?)
}

async fn insert_user_with_password(
    pool: &PostgresPool,
    username: &str,
    password: &str,
) -> Result<i64, Box<dyn Error>> {
    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
    Ok(sqlx::query(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(username)
    .bind(format!("{username}@example.test"))
    .bind(password_hash)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_account(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type, currency) VALUES ($1, $2, 'cash', 'CNY') RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_bill(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, description, source_hash, standard_payload
        )
        VALUES ($1, now(), 'expense', 'expense', 100, $2, $2, $3, $4, '{}'::jsonb)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind("account-sensitive-operation-contract")
    .bind(format!("account-sensitive-{user_id}-{account_id}"))
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}
