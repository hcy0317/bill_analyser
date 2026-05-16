
fn synchronize_budget_period_hierarchy(
    tx: &Transaction<'_>,
    budget: &BudgetRecord,
    user_id: i64,
) -> DbResult<()> {
    let period_type = record_text(budget, "period_type");
    let parent_period_types = match period_type.as_str() {
        "monthly" => &["quarterly", "yearly"][..],
        "quarterly" => &["yearly"][..],
        _ => return Ok(()),
    };
    for parent_period_type in parent_period_types {
        let Some(parent_period) = resolve_parent_budget_period(
            &period_type,
            &record_text(budget, "start_date"),
            parent_period_type,
        )
        .map_err(DbError::InvalidOperation)?
        else {
            continue;
        };
        let mut reference_data = budget.clone();
        reference_data.insert(
            "period_type".to_string(),
            Value::String((*parent_period_type).to_string()),
        );
        reference_data.insert(
            "start_date".to_string(),
            Value::String(parent_period.start_date.clone()),
        );
        reference_data.insert(
            "end_date".to_string(),
            Value::String(parent_period.end_date.clone()),
        );
        synchronize_period_parent_budget_for_group(
            tx,
            build_budget_period_group_key(
                budget,
                parent_period_type,
                &parent_period.start_date,
                user_id,
            ),
            Some(&parent_period.end_date),
            Some(&reference_data),
        )?;
        synchronize_primary_budget_for_group(
            tx,
            build_budget_group_key(&reference_data, user_id),
            Some(&reference_data),
        )?;
    }
    Ok(())
}

fn synchronize_period_parent_budget_for_group(
    tx: &Transaction<'_>,
    group_key: Option<BudgetPeriodGroupKey>,
    period_end: Option<&str>,
    reference_data: Option<&BudgetRecord>,
) -> DbResult<()> {
    let (Some(group_key), Some(period_end)) = (group_key, period_end) else {
        return Ok(());
    };
    let child_total = get_period_child_budgets_total(tx, &group_key, period_end)?;
    let parent_budget = get_period_parent_budget(tx, &group_key)?;
    if child_total <= 0.0 {
        if let Some(parent_budget) = parent_budget {
            delete_budget_record_on_tx(tx, &parent_budget)?;
        }
        return Ok(());
    }
    let now = now_text();
    if let Some(parent_budget) = parent_budget {
        update_parent_budget_floor(tx, &parent_budget, child_total, period_end, &now)?;
    } else {
        let mut payload = reference_data.cloned().unwrap_or_default();
        payload.insert("category".to_string(), Value::String(group_key.category));
        payload.insert(
            "sub_category".to_string(),
            Value::String(group_key.sub_category),
        );
        payload.insert(
            "period_type".to_string(),
            Value::String(group_key.period_type),
        );
        payload.insert("amount".to_string(), json_real(child_total));
        payload.insert(
            "start_date".to_string(),
            Value::String(group_key.start_date),
        );
        payload.insert(
            "end_date".to_string(),
            Value::String(period_end.to_string()),
        );
        payload.insert("updated_at".to_string(), Value::String(now.clone()));
        payload
            .entry("created_at".to_string())
            .or_insert_with(|| Value::String(now));
        insert_budget_on_tx(tx, group_key.user_id, &mut payload)?;
    }
    Ok(())
}

fn synchronize_primary_budget_for_group(
    tx: &Transaction<'_>,
    group_key: Option<BudgetGroupKey>,
    reference_data: Option<&BudgetRecord>,
) -> DbResult<()> {
    let Some(group_key) = group_key else {
        return Ok(());
    };
    let sub_total = get_sub_category_budgets_total(tx, &group_key)?;
    if sub_total <= 0.0 {
        if reference_data
            .map(|record| !normalize_sub_category(record.get("sub_category")).is_empty())
            .unwrap_or(false)
        {
            if let Some(primary_budget) = get_primary_category_budget(tx, &group_key)? {
                delete_budget_record_on_tx(tx, &primary_budget)?;
            }
        }
        return Ok(());
    }
    let primary_budget = get_primary_category_budget(tx, &group_key)?;
    let now = now_text();
    if let Some(primary_budget) = primary_budget {
        let expected_end_date = reference_data
            .map(|record| record_text(record, "end_date"))
            .unwrap_or_default();
        update_parent_budget_floor(tx, &primary_budget, sub_total, &expected_end_date, &now)?;
    } else {
        let reference = reference_data.cloned().unwrap_or_default();
        let mut payload = BudgetRecord::new();
        payload.insert(
            "name".to_string(),
            reference
                .get("name")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        payload.insert("category".to_string(), Value::String(group_key.category));
        payload.insert("sub_category".to_string(), Value::String(String::new()));
        payload.insert(
            "period_type".to_string(),
            Value::String(group_key.period_type),
        );
        payload.insert("amount".to_string(), json_real(sub_total));
        payload.insert(
            "start_date".to_string(),
            Value::String(group_key.start_date),
        );
        payload.insert(
            "end_date".to_string(),
            reference.get("end_date").cloned().unwrap_or(Value::Null),
        );
        payload.insert(
            "alert_threshold".to_string(),
            reference
                .get("alert_threshold")
                .cloned()
                .unwrap_or_else(|| json_i64(80)),
        );
        payload.insert(
            "enabled".to_string(),
            reference
                .get("enabled")
                .cloned()
                .unwrap_or_else(|| json_i64(1)),
        );
        payload.insert(
            "created_at".to_string(),
            reference
                .get("created_at")
                .cloned()
                .unwrap_or_else(|| Value::String(now.clone())),
        );
        payload.insert("updated_at".to_string(), Value::String(now));
        insert_budget_on_tx(tx, group_key.user_id, &mut payload)?;
    }
    Ok(())
}

fn delete_budget_record_on_tx(tx: &Transaction<'_>, budget: &BudgetRecord) -> DbResult<()> {
    let budget_id = record_i64(budget, "id").unwrap_or_default();
    let user_id = record_i64(budget, "user_id").unwrap_or_default();
    if budget_id <= 0 || user_id <= 0 {
        return Ok(());
    }
    tx.execute(
        "DELETE FROM budgets WHERE id = ?1 AND user_id = ?2",
        params![budget_id, user_id],
    )?;
    Ok(())
}

fn update_parent_budget_floor(
    tx: &Transaction<'_>,
    budget: &BudgetRecord,
    child_total: f64,
    expected_end_date: &str,
    now: &str,
) -> DbResult<()> {
    let current_amount = record_f64(budget, "amount").unwrap_or_default();
    let mut assignments = vec!["updated_at = ?".to_string()];
    let mut values = vec![SqlValue::Text(now.to_string())];
    if current_amount < child_total {
        assignments.insert(0, "amount = ?".to_string());
        values.insert(0, SqlValue::Real(child_total));
    }
    if !expected_end_date.trim().is_empty()
        && record_text(budget, "end_date").trim() != expected_end_date.trim()
    {
        assignments.insert(assignments.len() - 1, "end_date = ?".to_string());
        values.insert(
            values.len() - 1,
            SqlValue::Text(expected_end_date.to_string()),
        );
    }
    if assignments == ["updated_at = ?"] {
        return Ok(());
    }
    values.push(SqlValue::Integer(
        record_i64(budget, "id").unwrap_or_default(),
    ));
    values.push(SqlValue::Integer(
        record_i64(budget, "user_id").unwrap_or_default(),
    ));
    tx.execute(
        &format!(
            "UPDATE budgets SET {} WHERE id = ? AND user_id = ?",
            assignments.join(", ")
        ),
        params_from_iter(values),
    )?;
    Ok(())
}

fn get_primary_category_budget(
    tx: &Transaction<'_>,
    group_key: &BudgetGroupKey,
) -> DbResult<Option<BudgetRecord>> {
    let sql = format!(
        "
        SELECT {} FROM budgets
        WHERE category = ?1
          AND (sub_category IS NULL OR sub_category = '')
          AND period_type = ?2
          AND start_date = ?3
          AND user_id = ?4
        ",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    tx.query_row(
        &sql,
        params![
            group_key.category,
            group_key.period_type,
            group_key.start_date,
            group_key.user_id
        ],
        budget_record_from_row,
    )
    .optional()
    .map_err(DbError::from)
}

fn get_period_parent_budget(
    tx: &Transaction<'_>,
    group_key: &BudgetPeriodGroupKey,
) -> DbResult<Option<BudgetRecord>> {
    let (sub_condition, mut values) = if group_key.sub_category.is_empty() {
        (
            "(sub_category IS NULL OR sub_category = '')",
            vec![SqlValue::Text(group_key.category.clone())],
        )
    } else {
        (
            "sub_category = ?",
            vec![
                SqlValue::Text(group_key.category.clone()),
                SqlValue::Text(group_key.sub_category.clone()),
            ],
        )
    };
    values.push(SqlValue::Text(group_key.period_type.clone()));
    values.push(SqlValue::Text(group_key.start_date.clone()));
    values.push(SqlValue::Integer(group_key.user_id));
    let sql = format!(
        "
        SELECT {} FROM budgets
        WHERE category = ?
          AND {sub_condition}
          AND period_type = ?
          AND start_date = ?
          AND user_id = ?
        ",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    tx.query_row(&sql, params_from_iter(values), budget_record_from_row)
        .optional()
        .map_err(DbError::from)
}

fn get_sub_category_budgets_total(
    tx: &Transaction<'_>,
    group_key: &BudgetGroupKey,
) -> DbResult<f64> {
    tx.query_row(
        "
        SELECT COALESCE(SUM(amount), 0)
        FROM budgets
        WHERE category = ?1
          AND sub_category IS NOT NULL
          AND sub_category != ''
          AND period_type = ?2
          AND start_date = ?3
          AND user_id = ?4
        ",
        params![
            group_key.category,
            group_key.period_type,
            group_key.start_date,
            group_key.user_id
        ],
        |row| row.get::<_, f64>(0),
    )
    .map_err(DbError::from)
}

fn get_period_child_budgets_total(
    tx: &Transaction<'_>,
    group_key: &BudgetPeriodGroupKey,
    period_end: &str,
) -> DbResult<f64> {
    match group_key.period_type.as_str() {
        "quarterly" => sum_budget_period_children(tx, group_key, "monthly", period_end),
        "yearly" => sum_yearly_budget_period_children(tx, group_key, period_end),
        _ => Ok(0.0),
    }
}

fn sum_budget_period_children(
    tx: &Transaction<'_>,
    group_key: &BudgetPeriodGroupKey,
    child_period_type: &str,
    period_end: &str,
) -> DbResult<f64> {
    Ok(
        get_budget_period_child_amounts_by_start(tx, group_key, child_period_type, period_end)?
            .values()
            .copied()
            .sum(),
    )
}

fn sum_yearly_budget_period_children(
    tx: &Transaction<'_>,
    group_key: &BudgetPeriodGroupKey,
    period_end: &str,
) -> DbResult<f64> {
    let quarterly_amounts =
        get_budget_period_child_amounts_by_start(tx, group_key, "quarterly", period_end)?;
    let monthly_amounts =
        get_budget_period_child_amounts_by_start(tx, group_key, "monthly", period_end)?;
    let mut monthly_totals_by_quarter: BTreeMap<u32, f64> = BTreeMap::new();
    for (monthly_start, amount) in monthly_amounts {
        let Ok(date) =
            NaiveDate::parse_from_str(&monthly_start[..10.min(monthly_start.len())], "%Y-%m-%d")
        else {
            continue;
        };
        let quarter = ((date.month() - 1) / 3) + 1;
        *monthly_totals_by_quarter.entry(quarter).or_default() += amount;
    }
    let mut quarterly_by_quarter: BTreeMap<u32, f64> = BTreeMap::new();
    for (quarterly_start, amount) in quarterly_amounts {
        let Ok(date) = NaiveDate::parse_from_str(
            &quarterly_start[..10.min(quarterly_start.len())],
            "%Y-%m-%d",
        ) else {
            continue;
        };
        let quarter = ((date.month() - 1) / 3) + 1;
        quarterly_by_quarter.insert(quarter, amount);
    }
    let mut total = 0.0;
    for quarter in 1..=4 {
        total += quarterly_by_quarter
            .get(&quarter)
            .copied()
            .unwrap_or_default()
            .max(
                monthly_totals_by_quarter
                    .get(&quarter)
                    .copied()
                    .unwrap_or_default(),
            );
    }
    Ok(total)
}

fn get_budget_period_child_amounts_by_start(
    tx: &Transaction<'_>,
    group_key: &BudgetPeriodGroupKey,
    child_period_type: &str,
    period_end: &str,
) -> DbResult<BTreeMap<String, f64>> {
    let (sub_condition, mut values) = if group_key.sub_category.is_empty() {
        (
            "(sub_category IS NULL OR sub_category = '')",
            vec![SqlValue::Text(group_key.category.clone())],
        )
    } else {
        (
            "sub_category = ?",
            vec![
                SqlValue::Text(group_key.category.clone()),
                SqlValue::Text(group_key.sub_category.clone()),
            ],
        )
    };
    values.push(SqlValue::Text(child_period_type.to_string()));
    values.push(SqlValue::Text(group_key.start_date.clone()));
    values.push(SqlValue::Text(period_end.to_string()));
    values.push(SqlValue::Integer(group_key.user_id));
    let mut statement = tx.prepare(&format!(
        "
        SELECT start_date, amount
        FROM budgets
        WHERE category = ?
          AND {sub_condition}
          AND period_type = ?
          AND start_date >= ?
          AND start_date <= ?
          AND user_id = ?
        "
    ))?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut amounts = BTreeMap::new();
    for row in rows {
        let (start_date, amount) = row?;
        let start_date = start_date.trim().to_string();
        if start_date.is_empty() {
            continue;
        }
        let entry = amounts.entry(start_date).or_insert(0.0_f64);
        *entry = (*entry).max(amount);
    }
    Ok(amounts)
}

fn collect_budget_sync_group_keys<'a>(
    user_id: i64,
    budgets: impl IntoIterator<Item = &'a BudgetRecord>,
) -> BTreeSet<BudgetGroupKey> {
    budgets
        .into_iter()
        .filter_map(|budget| build_budget_group_key(budget, user_id))
        .collect()
}

fn build_budget_group_key(budget: &BudgetRecord, user_id: i64) -> Option<BudgetGroupKey> {
    let category = record_text(budget, "category");
    let period_type = record_text(budget, "period_type");
    let start_date = record_text(budget, "start_date");
    if category.trim().is_empty() || period_type.trim().is_empty() || start_date.trim().is_empty() {
        return None;
    }
    Some(BudgetGroupKey {
        category,
        period_type,
        start_date,
        user_id,
    })
}

fn build_budget_period_group_key(
    budget: &BudgetRecord,
    period_type: &str,
    start_date: &str,
    user_id: i64,
) -> Option<BudgetPeriodGroupKey> {
    let category = record_text(budget, "category");
    if category.trim().is_empty() || period_type.trim().is_empty() || start_date.trim().is_empty() {
        return None;
    }
    Some(BudgetPeriodGroupKey {
        category,
        sub_category: normalize_sub_category(budget.get("sub_category")),
        period_type: period_type.to_string(),
        start_date: start_date.to_string(),
        user_id,
    })
}
