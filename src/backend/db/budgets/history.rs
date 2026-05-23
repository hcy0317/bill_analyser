// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


pub fn query_budget_execution_history(
    connection: &Connection,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<Value>> {
    let user_id_value = UserScope::new(user_id).bind_value()?;
    let categories = load_category_context_values(connection, user_id_value)?;
    let category_context = build_budget_category_context(&categories);
    let filter_summary = budget_history_filter_summary(filters);
    let mut history_items = if table_exists(connection, "budget_history")? {
        enrich_budget_execution_history_items(
            fetch_budget_execution_history_items(
                connection,
                user_id_value,
                filters,
                &filter_summary,
            )?,
            filters,
            &category_context,
        )
    } else {
        Vec::new()
    };

    let (Some(start_date), Some(end_date)) =
        (filters.start_date.as_deref(), filters.end_date.as_deref())
    else {
        return Ok(history_items.into_iter().map(Value::Object).collect());
    };

    let exact_items = extract_exact_budget_history_items(&history_items, start_date, end_date);
    if !exact_items.is_empty() {
        return Ok(exact_items.into_iter().map(Value::Object).collect());
    }

    let on_demand_items =
        build_budget_execution_history_on_demand(connection, user_id, filters, &filter_summary)?;
    if history_items.is_empty() {
        return Ok(on_demand_items);
    }

    let on_demand_records = on_demand_items
        .into_iter()
        .filter_map(|item| match item {
            Value::Object(object) => Some(object),
            _ => None,
        })
        .collect::<Vec<_>>();
    history_items = merge_budget_execution_history_items(history_items, on_demand_records);
    sort_budget_execution_history_items(&mut history_items);
    Ok(history_items.into_iter().map(Value::Object).collect())
}

pub fn query_budget_forecast(
    connection: &Connection,
    user_id: UserId,
    filters: &BudgetForecastFilters,
) -> DbResult<Vec<Value>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    if !table_exists(connection, "bills")? {
        return Ok(Vec::new());
    }

    let history_window = expand_forecast_history_window(
        &filters.period_type,
        &filters.start_date,
        &filters.end_date,
        filters.history_periods,
    )
    .map_err(DbError::InvalidOperation)?;
    let forecast_rows = query_budget_forecast_rows(connection, user_id, filters, &history_window)?;
    let (category_totals, period_count) = aggregate_budget_forecast_rows(forecast_rows);

    let categories = load_category_context_values(connection, user_id)?;
    let category_context = build_budget_category_context(&categories);
    let budget_map =
        query_budget_forecast_budget_map(connection, user_id, filters, &category_context)?;
    let target_period_key = build_forecast_period_key(
        &filters.period_type,
        parse_date_prefix(&filters.start_date)?,
    );

    let mut results = Vec::new();
    for (category, mut totals) in category_totals {
        totals
            .periods
            .sort_by(|left, right| left.period.cmp(&right.period));
        let history_periods = usize::try_from(filters.history_periods.max(1)).unwrap_or(usize::MAX);
        let start_index = totals.periods.len().saturating_sub(history_periods);
        let recent_periods = &totals.periods[start_index..];
        if recent_periods.is_empty() {
            continue;
        }
        let amounts = recent_periods
            .iter()
            .map(|item| item.amount)
            .collect::<Vec<_>>();
        let period_labels = recent_periods
            .iter()
            .map(|item| item.period.clone())
            .collect::<Vec<_>>();
        let current_spent = totals
            .periods
            .iter()
            .find(|item| item.period == target_period_key)
            .map(|item| item.amount)
            .unwrap_or_default();
        let budget_amount = budget_map.get(&category).cloned().unwrap_or_default();
        let category_info = resolve_budget_category_info(
            &category_context,
            &category,
            Some(""),
            Some(filters.budget_type),
        );
        results.push(build_budget_forecast_item_from_input(
            BudgetForecastItemInput {
                category: &category,
                category_info,
                amounts: &amounts,
                current_spent,
                primary_budget_amount: budget_amount.primary,
                sub_budget_total: budget_amount.sub_total,
                strategy: &filters.forecast_strategy,
                period_count,
                period_labels: Some(&period_labels),
            },
        ));
    }
    results.sort_by(|left, right| {
        let right_amount = right
            .get("forecast_amount")
            .and_then(value_to_f64)
            .unwrap_or_default();
        let left_amount = left
            .get("forecast_amount")
            .and_then(value_to_f64)
            .unwrap_or_default();
        right_amount
            .partial_cmp(&left_amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(results)
}

fn fetch_budget_execution_history_items(
    connection: &Connection,
    user_id: i64,
    filters: &BudgetExecutionFilters,
    filter_summary: &str,
) -> DbResult<Vec<BudgetRecord>> {
    let mut query = String::from(
        "
        SELECT
            bh.id AS id,
            bh.budget_id AS budget_id,
            bh.period_start AS period_start,
            bh.period_end AS period_end,
            bh.budget_amount AS budget_amount,
            bh.spent_amount AS spent_amount,
            bh.remaining_amount AS remaining_amount,
            bh.execution_rate AS execution_rate,
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
        WHERE bh.user_id = ? AND b.user_id = ?
        ",
    );
    let mut values = vec![SqlValue::Integer(user_id), SqlValue::Integer(user_id)];
    if let Some(budget_id) = filters.budget_id {
        query.push_str(" AND bh.budget_id = ?");
        values.push(SqlValue::Integer(budget_id));
    }
    if let Some(start_date) = text_filter(filters.start_date.as_deref()) {
        query.push_str(" AND bh.period_end >= ?");
        values.push(SqlValue::Text(start_date));
    }
    if let Some(end_date) = text_filter(filters.end_date.as_deref()) {
        query.push_str(" AND bh.period_start <= ?");
        values.push(SqlValue::Text(end_date));
    }
    query.push_str(" AND bh.filter_summary = ?");
    values.push(SqlValue::Text(filter_summary.to_string()));
    query.push_str(" ORDER BY bh.period_start DESC, bh.calculated_at DESC, bh.budget_id ASC");

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(params_from_iter(values), budget_history_record_from_row)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn enrich_budget_execution_history_items(
    items: Vec<BudgetRecord>,
    filters: &BudgetExecutionFilters,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
) -> Vec<BudgetRecord> {
    items
        .into_iter()
        .filter_map(|mut item| {
            let resolved_budget_type = resolve_budget_category_type(
                category_context,
                &record_text(&item, "category"),
                Some(&normalize_sub_category(item.get("sub_category"))),
                Some(filters.budget_type),
            )?;
            if resolved_budget_type.code() != filters.budget_type {
                return None;
            }
            let category_info = resolve_budget_category_info(
                category_context,
                &record_text(&item, "category"),
                Some(&normalize_sub_category(item.get("sub_category"))),
                Some(resolved_budget_type.code()),
            );
            item.insert(
                "type".to_string(),
                json_i64(i64::from(resolved_budget_type.code())),
            );
            item.insert(
                "category_id".to_string(),
                category_info
                    .get("id")
                    .and_then(value_to_i64)
                    .map(|id| Value::String(id.to_string()))
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            item.insert("category_info".to_string(), category_info);
            Some(item)
        })
        .collect()
}

fn extract_exact_budget_history_items(
    items: &[BudgetRecord],
    start_date: &str,
    end_date: &str,
) -> Vec<BudgetRecord> {
    items
        .iter()
        .filter(|item| {
            record_text(item, "period_start") == start_date
                && record_text(item, "period_end") == end_date
        })
        .cloned()
        .collect()
}

fn build_budget_execution_history_on_demand(
    connection: &Connection,
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
        let details = query_budget_execution_details(connection, user_id, &period_filters)?;
        for detail in details {
            if !budget_detail_overlaps_period(&detail, &period)? {
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
    sort_budget_execution_history_items(&mut records);
    Ok(records.into_iter().map(Value::Object).collect())
}

fn budget_detail_overlaps_period(
    detail: &Value,
    period: &bill_analyser_core::budgets::BudgetPeriodRange,
) -> DbResult<bool> {
    let budget_start = value_field_string(detail, "start_date");
    if budget_start.trim().is_empty() {
        return Ok(true);
    }
    let budget_end = value_field_string(detail, "end_date");
    budget_overlaps_period(
        &budget_start,
        text_filter(Some(&budget_end)).as_deref(),
        &period.start_date,
        &period.end_date,
    )
    .map_err(DbError::InvalidOperation)
}

fn merge_budget_execution_history_items(
    mut history_items: Vec<BudgetRecord>,
    on_demand_items: Vec<BudgetRecord>,
) -> Vec<BudgetRecord> {
    let mut history_keys = history_items
        .iter()
        .map(budget_history_identity_key)
        .collect::<BTreeSet<_>>();
    for item in on_demand_items {
        if history_keys.insert(budget_history_identity_key(&item)) {
            history_items.push(item);
        }
    }
    history_items
}

fn sort_budget_execution_history_items(items: &mut [BudgetRecord]) {
    items.sort_by_key(|item| std::cmp::Reverse(budget_history_sort_key(item)));
}

fn budget_history_identity_key(item: &BudgetRecord) -> (i64, String, String) {
    (
        record_i64(item, "budget_id").unwrap_or_default(),
        record_text(item, "period_start"),
        record_text(item, "period_end"),
    )
}

fn budget_history_sort_key(item: &BudgetRecord) -> (String, String, String, String, i64) {
    (
        record_text(item, "period_start"),
        record_text(item, "period_end"),
        record_text(item, "category"),
        record_text(item, "sub_category"),
        record_i64(item, "budget_id").unwrap_or_default(),
    )
}

fn budget_history_filter_summary(filters: &BudgetExecutionFilters) -> String {
    build_budget_history_filter_summary(&BudgetHistoryFilterSummaryInput {
        budget_type: filters.budget_type,
        period_type: filters.period_type.clone(),
        budget_id: filters.budget_id,
        category_id: filters.category_id,
        account_ids: filters.account_ids.clone(),
        tag_ids: filters.tag_ids.clone(),
    })
}
