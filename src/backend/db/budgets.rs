use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::budgets::{
    budget_overlaps_period, build_budget_category_context, build_budget_forecast_item_from_input,
    build_budget_history_filter_summary, build_budget_history_item_from_detail_with_context,
    build_forecast_period_key, expand_forecast_history_window, get_budget_type_name,
    iter_budget_history_period_ranges, normalize_budget_query_end_date,
    resolve_budget_category_info, resolve_budget_category_type, resolve_parent_budget_period,
    BudgetForecastItemInput, BudgetHistoryFilterSummaryInput,
};
use bill_analyser_core::UserId;
use chrono::{Datelike, NaiveDate, Utc};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Number, Value};

use crate::{run_transaction, DbError, DbResult, UserScope};

pub type BudgetRecord = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BudgetFilters {
    pub period_type: Option<String>,
    pub enabled: Option<bool>,
    pub category: Option<String>,
    pub budget_type: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BudgetCreateDraft {
    pub fields: BudgetRecord,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BudgetUpdateDraft {
    pub fields: BudgetRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BudgetExecutionFilters {
    pub budget_type: i32,
    pub period_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub budget_id: Option<i64>,
    pub category_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetForecastFilters {
    pub budget_type: i32,
    pub period_type: String,
    pub start_date: String,
    pub end_date: String,
    pub forecast_strategy: String,
    pub history_periods: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct BudgetGroupKey {
    category: String,
    period_type: String,
    start_date: String,
    user_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct BudgetPeriodGroupKey {
    category: String,
    sub_category: String,
    period_type: String,
    start_date: String,
    user_id: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct BudgetForecastBudgetAmount {
    primary: f64,
    sub_total: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct BudgetForecastCategoryTotals {
    periods: Vec<BudgetForecastPeriodAmount>,
}

#[derive(Debug, Clone, PartialEq)]
struct BudgetForecastPeriodAmount {
    period: String,
    amount: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct BudgetForecastRow {
    period: String,
    category: String,
    amount: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportBudgetRowAction {
    Created,
    Updated,
}

const BUDGET_SELECT_COLUMNS: &[&str] = &[
    "id",
    "user_id",
    "name",
    "category",
    "sub_category",
    "period_type",
    "amount",
    "start_date",
    "end_date",
    "alert_threshold",
    "enabled",
    "created_at",
    "updated_at",
];

const BUDGET_WRITE_COLUMNS: &[&str] = &[
    "name",
    "category",
    "sub_category",
    "period_type",
    "amount",
    "start_date",
    "end_date",
    "alert_threshold",
    "enabled",
    "created_at",
    "updated_at",
];

const BUDGET_UPDATE_COLUMNS: &[&str] = &[
    "name",
    "category",
    "sub_category",
    "period_type",
    "amount",
    "start_date",
    "end_date",
    "alert_threshold",
    "enabled",
    "updated_at",
];

pub fn query_budgets_for_listing(
    connection: &Connection,
    user_id: UserId,
    filters: &BudgetFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut budgets = query_budgets_raw(connection, user_id, filters)?;
    enrich_budget_listing(connection, user_id, filters.budget_type, &mut budgets)
}

pub fn get_budget_by_id(
    connection: &Connection,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    get_budget_by_id_on_connection(connection, user_id, budget_id)
}

pub fn create_budget(
    connection: &mut Connection,
    user_id: UserId,
    draft: &BudgetCreateDraft,
) -> DbResult<i64> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let now = now_text();
        let mut payload = normalize_create_payload(&draft.fields, &now)?;
        let budget_id = insert_budget_on_tx(tx, user_id, &mut payload)?;
        let effective_budget = get_budget_by_id_on_tx(tx, user_id, budget_id)?
            .ok_or_else(|| DbError::InvalidOperation("created budget not found".to_string()))?;
        synchronize_primary_budget_for_group(
            tx,
            build_budget_group_key(&effective_budget, user_id),
            Some(&effective_budget),
        )?;
        synchronize_budget_period_hierarchy(tx, &effective_budget, user_id)?;
        Ok(budget_id)
    })
}

pub fn update_budget(
    connection: &mut Connection,
    user_id: UserId,
    budget_id: i64,
    draft: &BudgetUpdateDraft,
) -> DbResult<bool> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(existing_budget) = get_budget_by_id_on_tx(tx, user_id, budget_id)? else {
            return Ok(false);
        };
        let mut payload = normalize_update_payload(&existing_budget, &draft.fields)?;
        if payload.is_empty() {
            return Ok(false);
        }
        let (assignments, values) = update_payload_to_sql(&mut payload)?;
        let mut values = values;
        values.push(SqlValue::Integer(budget_id));
        values.push(SqlValue::Integer(user_id));
        let updated = tx.execute(
            &format!(
                "UPDATE budgets SET {} WHERE id = ? AND user_id = ?",
                assignments.join(", ")
            ),
            params_from_iter(values),
        )?;
        if updated == 0 {
            return Ok(false);
        }
        let updated_budget = get_budget_by_id_on_tx(tx, user_id, budget_id)?
            .ok_or_else(|| DbError::InvalidOperation("updated budget not found".to_string()))?;
        for group_key in
            collect_budget_sync_group_keys(user_id, [&existing_budget, &updated_budget])
        {
            synchronize_primary_budget_for_group(tx, Some(group_key), Some(&updated_budget))?;
        }
        synchronize_budget_period_hierarchy(tx, &existing_budget, user_id)?;
        synchronize_budget_period_hierarchy(tx, &updated_budget, user_id)?;
        Ok(true)
    })
}

pub fn delete_budget(
    connection: &mut Connection,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<bool> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(budget) = get_budget_by_id_on_tx(tx, user_id, budget_id)? else {
            return Ok(false);
        };
        let category = record_text(&budget, "category");
        let period_type = record_text(&budget, "period_type");
        let start_date = record_text(&budget, "start_date");
        let sub_category = normalize_sub_category(record_value(&budget, "sub_category"));
        let affected_budgets = if sub_category.is_empty() {
            get_budgets_for_sync_group_on_tx(tx, user_id, &category, &period_type, &start_date)?
        } else {
            vec![budget.clone()]
        };

        let deleted = if sub_category.is_empty() {
            tx.execute(
                "
                DELETE FROM budgets
                WHERE category = ?1
                  AND period_type = ?2
                  AND start_date = ?3
                  AND user_id = ?4
                ",
                params![category, period_type, start_date, user_id],
            )?
        } else {
            let deleted = tx.execute(
                "DELETE FROM budgets WHERE id = ?1 AND user_id = ?2",
                params![budget_id, user_id],
            )?;
            synchronize_primary_budget_for_group(
                tx,
                build_budget_group_key(&budget, user_id),
                Some(&budget),
            )?;
            deleted
        };
        for affected_budget in &affected_budgets {
            synchronize_budget_period_hierarchy(tx, affected_budget, user_id)?;
        }
        Ok(deleted > 0)
    })
}

pub fn export_budgets(connection: &Connection, user_id: UserId) -> DbResult<Vec<Value>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let budgets = query_budgets_raw(connection, user_id, &BudgetFilters::default())?;
    Ok(budgets
        .iter()
        .map(|row| {
            json!({
                "name": field_or_null(row, "name"),
                "category": field_or_null(row, "category"),
                "sub_category": field_or_null(row, "sub_category"),
                "period_type": field_or_null(row, "period_type"),
                "amount": field_or_null(row, "amount"),
                "start_date": field_or_null(row, "start_date"),
                "end_date": field_or_null(row, "end_date"),
                "alert_threshold": field_or_null(row, "alert_threshold"),
                "enabled": field_or_null(row, "enabled"),
            })
        })
        .collect())
}

pub fn import_budgets(
    connection: &mut Connection,
    user_id: UserId,
    budgets: &[BudgetRecord],
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let now = now_text();
        let mut created_count = 0_i64;
        let mut updated_count = 0_i64;
        let mut error_count = 0_i64;
        let mut errors = Vec::new();

        for (index, budget) in budgets.iter().enumerate() {
            let row_number = index + 1;
            if !budget_import_has_required_name_and_amount(budget) {
                errors.push(format!("第{row_number}条: 缺少必填字段(name或amount)"));
                error_count += 1;
                continue;
            }
            match import_budget_row_on_tx(tx, user_id, budget, &now) {
                Ok(ImportBudgetRowAction::Created) => created_count += 1,
                Ok(ImportBudgetRowAction::Updated) => updated_count += 1,
                Err(error) => {
                    errors.push(format!("第{row_number}条: {error}"));
                    error_count += 1;
                }
            }
        }

        Ok(json!({
            "created": created_count,
            "updated": updated_count,
            "errors": error_count,
            "error_details": errors
        }))
    })
}

pub fn query_budget_execution_details(
    connection: &Connection,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Vec<Value>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let categories = load_category_context_values(connection, user_id)?;
    let category_context = build_budget_category_context(&categories);
    let category_filter = match filters.category_id {
        Some(category_id) => load_category_filter(connection, user_id, category_id)?,
        None => None,
    };
    if filters.category_id.is_some() && category_filter.is_none() {
        return Ok(Vec::new());
    }
    let mut budgets = query_budget_execution_candidates(connection, user_id, filters)?;
    if let Some((category, sub_category)) = category_filter {
        budgets.retain(|budget| {
            record_text(budget, "category") == category
                && (sub_category.is_empty()
                    || normalize_sub_category(budget.get("sub_category")) == sub_category)
        });
    }
    budgets = filter_budget_execution_candidates(budgets, filters, &category_context);
    budgets = dedupe_budget_execution_candidates(budgets);

    let type_name = get_budget_type_name(filters.budget_type);
    let mut results = Vec::new();
    for budget in budgets {
        let spent = get_budget_spent_amount(connection, user_id, &budget, type_name, filters)?;
        results.push(build_budget_execution_item(
            &budget,
            spent,
            &category_context,
            filters.budget_type,
        ));
    }
    Ok(results)
}

pub fn create_budget_execution_snapshots(
    connection: &mut Connection,
    user_id: UserId,
    filters: &BudgetExecutionFilters,
) -> DbResult<Value> {
    let user_id_value = UserScope::new(user_id).bind_value()?;
    let snapshots = query_budget_execution_details(connection, user_id, filters)?;
    let calculated_at = now_text();
    let filter_summary = budget_history_filter_summary(filters);
    let period_start = filters.start_date.clone().unwrap_or_default();
    let period_end = filters.end_date.clone().unwrap_or_default();

    run_transaction(connection, |tx| {
        let mut created_count = 0_i64;
        for snapshot in &snapshots {
            let budget_id = value_field_i64(snapshot, "id").unwrap_or_default();
            tx.execute(
                "
                DELETE FROM budget_history
                WHERE user_id = ?1 AND budget_id = ?2 AND period_start = ?3 AND period_end = ?4
                  AND filter_summary = ?5
                ",
                params![
                    user_id_value,
                    budget_id,
                    period_start.as_str(),
                    period_end.as_str(),
                    filter_summary.as_str()
                ],
            )?;

            let budget_amount = value_field_f64(snapshot, "budget_amount").unwrap_or_default();
            let spent_amount = value_field_f64(snapshot, "spent_amount").unwrap_or_default();
            let status = if spent_amount > budget_amount {
                "over_budget"
            } else {
                "within_budget"
            };
            tx.execute(
                "
                INSERT INTO budget_history (
                    user_id, budget_id, period_start, period_end,
                    budget_amount, spent_amount, remaining_amount,
                    execution_rate, status, filter_summary, calculated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ",
                params![
                    user_id_value,
                    budget_id,
                    period_start.as_str(),
                    period_end.as_str(),
                    budget_amount,
                    spent_amount,
                    value_field_f64(snapshot, "remaining_amount").unwrap_or_default(),
                    value_field_f64(snapshot, "execution_rate").unwrap_or_default(),
                    status,
                    filter_summary.as_str(),
                    calculated_at.as_str()
                ],
            )?;
            created_count += 1;
        }

        Ok(json!({
            "created_count": created_count,
            "period_start": period_start,
            "period_end": period_end,
            "filter_summary": filter_summary,
            "calculated_at": calculated_at
        }))
    })
}

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

fn import_budget_row_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    budget: &BudgetRecord,
    now: &str,
) -> DbResult<ImportBudgetRowAction> {
    let name = record_text(budget, "name");
    if let Some(existing_id) = tx
        .query_row(
            "SELECT id FROM budgets WHERE name = ?1 AND user_id = ?2 LIMIT 1",
            params![name.as_str(), user_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        let values = vec![
            budget_import_sql_value(budget, "category", Value::Null),
            budget_import_sql_value(budget, "sub_category", Value::Null),
            budget_import_sql_value(budget, "period_type", Value::String("monthly".to_string())),
            budget_import_sql_value(budget, "amount", Value::Null),
            budget_import_sql_value(budget, "start_date", Value::Null),
            budget_import_sql_value(budget, "end_date", Value::Null),
            budget_import_sql_value(budget, "alert_threshold", json_i64(80)),
            budget_import_sql_value(budget, "enabled", json_i64(1)),
            SqlValue::Text(now.to_string()),
            SqlValue::Integer(existing_id),
            SqlValue::Integer(user_id),
        ];
        tx.execute(
            "
            UPDATE budgets SET
                category = ?,
                sub_category = ?,
                period_type = ?,
                amount = ?,
                start_date = ?,
                end_date = ?,
                alert_threshold = ?,
                enabled = ?,
                updated_at = ?
            WHERE id = ? AND user_id = ?
            ",
            params_from_iter(values),
        )?;
        Ok(ImportBudgetRowAction::Updated)
    } else {
        let values = vec![
            SqlValue::Text(name),
            budget_import_sql_value(budget, "category", Value::Null),
            budget_import_sql_value(budget, "sub_category", Value::Null),
            budget_import_sql_value(budget, "period_type", Value::String("monthly".to_string())),
            budget_import_sql_value(budget, "amount", Value::Null),
            budget_import_sql_value(budget, "start_date", Value::Null),
            budget_import_sql_value(budget, "end_date", Value::Null),
            budget_import_sql_value(budget, "alert_threshold", json_i64(80)),
            budget_import_sql_value(budget, "enabled", json_i64(1)),
            SqlValue::Text(now.to_string()),
            SqlValue::Text(now.to_string()),
            SqlValue::Integer(user_id),
        ];
        tx.execute(
            "
            INSERT INTO budgets (
                name, category, sub_category, period_type, amount,
                start_date, end_date, alert_threshold, enabled,
                created_at, updated_at, user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
            params_from_iter(values),
        )?;
        Ok(ImportBudgetRowAction::Created)
    }
}

fn budget_import_has_required_name_and_amount(budget: &BudgetRecord) -> bool {
    budget
        .get("name")
        .is_some_and(budget_import_value_is_truthy)
        && budget
            .get("amount")
            .is_some_and(budget_import_value_is_truthy)
}

fn budget_import_value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn budget_import_sql_value(budget: &BudgetRecord, key: &str, default: Value) -> SqlValue {
    json_to_sql_value(
        budget
            .get(key)
            .cloned()
            .filter(|value| !value.is_null())
            .unwrap_or(default),
    )
}

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

fn enrich_budget_listing(
    connection: &Connection,
    user_id: i64,
    budget_type: Option<i32>,
    budgets: &mut [BudgetRecord],
) -> DbResult<Vec<BudgetRecord>> {
    let categories = load_category_context_values(connection, user_id)?;
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

fn load_category_context_values(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "
        SELECT id, type, main_category, sub_category, icon, color
        FROM categories
        WHERE user_id = ?
        ",
    )?;
    let rows = statement.query_map([user_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, Option<i32>>(1)?,
            "main_category": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            "sub_category": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            "icon": row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            "color": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        }))
    })?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row?);
    }
    Ok(categories)
}

fn insert_budget_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    payload: &mut BudgetRecord,
) -> DbResult<i64> {
    payload.insert("user_id".to_string(), json_i64(user_id));
    let mut columns = BUDGET_WRITE_COLUMNS
        .iter()
        .copied()
        .filter(|column| payload.contains_key(*column))
        .collect::<Vec<_>>();
    columns.push("user_id");
    let placeholders = std::iter::repeat_n("?", columns.len())
        .collect::<Vec<_>>()
        .join(", ");
    let values = columns
        .iter()
        .map(|column| json_to_sql_value(payload.get(*column).cloned().unwrap_or(Value::Null)))
        .collect::<Vec<_>>();
    tx.execute(
        &format!(
            "INSERT INTO budgets ({}) VALUES ({})",
            columns.join(", "),
            placeholders
        ),
        params_from_iter(values),
    )?;
    Ok(tx.last_insert_rowid())
}

fn normalize_create_payload(fields: &BudgetRecord, now: &str) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    payload
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    payload.insert(
        "sub_category".to_string(),
        Value::String(normalize_sub_category(payload.get("sub_category"))),
    );
    payload
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Bool(true));
    payload
        .entry("alert_threshold".to_string())
        .or_insert_with(|| json_i64(80));
    payload
        .entry("created_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    for field in ["category", "period_type", "amount", "start_date"] {
        if missing_required_field(&payload, field) {
            return Err(DbError::InvalidOperation(format!(
                "missing required budget field: {field}"
            )));
        }
    }
    Ok(payload)
}

fn normalize_update_payload(
    existing: &BudgetRecord,
    fields: &BudgetRecord,
) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    if payload.contains_key("sub_category") {
        payload.insert(
            "sub_category".to_string(),
            Value::String(normalize_sub_category(payload.get("sub_category"))),
        );
    } else if should_copy_existing_sub_category(&payload) {
        payload.insert(
            "sub_category".to_string(),
            Value::String(normalize_sub_category(existing.get("sub_category"))),
        );
    }
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now_text()));
    for key in payload.keys() {
        if !BUDGET_UPDATE_COLUMNS.contains(&key.as_str()) {
            return Err(DbError::InvalidOperation(format!(
                "invalid budget update field: {key}"
            )));
        }
    }
    Ok(payload)
}

fn should_copy_existing_sub_category(payload: &BudgetRecord) -> bool {
    [
        "category",
        "period_type",
        "amount",
        "start_date",
        "end_date",
    ]
    .iter()
    .any(|key| payload.contains_key(*key))
}

fn update_payload_to_sql(payload: &mut BudgetRecord) -> DbResult<(Vec<String>, Vec<SqlValue>)> {
    let mut keys = payload
        .keys()
        .filter(|key| BUDGET_UPDATE_COLUMNS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    if keys.is_empty() {
        return Err(DbError::InvalidOperation("empty budget update".to_string()));
    }
    let assignments = keys
        .iter()
        .map(|key| format!("{key} = ?"))
        .collect::<Vec<_>>();
    let values = keys
        .iter()
        .map(|key| json_to_sql_value(payload.get(key).cloned().unwrap_or(Value::Null)))
        .collect::<Vec<_>>();
    Ok((assignments, values))
}

fn get_budget_by_id_on_connection(
    connection: &Connection,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let sql = format!(
        "SELECT {} FROM budgets WHERE id = ?1 AND user_id = ?2",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    connection
        .query_row(&sql, params![budget_id, user_id], budget_record_from_row)
        .optional()
        .map_err(DbError::from)
}

fn get_budget_by_id_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let sql = format!(
        "SELECT {} FROM budgets WHERE id = ?1 AND user_id = ?2",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    tx.query_row(&sql, params![budget_id, user_id], budget_record_from_row)
        .optional()
        .map_err(DbError::from)
}

fn get_budgets_for_sync_group_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    category: &str,
    period_type: &str,
    start_date: &str,
) -> DbResult<Vec<BudgetRecord>> {
    let sql = format!(
        "
        SELECT {} FROM budgets
        WHERE category = ?1
          AND period_type = ?2
          AND start_date = ?3
          AND user_id = ?4
        ",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    let mut statement = tx.prepare(&sql)?;
    let rows = statement.query_map(
        params![category, period_type, start_date, user_id],
        budget_record_from_row,
    )?;
    let mut budgets = Vec::new();
    for row in rows {
        budgets.push(row?);
    }
    Ok(budgets)
}

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

fn budget_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.get::<_, i64>("id")?));
    record.insert(
        "user_id".to_string(),
        json_i64(row.get::<_, i64>("user_id")?),
    );
    for key in [
        "name",
        "category",
        "sub_category",
        "period_type",
        "start_date",
        "end_date",
        "created_at",
        "updated_at",
    ] {
        record.insert(
            key.to_string(),
            optional_string_value(row.get::<_, Option<String>>(key)?),
        );
    }
    record.insert(
        "amount".to_string(),
        json_real(row.get::<_, f64>("amount")?),
    );
    record.insert(
        "alert_threshold".to_string(),
        json_i64(row.get::<_, Option<i64>>("alert_threshold")?.unwrap_or(80)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(row.get::<_, Option<i64>>("enabled")?.unwrap_or(1)),
    );
    Ok(record)
}

fn budget_history_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.get::<_, i64>("id")?));
    record.insert(
        "budget_id".to_string(),
        json_i64(row.get::<_, i64>("budget_id")?),
    );
    for key in [
        "period_start",
        "period_end",
        "status",
        "filter_summary",
        "calculated_at",
        "name",
        "category",
        "sub_category",
        "period_type",
    ] {
        record.insert(
            key.to_string(),
            optional_string_value(row.get::<_, Option<String>>(key)?),
        );
    }
    for key in [
        "budget_amount",
        "spent_amount",
        "remaining_amount",
        "execution_rate",
    ] {
        record.insert(
            key.to_string(),
            row.get::<_, Option<f64>>(key)?
                .map(json_real)
                .unwrap_or(Value::Null),
        );
    }
    record.insert(
        "alert_threshold".to_string(),
        json_i64(row.get::<_, Option<i64>>("alert_threshold")?.unwrap_or(80)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(row.get::<_, Option<i64>>("enabled")?.unwrap_or(1)),
    );
    Ok(record)
}

fn table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn normalize_sub_category(value: Option<&Value>) -> String {
    value_string(value).trim().to_string()
}

fn missing_required_field(payload: &BudgetRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn where_suffix(conditions: &[String]) -> String {
    if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    }
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn record_value<'a>(record: &'a BudgetRecord, key: &str) -> Option<&'a Value> {
    record.get(key)
}

fn record_text(record: &BudgetRecord, key: &str) -> String {
    value_string(record.get(key))
}

fn record_i64(record: &BudgetRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn record_f64(record: &BudgetRecord, key: &str) -> Option<f64> {
    record.get(key).and_then(value_to_f64)
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
        Value::Bool(value) => Some(i64::from(*value)),
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

fn value_field_string(value: &Value, key: &str) -> String {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .map(|value| value_string(Some(value)))
        .unwrap_or_default()
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

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_date_prefix(value: &str) -> DbResult<NaiveDate> {
    let date_text = value
        .trim()
        .get(..10)
        .ok_or_else(|| DbError::InvalidOperation(format!("invalid date: {value}")))?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|_| DbError::InvalidOperation(format!("invalid date: {value}")))
}

fn field_or_null(row: &BudgetRecord, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn json_real(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::String)
}

fn json_to_sql_value(value: Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(value)),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_u64().and_then(|value| i64::try_from(value).ok())
            {
                SqlValue::Integer(value)
            } else {
                SqlValue::Real(number.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(value) => SqlValue::Text(value),
        Value::Array(_) | Value::Object(_) => SqlValue::Text(value.to_string()),
    }
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
