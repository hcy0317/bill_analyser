mod postgres_test_support;

use std::{error::Error, io};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    query_postgres_budget_execution_history, query_postgres_budget_forecast,
    BudgetExecutionFilters, BudgetForecastFilters, DbError, PostgresPool,
};
use serde_json::Value;
use sqlx::Row;

use postgres_test_support::isolated_postgres_database;

#[tokio::test]
async fn weekly_forecast_uses_the_core_posix_monday_bucket_contract() -> Result<(), Box<dyn Error>>
{
    let database = isolated_postgres_database("budget_period_contract")
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for budget period contracts",
            )
        })?;

    let outcome = async {
        let user_id = seed_weekly_forecast_fixture(&database.pool).await?;
        let forecast = query_postgres_budget_forecast(
            &database.pool,
            UserId::new(u64::try_from(user_id)?)?,
            &BudgetForecastFilters {
                budget_type: 3,
                period_type: "weekly".to_string(),
                start_date: "2021-01-01".to_string(),
                end_date: "2021-01-10".to_string(),
                forecast_strategy: "historical_average".to_string(),
                history_periods: 3,
            },
        )
        .await?;

        let food = forecast
            .iter()
            .find(|item| item["category"] == "餐饮")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing food forecast"))?;
        let period_labels = food["periods"]
            .as_array()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "periods is not an array"))?
            .iter()
            .map(|period| {
                period["period"]
                    .as_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "period label is not text")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok::<_, Box<dyn Error>>((food["current_spent_cents"].clone(), period_labels))
    }
    .await;

    let cleanup = database.cleanup().await;
    let (current_spent_cents, period_labels) = outcome?;
    cleanup?;

    assert_eq!(current_spent_cents, Value::from(1_200));
    assert_eq!(period_labels, vec!["2021-00", "2021-01"]);
    Ok(())
}

#[tokio::test]
async fn budget_read_models_reject_an_unknown_period_before_querying() -> Result<(), Box<dyn Error>>
{
    let database = isolated_postgres_database("budget_period_invalid")
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for budget period contracts",
            )
        })?;
    let forecast_outcome = query_postgres_budget_forecast(
        &database.pool,
        UserId::new(1)?,
        &BudgetForecastFilters {
            budget_type: 3,
            period_type: "fortnight".to_string(),
            start_date: "2021-01-01".to_string(),
            end_date: "2021-01-10".to_string(),
            forecast_strategy: "historical_average".to_string(),
            history_periods: 3,
        },
    )
    .await;
    let history_outcome = query_postgres_budget_execution_history(
        &database.pool,
        UserId::new(1)?,
        &BudgetExecutionFilters {
            budget_type: 3,
            period_type: Some("fortnight".to_string()),
            start_date: Some("2021-01-01".to_string()),
            end_date: Some("2021-01-10".to_string()),
            ..BudgetExecutionFilters::default()
        },
    )
    .await;
    database.cleanup().await?;

    assert!(matches!(
        forecast_outcome,
        Err(DbError::InvalidOperation(message))
            if message == "Invalid period_type: fortnight"
    ));
    assert!(matches!(
        history_outcome,
        Err(DbError::InvalidOperation(message))
            if message == "Invalid period_type: fortnight"
    ));
    Ok(())
}

async fn seed_weekly_forecast_fixture(pool: &PostgresPool) -> Result<i64, Box<dyn Error>> {
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username) VALUES ('budget-period-owner') RETURNING id")
            .fetch_one(pool)
            .await?
            .try_get("id")?;
    let category_id: i64 = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path)
        VALUES ($1, '餐饮', 'expense', '餐饮')
        RETURNING id
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?
    .try_get("id")?;

    sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date
        )
        VALUES ($1, '每周餐饮', '餐饮', '', 'weekly', 20000, '2020-12-28', '2021-01-31')
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    for (occurred_at, amount_cents, source_hash) in [
        ("2021-01-01T12:00:00Z", 1_200_i64, "weekly-2021-00"),
        ("2021-01-04T12:00:00Z", 3_400_i64, "weekly-2021-01-a"),
        ("2021-01-10T12:00:00Z", 5_600_i64, "weekly-2021-01-b"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO bills (
                user_id, occurred_at, amount_cents, direction, transaction_type,
                category_id, source_hash, standard_payload
            )
            VALUES ($1, $2::timestamptz, $3, 'expense', 'expense', $4, $5, '{"type":"支出"}')
            "#,
        )
        .bind(user_id)
        .bind(occurred_at)
        .bind(amount_cents)
        .bind(category_id)
        .bind(source_hash)
        .execute(pool)
        .await?;
    }

    Ok(user_id)
}
