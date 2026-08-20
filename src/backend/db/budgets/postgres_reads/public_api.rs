/// 查询预算列表并补齐分类上下文，返回 API 层使用的预算记录。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_budgets_for_listing(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let user_id = i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))?;
    let mut budgets = query_postgres_budgets_raw(pool, user_id, filters).await?;
    enrich_postgres_budget_listing(pool, user_id, filters.budget_type, &mut budgets).await
}

/// 按用户和预算 ID 读取单条预算，未命中时返回 None。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_budget_by_id(
    pool: &PostgresPool,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let user_id = user_id_i64(user_id)?;
    get_postgres_budget_by_id_on_pool(pool, user_id, budget_id).await
}

/// 创建预算并同步主分类预算与周期父子预算层级。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_budget(
    pool: &PostgresPool,
    user_id: UserId,
    draft: &BudgetCreateDraft,
) -> DbResult<i64> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await?;
    let now = now_text();
    let payload = super::normalize_create_payload(&draft.fields, &now)?;
    let budget_id = insert_postgres_budget_on_tx(&mut tx, user_id, &payload).await?;
    let effective_budget = get_postgres_budget_by_id_on_tx(&mut tx, user_id, budget_id)
        .await?
        .ok_or_else(|| DbError::InvalidOperation("created budget not found".to_string()))?;
    synchronize_postgres_primary_budget_for_group(
        &mut tx,
        build_postgres_budget_group_key(&effective_budget, user_id),
        Some(&effective_budget),
    )
    .await?;
    synchronize_postgres_budget_period_hierarchy(&mut tx, &effective_budget, user_id).await?;
    tx.commit().await?;
    Ok(budget_id)
}

/// 更新预算字段并重新同步受影响的主分类预算和周期预算层级。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_budget(
    pool: &PostgresPool,
    user_id: UserId,
    budget_id: i64,
    draft: &BudgetUpdateDraft,
) -> DbResult<bool> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await?;
    let Some(existing_budget) =
        get_postgres_budget_by_id_on_tx(&mut tx, user_id, budget_id).await?
    else {
        return Ok(false);
    };
    let payload = super::normalize_update_payload(&existing_budget, &draft.fields)?;
    if payload.is_empty() {
        return Ok(false);
    }
    let updated = update_postgres_budget_on_tx(&mut tx, user_id, budget_id, &payload).await?;
    if !updated {
        return Ok(false);
    }
    let updated_budget = get_postgres_budget_by_id_on_tx(&mut tx, user_id, budget_id)
        .await?
        .ok_or_else(|| DbError::InvalidOperation("updated budget not found".to_string()))?;
    for group_key in
        collect_postgres_budget_sync_group_keys(user_id, [&existing_budget, &updated_budget])
    {
        synchronize_postgres_primary_budget_for_group(
            &mut tx,
            Some(group_key),
            Some(&updated_budget),
        )
        .await?;
    }
    synchronize_postgres_budget_period_hierarchy(&mut tx, &existing_budget, user_id).await?;
    synchronize_postgres_budget_period_hierarchy(&mut tx, &updated_budget, user_id).await?;
    tx.commit().await?;
    Ok(true)
}

/// 删除预算；删除主分类预算时会同时删除同周期子预算并同步层级。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_budget(
    pool: &PostgresPool,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<bool> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await?;
    let Some(budget) = get_postgres_budget_by_id_on_tx(&mut tx, user_id, budget_id).await? else {
        return Ok(false);
    };
    let category = record_text(&budget, "category");
    let period_type = record_text(&budget, "period_type");
    let start_date = record_text(&budget, "start_date");
    let sub_category = normalize_sub_category(record_value(&budget, "sub_category"));
    let affected_budgets = if sub_category.is_empty() {
        get_postgres_budgets_for_sync_group_on_tx(
            &mut tx,
            user_id,
            &category,
            &period_type,
            &start_date,
        )
        .await?
    } else {
        vec![budget.clone()]
    };

    let deleted = if sub_category.is_empty() {
        sqlx::query(
            r#"
            DELETE FROM budgets
            WHERE category = $1
              AND period_type = $2
              AND start_date = $3
              AND user_id = $4
            "#,
        )
        .bind(category)
        .bind(period_type)
        .bind(parse_date_prefix(&start_date)?)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
    } else {
        let deleted = sqlx::query("DELETE FROM budgets WHERE id = $1 AND user_id = $2")
            .bind(budget_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        synchronize_postgres_primary_budget_for_group(
            &mut tx,
            build_postgres_budget_group_key(&budget, user_id),
            Some(&budget),
        )
        .await?;
        deleted
    };
    for affected_budget in &affected_budgets {
        synchronize_postgres_budget_period_hierarchy(&mut tx, affected_budget, user_id).await?;
    }
    tx.commit().await?;
    Ok(deleted > 0)
}

/// 导出用户预算为可迁移 JSON，金额字段保持整数分。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn export_postgres_budgets(pool: &PostgresPool, user_id: UserId) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let budgets = query_postgres_budgets_raw(pool, user_id, &BudgetFilters::default()).await?;
    Ok(budgets
        .iter()
        .map(|row| {
            json!({
                "name": field_or_null(row, "name"),
                "category": field_or_null(row, "category"),
                "sub_category": field_or_null(row, "sub_category"),
                "period_type": field_or_null(row, "period_type"),
                "amount_cents": field_or_null(row, "amount_cents"),
                "start_date": field_or_null(row, "start_date"),
                "end_date": field_or_null(row, "end_date"),
                "alert_threshold": field_or_null(row, "alert_threshold"),
                "enabled": field_or_null(row, "enabled"),
            })
        })
        .collect())
}

/// 批量导入预算记录，按行返回创建、更新和错误统计。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_postgres_budgets(
    pool: &PostgresPool,
    user_id: UserId,
    budgets: &[BudgetRecord],
) -> DbResult<Value> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await?;
    let mut created_count = 0_i64;
    let mut updated_count = 0_i64;
    let mut error_count = 0_i64;
    let mut errors = Vec::new();

    for (index, budget) in budgets.iter().enumerate() {
        let row_number = index + 1;
        if !super::budget_import_has_required_name_and_amount(budget) {
            errors.push(format!(
                "第{row_number}条: 缺少必填字段(name或amount_cents)"
            ));
            error_count += 1;
            continue;
        }
        match import_postgres_budget_row_on_tx(&mut tx, user_id, budget).await {
            Ok(ImportBudgetRowAction::Created) => created_count += 1,
            Ok(ImportBudgetRowAction::Updated) => updated_count += 1,
            Err(error) => {
                errors.push(format!("第{row_number}条: {error}"));
                error_count += 1;
            }
        }
    }

    tx.commit().await?;
    Ok(json!({
        "created": created_count,
        "updated": updated_count,
        "errors": error_count,
        "error_details": errors
    }))
}

/// 查询预算执行明细，按筛选条件计算每个预算的已花费、剩余和执行率。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_budget_execution_details(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let categories = load_postgres_category_context_values(pool, user_id).await?;
    let category_context = build_budget_category_context(&categories);
    let category_filter = match filters.category_id {
        Some(category_id) => load_postgres_category_filter(pool, user_id, category_id).await?,
        None => None,
    };
    if filters.category_id.is_some() && category_filter.is_none() {
        return Ok(Vec::new());
    }

    let mut budgets = query_postgres_budget_execution_candidates(pool, user_id, filters).await?;
    if let Some((category, sub_category)) = category_filter {
        budgets.retain(|budget| {
            record_text(budget, "category") == category
                && (sub_category.is_empty()
                    || normalize_sub_category(budget.get("sub_category")) == sub_category)
        });
    }
    budgets = super::filter_budget_execution_candidates(budgets, filters, &category_context);
    budgets = super::dedupe_budget_execution_candidates(budgets);

    let type_name = get_budget_type_name(filters.budget_type);
    let mut results = Vec::new();
    for budget in budgets {
        let spent =
            get_postgres_budget_spent_amount(pool, user_id, &budget, type_name, filters).await?;
        results.push(super::build_budget_execution_item(
            &budget,
            spent,
            &category_context,
            filters.budget_type,
        ));
    }
    Ok(results)
}

/// 查询预算预测结果，基于历史账单窗口估算当前预算周期支出。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_budget_forecast(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetForecastFilters,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let period_kind = BudgetPeriodKind::parse(filters.period_type.trim())
        .map_err(DbError::InvalidOperation)?;
    let history_window = expand_forecast_history_window(
        period_kind,
        &filters.start_date,
        &filters.end_date,
        filters.history_periods,
    )
    .map_err(DbError::InvalidOperation)?;
    let forecast_rows =
        query_postgres_budget_forecast_rows(
            pool,
            user_id,
            filters,
            period_kind,
            &history_window,
        )
        .await?;
    let (category_totals, period_count) = aggregate_postgres_budget_forecast_rows(forecast_rows);

    let categories = load_postgres_category_context_values(pool, user_id).await?;
    let category_context = build_budget_category_context(&categories);
    let budget_map =
        query_postgres_budget_forecast_budget_map(
            pool,
            user_id,
            filters,
            period_kind,
            &category_context,
        )
        .await?;
    let target_period_key = period_kind.bucket_key(parse_date_prefix(&filters.start_date)?);

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
            .map(|item| item.amount_cents)
            .collect::<Vec<_>>();
        let period_labels = recent_periods
            .iter()
            .map(|item| item.period.clone())
            .collect::<Vec<_>>();
        let current_spent_cents = totals
            .periods
            .iter()
            .find(|item| item.period == target_period_key)
            .map(|item| item.amount_cents)
            .unwrap_or_default();
        let budget_amount_cents = budget_map.get(&category).cloned().unwrap_or_default();
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
                amounts_cents: &amounts,
                current_spent_cents,
                primary_budget_amount_cents: budget_amount_cents.primary_cents,
                sub_budget_total_cents: budget_amount_cents.sub_total_cents,
                strategy: &filters.forecast_strategy,
                period_count,
                period_labels: Some(&period_labels),
            },
        ));
    }
    results.sort_by(|left, right| {
        let right_amount = right
            .get("forecast_amount_cents")
            .and_then(value_to_i64)
            .unwrap_or_default();
        let left_amount = left
            .get("forecast_amount_cents")
            .and_then(value_to_i64)
            .unwrap_or_default();
        right_amount.cmp(&left_amount)
    });
    Ok(results)
}

/// 查询预算执行历史，优先复用快照，缺失精确周期时按需计算并合并。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_budget_execution_history(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<Value>> {
    let period_kind = BudgetPeriodKind::parse(
        filters.period_type.as_deref().unwrap_or("monthly").trim(),
    )
    .map_err(DbError::InvalidOperation)?;
    let user_id_value = user_id_i64(user_id)?;
    let categories = load_postgres_category_context_values(pool, user_id_value).await?;
    let category_context = build_budget_category_context(&categories);
    let filter_summary = super::budget_history_filter_summary(filters);
    let mut history_items = super::enrich_budget_execution_history_items(
        fetch_postgres_budget_execution_history_items(
            pool,
            user_id_value,
            filters,
            &filter_summary,
        )
        .await?,
        filters,
        &category_context,
    );

    let (Some(start_date), Some(end_date)) =
        (filters.start_date.as_deref(), filters.end_date.as_deref())
    else {
        return Ok(history_items.into_iter().map(Value::Object).collect());
    };

    let exact_items =
        super::extract_exact_budget_history_items(&history_items, start_date, end_date);
    if !exact_items.is_empty() {
        return Ok(exact_items.into_iter().map(Value::Object).collect());
    }

    let on_demand_items =
        build_postgres_budget_execution_history_on_demand(
            pool,
            user_id,
            filters,
            period_kind,
            &filter_summary,
        )
        .await?;
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
    history_items = super::merge_budget_execution_history_items(history_items, on_demand_records);
    super::sort_budget_execution_history_items(&mut history_items);
    Ok(history_items.into_iter().map(Value::Object).collect())
}

/// 为当前预算执行结果写入历史快照，金额字段保持整数分。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_budget_execution_snapshots(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Value> {
    let user_id_value = user_id_i64(user_id)?;
    let snapshots = query_postgres_budget_execution_details(pool, user_id, filters).await?;
    let calculated_at = now_text();
    let filter_summary = super::budget_history_filter_summary(filters);
    let period_start = filters.start_date.clone().unwrap_or_default();
    let period_end = filters.end_date.clone().unwrap_or_default();
    let period_start_date = parse_date_prefix(&period_start)?;
    let period_end_date = parse_date_prefix(&period_end)?;

    let mut tx = pool.begin().await?;
    let mut created_count = 0_i64;
    for snapshot in &snapshots {
        let budget_id = value_field_i64(snapshot, "id").unwrap_or_default();
        sqlx::query(
            r#"
            DELETE FROM budget_history
            WHERE user_id = $1
              AND budget_id = $2
              AND period_start = $3
              AND period_end = $4
              AND filter_summary = $5
            "#,
        )
        .bind(user_id_value)
        .bind(budget_id)
        .bind(period_start_date)
        .bind(period_end_date)
        .bind(&filter_summary)
        .execute(&mut *tx)
        .await?;

        let budget_amount_cents =
            value_field_i64(snapshot, "budget_amount_cents").unwrap_or_default();
        let spent_amount_cents =
            value_field_i64(snapshot, "spent_amount_cents").unwrap_or_default();
        let remaining_amount_cents =
            value_field_i64(snapshot, "remaining_amount_cents").unwrap_or_default();
        let status = if spent_amount_cents > budget_amount_cents {
            "over_budget"
        } else {
            "within_budget"
        };
        sqlx::query(
            r#"
            INSERT INTO budget_history (
                user_id, budget_id, period_start, period_end,
                budget_amount_cents, spent_amount_cents, remaining_amount_cents,
                execution_rate, status, filter_summary, calculated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz)
            "#,
        )
        .bind(user_id_value)
        .bind(budget_id)
        .bind(period_start_date)
        .bind(period_end_date)
        .bind(budget_amount_cents)
        .bind(spent_amount_cents)
        .bind(remaining_amount_cents)
        .bind(value_field_f64(snapshot, "execution_rate").unwrap_or_default())
        .bind(status)
        .bind(&filter_summary)
        .bind(&calculated_at)
        .execute(&mut *tx)
        .await?;
        created_count += 1;
    }
    tx.commit().await?;

    Ok(json!({
        "created_count": created_count,
        "period_start": period_start,
        "period_end": period_end,
        "filter_summary": filter_summary,
        "calculated_at": calculated_at
    }))
}
