use std::{error::Error, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use bill_analyser_db::PostgresPool;
use bill_analyser_http::{bill_runtime_router, HttpAppState, HttpShellConfig};
use serde_json::{json, Value};
use sqlx::Row;
use tower::ServiceExt;

#[path = "../db/postgres_test_support.rs"]
mod postgres_test_support;

const TRUST_SECRET: &str = "ledger-list-contract-secret";

#[tokio::test]
async fn get_bills_preserves_legacy_rest_contract_over_typed_ledger_boundary(
) -> Result<(), Box<dyn Error>> {
    let test_db = postgres_test_support::isolated_postgres_database("ledger_list_http")
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for Ledger REST contract tests",
            )
        })?;
    let user_id = insert_user(&test_db.pool, "ledger-http-user").await?;
    let other_user_id = insert_user(&test_db.pool, "ledger-http-other").await?;
    let account_id = insert_account(&test_db.pool, user_id, "现金钱包").await?;
    let other_account_id = insert_account(&test_db.pool, other_user_id, "其他钱包").await?;
    let category_id = insert_category(&test_db.pool, user_id, "咖啡", "餐饮/咖啡").await?;
    let other_category_id =
        insert_category(&test_db.pool, other_user_id, "咖啡", "餐饮/咖啡").await?;
    let tag_id = insert_tag(&test_db.pool, user_id, "咖啡标签").await?;
    let bill_id = insert_bill(
        &test_db.pool,
        user_id,
        account_id,
        category_id,
        "Cafe Alpha",
        12_345,
    )
    .await?;
    let other_bill_id = insert_bill(
        &test_db.pool,
        other_user_id,
        other_account_id,
        other_category_id,
        "Cafe Other",
        12_345,
    )
    .await?;
    sqlx::query("INSERT INTO bill_tags (user_id, bill_id, tag_id) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(bill_id)
        .bind(tag_id)
        .execute(&test_db.pool)
        .await?;

    let app = bill_runtime_router().with_state(state_for_database(&test_db.db_name)?);
    let request = Request::builder()
        .uri(format!(
            "/api/bills?page=1&pageSize=10&categoryIds={category_id}&tagIds={tag_id}&amountFilterCents=between:12000:13000&keyword=Cafe"
        ))
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TRUST_SECRET)
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;

    assert_eq!(
        payload.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["result", "success"]
    );
    assert_eq!(payload["success"], true);
    let result = &payload["result"];
    assert_eq!(result["totalCount"], 1);
    assert_eq!(result["total"], 1);
    assert_eq!(result["page"], 1);
    assert_eq!(result["pageSize"], 10);
    assert_eq!(result["page_size"], 10);
    assert_eq!(result["total_pages"], 1);
    let item = &result["items"][0];
    assert_eq!(item["id"], bill_id.to_string());
    assert_ne!(item["id"], other_bill_id.to_string());
    assert_eq!(item["type"], 3);
    assert_eq!(item["categoryId"], category_id.to_string());
    assert_eq!(item["categoryName"], "餐饮");
    assert_eq!(item["subCategoryName"], "咖啡");
    assert_eq!(item["amountCents"], 12_345);
    assert_eq!(item["sourceAmountCents"], 12_345);
    assert_eq!(item["destinationAmountCents"], 12_345);
    assert_eq!(item["sourceAccountId"], account_id.to_string());
    assert_eq!(item["tagIds"], json!([tag_id.to_string()]));
    assert_eq!(
        item["tags"],
        json!([{"id": tag_id.to_string(), "name": "咖啡标签"}])
    );
    assert_eq!(item["comment"], "REST typed ledger boundary");

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn get_bills_by_month_returns_the_complete_bounded_month_with_tags_and_user_scope(
) -> Result<(), Box<dyn Error>> {
    let test_db = postgres_test_support::isolated_postgres_database("ledger_month_http")
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for monthly Ledger REST contracts",
            )
        })?;
    let user_id = insert_user(&test_db.pool, "ledger-month-user").await?;
    let other_user_id = insert_user(&test_db.pool, "ledger-month-other").await?;
    let account_id = insert_account(&test_db.pool, user_id, "月度钱包").await?;
    let other_account_id = insert_account(&test_db.pool, other_user_id, "其他月度钱包").await?;
    let category_id = insert_category(&test_db.pool, user_id, "月度分类", "日常/月度").await?;
    let other_category_id =
        insert_category(&test_db.pool, other_user_id, "其他分类", "日常/其他").await?;
    let tag_id = insert_tag(&test_db.pool, user_id, "月度标签").await?;

    insert_month_bills_with_tag(&test_db.pool, user_id, account_id, category_id, tag_id, 501)
        .await?;
    insert_bill(
        &test_db.pool,
        other_user_id,
        other_account_id,
        other_category_id,
        "Other Month Bill",
        99_999,
    )
    .await?;

    let app = bill_runtime_router().with_state(state_for_database(&test_db.db_name)?);
    let request = Request::builder()
        .uri(format!(
            "/api/bills/by-month?year=2026&month=4&type=0&categoryIds={category_id}&accountIds={account_id}&tagIds={tag_id}&tagFilterType=0&amountFilterCents=between:1001:1501&keyword=Month"
        ))
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TRUST_SECRET)
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    let result = &payload["result"];
    let items = result["items"].as_array().expect("monthly ledger items");

    assert_eq!(result["totalCount"], 501);
    assert_eq!(result["total"], 501);
    assert_eq!(result["page"], 1);
    assert_eq!(result["pageSize"], 100_000);
    assert_eq!(result["page_size"], 100_000);
    assert_eq!(result["total_pages"], 1);
    assert_eq!(
        items.len(),
        501,
        "a bounded month must not be silently truncated"
    );
    assert_eq!(items.first().unwrap()["amountCents"], 1_501);
    assert_eq!(items.last().unwrap()["amountCents"], 1_001);
    assert!(items.iter().all(|item| {
        item["tagIds"] == json!([tag_id.to_string()])
            && item["tags"] == json!([{"id": tag_id.to_string(), "name": "月度标签"}])
    }));

    test_db.cleanup().await?;
    Ok(())
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
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type, currency, balance_cents) VALUES ($1, $2, 'cash', 'CNY', 0) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_category(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
    path: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO categories (user_id, name, category_type, path) VALUES ($1, $2, 'expense', $3) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(path)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_tag(pool: &PostgresPool, user_id: i64, name: &str) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO tags (user_id, name, color, metadata) VALUES ($1, $2, '#663300', '{}'::jsonb) RETURNING id",
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
    category_id: i64,
    merchant: &str,
    amount_cents: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, category_id, merchant, description,
            payment_method, source_hash, standard_payload
        )
        VALUES ($1, '2026-04-02T01:00:00Z', 'expense', 'expense', $2, $3, $3, $4, $5,
                'REST typed ledger boundary', '现金', $6, '{}'::jsonb)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(amount_cents)
    .bind(account_id)
    .bind(category_id)
    .bind(merchant)
    .bind(format!("ledger-http-{user_id}-{merchant}"))
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_month_bills_with_tag(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
    category_id: i64,
    tag_id: i64,
    count: i64,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, category_id, merchant, description,
            payment_method, source_hash, standard_payload
        )
        SELECT $1,
               '2026-04-01T00:00:00Z'::TIMESTAMPTZ + value * INTERVAL '1 minute',
               'expense', 'expense', 1000 + value,
               $2, $2, $3, 'Month Merchant ' || value,
               'Monthly typed ledger boundary', '现金',
               'ledger-month-' || $1 || '-' || value, '{}'::JSONB
        FROM generate_series(1, $4) AS value
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(category_id)
    .bind(count)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO bill_tags (user_id, bill_id, tag_id)
        SELECT user_id, id, $2
        FROM bills
        WHERE user_id = $1 AND source_hash LIKE 'ledger-month-%'
        "#,
    )
    .bind(user_id)
    .bind(tag_id)
    .execute(pool)
    .await?;
    Ok(())
}
