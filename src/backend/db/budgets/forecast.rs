// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


#[tracing::instrument(level = "debug", skip_all)]
fn query_budgets_raw(
    connection: &Connection,
    user_id: i64,
    filters: &BudgetFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let mut conditions = Vec::new();
    let mut values = vec![SqlValue::Integer(user_id)];
    if let Some(period_type) = text_filter(filters.period_type.as_deref()) {
        conditions.push("period_type = ?".to_string());
        values.push(SqlValue::Text(period_type));
    }
    if let Some(enabled) = filters.enabled {
        conditions.push("enabled = ?".to_string());
        values.push(SqlValue::Integer(i64::from(enabled)));
    }
    if let Some(category) = text_filter(filters.category.as_deref()) {
        conditions.push("category = ?".to_string());
        values.push(SqlValue::Text(category));
    }
    let sql = format!(
        "SELECT {} FROM budgets WHERE user_id = ?{} ORDER BY created_at DESC",
        BUDGET_SELECT_COLUMNS.join(", "),
        where_suffix(&conditions)
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), budget_record_from_row)?;
    let mut budgets = Vec::new();
    for row in rows {
        budgets.push(row?);
    }
    Ok(budgets)
}

#[tracing::instrument(level = "debug", skip_all)]
fn query_budget_forecast_rows(
    connection: &Connection,
    user_id: i64,
    filters: &BudgetForecastFilters,
    history_window: &bill_analyser_core::budgets::BudgetPeriodRange,
) -> DbResult<Vec<BudgetForecastRow>> {
    let group_by = budget_forecast_group_expr(&filters.period_type);
    let type_name = get_budget_type_name(filters.budget_type);
    let mut values = vec![
        SqlValue::Text(type_name.to_string()),
        SqlValue::Integer(user_id),
    ];
    let mut conditions = vec!["type = ?".to_string(), "user_id = ?".to_string()];
    if let Some(start_date) = text_filter(Some(&history_window.start_date)) {
        conditions.push("date >= ?".to_string());
        values.push(SqlValue::Text(start_date));
    }
    if let Some(end_date) = normalize_budget_query_end_date(Some(&history_window.end_date)) {
        conditions.push("date <= ?".to_string());
        values.push(SqlValue::Text(end_date));
    }
    let sql = format!(
        "
        SELECT {group_by} AS period,
               main_category,
               COALESCE(SUM(amount), 0) AS total_amount
        FROM bills
        WHERE {}
        GROUP BY {group_by}, main_category
        ORDER BY period DESC, total_amount DESC
        ",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        let category = row
            .get::<_, Option<String>>(1)?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未分类".to_string());
        Ok(BudgetForecastRow {
            period: row.get::<_, String>(0)?,
            category,
            amount: row.get::<_, f64>(2)?.abs(),
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

fn aggregate_budget_forecast_rows(
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
            period.amount += row.amount;
        } else {
            bucket.periods.push(BudgetForecastPeriodAmount {
                period: row.period,
                amount: row.amount,
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

#[tracing::instrument(level = "debug", skip_all)]
fn query_budget_forecast_budget_map(
    connection: &Connection,
    user_id: i64,
    filters: &BudgetForecastFilters,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
) -> DbResult<BTreeMap<String, BudgetForecastBudgetAmount>> {
    if !table_exists(connection, "budgets")? {
        return Ok(BTreeMap::new());
    }
    let mut conditions = vec!["period_type = ?".to_string(), "enabled = 1".to_string()];
    let mut values = vec![
        SqlValue::Integer(user_id),
        SqlValue::Text(filters.period_type.clone()),
    ];
    if let Some(start_date) = text_filter(Some(&filters.start_date)) {
        conditions.push("(end_date IS NULL OR end_date = '' OR end_date >= ?)".to_string());
        values.push(SqlValue::Text(start_date));
    }
    if let Some(end_date) = text_filter(Some(&filters.end_date)) {
        conditions.push("(start_date IS NULL OR start_date = '' OR start_date <= ?)".to_string());
        values.push(SqlValue::Text(end_date));
    }
    let sql = format!(
        "
        SELECT category, sub_category, amount
        FROM budgets
        WHERE user_id = ?{}
        ",
        where_suffix(&conditions)
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok((
            row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<f64>>(2)?.unwrap_or_default(),
        ))
    })?;
    let mut budget_map: BTreeMap<String, BudgetForecastBudgetAmount> = BTreeMap::new();
    for row in rows {
        let (category, sub_category, amount) = row?;
        let category = if category.trim().is_empty() {
            "未分类".to_string()
        } else {
            category
        };
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
            bucket.primary += amount;
        } else {
            bucket.sub_total += amount;
        }
    }
    Ok(budget_map)
}

fn budget_forecast_group_expr(period_type: &str) -> &'static str {
    match period_type {
        "daily" => "date(date)",
        "weekly" => "strftime('%Y-%W', date)",
        "quarterly" => {
            "strftime('%Y', date) || '-Q' || (CAST(((CAST(strftime('%m', date) AS INTEGER) - 1) / 3) AS INTEGER) + 1)"
        }
        "monthly" => "strftime('%Y-%m', date)",
        _ => "strftime('%Y', date)",
    }
}
