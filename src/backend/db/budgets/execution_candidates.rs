// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


fn query_budget_execution_candidates(
    connection: &Connection,
    user_id: i64,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let mut conditions = vec!["enabled = 1".to_string()];
    let mut values = vec![SqlValue::Integer(user_id)];
    if let Some(period_type) = text_filter(filters.period_type.as_deref()) {
        conditions.push("period_type = ?".to_string());
        values.push(SqlValue::Text(period_type));
    }
    if let Some(budget_id) = filters.budget_id {
        conditions.push("id = ?".to_string());
        values.push(SqlValue::Integer(budget_id));
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

fn filter_budget_execution_candidates(
    budgets: Vec<BudgetRecord>,
    filters: &BudgetExecutionFilters,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
) -> Vec<BudgetRecord> {
    budgets
        .into_iter()
        .filter_map(|mut budget| {
            let resolved_type = resolve_budget_category_type(
                category_context,
                &record_text(&budget, "category"),
                Some(&normalize_sub_category(budget.get("sub_category"))),
                Some(filters.budget_type),
            )?;
            if resolved_type.code() != filters.budget_type {
                return None;
            }
            if !budget_overlaps_request_period(&budget, filters) {
                return None;
            }
            budget.insert(
                "_resolved_budget_type".to_string(),
                json_i64(i64::from(resolved_type.code())),
            );
            Some(budget)
        })
        .collect()
}

fn budget_overlaps_request_period(budget: &BudgetRecord, filters: &BudgetExecutionFilters) -> bool {
    let (Some(request_start), Some(request_end)) = (
        text_filter(filters.start_date.as_deref()),
        text_filter(filters.end_date.as_deref()),
    ) else {
        return true;
    };
    let budget_start = record_text(budget, "start_date");
    let budget_end = text_filter(Some(record_text(budget, "end_date").as_str()));
    if budget_start.trim().is_empty() {
        return true;
    }
    budget_start <= request_end && budget_end.is_none_or(|end| end >= request_start)
}

fn dedupe_budget_execution_candidates(budgets: Vec<BudgetRecord>) -> Vec<BudgetRecord> {
    let mut selected: BTreeMap<(String, String, String, String, i64), BudgetRecord> =
        BTreeMap::new();
    let mut order = Vec::new();
    for budget in budgets {
        let category = record_text(&budget, "category").trim().to_string();
        let sub_category = normalize_sub_category(budget.get("sub_category"));
        let period_type = record_text(&budget, "period_type").trim().to_string();
        let start_date = record_text(&budget, "start_date").trim().to_string();
        let key = if category.is_empty() || period_type.is_empty() || start_date.is_empty() {
            (
                category,
                sub_category,
                period_type,
                start_date,
                record_i64(&budget, "id").unwrap_or_default(),
            )
        } else {
            (category, sub_category, period_type, start_date, 0)
        };
        let should_replace = selected.get(&key).is_none_or(|current| {
            (
                record_f64(&budget, "amount").unwrap_or_default(),
                record_i64(&budget, "id").unwrap_or_default(),
            ) >= (
                record_f64(current, "amount").unwrap_or_default(),
                record_i64(current, "id").unwrap_or_default(),
            )
        });
        if !selected.contains_key(&key) {
            order.push(key.clone());
        }
        if should_replace {
            selected.insert(key, budget);
        }
    }
    order
        .into_iter()
        .filter_map(|key| selected.remove(&key))
        .collect()
}

fn get_budget_spent_amount(
    connection: &Connection,
    user_id: i64,
    budget: &BudgetRecord,
    type_name: &str,
    filters: &BudgetExecutionFilters,
) -> DbResult<f64> {
    if !table_exists(connection, "bills")? {
        return Ok(0.0);
    }
    let mut conditions = vec!["type = ?".to_string(), "user_id = ?".to_string()];
    let mut values = vec![
        SqlValue::Text(type_name.to_string()),
        SqlValue::Integer(user_id),
    ];
    if let Some(category) = text_filter(Some(record_text(budget, "category").as_str())) {
        conditions.push("main_category = ?".to_string());
        values.push(SqlValue::Text(category));
    }
    if let Some(sub_category) = text_filter(Some(record_text(budget, "sub_category").as_str())) {
        conditions.push("sub_category = ?".to_string());
        values.push(SqlValue::Text(sub_category));
    }
    let (budget_start, budget_end) = resolve_budget_execution_window(budget, filters);
    if let Some(start_date) = budget_start {
        conditions.push("date >= ?".to_string());
        values.push(SqlValue::Text(start_date));
    }
    if let Some(end_date) = budget_end {
        conditions.push("date <= ?".to_string());
        values.push(SqlValue::Text(end_date));
    }
    if let Some(account_ids) = filters.account_ids.as_deref().filter(|ids| !ids.is_empty()) {
        let placeholders = placeholders(account_ids.len());
        conditions.push(format!(
            "(source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
        ));
        for account_id in account_ids {
            values.push(SqlValue::Integer(*account_id));
        }
        for account_id in account_ids {
            values.push(SqlValue::Integer(*account_id));
        }
    }
    if let Some(tag_ids) = filters.tag_ids.as_deref().filter(|ids| !ids.is_empty()) {
        if table_exists(connection, "bill_tags")? {
            conditions.push(format!(
                "id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({}))",
                placeholders(tag_ids.len())
            ));
            for tag_id in tag_ids {
                values.push(SqlValue::Integer(*tag_id));
            }
        } else {
            return Ok(0.0);
        }
    }

    let sql = format!(
        "SELECT COALESCE(SUM(amount), 0) FROM bills WHERE {}",
        conditions.join(" AND ")
    );
    let spent = connection.query_row(&sql, params_from_iter(values), |row| row.get::<_, f64>(0))?;
    Ok(spent.abs())
}

fn resolve_budget_execution_window(
    budget: &BudgetRecord,
    filters: &BudgetExecutionFilters,
) -> (Option<String>, Option<String>) {
    let budget_defined_start = text_filter(Some(record_text(budget, "start_date").as_str()));
    let budget_defined_end = text_filter(Some(record_text(budget, "end_date").as_str()));
    let mut budget_start = filters.start_date.clone().or(budget_defined_start.clone());
    let mut budget_end = filters.end_date.clone().or(budget_defined_end.clone());
    if let (Some(request_start), Some(defined_start)) =
        (filters.start_date.as_ref(), budget_defined_start.as_ref())
    {
        budget_start = Some(request_start.max(defined_start).clone());
    }
    if let (Some(request_end), Some(defined_end)) =
        (filters.end_date.as_ref(), budget_defined_end.as_ref())
    {
        budget_end = Some(request_end.min(defined_end).clone());
    }
    (
        budget_start,
        normalize_budget_query_end_date(budget_end.as_deref()),
    )
}

fn build_budget_execution_item(
    budget: &BudgetRecord,
    spent: f64,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
    fallback_budget_type: i32,
) -> Value {
    let budget_amount = record_f64(budget, "amount").unwrap_or_default();
    let resolved_budget_type = record_i64(budget, "_resolved_budget_type")
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(fallback_budget_type);
    let category = record_text(budget, "category");
    let sub_category = normalize_sub_category(budget.get("sub_category"));
    let category_info = resolve_budget_category_info(
        category_context,
        &category,
        Some(&sub_category),
        Some(resolved_budget_type),
    );
    let category_id = category_info
        .get("id")
        .and_then(value_to_i64)
        .map(|id| id.to_string())
        .unwrap_or_default();
    let execution_rate = if budget_amount > 0.0 {
        round2((spent / budget_amount) * 100.0)
    } else {
        0.0
    };
    json!({
        "id": record_i64(budget, "id").unwrap_or_default(),
        "name": record_text(budget, "name"),
        "category": category,
        "sub_category": sub_category,
        "category_info": category_info,
        "category_id": category_id,
        "period_type": record_text(budget, "period_type"),
        "budget_amount": budget_amount,
        "spent_amount": spent,
        "remaining_amount": budget_amount - spent,
        "execution_rate": execution_rate,
        "type": resolved_budget_type,
        "alert_threshold": record_i64(budget, "alert_threshold").unwrap_or(80),
        "start_date": field_or_null(budget, "start_date"),
        "end_date": field_or_null(budget, "end_date"),
        "enabled": record_i64(budget, "enabled").unwrap_or(1)
    })
}

fn load_category_filter(
    connection: &Connection,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<(String, String)>> {
    if !table_exists(connection, "categories")? {
        return Ok(None);
    }
    connection
        .query_row(
            "
            SELECT main_category, sub_category
            FROM categories
            WHERE id = ?1 AND user_id = ?2
            ",
            params![category_id, user_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            },
        )
        .optional()
        .map_err(DbError::from)
}
