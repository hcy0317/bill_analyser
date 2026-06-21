async fn fetch_postgres_budget_execution_history_items(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetExecutionFilters,
    filter_summary: &str,
) -> DbResult<Vec<BudgetRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT
            bh.id AS id,
            bh.budget_id AS budget_id,
            bh.period_start AS period_start,
            bh.period_end AS period_end,
            bh.budget_amount_cents AS budget_amount_cents,
            bh.spent_amount_cents AS spent_amount_cents,
            bh.remaining_amount_cents AS remaining_amount_cents,
            bh.execution_rate::DOUBLE PRECISION AS execution_rate,
            bh.status AS status,
            bh.filter_summary AS filter_summary,
            bh.calculated_at AS calculated_at,
            b.name AS name,
            b.category AS category,
            b.sub_category AS sub_category,
            b.period_type AS period_type,
            b.alert_threshold AS alert_threshold,
            b.enabled AS enabled
        FROM budget_history bh
        INNER JOIN budgets b ON b.id = bh.budget_id
        WHERE bh.user_id =
        "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND b.user_id = ");
    builder.push_bind(user_id);
    if let Some(budget_id) = filters.budget_id {
        builder.push(" AND bh.budget_id = ");
        builder.push_bind(budget_id);
    }
    if let Some(start_date) = text_filter(filters.start_date.as_deref()) {
        builder.push(" AND bh.period_end >= ");
        builder.push_bind(start_date);
        builder.push("::date");
    }
    if let Some(end_date) = text_filter(filters.end_date.as_deref()) {
        builder.push(" AND bh.period_start <= ");
        builder.push_bind(end_date);
        builder.push("::date");
    }
    builder.push(" AND bh.filter_summary = ");
    builder.push_bind(filter_summary.to_string());
    builder.push(" ORDER BY bh.period_start DESC, bh.calculated_at DESC, bh.budget_id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(budget_history_record_from_postgres_row)
        .collect()
}

async fn build_postgres_budget_execution_history_on_demand(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
    filter_summary: &str,
) -> DbResult<Vec<Value>> {
    let period_type = filters.period_type.as_deref().unwrap_or("monthly");
    let (Some(start_date), Some(end_date)) =
        (filters.start_date.as_deref(), filters.end_date.as_deref())
    else {
        return Ok(Vec::new());
    };
    let period_ranges = iter_budget_history_period_ranges(period_type, start_date, end_date)
        .map_err(DbError::InvalidOperation)?;
    let mut history_items = Vec::new();
    for period in period_ranges {
        let mut period_filters = filters.clone();
        period_filters.start_date = Some(period.start_date.clone());
        period_filters.end_date = Some(period.end_date.clone());
        let details =
            query_postgres_budget_execution_details(pool, user_id, &period_filters).await?;
        for detail in details {
            if !super::budget_detail_overlaps_period(&detail, &period)? {
                continue;
            }
            history_items.push(build_budget_history_item_from_detail_with_context(
                &detail,
                &period,
                filters.budget_type,
                period_type,
                filter_summary,
            ));
        }
    }
    let mut records = history_items
        .into_iter()
        .filter_map(|item| match item {
            Value::Object(object) => Some(object),
            _ => None,
        })
        .collect::<Vec<_>>();
    super::sort_budget_execution_history_items(&mut records);
    Ok(records.into_iter().map(Value::Object).collect())
}

fn budget_history_record_from_postgres_row(row: PgRow) -> DbResult<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.try_get("id")?));
    record.insert("budget_id".to_string(), json_i64(row.try_get("budget_id")?));
    record.insert(
        "period_start".to_string(),
        row.try_get::<NaiveDate, _>("period_start")?
            .to_string()
            .into(),
    );
    record.insert(
        "period_end".to_string(),
        row.try_get::<NaiveDate, _>("period_end")?
            .to_string()
            .into(),
    );
    for key in [
        "status",
        "filter_summary",
        "name",
        "category",
        "sub_category",
        "period_type",
    ] {
        record.insert(
            key.to_string(),
            optional_string_value(row.try_get::<Option<String>, _>(key)?),
        );
    }
    for key in [
        "budget_amount_cents",
        "spent_amount_cents",
        "remaining_amount_cents",
    ] {
        let cents: i64 = row.try_get(key)?;
        record.insert(key.to_string(), json_i64(cents));
    }
    record.insert(
        "execution_rate".to_string(),
        json_real(
            row.try_get::<Option<f64>, _>("execution_rate")?
                .unwrap_or_default(),
        ),
    );
    insert_timestamp(&mut record, "calculated_at", row.try_get("calculated_at")?);
    record.insert(
        "alert_threshold".to_string(),
        json_i64(i64::from(row.try_get::<i32, _>("alert_threshold")?)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(i64::from(row.try_get::<bool, _>("enabled")?)),
    );
    Ok(record)
}
