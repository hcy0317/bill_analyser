#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::{env, error::Error, io};

    #[test]
    fn category_names_from_postgres_path_splits_main_and_subcategory() {
        assert_eq!(
            category_names_from_postgres_path(Some("Food/Lunch"), "Lunch"),
            ("Food".to_string(), "Lunch".to_string())
        );
        assert_eq!(
            category_names_from_postgres_path(None, "Food"),
            ("Food".to_string(), String::new())
        );
    }

    #[test]
    fn budget_forecast_and_amount_helpers_preserve_cents() {
        let (totals, period_count) = aggregate_postgres_budget_forecast_rows(vec![
            BudgetForecastRow {
                period: "2026-05".to_string(),
                category: "餐饮".to_string(),
                amount_cents: 1200,
            },
            BudgetForecastRow {
                period: "2026-05".to_string(),
                category: "餐饮".to_string(),
                amount_cents: 300,
            },
            BudgetForecastRow {
                period: "2026-06".to_string(),
                category: "餐饮".to_string(),
                amount_cents: 4500,
            },
        ]);

        assert_eq!(period_count, 2);
        let food = totals.get("餐饮").expect("food totals");
        assert_eq!(food.periods[0].period, "2026-05");
        assert_eq!(food.periods[0].amount_cents, 1500);
        assert_eq!(food.periods[1].amount_cents, 4500);

        let record = BudgetRecord::from_iter([("amount_cents".to_string(), json!("9876"))]);
        assert_eq!(record_amount_cents(&record), Some(9876));
        assert!(record_amount_cents(&BudgetRecord::from_iter([(
            "amount_cents".to_string(),
            json!(true)
        )]))
        .is_none());
        assert!(amount_cents_from_value(&json!(true)).is_err());
        assert_eq!(
            amount_cents_from_value(&json!("12345")).expect("text cents"),
            12345
        );
        assert!(amount_cents_from_value(&json!("12.34")).is_err());
        assert!(amount_cents_from_record(&BudgetRecord::new(), "amount_cents").is_err());
    }

    #[test]
    fn budget_period_group_keys_require_category_and_start_date() {
        let mut budget = BudgetRecord::new();
        budget.insert("category".to_string(), json!("餐饮"));
        budget.insert("sub_category".to_string(), json!("午餐"));

        assert!(build_postgres_budget_period_group_key(
            &budget,
            BudgetPeriodKind::Quarterly,
            "",
            7,
        )
        .is_none());

        budget.insert("category".to_string(), json!("   "));
        assert!(build_postgres_budget_period_group_key(
            &budget,
            BudgetPeriodKind::Yearly,
            "2026-01-01",
            7,
        )
        .is_none());

        budget.insert("category".to_string(), json!("餐饮"));
        let key = build_postgres_budget_period_group_key(
            &budget,
            BudgetPeriodKind::Quarterly,
            "2026-04-01",
            7,
        )
        .expect("complete group key");
        assert_eq!(key.category, "餐饮");
        assert_eq!(key.sub_category, "午餐");
        assert_eq!(key.period_kind, BudgetPeriodKind::Quarterly);
        assert_eq!(key.period_type(), "quarterly");
        assert_eq!(key.start_date, "2026-04-01");
        assert_eq!(key.user_id, 7);
    }

    #[tokio::test]
    async fn postgres_row_projectors_emit_explicit_cents_when_database_available(
    ) -> Result<(), Box<dyn Error>> {
        let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return Ok(());
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await?;

        let budget_row = sqlx::query(
            r#"
            SELECT
                1::BIGINT AS id,
                2::BIGINT AS user_id,
                '餐饮预算'::TEXT AS name,
                '餐饮'::TEXT AS category,
                '午餐'::TEXT AS sub_category,
                'monthly'::TEXT AS period_type,
                '2026-06-01'::DATE AS start_date,
                NULL::DATE AS end_date,
                12345::BIGINT AS amount_cents,
                80::INT AS alert_threshold,
                true AS enabled,
                now() AS created_at,
                now() AS updated_at
            "#,
        )
        .fetch_one(&pool)
        .await?;
        let budget = budget_record_from_postgres_row(budget_row).expect("budget record");
        assert_eq!(budget.get("amount_cents"), Some(&json!(12345)));
        assert!(budget.get("amount").is_none());

        let history_row = sqlx::query(
            r#"
            SELECT
                10::BIGINT AS id,
                1::BIGINT AS budget_id,
                '2026-06-01'::DATE AS period_start,
                '2026-06-30'::DATE AS period_end,
                12345::BIGINT AS budget_amount_cents,
                4500::BIGINT AS spent_amount_cents,
                7845::BIGINT AS remaining_amount_cents,
                36.45::DOUBLE PRECISION AS execution_rate,
                'normal'::TEXT AS status,
                'monthly'::TEXT AS filter_summary,
                '餐饮预算'::TEXT AS name,
                '餐饮'::TEXT AS category,
                '午餐'::TEXT AS sub_category,
                'monthly'::TEXT AS period_type,
                now() AS calculated_at,
                80::INT AS alert_threshold,
                true AS enabled
            "#,
        )
        .fetch_one(&pool)
        .await?;
        let history = budget_history_record_from_postgres_row(history_row).expect("history record");
        assert_eq!(history.get("budget_amount_cents"), Some(&json!(12345)));
        assert_eq!(history.get("spent_amount_cents"), Some(&json!(4500)));
        assert_eq!(history.get("remaining_amount_cents"), Some(&json!(7845)));

        Ok(())
    }

    #[tokio::test]
    async fn postgres_forecast_group_keys_match_the_core_period_contract(
    ) -> Result<(), Box<dyn Error>> {
        let postgres_url = env::var("BILL_ANALYSER_TEST_POSTGRES_URL").map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for budget period contracts",
            )
        })?;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await?;
        let cases = [
            (BudgetPeriodKind::Daily, "2021-01-01", "2021-01-01"),
            (BudgetPeriodKind::Weekly, "2021-01-01", "2021-00"),
            (BudgetPeriodKind::Weekly, "2021-01-10", "2021-01"),
            (BudgetPeriodKind::Weekly, "2026-05-07", "2026-18"),
            (BudgetPeriodKind::Monthly, "2021-01-31", "2021-01"),
            (BudgetPeriodKind::Monthly, "2021-02-01", "2021-02"),
            (BudgetPeriodKind::Quarterly, "2021-03-31", "2021-Q1"),
            (BudgetPeriodKind::Quarterly, "2021-04-01", "2021-Q2"),
            (BudgetPeriodKind::Yearly, "2020-12-31", "2020"),
            (BudgetPeriodKind::Yearly, "2021-01-01", "2021"),
        ];

        for (period_kind, day, expected) in cases {
            let expression = postgres_budget_forecast_group_expr(period_kind);
            let statement = format!(
                "SELECT {expression} AS period FROM (SELECT $1::timestamptz AS occurred_at) b"
            );
            let actual: String = sqlx::query_scalar(&statement)
                .bind(format!("{day}T12:00:00Z"))
                .fetch_one(&pool)
                .await?;
            let core_key = period_kind.bucket_key(
                NaiveDate::parse_from_str(day, "%Y-%m-%d").expect("contract date"),
            );
            assert_eq!(actual, expected, "PostgreSQL key for {day}");
            assert_eq!(actual, core_key, "Rust/PostgreSQL parity for {day}");
        }

        Ok(())
    }
}
