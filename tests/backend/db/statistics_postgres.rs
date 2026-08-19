use std::{env, error::Error, str::FromStr};

use bill_analyser_core::{
    statistics::{parse_statistics_year_month_range, StatisticsYearMonthRangeMode},
    UserId,
};
use bill_analyser_db::{
    query_postgres_asset_trends_payload, query_postgres_category_statistics_payload,
    query_postgres_category_trends_payload, query_postgres_statistics_analyzer_report_payload,
    query_postgres_transaction_amount_period, run_postgres_migrations, StatisticsBillFilters,
};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};

#[tokio::test]
async fn statistics_postgres_queries_preserve_explicit_cents_and_transfer_boundaries(
) -> Result<(), Box<dyn Error>> {
    let postgres_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for PostgreSQL statistics contracts",
        )
    })?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("statistics_contract_{unique}");
    let base_options = PgConnectOptions::from_str(&postgres_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.clone().database("postgres"))
        .await?;
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{}""#, test_db).as_str())
        .await?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(base_options.database(&test_db))
        .await?;
    run_postgres_migrations(&pool).await?;

    let user_id = seed_statistics_fixture(&pool, unique).await?;
    let cash_id = account_id(&pool, user_id, &format!("现金-{unique}")).await?;
    let bank_id = account_id(&pool, user_id, &format!("工资卡-{unique}")).await?;
    let breakfast_id = category_id(&pool, user_id, "餐饮/早餐").await?;
    let salary_id = category_id(&pool, user_id, "工资/主业").await?;
    let transfer_id = category_id(&pool, user_id, "转账").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("seeded positive user id");

    let filters = StatisticsBillFilters {
        start_date: Some("2026-03-01".to_string()),
        end_date: Some("2026-03-31".to_string()),
        ..StatisticsBillFilters::default()
    };
    let category_items =
        query_postgres_category_statistics_payload(&pool, scoped_user_id, &filters).await?;
    assert_statistic_amount(
        &category_items,
        breakfast_id,
        cash_id,
        -1234,
        "expense cents are projected as a negative minor-unit statistic",
    );
    assert_statistic_amount(
        &category_items,
        salary_id,
        bank_id,
        5678,
        "income cents are projected as a positive minor-unit statistic",
    );
    assert_statistic_amount(
        &category_items,
        transfer_id,
        bank_id,
        2000,
        "current transfer DB mapping keeps destination id but no destination name, so category stats stay positive",
    );

    let trend_range = match parse_statistics_year_month_range(Some("202603"), Some("202603"))
        .expect("bounded March range")
    {
        StatisticsYearMonthRangeMode::Bounded(range) => range,
        StatisticsYearMonthRangeMode::All => panic!("expected bounded March range"),
    };
    let trend =
        query_postgres_category_trends_payload(&pool, scoped_user_id, &filters, &trend_range)
            .await?;
    assert_eq!(trend[0]["year"], 2026);
    assert_eq!(trend[0]["month"], 3);
    assert_json_statistic_amount(
        &trend[0]["items"],
        breakfast_id,
        cash_id,
        -1234,
        "trend expense amount",
    );

    let amounts = query_postgres_transaction_amount_period(
        &pool,
        scoped_user_id,
        1_772_496_000,
        1_775_087_999,
        "2026-03-01".to_string(),
        "2026-03-31".to_string(),
    )
    .await?;
    assert_eq!(amounts.amounts[0].currency, "CNY");
    assert_eq!(amounts.amounts[0].income_amount_cents, 5678);
    assert_eq!(amounts.amounts[0].expense_amount_cents, 1234);

    let analyzer =
        query_postgres_statistics_analyzer_report_payload(&pool, scoped_user_id, "year").await?;
    assert_eq!(analyzer["summary"]["total_income_cents"], 5678);
    assert_eq!(analyzer["summary"]["total_expense_cents"], -1234);
    assert_eq!(analyzer["summary"]["net_income_cents"], 4444);
    assert_eq!(analyzer["by_type"]["expense"]["total_cents"], -1234);
    assert_eq!(analyzer["by_type"]["transfer"]["total_cents"], -2300);
    let march_analyzer_trend = analyzer["trend"]
        .as_array()
        .expect("analyzer trend array")
        .iter()
        .find(|bucket| {
            bucket["date"]
                .as_str()
                .is_some_and(|date| date.starts_with("2026-03"))
        })
        .expect("March analyzer trend");
    assert_eq!(march_analyzer_trend["expense_cents"], -1234);
    assert_eq!(march_analyzer_trend["net_cents"], 4444);

    let asset = query_postgres_asset_trends_payload(
        &pool,
        scoped_user_id,
        NaiveDate::from_ymd_opt(2026, 3, 1).expect("start date"),
        NaiveDate::from_ymd_opt(2026, 3, 1).expect("end date"),
    )
    .await?;
    let first_day = &asset["items"][0];
    assert_json_asset_amount(first_day, cash_id, 10375, 11741);
    assert_json_asset_amount(first_day, bank_id, 49400, 52678);
    let explicit_id = account_id(&pool, user_id, &format!("零钱包-{unique}")).await?;
    assert_json_asset_amount(first_day, explicit_id, 1234, 1234);
    let cash_id_text = cash_id.to_string();
    assert!(asset["legend"]
        .as_array()
        .expect("asset legend")
        .iter()
        .any(|entry| entry["id"].as_str() == Some(cash_id_text.as_str())));

    insert_bill(
        &pool,
        user_id,
        "2026-03-01T13:00:00Z",
        "expense",
        "expense",
        i64::MIN,
        cash_id,
        None,
        breakfast_id,
        "非法最小金额",
        json!({"main_category": "餐饮", "sub_category": "早餐"}),
    )
    .await?;
    let minimum_amount_error = query_postgres_asset_trends_payload(
        &pool,
        scoped_user_id,
        NaiveDate::from_ymd_opt(2026, 3, 1).expect("start date"),
        NaiveDate::from_ymd_opt(2026, 3, 1).expect("end date"),
    )
    .await
    .expect_err("i64::MIN statistics amount must fail closed");
    assert!(minimum_amount_error
        .to_string()
        .contains("invalid statistics amount_cents"));

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;

    Ok(())
}

async fn seed_statistics_fixture(
    pool: &bill_analyser_db::PostgresPool,
    unique: i64,
) -> Result<i64, Box<dyn Error>> {
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("statistics-{unique}"))
            .bind(format!("statistics-{unique}@example.test"))
            .fetch_one(pool)
            .await?
            .try_get("id")?;

    let cash_id = insert_account(pool, user_id, &format!("现金-{unique}"), 123_456, 10_025).await?;
    let bank_id =
        insert_account(pool, user_id, &format!("工资卡-{unique}"), 45_678, 50_000).await?;
    insert_explicit_initial_balance_account(pool, user_id, &format!("零钱包-{unique}")).await?;
    let breakfast_id = insert_category(pool, user_id, "早餐", "餐饮/早餐").await?;
    let salary_id = insert_category(pool, user_id, "主业", "工资/主业").await?;
    let transfer_id = insert_category(pool, user_id, "转账", "转账").await?;
    let investment_id = insert_category(pool, user_id, "投资", "投资").await?;

    insert_bill(
        pool,
        user_id,
        "2026-03-01T08:00:00Z",
        "expense",
        "expense",
        1234,
        cash_id,
        None,
        breakfast_id,
        "早餐店",
        json!({"main_category": "餐饮", "sub_category": "早餐"}),
    )
    .await?;
    insert_bill(
        pool,
        user_id,
        "2026-03-01T09:00:00Z",
        "income",
        "income",
        5678,
        bank_id,
        None,
        salary_id,
        "公司",
        json!({"main_category": "工资", "sub_category": "主业"}),
    )
    .await?;
    insert_bill(
        pool,
        user_id,
        "2026-03-01T10:00:00Z",
        "expense",
        "transfer",
        2000,
        bank_id,
        Some(cash_id),
        transfer_id,
        "现金",
        json!({
            "main_category": "转账",
            "destination_amount_cents": 2150
        }),
    )
    .await?;
    insert_bill(
        pool,
        user_id,
        "2026-02-28T10:00:00Z",
        "expense",
        "investment",
        300,
        bank_id,
        Some(cash_id),
        investment_id,
        "区间前投资",
        json!({
            "main_category": "投资",
            "destination_amount_cents": 350
        }),
    )
    .await?;
    insert_bill(
        pool,
        user_id,
        "2026-03-01T11:00:00Z",
        "expense",
        "investment",
        400,
        bank_id,
        Some(cash_id),
        investment_id,
        "区间内投资",
        json!({
            "main_category": "投资",
            "destination_amount_cents": 450
        }),
    )
    .await?;
    insert_bill(
        pool,
        user_id,
        "2026-02-28T11:00:00Z",
        "expense",
        "transfer",
        300,
        bank_id,
        Some(cash_id),
        transfer_id,
        "显式零目的金额",
        json!({
            "main_category": "转账",
            "destination_amount_cents": 0
        }),
    )
    .await?;

    Ok(user_id)
}

async fn insert_explicit_initial_balance_account(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    name: &str,
) -> Result<i64, Box<dyn Error>> {
    let id = sqlx::query(
        r#"
        INSERT INTO accounts (user_id, name, account_type, currency, balance_cents, metadata)
        VALUES ($1, $2, 'cash', 'CNY', 9999, $3)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(json!({"initial_balance_cents": 1234, "icon": "wallet"}))
    .fetch_one(pool)
    .await?
    .try_get("id")?;
    Ok(id)
}

async fn insert_account(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    name: &str,
    balance_cents: i64,
    initial_balance_cents: i64,
) -> Result<i64, Box<dyn Error>> {
    let id = sqlx::query(
        r#"
        INSERT INTO accounts (user_id, name, account_type, currency, balance_cents, metadata)
        VALUES ($1, $2, 'cash', 'CNY', $3, $4)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(balance_cents)
    .bind(json!({"initial_balance_cents": initial_balance_cents, "icon": "wallet"}))
    .fetch_one(pool)
    .await?
    .try_get("id")?;
    Ok(id)
}

async fn insert_category(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    name: &str,
    path: &str,
) -> Result<i64, Box<dyn Error>> {
    let id = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path)
        VALUES ($1, $2, 'expense', $3)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(path)
    .fetch_one(pool)
    .await?
    .try_get("id")?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_bill(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    occurred_at: &str,
    direction: &str,
    transaction_type: &str,
    amount_cents: i64,
    source_account_id: i64,
    target_account_id: Option<i64>,
    category_id: i64,
    merchant: &str,
    standard_payload: Value,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, target_account_id, transfer_target_account_id,
            category_id, merchant, payment_method, description, source_hash, standard_payload
        )
        VALUES ($1, $2::timestamptz, $3, $4, $5, $6, $6, $7, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(user_id)
    .bind(occurred_at)
    .bind(direction)
    .bind(transaction_type)
    .bind(amount_cents)
    .bind(source_account_id)
    .bind(target_account_id)
    .bind(category_id)
    .bind(merchant)
    .bind("现金")
    .bind(format!("statistics contract {merchant}"))
    .bind(format!("statistics-{user_id}-{merchant}-{amount_cents}"))
    .bind(standard_payload)
    .execute(pool)
    .await?;
    Ok(())
}

async fn account_id(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    name: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM accounts WHERE user_id = $1 AND name = $2")
            .bind(user_id)
            .bind(name)
            .fetch_one(pool)
            .await?,
    )
}

async fn category_id(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    path: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM categories WHERE user_id = $1 AND path = $2")
            .bind(user_id)
            .bind(path)
            .fetch_one(pool)
            .await?,
    )
}

fn assert_statistic_amount(
    items: &[bill_analyser_core::statistics::CategoryStatisticItem],
    category_id: i64,
    account_id: i64,
    expected: i64,
    context: &str,
) {
    let item = items
        .iter()
        .find(|item| {
            item.category_id == category_id.to_string() && item.account_id == account_id.to_string()
        })
        .unwrap_or_else(|| panic!("missing statistic item: {context}"));
    assert_eq!(item.amount_cents, expected, "{context}");
}

fn assert_json_statistic_amount(
    items: &Value,
    category_id: i64,
    account_id: i64,
    expected: i64,
    context: &str,
) {
    let expected_category_id = category_id.to_string();
    let expected_account_id = account_id.to_string();
    let item = items
        .as_array()
        .expect("statistic items")
        .iter()
        .find(|item| {
            item["categoryId"].as_str() == Some(expected_category_id.as_str())
                && item["accountId"].as_str() == Some(expected_account_id.as_str())
        })
        .unwrap_or_else(|| panic!("missing statistic item: {context}"));
    assert_eq!(item["amountCents"], expected, "{context}");
}

fn assert_json_asset_amount(day: &Value, account_id: i64, opening: i64, closing: i64) {
    let expected_account_id = account_id.to_string();
    let item = day["items"]
        .as_array()
        .expect("asset trend items")
        .iter()
        .find(|item| item["accountId"].as_str() == Some(expected_account_id.as_str()))
        .unwrap_or_else(|| panic!("missing asset trend item for account {account_id}"));
    assert_eq!(item["accountOpeningBalanceCents"], opening);
    assert_eq!(item["accountClosingBalanceCents"], closing);
}
