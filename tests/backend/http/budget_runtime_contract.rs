use std::{env, error::Error, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{Method, StatusCode},
    Router,
};
use bill_analyser_db::run_postgres_migrations;
use bill_analyser_http::{
    build_router, config::DatabaseBackend, HttpAppState, HttpShellConfig, ImportRouteMode,
    BUDGET_CRUD_ROUTE_PATTERNS,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, Row};
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "budget-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn budgets_postgres_runtime_serves_list_without_sqlite_fallback() -> Result<(), Box<dyn Error>>
{
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("budget-pg-{unique}"))
            .bind(format!("budget-pg-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, icon, color)
        VALUES ($1, '餐饮', '1', '餐饮', 'folder', '#ffaa00'),
               ($1, '午餐', '3', '餐饮/午餐', 'tag', '#ffaa00')
        "#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type, amount_cents,
            start_date, end_date, alert_threshold, enabled
        )
        VALUES ($1, '午餐预算', '餐饮', '午餐', 'monthly', 10000, '2026-03-01', '2026-03-31', 75, true),
               ($1, '年度预算', '餐饮', '', 'yearly', 500000, '2026-01-01', '2026-12-31', 80, true)
        "#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?;

    let app = postgres_runtime_router(&postgres_url)?;
    let response = app
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/budgets/?budget_type=3&period_type=monthly",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    let items = body["result"].as_array().expect("postgres budget list");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "午餐预算");
    assert_eq!(items[0]["amount"], 100.0);
    assert_eq!(items[0]["enabled"], 1);
    assert_eq!(items[0]["type"], 3);
    assert!(!items[0]["category_id"]
        .as_str()
        .unwrap_or_default()
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn budgets_postgres_runtime_serves_crud_export_and_import_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("budget-pg-crud-{unique}"))
            .bind(format!("budget-pg-crud-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, icon, color)
        VALUES ($1, '餐饮', '1', '餐饮', 'folder', '#ffaa00'),
               ($1, '午餐', '3', '餐饮/午餐', 'tag', '#ffaa00'),
               ($1, '交通', '3', '交通', 'bus', '#00aaff')
        "#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type, amount_cents,
            start_date, end_date, alert_threshold, enabled
        )
        VALUES ($1, '导入既有预算', '交通', '', 'monthly', 5000, '2026-03-01', '2026-03-31', 80, true)
        "#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?;

    let app = postgres_runtime_router(&postgres_url)?;
    let create_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::POST,
            "/api/budgets/",
            Body::from(
                json!({
                    "name": "午餐预算",
                    "category": "餐饮",
                    "sub_category": "午餐",
                    "period_type": "monthly",
                    "amount": "123.45",
                    "start_date": "2026-03-01",
                    "end_date": "2026-03-31",
                    "alert_threshold": 75,
                    "enabled": true
                })
                .to_string(),
            ),
            user_id,
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = read_json(create_response).await;
    let budget_id = created["result"]["id"].as_i64().expect("created budget id");
    assert_eq!(created["result"]["amount"], 123.45);
    assert_eq!(created["result"]["sub_category"], "午餐");
    assert_eq!(
        postgres_budget_amount_cents(&pool, user_id, "午餐预算", "午餐").await?,
        12345
    );
    assert_eq!(
        postgres_budget_amount_cents(&pool, user_id, "午餐预算", "").await?,
        12345
    );

    let update_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::PUT,
            &format!("/api/budgets/{budget_id}"),
            Body::from(json!({ "amount": "150.25" }).to_string()),
            user_id,
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    assert_eq!(
        postgres_budget_amount_cents(&pool, user_id, "午餐预算", "午餐").await?,
        15025
    );
    assert_eq!(
        postgres_budget_amount_cents(&pool, user_id, "午餐预算", "").await?,
        15025
    );

    let export_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/budgets/export",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(export_response.status(), StatusCode::OK);
    let exported = read_json(export_response).await;
    let exported_rows = exported["result"].as_array().expect("export rows");
    assert!(exported_rows
        .iter()
        .any(|row| row["name"] == "午餐预算" && row["amount"] == 150.25));

    let delete_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/budgets/{budget_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(
        postgres_budget_row_count(&pool, user_id, "午餐预算").await?,
        0
    );
    assert_eq!(
        postgres_budget_row_count(&pool, user_id, "导入既有预算").await?,
        1
    );

    let import_response = app
        .oneshot(authed_request_for_user(
            Method::POST,
            "/api/budgets/import",
            Body::from(
                json!([
                    {
                        "name": "导入既有预算",
                        "category": "交通",
                        "period_type": "monthly",
                        "amount": "66.66",
                        "start_date": "2026-03-01",
                        "end_date": "2026-03-31",
                        "alert_threshold": 70,
                        "enabled": false
                    },
                    {
                        "name": "导入新增预算",
                        "category": "餐饮",
                        "sub_category": "午餐",
                        "period_type": "monthly",
                        "amount": 88.88,
                        "start_date": "2026-03-01"
                    }
                ])
                .to_string(),
            ),
            user_id,
        ))
        .await?;
    assert_eq!(import_response.status(), StatusCode::OK);
    let imported = read_json(import_response).await;
    assert_eq!(imported["result"]["created"], 1);
    assert_eq!(imported["result"]["updated"], 1);
    assert_eq!(imported["result"]["errors"], 0);
    assert_eq!(
        postgres_budget_amount_cents(&pool, user_id, "导入既有预算", "").await?,
        6666
    );
    assert!(!postgres_budget_enabled(&pool, user_id, "导入既有预算").await?);
    assert_eq!(
        postgres_budget_amount_cents(&pool, user_id, "导入新增预算", "午餐").await?,
        8888
    );

    Ok(())
}

#[tokio::test]
async fn budgets_postgres_runtime_serves_execution_forecast_and_history_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("budget-pg-exec-{unique}"))
            .bind(format!("budget-pg-exec-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let account_primary: i64 = sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type) VALUES ($1, $2, 'cash') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("budget-card-a-{unique}"))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let account_secondary: i64 = sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type) VALUES ($1, $2, 'cash') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("budget-card-b-{unique}"))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let tag_scope: i64 =
        sqlx::query("INSERT INTO tags (user_id, name) VALUES ($1, $2) RETURNING id")
            .bind(user_id)
            .bind(format!("budget-scope-{unique}"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let tag_other: i64 =
        sqlx::query("INSERT INTO tags (user_id, name) VALUES ($1, $2) RETURNING id")
            .bind(user_id)
            .bind(format!("budget-other-{unique}"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, icon, color)
        VALUES ($1, '餐饮', '1', '餐饮', 'folder', '#ffaa00')
        "#,
    )
    .bind(user_id)
    .execute(&pool)
    .await?;
    let lunch_category_id: i64 = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, icon, color)
        VALUES ($1, '午餐', '3', '餐饮/午餐', 'tag', '#ffaa00')
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let transport_category_id: i64 = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, icon, color)
        VALUES ($1, '交通', '3', '交通', 'bus', '#00aaff')
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let primary_budget_id: i64 = sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type, amount_cents,
            start_date, end_date, alert_threshold, enabled
        )
        VALUES ($1, '午餐预算', '餐饮', '', 'monthly', 10000, '2026-03-01', '2026-03-31', 75, true)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let lunch_budget_id: i64 = sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type, amount_cents,
            start_date, end_date, alert_threshold, enabled
        )
        VALUES ($1, '午餐预算', '餐饮', '午餐', 'monthly', 10000, '2026-03-01', '2026-03-31', 75, true)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;

    let lunch_payload = json!({"main_category": "餐饮", "sub_category": "午餐", "type": "支出"});
    let bill_one = insert_postgres_budget_bill(
        &pool,
        user_id,
        "2026-03-15T12:00:00Z",
        3550,
        account_primary,
        lunch_category_id,
        &lunch_payload,
    )
    .await?;
    let bill_two = insert_postgres_budget_bill(
        &pool,
        user_id,
        "2026-03-31T23:00:00Z",
        1500,
        account_secondary,
        lunch_category_id,
        &lunch_payload,
    )
    .await?;
    let bill_outside = insert_postgres_budget_bill(
        &pool,
        user_id,
        "2026-04-01T00:00:00Z",
        900,
        account_primary,
        lunch_category_id,
        &lunch_payload,
    )
    .await?;
    for (bill_id, tag_id) in [
        (bill_one, tag_scope),
        (bill_two, tag_other),
        (bill_outside, tag_scope),
    ] {
        sqlx::query("INSERT INTO bill_tags (bill_id, tag_id, user_id) VALUES ($1, $2, $3)")
            .bind(bill_id)
            .bind(tag_id)
            .bind(user_id)
            .execute(&pool)
            .await?;
    }
    insert_postgres_budget_bill(
        &pool,
        user_id,
        "2026-01-15T12:00:00Z",
        1000,
        account_primary,
        lunch_category_id,
        &lunch_payload,
    )
    .await?;
    insert_postgres_budget_bill(
        &pool,
        user_id,
        "2026-02-15T12:00:00Z",
        2000,
        account_primary,
        lunch_category_id,
        &lunch_payload,
    )
    .await?;
    for occurred_at in [
        "2026-01-10T08:00:00Z",
        "2026-02-10T08:00:00Z",
        "2026-03-10T08:00:00Z",
    ] {
        insert_postgres_budget_bill(
            &pool,
            user_id,
            occurred_at,
            500,
            account_primary,
            transport_category_id,
            &json!({"main_category": "交通", "sub_category": "", "type": "支出"}),
        )
        .await?;
    }

    let app = postgres_runtime_router(&postgres_url)?;
    let execution_uri = format!(
        "/api/budgets/execution?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&account_ids={account_primary}&tag_ids={tag_scope}"
    );
    let execution_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &execution_uri,
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(execution_response.status(), StatusCode::OK);
    let execution_body = read_json(execution_response).await;
    assert_eq!(execution_body["success"], true);
    let execution_items = execution_body["result"]["items"]
        .as_array()
        .expect("execution items");
    let lunch_execution = execution_items
        .iter()
        .find(|item| item["id"] == lunch_budget_id)
        .expect("lunch execution item");
    assert_eq!(lunch_execution["spent_amount"], 35.5);
    assert_eq!(lunch_execution["remaining_amount"], 64.5);
    assert_eq!(
        lunch_execution["category_id"],
        lunch_category_id.to_string()
    );
    assert!(execution_items
        .iter()
        .any(|item| item["id"] == primary_budget_id && item["spent_amount"] == 35.5));

    let category_filtered_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &format!(
                "/api/budgets/execution?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&category_id={lunch_category_id}"
            ),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(category_filtered_response.status(), StatusCode::OK);
    let category_filtered_body = read_json(category_filtered_response).await;
    let category_filtered_items = category_filtered_body["result"]["items"]
        .as_array()
        .expect("category-filtered execution items");
    assert_eq!(category_filtered_items.len(), 1);
    assert_eq!(category_filtered_items[0]["id"], lunch_budget_id);

    let missing_category_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/budgets/execution?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&category_id=999999999",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(missing_category_response.status(), StatusCode::OK);
    let missing_category_body = read_json(missing_category_response).await;
    assert_eq!(
        missing_category_body["result"]["items"]
            .as_array()
            .expect("missing category items")
            .len(),
        0
    );

    let forecast_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/budgets/forecast?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&months_history=3&forecast_strategy=historical_average",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(forecast_response.status(), StatusCode::OK);
    let forecast_body = read_json(forecast_response).await;
    assert_eq!(forecast_body["result"]["summary"]["history_periods"], 3);
    let forecast_items = forecast_body["result"]["items"]
        .as_array()
        .expect("forecast items");
    assert_eq!(forecast_items[0]["category"], "餐饮");
    assert_eq!(forecast_items[0]["budget_amount"], 100.0);
    assert_eq!(forecast_items[0]["forecast_amount"], 26.83);
    assert!(forecast_items
        .iter()
        .any(|item| item["category"] == "交通" && item["forecast_amount"] == 5.0));

    let on_demand_history_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/budgets/history?period_type=monthly&start_date=2026-02-01&end_date=2026-03-31",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(on_demand_history_response.status(), StatusCode::OK);
    let on_demand_history_body = read_json(on_demand_history_response).await;
    let on_demand_items = on_demand_history_body["result"]["items"]
        .as_array()
        .expect("on-demand history items");
    assert!(on_demand_items
        .iter()
        .any(|item| item["budget_id"] == lunch_budget_id
            && item["period_start"] == "2026-03-01"
            && item["period_end"] == "2026-03-31"));

    let snapshot_payload = json!({
        "budget_type": 3,
        "period_type": "monthly",
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "account_ids": [account_primary],
        "tag_ids": [tag_scope]
    });
    let snapshot_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::POST,
            "/api/budgets/history/snapshot",
            Body::from(snapshot_payload.to_string()),
            user_id,
        ))
        .await?;
    assert_eq!(snapshot_response.status(), StatusCode::OK);
    let snapshot_body = read_json(snapshot_response).await;
    assert_eq!(snapshot_body["result"]["created_count"], 2);

    let history_uri = format!(
        "/api/budgets/history?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&account_ids={account_primary}&tag_ids={tag_scope}"
    );
    let history_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &history_uri,
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(history_response.status(), StatusCode::OK);
    let history_body = read_json(history_response).await;
    let history_items = history_body["result"]["items"]
        .as_array()
        .expect("history items");
    let stored_lunch = history_items
        .iter()
        .find(|item| item["budget_id"] == lunch_budget_id)
        .expect("stored lunch history");
    assert_eq!(stored_lunch["spent_amount"], 35.5);
    assert_eq!(stored_lunch["status"], "within_budget");

    let budget_filtered_history_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &format!("{history_uri}&budget_id={lunch_budget_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(budget_filtered_history_response.status(), StatusCode::OK);
    let budget_filtered_history_body = read_json(budget_filtered_history_response).await;
    let budget_filtered_items = budget_filtered_history_body["result"]["items"]
        .as_array()
        .expect("budget-filtered history items");
    assert_eq!(budget_filtered_items.len(), 1);
    assert_eq!(budget_filtered_items[0]["budget_id"], lunch_budget_id);

    let appended_bill = insert_postgres_budget_bill(
        &pool,
        user_id,
        "2026-03-20T12:00:00Z",
        4000,
        account_primary,
        lunch_category_id,
        &lunch_payload,
    )
    .await?;
    sqlx::query("INSERT INTO bill_tags (bill_id, tag_id, user_id) VALUES ($1, $2, $3)")
        .bind(appended_bill)
        .bind(tag_scope)
        .bind(user_id)
        .execute(&pool)
        .await?;

    let resnapshot_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::POST,
            "/api/budgets/history/snapshot",
            Body::from(snapshot_payload.to_string()),
            user_id,
        ))
        .await?;
    assert_eq!(resnapshot_response.status(), StatusCode::OK);
    let updated_history_response = app
        .oneshot(authed_request_for_user(
            Method::GET,
            &history_uri,
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(updated_history_response.status(), StatusCode::OK);
    let updated_history_body = read_json(updated_history_response).await;
    let updated_lunch = updated_history_body["result"]["items"]
        .as_array()
        .expect("updated history items")
        .iter()
        .find(|item| item["budget_id"] == lunch_budget_id)
        .expect("updated lunch history");
    assert_eq!(updated_lunch["spent_amount"], 75.5);
    assert_eq!(
        postgres_budget_history_spent_cents(&pool, user_id, lunch_budget_id).await?,
        7550
    );

    Ok(())
}

#[tokio::test]
async fn budgets_crud_runtime_serves_owned_routes_and_writes_db() -> Result<(), Box<dyn Error>> {
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/budgets/")));
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/budgets/execution")));
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/budgets/forecast")));
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/budgets/history")));
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/budgets/history/snapshot")));
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/budgets/import")));
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/",
            json!({
                "name": "午餐预算",
                "category": "餐饮",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 100.0,
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "alert_threshold": 75,
                "enabled": true
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["result"]["category"], "餐饮");
    let budget_id = create_body["result"]["id"].as_i64().expect("budget id");

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/?budget_type=3&period_type=monthly&category=%E9%A4%90%E9%A5%AE",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let list_items = list_body["result"].as_array().expect("budget list");
    assert_eq!(list_items.len(), 2);
    assert!(list_items
        .iter()
        .any(|item| item["sub_category"] == "午餐" && item["category_id"] == "2"));

    let get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/budgets/{budget_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    assert_eq!(read_json(get_response).await["result"]["name"], "午餐预算");

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/budgets/{budget_id}"),
            json!({
                "amount": 150.0,
                "start_date": "2026-03-01",
                "end_date": "2026-03-31"
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    assert_eq!(read_json(update_response).await["success"], true);
    assert_eq!(budget_amount(&fixture.db_path, budget_id)?, 150.0);

    let export_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(export_response.status(), StatusCode::OK);
    assert!(!read_json(export_response).await["result"]
        .as_array()
        .expect("export rows")
        .is_empty());

    let delete_response = app
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/budgets/{budget_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["success"], true);

    Ok(())
}

#[tokio::test]
async fn budgets_execution_runtime_serves_owned_route_and_reads_db() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/",
            json!({
                "name": "午餐预算",
                "category": "餐饮",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 100.0,
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "alert_threshold": 75,
                "enabled": true
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    seed_execution_bills(&fixture.db_path)?;

    let response = app
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/execution?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&account_ids=10&tag_ids=8",
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["period_start"], "2026-03-01");
    assert_eq!(body["result"]["period_end"], "2026-03-31");
    let items = body["result"]["items"].as_array().expect("execution items");
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .any(|item| item["sub_category"] == "午餐" && item["spent_amount"] == 35.5));
    assert_eq!(body["result"]["summary"]["total_spent"], 35.5);
    assert_eq!(body["runtime"], Value::Null);

    Ok(())
}

#[tokio::test]
async fn budgets_forecast_runtime_serves_owned_route_and_reads_db() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/",
            json!({
                "name": "午餐预算",
                "category": "餐饮",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 100.0,
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "alert_threshold": 75,
                "enabled": true
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    seed_forecast_bills(&fixture.db_path)?;

    let response = app
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/forecast?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&months_history=3&forecast_strategy=historical_average",
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["period_start"], "2026-03-01");
    assert_eq!(body["result"]["periodEnd"], "2026-03-31");
    assert_eq!(body["result"]["summary"]["history_periods"], 3);
    assert_eq!(body["result"]["summary"]["total_forecast"], 21.67);
    let items = body["result"]["items"].as_array().expect("forecast items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["category"], "餐饮");
    assert_eq!(items[0]["forecast_amount"], 21.67);
    assert_eq!(items[0]["current_spent"], 35.0);
    assert_eq!(items[0]["budget_amount"], 100.0);
    assert_eq!(items[0]["periods"][0]["period"], "2026-01");
    assert_eq!(body["runtime"], Value::Null);

    Ok(())
}

#[tokio::test]
async fn budgets_history_snapshot_runtime_serves_owned_routes_and_writes_db(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/",
            json!({
                "name": "午餐预算",
                "category": "餐饮",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 100.0,
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "alert_threshold": 75,
                "enabled": true
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let budget_id = read_json(create_response).await["result"]["id"]
        .as_i64()
        .expect("budget id");
    seed_execution_bills(&fixture.db_path)?;

    let snapshot_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/history/snapshot",
            json!({
                "period_type": "monthly",
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "tag_ids": [8]
            }),
        ))
        .await?;
    assert_eq!(snapshot_response.status(), StatusCode::OK);
    let snapshot_body = read_json(snapshot_response).await;
    assert_eq!(snapshot_body["success"], true);
    assert_eq!(snapshot_body["result"]["created_count"], 2);
    assert!(snapshot_body["result"]["filter_summary"]
        .as_str()
        .unwrap_or_default()
        .contains("\"tag_ids\": [8]"));

    append_matching_bill(&fixture.db_path)?;
    let stored_history_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/history?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&tag_ids=8",
            Body::empty(),
        ))
        .await?;
    assert_eq!(stored_history_response.status(), StatusCode::OK);
    let stored_history_body = read_json(stored_history_response).await;
    assert_eq!(stored_history_body["success"], true);
    assert_eq!(stored_history_body["result"]["summary"]["count"], 2);
    let stored_items = stored_history_body["result"]["items"]
        .as_array()
        .expect("history items");
    let stored_lunch = stored_items
        .iter()
        .find(|item| item["budget_id"] == budget_id)
        .expect("stored lunch row");
    assert_eq!(stored_lunch["spent_amount"], 35.5);
    assert_eq!(stored_lunch["status"], "within_budget");

    let replacement_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/history/snapshot",
            json!({
                "period_type": "monthly",
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "tag_ids": [8]
            }),
        ))
        .await?;
    assert_eq!(replacement_response.status(), StatusCode::OK);

    let updated_history_response = app
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/history?period_type=monthly&start_date=2026-03-01&end_date=2026-03-31&tag_ids=8",
            Body::empty(),
        ))
        .await?;
    assert_eq!(updated_history_response.status(), StatusCode::OK);
    let updated_body = read_json(updated_history_response).await;
    let updated_lunch = updated_body["result"]["items"]
        .as_array()
        .expect("updated history items")
        .iter()
        .find(|item| item["budget_id"] == budget_id)
        .expect("updated lunch row");
    assert_eq!(updated_lunch["spent_amount"], 75.5);
    assert_eq!(updated_body["runtime"], Value::Null);

    Ok(())
}

#[tokio::test]
async fn budgets_import_runtime_serves_owned_route_and_writes_db() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/import",
            json!([
                {
                    "name": "通勤预算",
                    "category": "交通",
                    "sub_category": "地铁",
                    "period_type": "monthly",
                    "amount": 80.0,
                    "start_date": "2026-03-01",
                    "end_date": "2026-03-31"
                },
                {
                    "name": "通勤预算",
                    "category": "交通",
                    "sub_category": "公交",
                    "period_type": "monthly",
                    "amount": 55.0,
                    "start_date": "2026-03-01",
                    "end_date": "2026-03-31",
                    "enabled": false
                },
                {
                    "name": "",
                    "category": "交通",
                    "period_type": "monthly",
                    "amount": 20.0,
                    "start_date": "2026-03-01"
                }
            ]),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["created"], 1);
    assert_eq!(body["result"]["updated"], 1);
    assert_eq!(body["result"]["errors"], 1);
    assert_eq!(
        body["result"]["error_details"][0],
        "第3条: 缺少必填字段(name或amount)"
    );
    assert_eq!(body["runtime"], Value::Null);

    let imported = imported_budget_row(&fixture.db_path, "通勤预算")?;
    assert_eq!(imported.amount, 55.0);
    assert_eq!(imported.sub_category, "公交");
    assert_eq!(imported.alert_threshold, 80);
    assert_eq!(imported.enabled, 0);
    assert_eq!(budget_name_count(&fixture.db_path, "")?, 0);

    Ok(())
}

#[tokio::test]
async fn budgets_runtime_covers_error_edges_and_auth() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    for (method, path, body, status) in [
        (
            Method::POST,
            "/api/budgets/",
            json!({"category": "餐饮", "period_type": "bad", "amount": 1.0, "start_date": "2026-03-01"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/",
            json!({"period_type": "monthly", "amount": 1.0, "start_date": "2026-03-01"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/999",
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            Method::PUT,
            "/api/budgets/999",
            json!({"amount": 1.0}),
            StatusCode::NOT_FOUND,
        ),
        (
            Method::GET,
            "/api/budgets/?budget_type=bad",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/?enabled=maybe",
            Value::Null,
            StatusCode::OK,
        ),
        (
            Method::GET,
            "/api/budgets/execution?year=bad",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/execution?budget_id=bad",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/execution?month=bad",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/execution?account_ids=1,bad",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/forecast?months_history=0",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/budgets/history?account_ids=1,bad",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/history/snapshot",
            json!(["not", "object"]),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/history/snapshot",
            json!({"tag_ids": "8"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/import",
            json!({"name": "not-array"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/import",
            json!(["not-object"]),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/import",
            json!([{"name": "预算", "period_type": "monthly", "amount": 1.0, "start_date": "2026-03-01"}]),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/import",
            json!([{"name": "预算", "category": "餐饮", "period_type": "bad", "amount": 1.0, "start_date": "2026-03-01"}]),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/budgets/",
            json!({"category": "餐饮", "period_type": "monthly", "amount": 1.0}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::PUT,
            "/api/budgets/999",
            json!({"period_type": true, "start_date": 20260301}),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let request = if body.is_null() {
            authed_request(method, path, Body::empty())
        } else {
            json_request(method, path, body)
        };
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), status, "{path}");
    }

    let empty_payload_response = app
        .clone()
        .oneshot(json_request(Method::POST, "/api/budgets/", json!({})))
        .await?;
    assert_eq!(empty_payload_response.status(), StatusCode::BAD_REQUEST);

    let malformed_import_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/budgets/import",
            Body::from("[{\"name\":"),
        ))
        .await?;
    assert_eq!(malformed_import_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(malformed_import_response).await["error"],
        "Invalid data format. Expected array of budgets."
    );

    let valid_budget_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/",
            json!({
                "category": "餐饮",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 10.0,
                "start_date": "2026-03-01"
            }),
        ))
        .await?;
    assert_eq!(valid_budget_response.status(), StatusCode::CREATED);
    let valid_budget_id = read_json(valid_budget_response).await["result"]["id"]
        .as_i64()
        .expect("created budget id");
    let invalid_update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/budgets/{valid_budget_id}"),
            json!({"unknown_field": true}),
        ))
        .await?;
    assert_eq!(invalid_update_response.status(), StatusCode::BAD_REQUEST);
    assert!(read_json(invalid_update_response).await["error"]
        .as_str()
        .unwrap_or_default()
        .contains("invalid budget update field"));

    let missing_db_state = HttpAppState::new(
        HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            Duration::from_secs(1),
            1024 * 1024,
            ImportRouteMode::ImportDbRuntime,
        )?
        .with_database_backend(DatabaseBackend::Sqlite)
        .with_require_postgres_after_cutover(false)
        .with_legacy_sqlite_runtime_for_tests()
        .with_trusted_user_header_secret(TEST_AUTH_SECRET),
    )?;
    let missing_db_response = build_router(missing_db_state.clone())
        .oneshot(authed_request(Method::GET, "/api/budgets/", Body::empty()))
        .await?;
    assert_eq!(
        missing_db_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let missing_db_execution_response = build_router(missing_db_state.clone())
        .oneshot(authed_request(
            Method::GET,
            "/api/budgets/execution",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        missing_db_execution_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    for (method, path, body) in [
        (Method::GET, "/api/budgets/forecast", Value::Null),
        (Method::GET, "/api/budgets/history", Value::Null),
        (
            Method::POST,
            "/api/budgets/history/snapshot",
            json!({"period_type": "monthly", "start_date": "2026-03-01", "end_date": "2026-03-31"}),
        ),
        (Method::GET, "/api/budgets/999", Value::Null),
        (
            Method::POST,
            "/api/budgets/",
            json!({"category": "餐饮", "period_type": "monthly", "amount": 1.0, "start_date": "2026-03-01"}),
        ),
        (Method::PUT, "/api/budgets/999", json!({"amount": 2.0})),
        (Method::DELETE, "/api/budgets/999", Value::Null),
        (Method::GET, "/api/budgets/export", Value::Null),
    ] {
        let request = if body.is_null() {
            authed_request(method, path, Body::empty())
        } else {
            json_request(method, path, body)
        };
        let response = build_router(missing_db_state.clone())
            .oneshot(request)
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
    }

    let missing_db_import_response = build_router(missing_db_state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/import",
            json!([{
                "name": "预算",
                "category": "餐饮",
                "period_type": "monthly",
                "amount": 1.0,
                "start_date": "2026-03-01"
            }]),
        ))
        .await?;
    assert_eq!(
        missing_db_import_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let invalid_parent = fixture
        ._temp_dir
        .path()
        .join("missing-parent")
        .join("budget.db");
    let invalid_path_state = HttpAppState::new(
        HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            Duration::from_secs(1),
            1024 * 1024,
            ImportRouteMode::ImportDbRuntime,
        )?
        .with_database_backend(DatabaseBackend::Sqlite)
        .with_require_postgres_after_cutover(false)
        .with_legacy_sqlite_runtime_for_tests()
        .with_sqlite_db_path(invalid_parent.display().to_string())
        .with_trusted_user_header_secret(TEST_AUTH_SECRET),
    )?;
    let invalid_path_response = build_router(invalid_path_state)
        .oneshot(authed_request(Method::GET, "/api/budgets/", Body::empty()))
        .await?;
    assert_eq!(
        invalid_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let empty_schema_dir = tempfile::tempdir()?;
    let empty_schema_path = empty_schema_dir.path().join("empty-budget.db");
    Connection::open(&empty_schema_path)?;
    let empty_schema_state = HttpAppState::new(
        HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            Duration::from_secs(1),
            1024 * 1024,
            ImportRouteMode::ImportDbRuntime,
        )?
        .with_database_backend(DatabaseBackend::Sqlite)
        .with_require_postgres_after_cutover(false)
        .with_legacy_sqlite_runtime_for_tests()
        .with_sqlite_db_path(empty_schema_path.display().to_string())
        .with_trusted_user_header_secret(TEST_AUTH_SECRET),
    )?;
    let empty_schema_response = build_router(empty_schema_state)
        .oneshot(authed_request(Method::GET, "/api/budgets/", Body::empty()))
        .await?;
    assert_eq!(
        empty_schema_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let unauthenticated_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/budgets/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn budgets_runtime_has_no_legacy_budget_route_fallbacks_after_import_takeover(
) -> Result<(), Box<dyn Error>> {
    assert!(BUDGET_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/budgets/import")));
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/api/budgets/import",
            json!([{
                "name": "预算",
                "category": "餐饮",
                "period_type": "monthly",
                "amount": 1.0,
                "start_date": "2026-03-01"
            }]),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read_json(response).await["runtime"], Value::Null);

    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
    upstream: String,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        Self::new_with_upstream("http://127.0.0.1:9".to_string())
    }

    fn new_with_upstream(upstream: String) -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("budgets-http.db");
        init_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            upstream,
        })
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        fixture.upstream.clone(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_database_backend(DatabaseBackend::Sqlite)
    .with_require_postgres_after_cutover(false)
    .with_legacy_sqlite_runtime_for_tests()
    .with_sqlite_db_path(fixture.db_path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn postgres_runtime_router(postgres_url: &str) -> Result<Router, Box<dyn Error>> {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:9".to_string(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )?
    .with_database_backend(DatabaseBackend::Postgres)
    .with_require_postgres_after_cutover(true)
    .with_postgres_url(postgres_url)?
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = HttpAppState::new(config).expect("http app state");
    Ok(build_router(state))
}

fn init_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL
        );
        CREATE TABLE budgets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            category TEXT,
            sub_category TEXT,
            period_type TEXT NOT NULL,
            amount REAL NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            alert_threshold INTEGER DEFAULT 80,
            enabled BOOLEAN DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            icon TEXT,
            color TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            date TEXT NOT NULL,
            main_category TEXT,
            sub_category TEXT,
            source_account_id INTEGER,
            destination_account_id INTEGER
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL
        );
        CREATE TABLE budget_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            budget_id INTEGER NOT NULL,
            period_start TEXT NOT NULL,
            period_end TEXT NOT NULL,
            budget_amount REAL DEFAULT 0,
            spent_amount REAL DEFAULT 0,
            remaining_amount REAL,
            execution_rate REAL DEFAULT 0,
            status TEXT,
            filter_summary TEXT DEFAULT '',
            calculated_at TEXT NOT NULL
        );
        ",
    )?;
    connection.execute("INSERT INTO users(id, username) VALUES (42, 'owner')", [])?;
    connection.execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, icon, color, created_at)
         VALUES (1, 42, 1, '餐饮', '', 'folder', '#ffaa00', 'now'),
                (2, 42, 3, '餐饮', '午餐', 'tag', '#ffaa00', 'now')",
        [],
    )?;
    Ok(())
}

fn seed_execution_bills(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -35.5, '2026-03-15 12:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -15.0, '2026-03-31 23:00:00', '餐饮', '午餐', 11, NULL),
                (42, '支出', -9.0, '2026-04-01 00:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_tags(bill_id, tag_id) VALUES (1, 8), (2, 9), (3, 8)",
        [],
    )?;
    Ok(())
}

fn seed_forecast_bills(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -10.0, '2026-01-15 12:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -20.0, '2026-02-15 12:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -35.0, '2026-03-31 23:00:00', '餐饮', '午餐', 10, NULL),
                (77, '支出', -999.0, '2026-03-15 12:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;
    Ok(())
}

fn append_matching_bill(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -40.0, '2026-03-20 12:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;
    let bill_id = connection.last_insert_rowid();
    connection.execute(
        "INSERT INTO bill_tags(bill_id, tag_id) VALUES (?1, 8)",
        [bill_id],
    )?;
    Ok(())
}

fn budget_amount(path: &Path, budget_id: i64) -> Result<f64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT amount FROM budgets WHERE id = ?1",
        [budget_id],
        |row| row.get::<_, f64>(0),
    )?)
}

struct ImportedBudgetRow {
    amount: f64,
    sub_category: String,
    alert_threshold: i64,
    enabled: i64,
}

fn imported_budget_row(path: &Path, name: &str) -> Result<ImportedBudgetRow, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT amount, COALESCE(sub_category, ''), COALESCE(alert_threshold, 80), COALESCE(enabled, 1)
         FROM budgets WHERE user_id = 42 AND name = ?1",
        [name],
        |row| {
            Ok(ImportedBudgetRow {
                amount: row.get::<_, f64>(0)?,
                sub_category: row.get::<_, String>(1)?,
                alert_threshold: row.get::<_, i64>(2)?,
                enabled: row.get::<_, i64>(3)?,
            })
        },
    )?)
}

fn budget_name_count(path: &Path, name: &str) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) FROM budgets WHERE user_id = 42 AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )?)
}

async fn postgres_budget_amount_cents(
    pool: &sqlx::PgPool,
    user_id: i64,
    name: &str,
    sub_category: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "
        SELECT amount_cents
        FROM budgets
        WHERE user_id = $1
          AND name = $2
          AND COALESCE(sub_category, '') = $3
        ORDER BY id DESC
        LIMIT 1
        ",
    )
    .bind(user_id)
    .bind(name)
    .bind(sub_category)
    .fetch_one(pool)
    .await?
    .try_get("amount_cents")?)
}

async fn postgres_budget_row_count(
    pool: &sqlx::PgPool,
    user_id: i64,
    name: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query("SELECT COUNT(*) FROM budgets WHERE user_id = $1 AND name = $2")
            .bind(user_id)
            .bind(name)
            .fetch_one(pool)
            .await?
            .try_get("count")?,
    )
}

async fn postgres_budget_enabled(
    pool: &sqlx::PgPool,
    user_id: i64,
    name: &str,
) -> Result<bool, Box<dyn Error>> {
    Ok(sqlx::query(
        "
        SELECT enabled
        FROM budgets
        WHERE user_id = $1 AND name = $2
        ORDER BY id DESC
        LIMIT 1
        ",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?
    .try_get("enabled")?)
}

async fn insert_postgres_budget_bill(
    pool: &sqlx::PgPool,
    user_id: i64,
    occurred_at: &str,
    amount_cents: i64,
    account_id: i64,
    category_id: i64,
    payload: &Value,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, amount_cents, direction, transaction_type,
            account_id, source_account_id, category_id, merchant, description, standard_payload
        )
        VALUES ($1, $2::timestamptz, $3, 'expense', 'expense', $4, $4, $5, 'Cafe', 'lunch', $6)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(occurred_at)
    .bind(amount_cents)
    .bind(account_id)
    .bind(category_id)
    .bind(payload)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn postgres_budget_history_spent_cents(
    pool: &sqlx::PgPool,
    user_id: i64,
    budget_id: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        SELECT spent_amount_cents
        FROM budget_history
        WHERE user_id = $1 AND budget_id = $2
        ORDER BY calculated_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(budget_id)
    .fetch_one(pool)
    .await?
    .try_get("spent_amount_cents")?)
}

fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
    authed_request(method, uri, Body::from(body.to_string()))
}

fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", TEST_USER_ID)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
}

fn authed_request_for_user(method: Method, uri: &str, body: Body, user_id: i64) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}
