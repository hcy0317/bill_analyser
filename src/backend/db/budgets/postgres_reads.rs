use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::{
    budgets::{
        build_budget_category_context, build_budget_forecast_item_from_input,
        build_budget_history_item_from_detail_with_context, build_forecast_period_key,
        expand_forecast_history_window, get_budget_type_name, iter_budget_history_period_ranges,
        resolve_budget_category_info, resolve_budget_category_type, resolve_parent_budget_period,
        BudgetForecastItemInput,
    },
    UserId,
};
use chrono::{DateTime, Datelike, NaiveDate, SecondsFormat, Utc};
use serde_json::{json, Map, Number, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row, Transaction};

use crate::{DbError, DbResult, PostgresPool};

use super::{
    BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastBudgetAmount,
    BudgetForecastCategoryTotals, BudgetForecastFilters, BudgetForecastPeriodAmount,
    BudgetForecastRow, BudgetGroupKey, BudgetPeriodGroupKey, BudgetRecord, BudgetUpdateDraft,
    ImportBudgetRowAction, BUDGET_UPDATE_COLUMNS,
};

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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_budget_by_id(
    pool: &PostgresPool,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let user_id = user_id_i64(user_id)?;
    get_postgres_budget_by_id_on_pool(pool, user_id, budget_id).await
}

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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_budget_forecast(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetForecastFilters,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let history_window = expand_forecast_history_window(
        &filters.period_type,
        &filters.start_date,
        &filters.end_date,
        filters.history_periods,
    )
    .map_err(DbError::InvalidOperation)?;
    let forecast_rows =
        query_postgres_budget_forecast_rows(pool, user_id, filters, &history_window).await?;
    let (category_totals, period_count) = aggregate_postgres_budget_forecast_rows(forecast_rows);

    let categories = load_postgres_category_context_values(pool, user_id).await?;
    let category_context = build_budget_category_context(&categories);
    let budget_map =
        query_postgres_budget_forecast_budget_map(pool, user_id, filters, &category_context)
            .await?;
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_budget_execution_history(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<Value>> {
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
        build_postgres_budget_execution_history_on_demand(pool, user_id, filters, &filter_summary)
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

async fn query_postgres_budgets_raw(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let period_type = text_filter(filters.period_type.as_deref());
    let category = text_filter(filters.category.as_deref());
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE user_id = $1
          AND ($2::TEXT IS NULL OR period_type = $2)
          AND ($3::BOOLEAN IS NULL OR enabled = $3)
          AND ($4::TEXT IS NULL OR category = $4)
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(user_id)
    .bind(period_type)
    .bind(filters.enabled)
    .bind(category)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(budget_record_from_postgres_row)
        .collect::<DbResult<Vec<_>>>()
}

async fn enrich_postgres_budget_listing(
    pool: &PostgresPool,
    user_id: i64,
    budget_type: Option<i32>,
    budgets: &mut [BudgetRecord],
) -> DbResult<Vec<BudgetRecord>> {
    let categories = load_postgres_category_context_values(pool, user_id).await?;
    let category_context = build_budget_category_context(&categories);
    let mut enriched = Vec::new();
    for budget in budgets {
        let resolved_type = resolve_budget_category_type(
            &category_context,
            &record_text(budget, "category"),
            Some(&normalize_sub_category(record_value(
                budget,
                "sub_category",
            ))),
            budget_type,
        )
        .map(|value| value.code());
        if budget_type.is_some() && resolved_type != budget_type {
            continue;
        }
        if let Some(resolved_type) = resolved_type {
            let category_info = resolve_budget_category_info(
                &category_context,
                &record_text(budget, "category"),
                Some(&normalize_sub_category(record_value(
                    budget,
                    "sub_category",
                ))),
                Some(resolved_type),
            );
            budget.insert("type".to_string(), json_i64(i64::from(resolved_type)));
            budget.insert(
                "category_id".to_string(),
                category_info
                    .get("id")
                    .and_then(value_to_i64)
                    .map(|id| Value::String(id.to_string()))
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            budget.insert("category_info".to_string(), category_info);
        } else {
            budget.insert("category_info".to_string(), Value::Null);
            budget.insert("category_id".to_string(), Value::String(String::new()));
        }
        enriched.push(budget.clone());
    }
    Ok(enriched)
}

async fn load_postgres_category_context_values(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_type, path, name, icon, color
        FROM categories
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) = category_names_from_path(&path, &name);
            Ok(json!({
                "id": row.try_get::<i64, _>("id")?,
                "type": row
                    .try_get::<Option<String>, _>("category_type")?
                    .and_then(|value| value.trim().parse::<i32>().ok()),
                "main_category": main_category,
                "sub_category": sub_category,
                "icon": row.try_get::<Option<String>, _>("icon")?.unwrap_or_default(),
                "color": row.try_get::<Option<String>, _>("color")?.unwrap_or_default(),
            }))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(DbError::from)
}

async fn load_postgres_category_filter(
    pool: &PostgresPool,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<(String, String)>> {
    sqlx::query(
        r#"
        SELECT path, name
        FROM categories
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(category_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(|row| {
        let path: Option<String> = row.try_get("path")?;
        let name: String = row.try_get("name")?;
        Ok(category_names_from_path(&path, &name))
    })
    .transpose()
}

async fn query_postgres_budget_execution_candidates(
    pool: &PostgresPool,
    user_id: i64,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE user_id = 
        "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND enabled = true");
    if let Some(period_type) = text_filter(filters.period_type.as_deref()) {
        builder.push(" AND period_type = ");
        builder.push_bind(period_type);
    }
    if let Some(budget_id) = filters.budget_id {
        builder.push(" AND id = ");
        builder.push_bind(budget_id);
    }
    builder.push(" ORDER BY created_at DESC, id DESC");
    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(budget_record_from_postgres_row)
        .collect()
}

async fn get_postgres_budget_spent_amount(
    pool: &PostgresPool,
    user_id: i64,
    budget: &BudgetRecord,
    type_name: &str,
    filters: &BudgetExecutionFilters,
) -> DbResult<i64> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COALESCE(SUM(ABS(amount_cents)), 0)::BIGINT AS spent_cents FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false");
    builder.push(" AND (b.transaction_type = ");
    builder.push_bind(canonical_budget_transaction_type(type_name));
    builder.push(" OR b.transaction_type = ");
    builder.push_bind(type_name.to_string());
    builder.push(" OR b.standard_payload->>'type' = ");
    builder.push_bind(type_name.to_string());
    builder.push(")");
    if let Some(category) = text_filter(Some(record_text(budget, "category").as_str())) {
        builder.push(" AND ");
        builder.push(postgres_bill_main_category_expr());
        builder.push(" = ");
        builder.push_bind(category);
    }
    if let Some(sub_category) = text_filter(Some(record_text(budget, "sub_category").as_str())) {
        builder.push(" AND ");
        builder.push(postgres_bill_sub_category_expr());
        builder.push(" = ");
        builder.push_bind(sub_category);
    }
    let (budget_start, budget_end) = super::resolve_budget_execution_window(budget, filters);
    if let Some(start_date) = budget_start {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(start_date);
        builder.push("::date");
    }
    if let Some(end_date) = budget_end {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(end_date);
        builder.push("::date + interval '1 day')");
    }
    if let Some(account_ids) = filters.account_ids.as_deref().filter(|ids| !ids.is_empty()) {
        builder.push(" AND (b.account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push(") OR b.source_account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push(") OR b.target_account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push(") OR b.transfer_target_account_id IN (");
        push_postgres_i64_bind_list(&mut builder, account_ids);
        builder.push("))");
    }
    if let Some(tag_ids) = filters.tag_ids.as_deref().filter(|ids| !ids.is_empty()) {
        builder.push(" AND b.id IN (SELECT bill_id FROM bill_tags WHERE user_id = ");
        builder.push_bind(user_id);
        builder.push(" AND tag_id IN (");
        push_postgres_i64_bind_list(&mut builder, tag_ids);
        builder.push("))");
    }
    let row = builder.build().fetch_one(pool).await?;
    let spent_cents: i64 = row.try_get("spent_cents")?;
    Ok(spent_cents)
}

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

fn postgres_budget_forecast_group_expr(period_type: &str) -> &'static str {
    match period_type {
        "daily" => "to_char(b.occurred_at, 'YYYY-MM-DD')",
        "weekly" => "to_char(b.occurred_at, 'YYYY-WW')",
        "quarterly" => {
            "to_char(b.occurred_at, 'YYYY') || '-Q' || EXTRACT(QUARTER FROM b.occurred_at)::INT"
        }
        "monthly" => "to_char(b.occurred_at, 'YYYY-MM')",
        _ => "to_char(b.occurred_at, 'YYYY')",
    }
}

fn postgres_bill_main_category_expr() -> &'static str {
    "COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(c.path, '/', 1), ''), c.name, '')"
}

fn postgres_bill_sub_category_expr() -> &'static str {
    "COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), CASE WHEN position('/' in COALESCE(c.path, '')) > 0 THEN substring(c.path from position('/' in c.path) + 1) ELSE '' END, '')"
}

fn canonical_budget_transaction_type(type_name: &str) -> String {
    match type_name.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => "income".to_string(),
        "支出" | "expense" | "3" => "expense".to_string(),
        "投资" | "investment" | "5" => "investment".to_string(),
        value => value.to_string(),
    }
}

fn budget_record_from_postgres_row(row: PgRow) -> DbResult<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.try_get("id")?));
    record.insert("user_id".to_string(), json_i64(row.try_get("user_id")?));
    for key in ["name", "category", "sub_category", "period_type"] {
        record.insert(
            key.to_string(),
            optional_string_value(row.try_get::<Option<String>, _>(key)?),
        );
    }
    record.insert(
        "start_date".to_string(),
        row.try_get::<NaiveDate, _>("start_date")?
            .to_string()
            .into(),
    );
    record.insert(
        "end_date".to_string(),
        row.try_get::<Option<NaiveDate>, _>("end_date")?
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );
    let amount_cents: i64 = row.try_get("amount_cents")?;
    record.insert("amount_cents".to_string(), json_i64(amount_cents));
    record.insert(
        "alert_threshold".to_string(),
        json_i64(i64::from(row.try_get::<i32, _>("alert_threshold")?)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(i64::from(row.try_get::<bool, _>("enabled")?)),
    );
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    Ok(record)
}

fn category_names_from_path(path: &Option<String>, name: &str) -> (String, String) {
    let parts = path
        .as_deref()
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

fn record_value<'a>(record: &'a BudgetRecord, key: &str) -> Option<&'a Value> {
    record.get(key)
}

fn record_text(record: &BudgetRecord, key: &str) -> String {
    value_string(record.get(key))
}

fn normalize_sub_category(value: Option<&Value>) -> String {
    value_string(value).trim().to_string()
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        Value::Bool(_) => None,
        _ => None,
    }
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::String)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn json_real(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn insert_timestamp(record: &mut BudgetRecord, key: &str, value: DateTime<Utc>) {
    record.insert(
        key.to_string(),
        Value::String(value.to_rfc3339_opts(SecondsFormat::Secs, true)),
    );
}

async fn get_postgres_budget_by_id_on_pool(
    pool: &PostgresPool,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(budget_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(budget_record_from_postgres_row)
    .transpose()
}

async fn get_postgres_budget_by_id_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    sqlx::query(
        r#"
        SELECT id, user_id, name, category, sub_category, period_type,
            amount_cents, start_date, end_date, alert_threshold, enabled,
            created_at, updated_at
        FROM budgets
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(budget_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(budget_record_from_postgres_row)
    .transpose()
}

async fn insert_postgres_budget_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    payload: &BudgetRecord,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO budgets (
            user_id, name, category, sub_category, period_type, amount_cents,
            start_date, end_date, alert_threshold, enabled, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(value_string(payload.get("name")))
    .bind(optional_text(payload.get("category")))
    .bind(normalize_sub_category(payload.get("sub_category")))
    .bind(required_text(payload, "period_type")?)
    .bind(amount_cents_from_record(payload, "amount_cents")?)
    .bind(required_date(payload, "start_date")?)
    .bind(optional_date(payload.get("end_date"))?)
    .bind(i32_value(payload.get("alert_threshold")).unwrap_or(80))
    .bind(bool_value(payload.get("enabled")).unwrap_or(true))
    .fetch_one(&mut **tx)
    .await?;
    row.try_get("id").map_err(DbError::from)
}

async fn update_postgres_budget_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    budget_id: i64,
    payload: &BudgetRecord,
) -> DbResult<bool> {
    let mut keys = payload
        .keys()
        .filter(|key| BUDGET_UPDATE_COLUMNS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    if keys.is_empty() {
        return Err(DbError::InvalidOperation("empty budget update".to_string()));
    }

    let mut builder = QueryBuilder::<Postgres>::new("UPDATE budgets SET ");
    let mut first_assignment = true;
    for key in keys {
        if !first_assignment {
            builder.push(", ");
        }
        first_assignment = false;
        let value = payload.get(&key);
        match key.as_str() {
            "name" => {
                builder.push("name = ").push_bind(value_string(value));
            }
            "category" => {
                builder.push("category = ").push_bind(optional_text(value));
            }
            "sub_category" => {
                builder
                    .push("sub_category = ")
                    .push_bind(normalize_sub_category(value));
            }
            "period_type" => {
                builder
                    .push("period_type = ")
                    .push_bind(required_text(payload, "period_type")?);
            }
            "amount_cents" => {
                builder
                    .push("amount_cents = ")
                    .push_bind(amount_cents_from_record(payload, "amount_cents")?);
            }
            "start_date" => {
                builder
                    .push("start_date = ")
                    .push_bind(required_date(payload, "start_date")?);
            }
            "end_date" => {
                builder.push("end_date = ").push_bind(optional_date(value)?);
            }
            "alert_threshold" => {
                builder
                    .push("alert_threshold = ")
                    .push_bind(i32_value(value).unwrap_or(80));
            }
            "enabled" => {
                builder
                    .push("enabled = ")
                    .push_bind(bool_value(value).unwrap_or(true));
            }
            "updated_at" => {
                builder.push("updated_at = now()");
            }
            invalid => {
                return Err(DbError::InvalidOperation(format!(
                    "invalid budget update field: {invalid}"
                )));
            }
        }
    }
    builder
        .push(" WHERE id = ")
        .push_bind(budget_id)
        .push(" AND user_id = ")
        .push_bind(user_id);
    let result = builder.build().execute(&mut **tx).await?;
    Ok(result.rows_affected() > 0)
}

async fn import_postgres_budget_row_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    budget: &BudgetRecord,
) -> DbResult<ImportBudgetRowAction> {
    let name = record_text(budget, "name");
    let existing_id =
        sqlx::query("SELECT id FROM budgets WHERE name = $1 AND user_id = $2 ORDER BY id LIMIT 1")
            .bind(&name)
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .map(|row| row.try_get::<i64, _>("id"))
            .transpose()?;
    if let Some(existing_id) = existing_id {
        sqlx::query(
            r#"
            UPDATE budgets SET
                category = $1,
                sub_category = $2,
                period_type = $3,
                amount_cents = $4,
                start_date = $5,
                end_date = $6,
                alert_threshold = $7,
                enabled = $8,
                updated_at = now()
            WHERE id = $9 AND user_id = $10
            "#,
        )
        .bind(optional_text(budget.get("category")))
        .bind(normalize_sub_category(budget.get("sub_category")))
        .bind(optional_text(budget.get("period_type")).unwrap_or_else(|| "monthly".to_string()))
        .bind(amount_cents_from_record(budget, "amount_cents")?)
        .bind(required_date(budget, "start_date")?)
        .bind(optional_date(budget.get("end_date"))?)
        .bind(i32_value(budget.get("alert_threshold")).unwrap_or(80))
        .bind(bool_value(budget.get("enabled")).unwrap_or(true))
        .bind(existing_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
        Ok(ImportBudgetRowAction::Updated)
    } else {
        let mut payload = budget.clone();
        payload.insert("name".to_string(), Value::String(name));
        payload
            .entry("period_type".to_string())
            .or_insert_with(|| Value::String("monthly".to_string()));
        payload
            .entry("sub_category".to_string())
            .or_insert_with(|| Value::String(String::new()));
        payload
            .entry("alert_threshold".to_string())
            .or_insert_with(|| json_i64(80));
        payload
            .entry("enabled".to_string())
            .or_insert_with(|| Value::Bool(true));
        insert_postgres_budget_on_tx(tx, user_id, &payload).await?;
        Ok(ImportBudgetRowAction::Created)
    }
}

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

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}

fn record_i64(record: &BudgetRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn value_field_i64(value: &Value, key: &str) -> Option<i64> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(value_to_i64)
}

fn value_field_f64(value: &Value, key: &str) -> Option<f64> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(value_to_f64)
}

fn record_amount_cents(record: &BudgetRecord) -> Option<i64> {
    record.get("amount_cents").and_then(value_to_i64)
}

fn amount_cents_from_record(record: &BudgetRecord, key: &str) -> DbResult<i64> {
    let value = record.get(key).ok_or_else(|| {
        DbError::InvalidOperation(format!("missing required budget field: {key}"))
    })?;
    amount_cents_from_value(value)
}

fn amount_cents_from_value(value: &Value) -> DbResult<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| DbError::InvalidOperation("invalid budget amount".to_string())),
        Value::String(value) => value
            .trim()
            .parse::<i64>()
            .map_err(|_| DbError::InvalidOperation("invalid budget amount".to_string())),
        Value::Bool(_) => Err(DbError::InvalidOperation(
            "invalid budget amount".to_string(),
        )),
        _ => Err(DbError::InvalidOperation(
            "invalid budget amount".to_string(),
        )),
    }
}

fn push_postgres_i64_bind_list(builder: &mut QueryBuilder<'_, Postgres>, values: &[i64]) {
    let mut separated = builder.separated(", ");
    for value in values {
        separated.push_bind(*value);
    }
}

fn required_text(record: &BudgetRecord, key: &str) -> DbResult<String> {
    optional_text(record.get(key))
        .ok_or_else(|| DbError::InvalidOperation(format!("missing required budget field: {key}")))
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .map(|value| value_string(Some(value)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_date(record: &BudgetRecord, key: &str) -> DbResult<NaiveDate> {
    parse_date_prefix(&required_text(record, key)?)
}

fn optional_date(value: Option<&Value>) -> DbResult<Option<NaiveDate>> {
    optional_text(value)
        .map(|value| parse_date_prefix(&value))
        .transpose()
}

fn parse_date_prefix(value: &str) -> DbResult<NaiveDate> {
    let date_text = value
        .trim()
        .get(..10)
        .ok_or_else(|| DbError::InvalidOperation(format!("invalid date: {value}")))?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|_| DbError::InvalidOperation(format!("invalid date: {value}")))
}

fn i32_value(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(value_to_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn bool_value(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Number(value)) => value.as_i64().map(|value| value != 0),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn field_or_null(row: &BudgetRecord, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::{env, error::Error};

    #[test]
    fn category_names_from_path_splits_main_and_subcategory() {
        assert_eq!(
            category_names_from_path(&Some("Food/Lunch".to_string()), "Lunch"),
            ("Food".to_string(), "Lunch".to_string())
        );
        assert_eq!(
            category_names_from_path(&None, "Food"),
            ("Food".to_string(), String::new())
        );
    }

    #[test]
    fn budget_forecast_and_amount_helpers_preserve_cents() {
        let (totals, period_count) = aggregate_postgres_budget_forecast_rows(vec![
            BudgetForecastRow {
                period: "2026-05".to_string(),
                category: "餐饮".to_string(),
                amount_cents: 1200,
            },
            BudgetForecastRow {
                period: "2026-05".to_string(),
                category: "餐饮".to_string(),
                amount_cents: 300,
            },
            BudgetForecastRow {
                period: "2026-06".to_string(),
                category: "餐饮".to_string(),
                amount_cents: 4500,
            },
        ]);

        assert_eq!(period_count, 2);
        let food = totals.get("餐饮").expect("food totals");
        assert_eq!(food.periods[0].period, "2026-05");
        assert_eq!(food.periods[0].amount_cents, 1500);
        assert_eq!(food.periods[1].amount_cents, 4500);

        let record = BudgetRecord::from_iter([("amount_cents".to_string(), json!("9876"))]);
        assert_eq!(record_amount_cents(&record), Some(9876));
        assert!(record_amount_cents(&BudgetRecord::from_iter([(
            "amount_cents".to_string(),
            json!(true)
        )]))
        .is_none());
        assert!(amount_cents_from_value(&json!(true)).is_err());
        assert_eq!(
            amount_cents_from_value(&json!("12345")).expect("text cents"),
            12345
        );
        assert!(amount_cents_from_value(&json!("12.34")).is_err());
        assert!(amount_cents_from_record(&BudgetRecord::new(), "amount_cents").is_err());
    }

    #[tokio::test]
    async fn postgres_row_projectors_emit_explicit_cents_when_database_available(
    ) -> Result<(), Box<dyn Error>> {
        let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return Ok(());
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await?;

        let budget_row = sqlx::query(
            r#"
            SELECT
                1::BIGINT AS id,
                2::BIGINT AS user_id,
                '餐饮预算'::TEXT AS name,
                '餐饮'::TEXT AS category,
                '午餐'::TEXT AS sub_category,
                'monthly'::TEXT AS period_type,
                '2026-06-01'::DATE AS start_date,
                NULL::DATE AS end_date,
                12345::BIGINT AS amount_cents,
                80::INT AS alert_threshold,
                true AS enabled,
                now() AS created_at,
                now() AS updated_at
            "#,
        )
        .fetch_one(&pool)
        .await?;
        let budget = budget_record_from_postgres_row(budget_row).expect("budget record");
        assert_eq!(budget.get("amount_cents"), Some(&json!(12345)));
        assert!(budget.get("amount").is_none());

        let history_row = sqlx::query(
            r#"
            SELECT
                10::BIGINT AS id,
                1::BIGINT AS budget_id,
                '2026-06-01'::DATE AS period_start,
                '2026-06-30'::DATE AS period_end,
                12345::BIGINT AS budget_amount_cents,
                4500::BIGINT AS spent_amount_cents,
                7845::BIGINT AS remaining_amount_cents,
                36.45::DOUBLE PRECISION AS execution_rate,
                'normal'::TEXT AS status,
                'monthly'::TEXT AS filter_summary,
                '餐饮预算'::TEXT AS name,
                '餐饮'::TEXT AS category,
                '午餐'::TEXT AS sub_category,
                'monthly'::TEXT AS period_type,
                now() AS calculated_at,
                80::INT AS alert_threshold,
                true AS enabled
            "#,
        )
        .fetch_one(&pool)
        .await?;
        let history = budget_history_record_from_postgres_row(history_row).expect("history record");
        assert_eq!(history.get("budget_amount_cents"), Some(&json!(12345)));
        assert_eq!(history.get("spent_amount_cents"), Some(&json!(4500)));
        assert_eq!(history.get("remaining_amount_cents"), Some(&json!(7845)));

        Ok(())
    }
}
