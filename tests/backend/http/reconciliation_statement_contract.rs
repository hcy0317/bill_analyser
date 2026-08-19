use std::{error::Error, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use bill_analyser_db::PostgresPool;
use bill_analyser_http::{bill_runtime_router, HttpAppState, HttpShellConfig};
use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate, TimeZone};
use serde_json::Value;
use sqlx::Row;
use tower::ServiceExt;

#[path = "../db/postgres_test_support.rs"]
mod postgres_test_support;

const TRUST_SECRET: &str = "reconciliation-statement-contract-secret";

#[tokio::test]
async fn bounded_reconciliation_replays_start_day_only_in_current_window(
) -> Result<(), Box<dyn Error>> {
    let test_db = postgres_test_support::isolated_postgres_database("reconciliation_window_http")
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for reconciliation REST contract tests",
            )
        })?;
    let user_id = insert_user(&test_db.pool, "reconciliation-window-user").await?;
    let account_id = insert_account(&test_db.pool, user_id, "对账账户", 1_000).await?;
    let other_user_id = insert_user(&test_db.pool, "reconciliation-window-other").await?;
    let other_account_id =
        insert_account(&test_db.pool, other_user_id, "其他用户账户", 50_000).await?;
    let start_time = local_start_time()?;
    let start_date = local_date(start_time)?;
    let previous_date = previous_date(&start_date)?;

    insert_bill(
        &test_db.pool,
        user_id,
        account_id,
        "expense",
        100,
        &previous_date,
        "previous-expense",
    )
    .await?;
    let start_bill_id = insert_bill(
        &test_db.pool,
        user_id,
        account_id,
        "income",
        200,
        &start_date,
        "start-income",
    )
    .await?;
    insert_bill(
        &test_db.pool,
        other_user_id,
        other_account_id,
        "income",
        99_999,
        &start_date,
        "other-user-income",
    )
    .await?;

    let payload = get_reconciliation_statement(
        &test_db.db_name,
        user_id,
        account_id,
        start_time,
        start_time,
    )
    .await?;

    assert_eq!(payload["success"], true);
    let result = &payload["result"];
    assert_eq!(result["openingBalanceCents"], 900);
    assert_eq!(result["closingBalanceCents"], 1_100);
    assert_eq!(result["totalInflowsCents"], 200);
    assert_eq!(result["totalOutflowsCents"], 0);
    assert_eq!(result["itemCount"], 1);
    let transactions = result["transactions"].as_array().unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0]["id"], start_bill_id.to_string());
    assert_eq!(transactions[0]["amountCents"], 200);
    assert_eq!(transactions[0]["accountOpeningBalanceCents"], 900);
    assert_eq!(transactions[0]["accountClosingBalanceCents"], 1_100);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn bounded_reconciliation_without_prior_bills_preserves_account_initial_balance(
) -> Result<(), Box<dyn Error>> {
    let test_db = postgres_test_support::isolated_postgres_database("reconciliation_initial_http")
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for reconciliation REST contract tests",
            )
        })?;
    let user_id = insert_user(&test_db.pool, "reconciliation-initial-user").await?;
    let account_id = insert_account(&test_db.pool, user_id, "初始余额账户", 700).await?;
    let start_time = local_start_time()?;
    let start_date = local_date(start_time)?;
    insert_bill(
        &test_db.pool,
        user_id,
        account_id,
        "expense",
        50,
        &start_date,
        "start-expense",
    )
    .await?;

    let payload = get_reconciliation_statement(
        &test_db.db_name,
        user_id,
        account_id,
        start_time,
        start_time,
    )
    .await?;

    assert_eq!(payload["success"], true);
    let result = &payload["result"];
    assert_eq!(result["openingBalanceCents"], 700);
    assert_eq!(result["closingBalanceCents"], 650);
    assert_eq!(result["totalInflowsCents"], 0);
    assert_eq!(result["totalOutflowsCents"], 50);
    assert_eq!(result["itemCount"], 1);

    test_db.cleanup().await?;
    Ok(())
}

async fn get_reconciliation_statement(
    database: &str,
    user_id: i64,
    account_id: i64,
    start_time: i64,
    end_time: i64,
) -> Result<Value, Box<dyn Error>> {
    let app = bill_runtime_router().with_state(state_for_database(database)?);
    let request = Request::builder()
        .uri(format!(
            "/api/bills/reconciliation_statements?account_id={account_id}&start_time={start_time}&end_time={end_time}"
        ))
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TRUST_SECRET)
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX).await?,
    )?)
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

fn local_date(timestamp: i64) -> Result<String, Box<dyn Error>> {
    let date = DateTime::from_timestamp(timestamp, 0)
        .ok_or("invalid test timestamp")?
        .with_timezone(&Local)
        .date_naive();
    Ok(date.format("%Y-%m-%d").to_string())
}

fn local_start_time() -> Result<i64, Box<dyn Error>> {
    Local
        .with_ymd_and_hms(2026, 4, 2, 12, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .ok_or_else(|| "invalid local reconciliation test datetime".into())
}

fn previous_date(date: &str) -> Result<String, Box<dyn Error>> {
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")? - ChronoDuration::days(1);
    Ok(date.format("%Y-%m-%d").to_string())
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

async fn insert_account(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
    initial_balance_cents: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type, currency, balance_cents, metadata) VALUES ($1, $2, 'cash', 'CNY', $3, jsonb_build_object('initial_balance_cents', $3::bigint)) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(initial_balance_cents)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_bill(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
    transaction_type: &str,
    amount_cents: i64,
    occurred_on: &str,
    description: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, description, source_hash, standard_payload
        )
        VALUES ($1, ($6 || ' 12:00:00+00')::timestamptz, $3, $3, $4,
                $2, $2, $7, $5, '{}'::jsonb)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(transaction_type)
    .bind(amount_cents)
    .bind(format!("reconciliation-{user_id}-{description}"))
    .bind(occurred_on)
    .bind(description)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}
