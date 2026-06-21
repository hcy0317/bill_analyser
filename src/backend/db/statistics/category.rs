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
