async fn query_postgres_budget_forecast_rows(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetForecastFilters,
    history_window: &bill_analyser_core::budgets::BudgetPeriodRange,
) -> DbResult<Vec<BudgetForecastRow>> {
    let group_by = postgres_budget_forecast_group_expr(&filters.period_type);
    let type_name = get_budget_type_name(filters.budget_type);
    let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
    builder.push(group_by);
    builder.push(" AS period, ");
    builder.push(postgres_bill_main_category_expr());
    builder.push(" AS main_category, COALESCE(SUM(ABS(b.amount_cents)), 0)::BIGINT AS total_cents FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false");
    builder.push(" AND (b.transaction_type = ");
    builder.push_bind(canonical_budget_transaction_type(type_name));
    builder.push(" OR b.transaction_type = ");
    builder.push_bind(type_name.to_string());
    builder.push(" OR b.standard_payload->>'type' = ");
    builder.push_bind(type_name.to_string());
    builder.push(")");
    if let Some(start_date) = text_filter(Some(&history_window.start_date)) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(start_date);
        builder.push("::date");
    }
    if let Some(end_date) = super::normalize_budget_query_end_date(Some(&history_window.end_date)) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(end_date);
        builder.push("::date + interval '1 day')");
    }
    builder.push(" GROUP BY ");
    builder.push(group_by);
    builder.push(", ");
    builder.push(postgres_bill_main_category_expr());
    builder.push(" ORDER BY period DESC, total_cents DESC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let category = row
                .try_get::<Option<String>, _>("main_category")?
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "未分类".to_string());
            let cents: i64 = row.try_get("total_cents")?;
            Ok(BudgetForecastRow {
                period: row.try_get("period")?,
                category,
                amount_cents: cents,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

fn aggregate_postgres_budget_forecast_rows(
    rows: Vec<BudgetForecastRow>,
) -> (BTreeMap<String, BudgetForecastCategoryTotals>, usize) {
    let mut category_totals: BTreeMap<String, BudgetForecastCategoryTotals> = BTreeMap::new();
    let mut period_keys = BTreeSet::new();
    for row in rows {
        period_keys.insert(row.period.clone());
        let bucket = category_totals.entry(row.category).or_default();
        if let Some(period) = bucket
            .periods
            .iter_mut()
            .find(|period| period.period == row.period)
        {
            period.amount_cents += row.amount_cents;
        } else {
            bucket.periods.push(BudgetForecastPeriodAmount {
                period: row.period,
                amount_cents: row.amount_cents,
            });
        }
    }
    let period_count = if period_keys.is_empty() {
        1
    } else {
        period_keys.len()
    };
    (category_totals, period_count)
}

async fn query_postgres_budget_forecast_budget_map(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetForecastFilters,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
) -> DbResult<BTreeMap<String, BudgetForecastBudgetAmount>> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT category, sub_category, amount_cents
        FROM budgets
        WHERE user_id =
        "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND period_type = ");
    builder.push_bind(filters.period_type.clone());
    builder.push(" AND enabled = true");
    if let Some(start_date) = text_filter(Some(&filters.start_date)) {
        builder.push(" AND (end_date IS NULL OR end_date >= ");
        builder.push_bind(start_date);
        builder.push("::date)");
    }
    if let Some(end_date) = text_filter(Some(&filters.end_date)) {
        builder.push(" AND start_date <= ");
        builder.push_bind(end_date);
        builder.push("::date");
    }
    let rows = builder.build().fetch_all(pool).await?;
    let mut budget_map: BTreeMap<String, BudgetForecastBudgetAmount> = BTreeMap::new();
    for row in rows {
        let category = row
            .try_get::<Option<String>, _>("category")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未分类".to_string());
        let sub_category = row
            .try_get::<Option<String>, _>("sub_category")?
            .unwrap_or_default();
        let amount_cents: i64 = row.try_get("amount_cents")?;
        let resolved_type = resolve_budget_category_type(
            category_context,
            &category,
            Some(&sub_category),
            Some(filters.budget_type),
        );
        if resolved_type.is_none_or(|category_type| category_type.code() != filters.budget_type) {
            continue;
        }
        let bucket = budget_map.entry(category).or_default();
        if sub_category.trim().is_empty() {
            bucket.primary_cents += amount_cents;
        } else {
            bucket.sub_total_cents += amount_cents;
        }
    }
    Ok(budget_map)
}
