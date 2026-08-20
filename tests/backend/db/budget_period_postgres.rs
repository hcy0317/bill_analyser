mod postgres_test_support;

use std::{error::Error, io};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_postgres_budget, delete_postgres_budget, get_postgres_budget_by_id,
    import_postgres_budgets, query_postgres_budget_execution_history,
    query_postgres_budget_forecast, run_postgres_migrations, update_postgres_budget,
    BudgetCreateDraft, BudgetExecutionFilters, BudgetForecastFilters, BudgetRecord,
    BudgetUpdateDraft, DbError, PostgresPool,
};
use serde_json::{json, Value};
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

#[tokio::test]
async fn budget_writers_are_strict_and_bulk_errors_do_not_abort_valid_rows(
) -> Result<(), Box<dyn Error>> {
    let database = isolated_postgres_database("budget_period_writer")
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for budget period contracts",
            )
        })?;

    let outcome = async {
        let user_id: i64 = sqlx::query(
            "INSERT INTO users (username) VALUES ('budget-period-writer') RETURNING id",
        )
        .fetch_one(&database.pool)
        .await?
        .try_get("id")?;
        let user = UserId::new(u64::try_from(user_id)?)?;

        let created_id = create_postgres_budget(
            &database.pool,
            user,
            &BudgetCreateDraft {
                fields: budget_record(json!({
                    "name": "规范月度预算",
                    "category": "餐饮",
                    "sub_category": "午餐",
                    "period_type": " monthly ",
                    "amount_cents": 12_345,
                    "start_date": "2026-06-01"
                })),
            },
        )
        .await?;
        let created = get_postgres_budget_by_id(&database.pool, user, created_id)
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "created budget missing"))?;
        let primary_id = create_postgres_budget(
            &database.pool,
            user,
            &BudgetCreateDraft {
                fields: budget_record(json!({
                    "name": "无子类月度预算",
                    "category": "无子类分类",
                    "period_type": "monthly",
                    "amount_cents": 500,
                    "start_date": "2026-09-01"
                })),
            },
        )
        .await?;
        let deleted_primary = delete_postgres_budget(&database.pool, user, primary_id).await?;
        let daily_id = create_postgres_budget(
            &database.pool,
            user,
            &BudgetCreateDraft {
                fields: budget_record(json!({
                    "name": "每日预算",
                    "category": "日常",
                    "period_type": "daily",
                    "amount_cents": 600,
                    "start_date": "2026-09-01"
                })),
            },
        )
        .await?;

        let invalid_create = create_postgres_budget(
            &database.pool,
            user,
            &BudgetCreateDraft {
                fields: budget_record(json!({
                    "name": "非法周期预算",
                    "category": "餐饮",
                    "period_type": "fortnight",
                    "amount_cents": 99,
                    "start_date": "2026-06-01"
                })),
            },
        )
        .await;
        let invalid_update = update_postgres_budget(
            &database.pool,
            user,
            created_id,
            &BudgetUpdateDraft {
                fields: budget_record(json!({"period_type": "fortnight"})),
            },
        )
        .await;
        let after_invalid_update = get_postgres_budget_by_id(&database.pool, user, created_id)
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "updated budget missing"))?;
        let valid_update = update_postgres_budget(
            &database.pool,
            user,
            created_id,
            &BudgetUpdateDraft {
                fields: budget_record(json!({"amount_cents": 13_579})),
            },
        )
        .await?;

        let import = import_postgres_budgets(
            &database.pool,
            user,
            &[
                budget_record(json!({
                    "name": "导入非法周期",
                    "category": "餐饮",
                    "period_type": "fortnight",
                    "amount_cents": 100,
                    "start_date": "2026-07-01"
                })),
                budget_record(json!({
                    "name": "导入缺省月度",
                    "category": "餐饮",
                    "amount_cents": 200,
                    "start_date": "2026-07-01"
                })),
                budget_record(json!({
                    "name": "导入空白月度",
                    "category": "餐饮",
                    "period_type": "   ",
                    "amount_cents": 300,
                    "start_date": "2026-08-01"
                })),
                budget_record(json!({
                    "name": "规范月度预算",
                    "category": "餐饮",
                    "sub_category": "午餐",
                    "amount_cents": 22_222,
                    "start_date": "2026-06-01"
                })),
            ],
        )
        .await?;
        let imported_periods = sqlx::query(
            r#"
            SELECT name, period_type
            FROM budgets
            WHERE user_id = $1 AND name LIKE '导入%'
            ORDER BY name
            "#,
        )
        .bind(user_id)
        .fetch_all(&database.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("name")?,
                row.try_get::<String, _>("period_type")?,
            ))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok::<_, Box<dyn Error>>((
            created,
            deleted_primary,
            daily_id,
            invalid_create,
            invalid_update,
            after_invalid_update,
            valid_update,
            import,
            imported_periods,
        ))
    }
    .await;

    let cleanup = database.cleanup().await;
    let (
        created,
        deleted_primary,
        daily_id,
        invalid_create,
        invalid_update,
        after_invalid_update,
        valid_update,
        import,
        imported_periods,
    ) = outcome?;
    cleanup?;

    assert_eq!(created["period_type"], "monthly");
    assert!(deleted_primary);
    assert!(daily_id > 0);
    assert!(matches!(
        invalid_create,
        Err(DbError::InvalidOperation(message))
            if message == "Invalid period_type: fortnight"
    ));
    assert!(matches!(
        invalid_update,
        Err(DbError::InvalidOperation(message))
            if message == "Invalid period_type: fortnight"
    ));
    assert_eq!(after_invalid_update["period_type"], "monthly");
    assert!(valid_update);
    assert_eq!(import["created"], 2);
    assert_eq!(import["updated"], 1);
    assert_eq!(import["errors"], 1);
    assert_eq!(
        imported_periods,
        vec![
            ("导入空白月度".to_string(), "monthly".to_string()),
            ("导入缺省月度".to_string(), "monthly".to_string()),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn budget_period_constraint_is_validated_and_rejects_raw_unknown_values(
) -> Result<(), Box<dyn Error>> {
    let database = isolated_postgres_database("budget_period_constraint")
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for budget period contracts",
            )
        })?;

    let outcome = async {
        let validated: bool = sqlx::query(
            r#"
            SELECT convalidated
            FROM pg_constraint
            WHERE conname = 'chk_budgets_period_type'
              AND conrelid = 'budgets'::regclass
            "#,
        )
        .fetch_one(&database.pool)
        .await?
        .try_get("convalidated")?;
        let user_id: i64 = sqlx::query(
            "INSERT INTO users (username) VALUES ('budget-period-constraint') RETURNING id",
        )
        .fetch_one(&database.pool)
        .await?
        .try_get("id")?;
        let error = sqlx::query(
            r#"
            INSERT INTO budgets (
                user_id, name, category, period_type, amount_cents, start_date
            )
            VALUES ($1, '非法周期', '餐饮', 'fortnight', 100, '2026-06-01')
            "#,
        )
        .bind(user_id)
        .execute(&database.pool)
        .await
        .expect_err("unknown period must violate the database constraint");
        Ok::<_, Box<dyn Error>>((validated, error))
    }
    .await;

    let cleanup = database.cleanup().await;
    let (validated, error) = outcome?;
    cleanup?;

    assert!(validated);
    let database_error = error
        .as_database_error()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing database error"))?;
    assert_eq!(database_error.code().as_deref(), Some("23514"));
    assert_eq!(database_error.constraint(), Some("chk_budgets_period_type"));
    Ok(())
}

#[tokio::test]
async fn budget_period_migration_upgrades_valid_history_and_rejects_dirty_history(
) -> Result<(), Box<dyn Error>> {
    let database = isolated_postgres_database("budget_period_upgrade")
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for budget period contracts",
            )
        })?;

    let outcome = async {
        sqlx::query("ALTER TABLE budgets DROP CONSTRAINT chk_budgets_period_type")
            .execute(&database.pool)
            .await?;
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 28")
            .execute(&database.pool)
            .await?;
        let user_id: i64 = sqlx::query(
            "INSERT INTO users (username) VALUES ('budget-period-upgrade') RETURNING id",
        )
        .fetch_one(&database.pool)
        .await?
        .try_get("id")?;
        sqlx::query(
            r#"
            INSERT INTO budgets (
                user_id, name, category, period_type, amount_cents, start_date
            )
            VALUES ($1, '合法旧数据', '餐饮', 'monthly', 100, '2026-06-01')
            "#,
        )
        .bind(user_id)
        .execute(&database.pool)
        .await?;
        run_postgres_migrations(&database.pool).await?;
        let validated_after_upgrade: bool = sqlx::query(
            r#"
            SELECT convalidated
            FROM pg_constraint
            WHERE conname = 'chk_budgets_period_type'
              AND conrelid = 'budgets'::regclass
            "#,
        )
        .fetch_one(&database.pool)
        .await?
        .try_get("convalidated")?;

        sqlx::query("ALTER TABLE budgets DROP CONSTRAINT chk_budgets_period_type")
            .execute(&database.pool)
            .await?;
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 28")
            .execute(&database.pool)
            .await?;
        sqlx::query(
            r#"
            INSERT INTO budgets (
                user_id, name, category, period_type, amount_cents, start_date
            )
            VALUES ($1, '非法旧数据', '餐饮', 'fortnight', 100, '2026-07-01')
            "#,
        )
        .bind(user_id)
        .execute(&database.pool)
        .await?;
        let dirty_upgrade = run_postgres_migrations(&database.pool).await;

        Ok::<_, Box<dyn Error>>((validated_after_upgrade, dirty_upgrade))
    }
    .await;

    let cleanup = database.cleanup().await;
    let (validated_after_upgrade, dirty_upgrade) = outcome?;
    cleanup?;

    assert!(validated_after_upgrade);
    assert!(
        dirty_upgrade.is_err(),
        "dirty legacy period must stop migration"
    );
    Ok(())
}

fn budget_record(value: Value) -> BudgetRecord {
    value
        .as_object()
        .cloned()
        .expect("budget fixture must be an object")
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
