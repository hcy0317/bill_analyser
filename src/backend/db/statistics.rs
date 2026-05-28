// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::statistics::{
    build_asset_trend_legend, build_asset_trends, build_calendar_events_data,
    build_category_pie_data, build_category_statistics_items, build_category_trend_statistics,
    build_insight_anomaly_summary, build_net_worth_snapshot,
    build_statistics_analyzer_category_result, build_statistics_analyzer_comparison_result,
    build_statistics_analyzer_report, build_statistics_analyzer_trend_bucket,
    build_statistics_analyzer_trends_result, build_top_merchants_data,
    build_transaction_amount_period_result, statistics_analyzer_period_range, CalendarEventsData,
    NameValueStatisticItem, RecurringRuleInput, StatisticsAccountInput,
    StatisticsAnalyzerTrendBucket, StatisticsBillInput, StatisticsCategoryInput,
    StatisticsYearMonthRange, TopMerchantStatisticItem, TransactionAmountBucket,
    TransactionAmountPeriodResult, UserCustomExchangeRateInput,
};
use bill_analyser_core::UserId;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, SecondsFormat, Utc};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};

use crate::{DbError, DbResult, PostgresPool, UserScope};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatisticsBillFilters {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub transaction_type: Option<String>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatisticsAllDateRange {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCustomExchangeRateUpsert {
    pub update_time: i64,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_category_statistics_payload(
    connection: &Connection,
    user_id: UserId,
    filters: &StatisticsBillFilters,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_category_statistics_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_statistics_bills(connection, user_id, filters)?;
    let categories = load_statistics_categories(connection, user_id)?;
    let accounts = load_statistics_accounts(connection, user_id)?;
    let items = build_category_statistics_items(&bills, &categories, &accounts);
    Ok(json!(items))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_category_trends_payload(
    connection: &Connection,
    user_id: UserId,
    filters: &StatisticsBillFilters,
    range: &StatisticsYearMonthRange,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_category_trends_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_statistics_bills(connection, user_id, filters)?;
    let categories = load_statistics_categories(connection, user_id)?;
    let accounts = load_statistics_accounts(connection, user_id)?;
    let buckets = build_category_trend_statistics(&bills, &categories, &accounts, range);
    Ok(json!(buckets))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_asset_trends_payload(
    connection: &Connection,
    user_id: UserId,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_asset_trends_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let filters = StatisticsBillFilters {
        start_date: Some(start_date.to_string()),
        end_date: Some(end_date.to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
    let accounts = load_statistics_accounts(connection, user_id)?;
    let balances_before = load_account_balance_deltas_before(connection, user_id, start_date)?;
    let days = build_asset_trends(&bills, &accounts, &balances_before, start_date, end_date);
    let legend = build_asset_trend_legend(&accounts, &days);
    Ok(json!({
        "items": days,
        "legend": legend
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_category_pie_payload(
    connection: &Connection,
    user_id: UserId,
    filters: &StatisticsBillFilters,
) -> DbResult<Vec<NameValueStatisticItem>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_category_pie_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_statistics_bills(connection, user_id, filters)?;
    Ok(build_category_pie_data(&bills))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_top_merchants_payload(
    connection: &Connection,
    user_id: UserId,
    filters: &StatisticsBillFilters,
    limit: usize,
) -> DbResult<Vec<TopMerchantStatisticItem>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_top_merchants_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_statistics_bills(connection, user_id, filters)?;
    Ok(build_top_merchants_data(&bills, limit))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_category_statistics_payload(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &StatisticsBillFilters,
) -> DbResult<Vec<bill_analyser_core::statistics::CategoryStatisticItem>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_postgres_statistics_bills(pool, user_id, filters).await?;
    let categories = load_postgres_statistics_categories(pool, user_id).await?;
    let accounts = load_postgres_statistics_accounts(pool, user_id).await?;
    Ok(build_category_statistics_items(
        &bills,
        &categories,
        &accounts,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_category_trends_payload(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &StatisticsBillFilters,
    range: &StatisticsYearMonthRange,
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_postgres_statistics_bills(pool, user_id, filters).await?;
    let categories = load_postgres_statistics_categories(pool, user_id).await?;
    let accounts = load_postgres_statistics_accounts(pool, user_id).await?;
    Ok(json!(build_category_trend_statistics(
        &bills,
        &categories,
        &accounts,
        range
    )))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_category_pie_payload(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &StatisticsBillFilters,
) -> DbResult<Vec<NameValueStatisticItem>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_postgres_statistics_bills(pool, user_id, filters).await?;
    Ok(build_category_pie_data(&bills))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_top_merchants_payload(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &StatisticsBillFilters,
    limit: usize,
) -> DbResult<Vec<TopMerchantStatisticItem>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let bills = load_postgres_statistics_bills(pool, user_id, filters).await?;
    Ok(build_top_merchants_data(&bills, limit))
}

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
pub fn query_transaction_amount_period(
    connection: &Connection,
    user_id: UserId,
    start_time: i64,
    end_time: i64,
    start_date: String,
    end_date: String,
) -> DbResult<TransactionAmountPeriodResult> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_transaction_amount_period",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let filters = StatisticsBillFilters {
        start_date: Some(start_date),
        end_date: Some(end_date),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
    Ok(build_transaction_amount_period_result(
        start_time, end_time, &bills,
    ))
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
            COALESCE(SUM(CASE WHEN lower(trim(transaction_type)) IN ('income', '收入', '2') THEN ABS(amount_cents) ELSE 0 END), 0)::BIGINT AS income_amount,
            COALESCE(SUM(CASE WHEN lower(trim(transaction_type)) IN ('expense', '支出', '3') THEN ABS(amount_cents) ELSE 0 END), 0)::BIGINT AS expense_amount
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
            income_amount: row.try_get("income_amount")?,
            expense_amount: row.try_get("expense_amount")?,
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
pub fn query_statistics_analyzer_report_payload(
    connection: &Connection,
    user_id: UserId,
    period: &str,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_statistics_analyzer_report_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let range = statistics_analyzer_period_range(period, today);
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date.clone()),
        end_date: Some(range.end_date.clone()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
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
pub fn query_statistics_analyzer_trends_payload(
    connection: &Connection,
    user_id: UserId,
    period: &str,
    category: Option<&str>,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_statistics_analyzer_trends_payload",
        "business operation entered"
    );
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
        let mut bills = load_statistics_bills(connection, user_id, &filters)?;
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
pub fn query_statistics_analyzer_comparison_payload(
    connection: &Connection,
    user_id: UserId,
    period: &str,
    compare_type: &str,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_statistics_analyzer_comparison_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let range = statistics_analyzer_period_range(period, today);
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date),
        end_date: Some(range.end_date),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
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
pub fn query_statistics_analyzer_category_payload(
    connection: &Connection,
    user_id: UserId,
    period: &str,
    main_category: Option<&str>,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_statistics_analyzer_category_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let today = Local::now().date_naive();
    let range = statistics_analyzer_period_range(period, today);
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date),
        end_date: Some(range.end_date),
        ..StatisticsBillFilters::default()
    };
    let mut bills = load_statistics_bills(connection, user_id, &filters)?;
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
pub fn query_net_worth_payload(connection: &Connection, user_id: UserId) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_net_worth_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let accounts = load_statistics_accounts(connection, user_id)?;
    Ok(json!(build_net_worth_snapshot(&accounts)))
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
pub fn query_calendar_events_payload(
    connection: &Connection,
    user_id: UserId,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> DbResult<CalendarEventsData> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_calendar_events_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let filters = StatisticsBillFilters {
        start_date: Some(start_date.to_string()),
        end_date: Some(end_date.to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
    let recurring_rules = load_calendar_recurring_rules(connection, user_id).unwrap_or_default();
    Ok(build_calendar_events_data(
        &bills,
        &recurring_rules,
        start_date,
        end_date,
    ))
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_insight_anomaly_summary_payload(
    connection: &Connection,
    user_id: UserId,
    analyzed_months: u32,
    today: NaiveDate,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "query_insight_anomaly_summary_payload",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let end_date = today.to_string();
    let start_date = (today - Duration::days(i64::from(analyzed_months) * 30)).to_string();
    let filters = StatisticsBillFilters {
        start_date: Some(start_date.clone()),
        end_date: Some(end_date.clone()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
    Ok(build_insight_anomaly_summary(
        &bills,
        analyzed_months,
        &start_date,
        &end_date,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_statistics_user_default_currency(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<String> {
    if !table_exists(connection, "users")?
        || !column_exists(connection, "users", "default_currency")?
    {
        return Ok("CNY".to_string());
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    let currency = connection
        .query_row(
            "SELECT default_currency FROM users WHERE id = ?1 LIMIT 1",
            params![user_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .unwrap_or_else(|| "CNY".to_string());
    let normalized = currency.trim().to_uppercase();
    Ok(if normalized.is_empty() {
        "CNY".to_string()
    } else {
        normalized
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_statistics_user_default_currency(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<String> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let currency = sqlx::query(
        "
        SELECT COALESCE(
            NULLIF(metadata->>'defaultCurrency', ''),
            NULLIF(metadata->>'default_currency', ''),
            'CNY'
        ) AS default_currency
        FROM users
        WHERE id = $1
        LIMIT 1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .and_then(|row| {
        row.try_get::<Option<String>, _>("default_currency")
            .ok()
            .flatten()
    })
    .unwrap_or_else(|| "CNY".to_string());
    let normalized = currency.trim().to_uppercase();
    Ok(if normalized.is_empty() {
        "CNY".to_string()
    } else {
        normalized
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_user_custom_exchange_rates(
    connection: &Connection,
    user_id: UserId,
    base_currency: &str,
) -> DbResult<Vec<UserCustomExchangeRateInput>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "list_user_custom_exchange_rates",
        "business operation entered"
    );
    if !table_exists(connection, "user_exchange_rates")? {
        return Ok(Vec::new());
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    let base_currency = base_currency.trim().to_uppercase();
    let mut statement = connection.prepare(
        "
        SELECT to_currency, rate, effective_date
        FROM user_exchange_rates
        WHERE user_id = ?1 AND UPPER(from_currency) = ?2
        ORDER BY effective_date DESC, updated_at DESC, created_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map(params![user_id, base_currency], |row| {
        let effective_date = row.get::<_, Option<String>>(2)?;
        Ok(UserCustomExchangeRateInput {
            to_currency: row
                .get::<_, Option<String>>(0)?
                .unwrap_or_default()
                .trim()
                .to_uppercase(),
            rate: sqlite_number_text(row.get::<_, Option<f64>>(1)?.unwrap_or(1.0)),
            effective_timestamp: effective_date
                .as_deref()
                .and_then(sqlite_effective_date_timestamp),
            effective_date,
        })
    })?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for row in rows {
        let item = row?;
        if item.to_currency.is_empty() || !seen.insert(item.to_currency.clone()) {
            continue;
        }
        result.push(item);
    }
    Ok(result)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_user_custom_exchange_rates(
    pool: &PostgresPool,
    user_id: UserId,
    base_currency: &str,
) -> DbResult<Vec<UserCustomExchangeRateInput>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let value = load_postgres_custom_exchange_rates_value(pool, user_id, base_currency).await?;
    Ok(parse_postgres_custom_exchange_rates(&value))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn upsert_user_custom_exchange_rate(
    connection: &Connection,
    user_id: UserId,
    base_currency: &str,
    currency: &str,
    rate: f64,
) -> DbResult<UserCustomExchangeRateUpsert> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "upsert_user_custom_exchange_rate",
        "business operation entered"
    );
    if !table_exists(connection, "user_exchange_rates")? {
        return Err(DbError::InvalidOperation(
            "user_exchange_rates table is not initialized".to_string(),
        ));
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    let base_currency = base_currency.trim().to_uppercase();
    let currency = currency.trim().to_uppercase();
    let now = chrono::Utc::now();
    let update_time = now.timestamp();
    let effective_date = now.date_naive().to_string();
    let timestamp = now.to_rfc3339_opts(SecondsFormat::Secs, true);
    connection.execute(
        "
        INSERT INTO user_exchange_rates(
            user_id, from_currency, to_currency, rate, source, effective_date, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, 'manual', ?5, ?6, ?6)
        ON CONFLICT(user_id, from_currency, to_currency, effective_date)
        DO UPDATE SET rate = excluded.rate, source = excluded.source, updated_at = excluded.updated_at
        ",
        params![user_id, base_currency, currency, rate, effective_date, timestamp],
    )?;
    Ok(UserCustomExchangeRateUpsert { update_time })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn upsert_postgres_user_custom_exchange_rate(
    pool: &PostgresPool,
    user_id: UserId,
    base_currency: &str,
    currency: &str,
    rate: f64,
) -> DbResult<UserCustomExchangeRateUpsert> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let base_currency = base_currency.trim().to_uppercase();
    let currency = currency.trim().to_uppercase();
    let now = chrono::Utc::now();
    let update_time = now.timestamp();
    let effective_date = now.date_naive().to_string();
    let mut rates = parse_postgres_custom_exchange_rates(
        &load_postgres_custom_exchange_rates_value(pool, user_id, &base_currency).await?,
    );
    rates.retain(|item| item.to_currency != currency);
    rates.push(UserCustomExchangeRateInput {
        to_currency: currency,
        rate: sqlite_number_text(rate),
        effective_timestamp: Some(update_time),
        effective_date: Some(effective_date),
    });
    rates.sort_by(|left, right| left.to_currency.cmp(&right.to_currency));
    let value = postgres_custom_exchange_rates_value(&rates);
    sqlx::query(
        "
        INSERT INTO settings(user_id, key, value, sensitive)
        VALUES ($1, $2, $3, false)
        ON CONFLICT(user_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now(), version = settings.version + 1
        ",
    )
    .bind(user_id)
    .bind(postgres_custom_exchange_rates_key(&base_currency))
    .bind(value)
    .execute(pool)
    .await?;
    Ok(UserCustomExchangeRateUpsert { update_time })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn delete_user_custom_exchange_rate(
    connection: &Connection,
    user_id: UserId,
    base_currency: &str,
    currency: &str,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "delete_user_custom_exchange_rate",
        "business operation entered"
    );
    if !table_exists(connection, "user_exchange_rates")? {
        return Ok(false);
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    let changed = connection.execute(
        "
        DELETE FROM user_exchange_rates
        WHERE user_id = ?1 AND UPPER(from_currency) = ?2 AND UPPER(to_currency) = ?3
        ",
        params![
            user_id,
            base_currency.trim().to_uppercase(),
            currency.trim().to_uppercase()
        ],
    )?;
    Ok(changed > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_user_custom_exchange_rate(
    pool: &PostgresPool,
    user_id: UserId,
    base_currency: &str,
    currency: &str,
) -> DbResult<bool> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let base_currency = base_currency.trim().to_uppercase();
    let currency = currency.trim().to_uppercase();
    let mut rates = parse_postgres_custom_exchange_rates(
        &load_postgres_custom_exchange_rates_value(pool, user_id, &base_currency).await?,
    );
    let old_len = rates.len();
    rates.retain(|item| item.to_currency != currency);
    if rates.len() == old_len {
        return Ok(false);
    }
    let value = postgres_custom_exchange_rates_value(&rates);
    sqlx::query(
        "
        INSERT INTO settings(user_id, key, value, sensitive)
        VALUES ($1, $2, $3, false)
        ON CONFLICT(user_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now(), version = settings.version + 1
        ",
    )
    .bind(user_id)
    .bind(postgres_custom_exchange_rates_key(&base_currency))
    .bind(value)
    .execute(pool)
    .await?;
    Ok(true)
}

async fn load_postgres_custom_exchange_rates_value(
    pool: &PostgresPool,
    user_id: i64,
    base_currency: &str,
) -> DbResult<Value> {
    let key = postgres_custom_exchange_rates_key(base_currency);
    let value = sqlx::query("SELECT value FROM settings WHERE user_id = $1 AND key = $2 LIMIT 1")
        .bind(user_id)
        .bind(key)
        .fetch_optional(pool)
        .await?
        .and_then(|row| row.try_get::<Value, _>("value").ok())
        .unwrap_or_else(|| json!({ "rates": [] }));
    Ok(value)
}

fn postgres_custom_exchange_rates_key(base_currency: &str) -> String {
    format!(
        "statistics.custom_exchange_rates.{}",
        base_currency.trim().to_uppercase()
    )
}

fn parse_postgres_custom_exchange_rates(value: &Value) -> Vec<UserCustomExchangeRateInput> {
    let Some(rates) = value.get("rates").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for item in rates {
        let currency = item
            .get("currency")
            .and_then(Value::as_str)
            .or_else(|| item.get("toCurrency").and_then(Value::as_str))
            .unwrap_or_default()
            .trim()
            .to_uppercase();
        if currency.is_empty() || !seen.insert(currency.clone()) {
            continue;
        }
        let rate = item
            .get("rate")
            .and_then(postgres_value_to_f64)
            .map(sqlite_number_text)
            .unwrap_or_else(|| "1.0".to_string());
        let effective_date = item
            .get("effectiveDate")
            .and_then(Value::as_str)
            .or_else(|| item.get("effective_date").and_then(Value::as_str))
            .map(ToOwned::to_owned);
        let effective_timestamp = item
            .get("effectiveTimestamp")
            .and_then(Value::as_i64)
            .or_else(|| item.get("effective_timestamp").and_then(Value::as_i64))
            .or_else(|| {
                effective_date
                    .as_deref()
                    .and_then(sqlite_effective_date_timestamp)
            });
        result.push(UserCustomExchangeRateInput {
            to_currency: currency,
            rate,
            effective_timestamp,
            effective_date,
        });
    }
    result
}

fn postgres_custom_exchange_rates_value(rates: &[UserCustomExchangeRateInput]) -> Value {
    json!({
        "rates": rates
            .iter()
            .map(|item| {
                json!({
                    "currency": item.to_currency.clone(),
                    "rate": item.rate.parse::<f64>().unwrap_or(1.0),
                    "effectiveDate": item.effective_date.clone(),
                    "effectiveTimestamp": item.effective_timestamp
                })
            })
            .collect::<Vec<_>>()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn find_statistics_all_date_range(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Option<StatisticsAllDateRange>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "find_statistics_all_date_range",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let value = connection
        .query_row(
            "SELECT MIN(substr(date, 1, 10)), MAX(substr(date, 1, 10)) FROM bills WHERE user_id = ?1",
            params![user_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?;
    let Some((Some(start_date), Some(end_date))) = value else {
        return Ok(None);
    };
    if start_date.trim().is_empty() || end_date.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(StatisticsAllDateRange {
        start_date,
        end_date,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn find_postgres_statistics_all_date_range(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<StatisticsAllDateRange>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let row = sqlx::query(
        "
        SELECT
            MIN(occurred_at::date)::TEXT AS start_date,
            MAX(occurred_at::date)::TEXT AS end_date
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
        ",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let start_date: Option<String> = row.try_get("start_date")?;
    let end_date: Option<String> = row.try_get("end_date")?;
    let (Some(start_date), Some(end_date)) = (start_date, end_date) else {
        return Ok(None);
    };
    if start_date.trim().is_empty() || end_date.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(StatisticsAllDateRange {
        start_date,
        end_date,
    }))
}

fn analyzer_trend_period_window(
    period: &str,
    today: NaiveDate,
    offset: i64,
) -> Option<(String, NaiveDate, NaiveDate)> {
    match period {
        "month" => {
            let target = today - Duration::days(30 * offset);
            let start = first_day(target.year(), target.month())?;
            let end = add_months(start, 1)? - Duration::days(1);
            Some((format!("{}-{:02}", start.year(), start.month()), start, end))
        }
        "year" => {
            let year = today.year() - offset as i32;
            let start = first_day(year, 1)?;
            let end = first_day(year, 12)?.with_day(31)?;
            Some((format!("{year:04}"), start, end))
        }
        _ => None,
    }
}

fn first_day(year: i32, month: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, 1)
}

fn add_months(date: NaiveDate, months: u32) -> Option<NaiveDate> {
    let zero_based = date.month0() + months;
    let year = date.year() + (zero_based / 12) as i32;
    let month = (zero_based % 12) + 1;
    first_day(year, month)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_statistics_bills(
    connection: &Connection,
    user_id: i64,
    filters: &StatisticsBillFilters,
) -> DbResult<Vec<StatisticsBillInput>> {
    if !table_exists(connection, "bills")? {
        return Ok(Vec::new());
    }
    let payment_method_expr = optional_column_expr(connection, "bills", "payment_method", "''")?;
    let counterparty_expr = optional_column_expr(connection, "bills", "counterparty", "''")?;
    let description_expr = optional_column_expr(connection, "bills", "description", "''")?;
    let main_category_expr = optional_column_expr(connection, "bills", "main_category", "''")?;
    let sub_category_expr = optional_column_expr(connection, "bills", "sub_category", "''")?;
    let source_account_expr =
        optional_column_expr(connection, "bills", "source_account_id", "NULL")?;
    let destination_account_expr =
        optional_column_expr(connection, "bills", "destination_account_id", "NULL")?;
    let destination_amount_expr =
        optional_column_expr(connection, "bills", "destination_amount", "NULL")?;

    let mut conditions = vec!["user_id = ?".to_string()];
    let mut values = vec![SqlValue::Integer(user_id)];
    if let Some(start_date) = text_filter(filters.start_date.as_deref()) {
        conditions.push("substr(date, 1, 10) >= ?".to_string());
        values.push(SqlValue::Text(start_date));
    }
    if let Some(end_date) = text_filter(filters.end_date.as_deref()) {
        conditions.push("substr(date, 1, 10) <= ?".to_string());
        values.push(SqlValue::Text(end_date));
    }
    if let Some(transaction_type) = text_filter(filters.transaction_type.as_deref()) {
        conditions.push("type = ?".to_string());
        values.push(SqlValue::Text(transaction_type));
    }
    if let Some(keyword) = text_filter(filters.keyword.as_deref()) {
        conditions.push(format!(
            "({description_expr} LIKE ? OR {counterparty_expr} LIKE ?)"
        ));
        values.push(SqlValue::Text(format!("%{keyword}%")));
        values.push(SqlValue::Text(format!("%{keyword}%")));
    }

    let sql = format!(
        "
        SELECT id, date, type, amount,
               {payment_method_expr} AS payment_method,
               {source_account_expr} AS source_account_id,
               {destination_account_expr} AS destination_account_id,
               {destination_amount_expr} AS destination_amount,
               {main_category_expr} AS main_category,
               {sub_category_expr} AS sub_category,
               {counterparty_expr} AS counterparty,
               {description_expr} AS description
        FROM bills
        WHERE {}
        ORDER BY date ASC, id ASC
        ",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok(StatisticsBillInput {
            id: row.get::<_, Option<i64>>("id")?,
            date: row.get::<_, String>("date")?,
            bill_type: row.get::<_, String>("type")?,
            amount_yuan: sqlite_number_text(row.get::<_, Option<f64>>("amount")?.unwrap_or(0.0)),
            channel: row
                .get::<_, Option<String>>("payment_method")?
                .unwrap_or_default(),
            source_account_id: positive_i64(row.get::<_, Option<i64>>("source_account_id")?),
            destination_account_id: positive_i64(
                row.get::<_, Option<i64>>("destination_account_id")?,
            ),
            destination_account: String::new(),
            destination_amount_yuan: row
                .get::<_, Option<f64>>("destination_amount")?
                .map(sqlite_number_text),
            main_category: row
                .get::<_, Option<String>>("main_category")?
                .unwrap_or_default(),
            sub_category: row
                .get::<_, Option<String>>("sub_category")?
                .unwrap_or_default(),
            counterparty: row
                .get::<_, Option<String>>("counterparty")?
                .unwrap_or_default(),
            description: row
                .get::<_, Option<String>>("description")?
                .unwrap_or_default(),
        })
    })?;
    collect_rows(rows)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_statistics_bills(
    pool: &PostgresPool,
    user_id: i64,
    filters: &StatisticsBillFilters,
) -> DbResult<Vec<StatisticsBillInput>> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT b.id, b.occurred_at, b.transaction_type, b.amount_cents, b.payment_method, b.account_id, b.source_account_id, b.target_account_id, b.transfer_target_account_id, b.standard_payload, ",
    );
    builder.push("COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(c.path, '/', 1), ''), c.name, '') AS main_category, ");
    builder.push("COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), CASE WHEN position('/' in COALESCE(c.path, '')) > 0 THEN substring(c.path from position('/' in c.path) + 1) ELSE '' END, '') AS sub_category, ");
    builder.push("b.merchant, b.description FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false");
    if let Some(start_date) = text_filter(filters.start_date.as_deref()) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(start_date);
        builder.push("::date");
    }
    if let Some(end_date) = text_filter(filters.end_date.as_deref()) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(end_date);
        builder.push("::date + interval '1 day')");
    }
    if let Some(transaction_type) = text_filter(filters.transaction_type.as_deref()) {
        builder.push(" AND (b.transaction_type = ");
        builder.push_bind(canonical_postgres_transaction_type(&transaction_type));
        builder.push(" OR b.standard_payload->>'type' = ");
        builder.push_bind(transaction_type);
        builder.push(")");
    }
    if let Some(keyword) = text_filter(filters.keyword.as_deref()) {
        let pattern = format!("%{keyword}%");
        builder.push(" AND (b.description ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR b.merchant ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    builder.push(" ORDER BY b.occurred_at ASC, b.id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let payload: Value = row.try_get("standard_payload")?;
            let amount_cents: i64 = row.try_get("amount_cents")?;
            Ok(StatisticsBillInput {
                id: row.try_get("id")?,
                date: postgres_timestamp_text(row.try_get("occurred_at")?),
                bill_type: row
                    .try_get::<Option<String>, _>("transaction_type")?
                    .unwrap_or_default(),
                amount_yuan: sqlite_number_text(amount_cents as f64 / 100.0),
                channel: row
                    .try_get::<Option<String>, _>("payment_method")?
                    .unwrap_or_default(),
                source_account_id: positive_i64(
                    row.try_get::<Option<i64>, _>("source_account_id")?
                        .or(row.try_get::<Option<i64>, _>("account_id")?),
                ),
                destination_account_id: positive_i64(
                    row.try_get::<Option<i64>, _>("target_account_id")?
                        .or(row.try_get::<Option<i64>, _>("transfer_target_account_id")?),
                ),
                destination_account: String::new(),
                destination_amount_yuan: payload
                    .get("destination_amount")
                    .or_else(|| payload.get("destinationAmount"))
                    .and_then(postgres_value_to_f64)
                    .map(sqlite_number_text),
                main_category: row
                    .try_get::<Option<String>, _>("main_category")?
                    .unwrap_or_default(),
                sub_category: row
                    .try_get::<Option<String>, _>("sub_category")?
                    .unwrap_or_default(),
                counterparty: row
                    .try_get::<Option<String>, _>("merchant")?
                    .unwrap_or_default(),
                description: row
                    .try_get::<Option<String>, _>("description")?
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_statistics_categories(
    connection: &Connection,
    user_id: i64,
) -> DbResult<Vec<StatisticsCategoryInput>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "
        SELECT id, main_category, sub_category
        FROM categories
        WHERE user_id = ?1
        ORDER BY id ASC
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(StatisticsCategoryInput {
            id: row.get::<_, i64>(0)?,
            main_category: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            sub_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        })
    })?;
    collect_rows(rows)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_statistics_categories(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<StatisticsCategoryInput>> {
    let rows =
        sqlx::query("SELECT id, path, name FROM categories WHERE user_id = $1 ORDER BY id ASC")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) =
                postgres_category_names_from_path(path.as_deref(), &name);
            Ok(StatisticsCategoryInput {
                id: row.try_get("id")?,
                main_category,
                sub_category,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_statistics_accounts(
    connection: &Connection,
    user_id: i64,
) -> DbResult<Vec<StatisticsAccountInput>> {
    if !table_exists(connection, "accounts")? {
        return Ok(Vec::new());
    }
    let type_expr = optional_column_expr(connection, "accounts", "type", "''")?;
    let hidden_expr = optional_column_expr(connection, "accounts", "hidden", "0")?;
    let balance_expr = optional_column_expr(connection, "accounts", "balance", "0")?;
    let initial_balance_expr =
        optional_column_expr(connection, "accounts", "initial_balance", "0")?;
    let currency_expr = optional_column_expr(connection, "accounts", "currency", "NULL")?;
    let icon_expr = optional_column_expr(connection, "accounts", "icon", "NULL")?;

    let sql = format!(
        "
        SELECT id, name,
               {type_expr} AS account_type,
               {hidden_expr} AS hidden,
               {balance_expr} AS balance,
               {initial_balance_expr} AS initial_balance,
               {currency_expr} AS currency,
               {icon_expr} AS icon
        FROM accounts
        WHERE user_id = ?1
        ORDER BY id ASC
        "
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(StatisticsAccountInput {
            id: row.get::<_, i64>("id")?,
            name: row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            account_type: sqlite_dynamic_text(row, "account_type")?,
            hidden: row.get::<_, Option<i64>>("hidden")?.unwrap_or_default() != 0,
            balance_yuan: sqlite_number_text(row.get::<_, Option<f64>>("balance")?.unwrap_or(0.0)),
            initial_balance_yuan: sqlite_number_text(
                row.get::<_, Option<f64>>("initial_balance")?.unwrap_or(0.0),
            ),
            currency: row.get::<_, Option<String>>("currency")?,
            icon: row.get::<_, Option<String>>("icon")?,
        })
    })?;
    collect_rows(rows)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_statistics_accounts(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<StatisticsAccountInput>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, account_type, is_active, balance_cents, currency, metadata
        FROM accounts
        WHERE user_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let metadata: Value = row.try_get("metadata")?;
            let balance_cents: i64 = row.try_get("balance_cents")?;
            let initial_balance_yuan = metadata
                .get("initial_balance")
                .and_then(postgres_value_to_f64)
                .unwrap_or(balance_cents as f64 / 100.0);
            Ok(StatisticsAccountInput {
                id: row.try_get("id")?,
                name: row
                    .try_get::<Option<String>, _>("name")?
                    .unwrap_or_default(),
                account_type: row
                    .try_get::<Option<String>, _>("account_type")?
                    .unwrap_or_default(),
                hidden: !row.try_get::<bool, _>("is_active")?,
                balance_yuan: sqlite_number_text(balance_cents as f64 / 100.0),
                initial_balance_yuan: sqlite_number_text(initial_balance_yuan),
                currency: row.try_get("currency")?,
                icon: metadata
                    .get("icon")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_calendar_recurring_rules(
    connection: &Connection,
    user_id: i64,
) -> DbResult<Vec<RecurringRuleInput>> {
    if !table_exists(connection, "recurring_bills")? {
        return Ok(Vec::new());
    }
    let id_expr = optional_column_expr(connection, "recurring_bills", "id", "NULL")?;
    let name_expr = optional_column_expr(connection, "recurring_bills", "name", "''")?;
    let amount_expr = optional_column_expr(connection, "recurring_bills", "amount", "0")?;
    let type_expr = optional_column_expr(connection, "recurring_bills", "type", "''")?;
    let frequency_expr = optional_column_expr(connection, "recurring_bills", "frequency", "''")?;
    let next_date_expr = optional_column_expr(connection, "recurring_bills", "next_date", "''")?;
    let enabled_expr = optional_column_expr(connection, "recurring_bills", "enabled", "1")?;
    let display_order_expr =
        optional_column_expr(connection, "recurring_bills", "display_order", "0")?;
    let sql = format!(
        "
        SELECT {id_expr} AS id,
               {name_expr} AS name,
               {amount_expr} AS amount,
               {type_expr} AS bill_type,
               {frequency_expr} AS frequency,
               {next_date_expr} AS next_date
        FROM recurring_bills
        WHERE user_id = ?1 AND {enabled_expr} = 1
        ORDER BY COALESCE({display_order_expr}, 0), name
        "
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(RecurringRuleInput {
            id: positive_i64(row.get::<_, Option<i64>>("id")?),
            name: row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            amount_yuan: sqlite_number_text(row.get::<_, Option<f64>>("amount")?.unwrap_or(0.0)),
            bill_type: row
                .get::<_, Option<String>>("bill_type")?
                .unwrap_or_default(),
            frequency: row
                .get::<_, Option<String>>("frequency")?
                .unwrap_or_default(),
            next_date: row
                .get::<_, Option<String>>("next_date")?
                .unwrap_or_default(),
        })
    })?;
    collect_rows(rows)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_calendar_recurring_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<RecurringRuleInput>> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            name,
            source_amount_minor_units,
            transaction_type,
            scheduled_frequency,
            scheduled_next_date
        FROM transaction_templates
        WHERE user_id = $1
          AND template_type = 2
          AND enabled = true
          AND hidden = false
        ORDER BY display_order ASC, name ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let amount_minor: i64 = row.try_get("source_amount_minor_units")?;
            Ok(RecurringRuleInput {
                id: Some(row.try_get("id")?),
                name: row
                    .try_get::<Option<String>, _>("name")?
                    .unwrap_or_default(),
                amount_yuan: sqlite_number_text(amount_minor as f64 / 100.0),
                bill_type: row
                    .try_get::<Option<String>, _>("transaction_type")?
                    .unwrap_or_default(),
                frequency: row
                    .try_get::<Option<String>, _>("scheduled_frequency")?
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "monthly".to_string()),
                next_date: row
                    .try_get::<Option<String>, _>("scheduled_next_date")?
                    .unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_account_balance_deltas_before(
    connection: &Connection,
    user_id: i64,
    start_date: NaiveDate,
) -> DbResult<BTreeMap<i64, String>> {
    let filters = StatisticsBillFilters {
        end_date: Some((start_date - chrono::Duration::days(1)).to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_statistics_bills(connection, user_id, &filters)?;
    let account_ids = load_statistics_accounts(connection, user_id)?
        .into_iter()
        .map(|account| account.id)
        .collect::<Vec<_>>();
    let mut deltas = BTreeMap::new();
    for account_id in account_ids {
        let mut cents = 0_i64;
        for bill in &bills {
            let amount = bill.amount_yuan.parse::<f64>().unwrap_or_default().abs();
            let amount_cents = (amount * 100.0).round() as i64;
            if is_income_type(&bill.bill_type) && bill.source_account_id == Some(account_id) {
                cents += amount_cents;
            } else if is_expense_type(&bill.bill_type) && bill.source_account_id == Some(account_id)
            {
                cents -= amount_cents;
            } else if is_transfer_type(&bill.bill_type) {
                if bill.source_account_id == Some(account_id) {
                    cents -= amount_cents;
                }
                if bill.destination_account_id == Some(account_id) {
                    let destination_cents = bill
                        .destination_amount_yuan
                        .as_deref()
                        .and_then(|value| value.parse::<f64>().ok())
                        .map(|value| (value.abs() * 100.0).round() as i64)
                        .filter(|value| *value != 0)
                        .unwrap_or(amount_cents);
                    cents += destination_cents;
                }
            }
        }
        deltas.insert(account_id, sqlite_number_text(cents as f64 / 100.0));
    }
    Ok(deltas)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_account_balance_deltas_before(
    pool: &PostgresPool,
    user_id: i64,
    start_date: NaiveDate,
) -> DbResult<BTreeMap<i64, String>> {
    let filters = StatisticsBillFilters {
        end_date: Some((start_date - chrono::Duration::days(1)).to_string()),
        ..StatisticsBillFilters::default()
    };
    let bills = load_postgres_statistics_bills(pool, user_id, &filters).await?;
    let account_ids = load_postgres_statistics_accounts(pool, user_id)
        .await?
        .into_iter()
        .map(|account| account.id)
        .collect::<Vec<_>>();
    let mut deltas = BTreeMap::new();
    for account_id in account_ids {
        let mut cents = 0_i64;
        for bill in &bills {
            let amount = bill.amount_yuan.parse::<f64>().unwrap_or_default().abs();
            let amount_cents = (amount * 100.0).round() as i64;
            if is_income_type(&bill.bill_type) && bill.source_account_id == Some(account_id) {
                cents += amount_cents;
            } else if is_expense_type(&bill.bill_type) && bill.source_account_id == Some(account_id)
            {
                cents -= amount_cents;
            } else if is_transfer_type(&bill.bill_type) {
                if bill.source_account_id == Some(account_id) {
                    cents -= amount_cents;
                }
                if bill.destination_account_id == Some(account_id) {
                    let destination_cents = bill
                        .destination_amount_yuan
                        .as_deref()
                        .and_then(|value| value.parse::<f64>().ok())
                        .map(|value| (value.abs() * 100.0).round() as i64)
                        .filter(|value| *value != 0)
                        .unwrap_or(amount_cents);
                    cents += destination_cents;
                }
            }
        }
        deltas.insert(account_id, sqlite_number_text(cents as f64 / 100.0));
    }
    Ok(deltas)
}

fn table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn column_exists(connection: &Connection, table_name: &str, column_name: &str) -> DbResult<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn optional_column_expr(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    fallback: &str,
) -> DbResult<String> {
    if column_exists(connection, table_name, column_name)? {
        Ok(column_name.to_string())
    } else {
        Ok(fallback.to_string())
    }
}

fn collect_rows<T>(rows: impl Iterator<Item = rusqlite::Result<T>>) -> Result<Vec<T>, DbError> {
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

fn sqlite_dynamic_text(row: &rusqlite::Row<'_>, key: &str) -> rusqlite::Result<String> {
    let value = row.get_ref(key)?;
    Ok(match value {
        rusqlite::types::ValueRef::Null => String::new(),
        rusqlite::types::ValueRef::Integer(value) => value.to_string(),
        rusqlite::types::ValueRef::Real(value) => sqlite_number_text(value),
        rusqlite::types::ValueRef::Text(value) => String::from_utf8_lossy(value).to_string(),
        rusqlite::types::ValueRef::Blob(_) => String::new(),
    })
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn canonical_postgres_transaction_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => "income",
        "支出" | "expense" | "3" => "expense",
        "转账" | "transfer" | "4" => "transfer",
        "投资" | "investment" | "5" => "investment",
        other => other,
    }
    .to_string()
}

fn postgres_category_names_from_path(path: Option<&str>, name: &str) -> (String, String) {
    let parts = path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

fn postgres_value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn postgres_timestamp_text(timestamp: DateTime<Utc>) -> String {
    timestamp
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn positive_i64(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

fn sqlite_number_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

fn sqlite_effective_date_timestamp(value: &str) -> Option<i64> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(timestamp) = text.parse::<i64>() {
        return Some(timestamp);
    }
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(datetime.timestamp());
    }
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S") {
        return Some(datetime.and_utc().timestamp());
    }
    NaiveDate::parse_from_str(text.get(0..10)?, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|datetime| datetime.and_utc().timestamp())
}

fn is_expense_type(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "expense" | "支出")
}

fn is_income_type(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "income" | "收入")
}

fn is_transfer_type(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "transfer" | "转账")
}
