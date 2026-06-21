#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_asset_trends_payload(
    pool: &PostgresPool,
    user_id: UserId,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let filters = StatisticsBillFilters {
        start_date: Some(start_date.to_string()),
        end_date: Some(end_date.to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    let accounts = load_postgres_statistics_accounts(pool, user_id).await?;
    let balances_before =
        load_postgres_account_balance_deltas_before(pool, user_id, start_date).await?;
    let days = build_asset_trends(&bills, &accounts, &balances_before, start_date, end_date);
    let legend = build_asset_trend_legend(&accounts, &days);
    Ok(json!({
        "items": days,
        "legend": legend
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_transaction_amount_period(
    pool: &PostgresPool,
    user_id: UserId,
    start_time: i64,
    end_time: i64,
    start_date: String,
    end_date: String,
) -> DbResult<TransactionAmountPeriodResult> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_postgres_transaction_amount_period",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let row = sqlx::query(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN lower(trim(transaction_type)) IN ('income', '收入', '2') THEN ABS(amount_cents) ELSE 0 END), 0)::BIGINT AS income_amount_cents,
            COALESCE(SUM(CASE WHEN lower(trim(transaction_type)) IN ('expense', '支出', '3') THEN ABS(amount_cents) ELSE 0 END), 0)::BIGINT AS expense_amount_cents
        FROM bills
        WHERE user_id = $1
          AND is_deleted = false
          AND occurred_at >= $2::date
          AND occurred_at < ($3::date + interval '1 day')
        "#,
    )
    .bind(user_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_one(pool)
    .await?;

    Ok(TransactionAmountPeriodResult {
        start_time,
        end_time,
        amounts: vec![TransactionAmountBucket {
            currency: "CNY".to_string(),
            income_amount_cents: row.try_get("income_amount_cents")?,
            expense_amount_cents: row.try_get("expense_amount_cents")?,
        }],
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_statistics_analyzer_report_payload(
    pool: &PostgresPool,
    user_id: UserId,
    period: &str,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let range = statistics_analyzer_period_range(period, today);
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date.clone()),
        end_date: Some(range.end_date.clone()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    let generated_at = Local::now().naive_local().to_string();
    Ok(build_statistics_analyzer_report(
        period,
        &range,
        &bills,
        &generated_at,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_statistics_analyzer_trends_payload(
    pool: &PostgresPool,
    user_id: UserId,
    period: &str,
    category: Option<&str>,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let mut buckets = Vec::<StatisticsAnalyzerTrendBucket>::new();
    for offset in (1..=12).rev() {
        let Some((period_key, start_date, end_date)) =
            analyzer_trend_period_window(period, today, offset)
        else {
            continue;
        };
        let filters = StatisticsBillFilters {
            start_date: Some(start_date.to_string()),
            end_date: Some(end_date.to_string()),
            ..StatisticsBillFilters::default()
        };
        let mut bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
        if let Some(category) = category.filter(|value| !value.trim().is_empty()) {
            bills.retain(|bill| bill.main_category == category);
        }
        buckets.push(build_statistics_analyzer_trend_bucket(&period_key, &bills));
    }
    Ok(build_statistics_analyzer_trends_result(
        period, category, &buckets,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_statistics_analyzer_comparison_payload(
    pool: &PostgresPool,
    user_id: UserId,
    period: &str,
    compare_type: &str,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let range = statistics_analyzer_period_range(period, today);
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date),
        end_date: Some(range.end_date),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    Ok(build_statistics_analyzer_comparison_result(
        period,
        compare_type,
        &bills,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_statistics_analyzer_category_payload(
    pool: &PostgresPool,
    user_id: UserId,
    period: &str,
    main_category: Option<&str>,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let range = statistics_analyzer_period_range(period, today);
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date),
        end_date: Some(range.end_date),
        ..StatisticsBillFilters::default()
    };
    let mut bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    if let Some(main_category) = main_category.filter(|value| !value.trim().is_empty()) {
        bills.retain(|bill| bill.main_category == main_category);
    }
    Ok(build_statistics_analyzer_category_result(
        period,
        main_category,
        &bills,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_net_worth_payload(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let accounts = load_postgres_statistics_accounts(pool, user_id).await?;
    Ok(json!(build_net_worth_snapshot(&accounts)))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_calendar_events_payload(
    pool: &PostgresPool,
    user_id: UserId,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> DbResult<CalendarEventsData> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let filters = StatisticsBillFilters {
        start_date: Some(start_date.to_string()),
        end_date: Some(end_date.to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    let recurring_rules = load_postgres_calendar_recurring_rules(pool, user_id).await?;
    Ok(build_calendar_events_data(
        &bills,
        &recurring_rules,
        start_date,
        end_date,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_insight_anomaly_summary_payload(
    pool: &PostgresPool,
    user_id: UserId,
    analyzed_months: u32,
    today: NaiveDate,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let end_date = today.to_string();
    let start_date = (today - Duration::days(i64::from(analyzed_months) * 30)).to_string();
    let filters = StatisticsBillFilters {
        start_date: Some(start_date.clone()),
        end_date: Some(end_date.clone()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    Ok(build_insight_anomaly_summary(
        &bills,
        analyzed_months,
        &start_date,
        &end_date,
    ))
}
