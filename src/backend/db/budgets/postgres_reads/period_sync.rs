async fn synchronize_postgres_budget_period_hierarchy(
    tx: &mut Transaction<'_, Postgres>,
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
        synchronize_postgres_period_parent_budget_for_group(
            tx,
            build_postgres_budget_period_group_key(
                budget,
                parent_period_type,
                &parent_period.start_date,
                user_id,
            ),
            Some(&parent_period.end_date),
            Some(&reference_data),
        )
        .await?;
        synchronize_postgres_primary_budget_for_group(
            tx,
            build_postgres_budget_group_key(&reference_data, user_id),
            Some(&reference_data),
        )
        .await?;
    }
    Ok(())
}

async fn synchronize_postgres_period_parent_budget_for_group(
    tx: &mut Transaction<'_, Postgres>,
    group_key: Option<BudgetPeriodGroupKey>,
    period_end: Option<&str>,
    reference_data: Option<&BudgetRecord>,
) -> DbResult<()> {
    let (Some(group_key), Some(period_end)) = (group_key, period_end) else {
        return Ok(());
    };
    let child_total = get_postgres_period_child_budgets_total(tx, &group_key, period_end).await?;
    let parent_budget = get_postgres_period_parent_budget(tx, &group_key).await?;
    if child_total <= 0 {
        if let Some(parent_budget) = parent_budget {
            delete_postgres_budget_record_on_tx(tx, &parent_budget).await?;
        }
        return Ok(());
    }
    if let Some(parent_budget) = parent_budget {
        update_postgres_parent_budget_floor(tx, &parent_budget, child_total, period_end).await?;
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
        payload.insert("amount_cents".to_string(), json_i64(child_total));
        payload.insert(
            "start_date".to_string(),
            Value::String(group_key.start_date),
        );
        payload.insert(
            "end_date".to_string(),
            Value::String(period_end.to_string()),
        );
        insert_postgres_budget_on_tx(tx, group_key.user_id, &payload).await?;
    }
    Ok(())
}

async fn synchronize_postgres_primary_budget_for_group(
    tx: &mut Transaction<'_, Postgres>,
    group_key: Option<BudgetGroupKey>,
    reference_data: Option<&BudgetRecord>,
) -> DbResult<()> {
    let Some(group_key) = group_key else {
        return Ok(());
    };
    let sub_total = get_postgres_sub_category_budgets_total(tx, &group_key).await?;
    if sub_total <= 0 {
        if reference_data
            .map(|record| !normalize_sub_category(record.get("sub_category")).is_empty())
            .unwrap_or(false)
        {
            if let Some(primary_budget) =
                get_postgres_primary_category_budget(tx, &group_key).await?
            {
                delete_postgres_budget_record_on_tx(tx, &primary_budget).await?;
            }
        }
        return Ok(());
    }
    let primary_budget = get_postgres_primary_category_budget(tx, &group_key).await?;
    if let Some(primary_budget) = primary_budget {
        let expected_end_date = reference_data
            .map(|record| record_text(record, "end_date"))
            .unwrap_or_default();
        update_postgres_parent_budget_floor(tx, &primary_budget, sub_total, &expected_end_date)
            .await?;
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
        payload.insert("amount_cents".to_string(), json_i64(sub_total));
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
        insert_postgres_budget_on_tx(tx, group_key.user_id, &payload).await?;
    }
    Ok(())
}

async fn get_postgres_primary_category_budget(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetGroupKey,
) -> DbResult<Option<BudgetRecord>> {
    sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE category = $1
          AND (sub_category IS NULL OR sub_category = '')
          AND period_type = $2
          AND start_date = $3
          AND user_id = $4
        ORDER BY id
        LIMIT 1
        "#,
    )
    .bind(&group_key.category)
    .bind(&group_key.period_type)
    .bind(parse_date_prefix(&group_key.start_date)?)
    .bind(group_key.user_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(budget_record_from_postgres_row)
    .transpose()
}

async fn get_postgres_period_parent_budget(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetPeriodGroupKey,
) -> DbResult<Option<BudgetRecord>> {
    let row = if group_key.sub_category.is_empty() {
        sqlx::query(
            r#"
            SELECT id, user_id, name, category, sub_category, period_type,
                amount_cents, start_date, end_date, alert_threshold, enabled,
                created_at, updated_at
            FROM budgets
            WHERE category = $1
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = $2
              AND start_date = $3
              AND user_id = $4
            ORDER BY id
            LIMIT 1
            "#,
        )
        .bind(&group_key.category)
        .bind(&group_key.period_type)
        .bind(parse_date_prefix(&group_key.start_date)?)
        .bind(group_key.user_id)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, user_id, name, category, sub_category, period_type,
                amount_cents, start_date, end_date, alert_threshold, enabled,
                created_at, updated_at
            FROM budgets
            WHERE category = $1
              AND sub_category = $2
              AND period_type = $3
              AND start_date = $4
              AND user_id = $5
            ORDER BY id
            LIMIT 1
            "#,
        )
        .bind(&group_key.category)
        .bind(&group_key.sub_category)
        .bind(&group_key.period_type)
        .bind(parse_date_prefix(&group_key.start_date)?)
        .bind(group_key.user_id)
        .fetch_optional(&mut **tx)
        .await?
    };
    row.map(budget_record_from_postgres_row).transpose()
}

async fn get_postgres_sub_category_budgets_total(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetGroupKey,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(SUM(amount_cents), 0)::BIGINT AS total_cents
        FROM budgets
        WHERE category = $1
          AND sub_category IS NOT NULL
          AND sub_category != ''
          AND period_type = $2
          AND start_date = $3
          AND user_id = $4
        "#,
    )
    .bind(&group_key.category)
    .bind(&group_key.period_type)
    .bind(parse_date_prefix(&group_key.start_date)?)
    .bind(group_key.user_id)
    .fetch_one(&mut **tx)
    .await?;
    row.try_get("total_cents").map_err(DbError::from)
}

async fn get_postgres_period_child_budgets_total(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetPeriodGroupKey,
    period_end: &str,
) -> DbResult<i64> {
    match group_key.period_type.as_str() {
        "quarterly" => {
            sum_postgres_budget_period_children(tx, group_key, "monthly", period_end).await
        }
        "yearly" => sum_postgres_yearly_budget_period_children(tx, group_key, period_end).await,
        _ => Ok(0),
    }
}

async fn sum_postgres_budget_period_children(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetPeriodGroupKey,
    child_period_type: &str,
    period_end: &str,
) -> DbResult<i64> {
    Ok(get_postgres_budget_period_child_amounts_by_start(
        tx,
        group_key,
        child_period_type,
        period_end,
    )
    .await?
    .values()
    .copied()
    .sum())
}

async fn sum_postgres_yearly_budget_period_children(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetPeriodGroupKey,
    period_end: &str,
) -> DbResult<i64> {
    let quarterly_amounts =
        get_postgres_budget_period_child_amounts_by_start(tx, group_key, "quarterly", period_end)
            .await?;
    let monthly_amounts =
        get_postgres_budget_period_child_amounts_by_start(tx, group_key, "monthly", period_end)
            .await?;
    let mut monthly_totals_by_quarter: BTreeMap<u32, i64> = BTreeMap::new();
    for (monthly_start, amount) in monthly_amounts {
        let Ok(date) =
            NaiveDate::parse_from_str(&monthly_start[..10.min(monthly_start.len())], "%Y-%m-%d")
        else {
            continue;
        };
        let quarter = ((date.month() - 1) / 3) + 1;
        *monthly_totals_by_quarter.entry(quarter).or_default() += amount;
    }
    let mut quarterly_by_quarter: BTreeMap<u32, i64> = BTreeMap::new();
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
    let mut total = 0_i64;
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

async fn get_postgres_budget_period_child_amounts_by_start(
    tx: &mut Transaction<'_, Postgres>,
    group_key: &BudgetPeriodGroupKey,
    child_period_type: &str,
    period_end: &str,
) -> DbResult<BTreeMap<String, i64>> {
    let rows = if group_key.sub_category.is_empty() {
        sqlx::query(
            r#"
            SELECT start_date, amount_cents
            FROM budgets
            WHERE category = $1
              AND (sub_category IS NULL OR sub_category = '')
              AND period_type = $2
              AND start_date >= $3
              AND start_date <= $4
              AND user_id = $5
            "#,
        )
        .bind(&group_key.category)
        .bind(child_period_type)
        .bind(parse_date_prefix(&group_key.start_date)?)
        .bind(parse_date_prefix(period_end)?)
        .bind(group_key.user_id)
        .fetch_all(&mut **tx)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT start_date, amount_cents
            FROM budgets
            WHERE category = $1
              AND sub_category = $2
              AND period_type = $3
              AND start_date >= $4
              AND start_date <= $5
              AND user_id = $6
            "#,
        )
        .bind(&group_key.category)
        .bind(&group_key.sub_category)
        .bind(child_period_type)
        .bind(parse_date_prefix(&group_key.start_date)?)
        .bind(parse_date_prefix(period_end)?)
        .bind(group_key.user_id)
        .fetch_all(&mut **tx)
        .await?
    };
    let mut amounts = BTreeMap::new();
    for row in rows {
        let start_date: NaiveDate = row.try_get("start_date")?;
        let amount: i64 = row.try_get("amount_cents")?;
        let entry = amounts.entry(start_date.to_string()).or_insert(0_i64);
        *entry = (*entry).max(amount);
    }
    Ok(amounts)
}

async fn get_postgres_budgets_for_sync_group_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    category: &str,
    period_type: &str,
    start_date: &str,
) -> DbResult<Vec<BudgetRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE category = $1
          AND period_type = $2
          AND start_date = $3
          AND user_id = $4
        "#,
    )
    .bind(category)
    .bind(period_type)
    .bind(parse_date_prefix(start_date)?)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(budget_record_from_postgres_row)
        .collect::<DbResult<Vec<_>>>()
}

async fn delete_postgres_budget_record_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    budget: &BudgetRecord,
) -> DbResult<()> {
    let budget_id = record_i64(budget, "id").unwrap_or_default();
    let user_id = record_i64(budget, "user_id").unwrap_or_default();
    if budget_id <= 0 || user_id <= 0 {
        return Ok(());
    }
    sqlx::query("DELETE FROM budgets WHERE id = $1 AND user_id = $2")
        .bind(budget_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn update_postgres_parent_budget_floor(
    tx: &mut Transaction<'_, Postgres>,
    budget: &BudgetRecord,
    child_total: i64,
    expected_end_date: &str,
) -> DbResult<()> {
    let current_amount = record_amount_cents(budget).unwrap_or_default();
    let set_amount = current_amount < child_total;
    let set_end_date = !expected_end_date.trim().is_empty()
        && record_text(budget, "end_date").trim() != expected_end_date.trim();
    if !set_amount && !set_end_date {
        return Ok(());
    }

    let mut builder = QueryBuilder::<Postgres>::new("UPDATE budgets SET ");
    let mut first_assignment = true;
    if set_amount {
        builder.push("amount_cents = ").push_bind(child_total);
        first_assignment = false;
    }
    if set_end_date {
        if !first_assignment {
            builder.push(", ");
        }
        builder
            .push("end_date = ")
            .push_bind(parse_date_prefix(expected_end_date)?);
        first_assignment = false;
    }
    if !first_assignment {
        builder.push(", ");
    }
    builder.push("updated_at = now()");
    builder
        .push(" WHERE id = ")
        .push_bind(record_i64(budget, "id").unwrap_or_default())
        .push(" AND user_id = ")
        .push_bind(record_i64(budget, "user_id").unwrap_or_default());
    builder.build().execute(&mut **tx).await?;
    Ok(())
}

fn collect_postgres_budget_sync_group_keys<'a>(
    user_id: i64,
    budgets: impl IntoIterator<Item = &'a BudgetRecord>,
) -> BTreeSet<BudgetGroupKey> {
    budgets
        .into_iter()
        .filter_map(|budget| build_postgres_budget_group_key(budget, user_id))
        .collect()
}

fn build_postgres_budget_group_key(budget: &BudgetRecord, user_id: i64) -> Option<BudgetGroupKey> {
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

fn build_postgres_budget_period_group_key(
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
