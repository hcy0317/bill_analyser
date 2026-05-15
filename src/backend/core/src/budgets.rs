use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::TransactionType;

pub const BUDGET_TYPE_EXPENSE: i32 = 3;
pub const BUDGET_TYPE_INVESTMENT: i32 = 5;
pub const LEGACY_EXPENSE_CATEGORY_TYPE: i32 = 1;
pub const VALID_BUDGET_PERIOD_TYPES: [&str; 5] =
    ["daily", "weekly", "monthly", "quarterly", "yearly"];

const SYNCHRONIZED_PRIMARY_TOLERANCE: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetPeriodRange {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BudgetPeriodScopeInput {
    pub budget_type: Option<i32>,
    pub period_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub quarter: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetPeriodScope {
    pub budget_type: i32,
    pub period_type: String,
    pub start_date: String,
    pub end_date: String,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub quarter: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BudgetRouteFilters {
    pub budget_id: Option<i64>,
    pub category_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetPeriodProgress {
    pub elapsed_days: i64,
    pub remaining_days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetCategoryInfo {
    pub id: Option<i64>,
    pub name: String,
    pub parent_name: String,
    pub main_category: String,
    pub sub_category: String,
    pub category_type: Option<i32>,
    pub icon: String,
    pub color: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BudgetCategoryContext {
    pub primary_by_name: BTreeMap<String, BudgetCategoryInfo>,
    pub sub_by_parent_name: BTreeMap<(String, String), BudgetCategoryInfo>,
    pub primary_by_key: BTreeMap<(i32, String), BudgetCategoryInfo>,
    pub sub_by_key: BTreeMap<(i32, String, String), BudgetCategoryInfo>,
    pub fallback_by_key: BTreeMap<(i32, String), BudgetCategoryInfo>,
    pub types_by_name: BTreeMap<String, BTreeSet<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetHistoryFilterSummaryInput {
    pub budget_type: i32,
    pub period_type: Option<String>,
    pub budget_id: Option<i64>,
    pub category_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone)]
pub struct BudgetForecastItemInput<'a> {
    pub category: &'a str,
    pub category_info: Value,
    pub amounts: &'a [f64],
    pub current_spent: f64,
    pub primary_budget_amount: f64,
    pub sub_budget_total: f64,
    pub strategy: &'a str,
    pub period_count: usize,
    pub period_labels: Option<&'a [String]>,
}

pub fn is_valid_budget_period_type(period_type: &str) -> bool {
    VALID_BUDGET_PERIOD_TYPES.contains(&period_type)
}

pub fn validate_budget_period_args(
    input: &BudgetPeriodScopeInput,
    months_history: Option<i64>,
) -> Result<(), String> {
    let period_type = input
        .period_type
        .as_deref()
        .unwrap_or("monthly")
        .trim()
        .to_string();
    if !is_valid_budget_period_type(&period_type) {
        return Err(format!("Invalid period_type: {period_type}"));
    }

    if input.month.is_some_and(|month| !(1..=12).contains(&month)) {
        return Err("month must be between 1 and 12".to_string());
    }
    if input
        .quarter
        .is_some_and(|quarter| !(1..=4).contains(&quarter))
    {
        return Err("quarter must be between 1 and 4".to_string());
    }
    if months_history.is_some_and(|history| history < 1) {
        return Err("months_history must be >= 1".to_string());
    }

    Ok(())
}

pub fn validate_budget_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(), String> {
    let Some(start_date) = non_empty_str(start_date) else {
        return Ok(());
    };
    let Some(end_date) = non_empty_str(end_date) else {
        return Ok(());
    };

    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    if start > end {
        return Err("Invalid date range: start_date must be <= end_date".to_string());
    }
    Ok(())
}

pub fn resolve_budget_period_range(
    input: &BudgetPeriodScopeInput,
    today: NaiveDate,
) -> Result<BudgetPeriodRange, String> {
    validate_budget_period_args(input, None)?;

    if non_empty_str(input.start_date.as_deref()).is_some()
        && non_empty_str(input.end_date.as_deref()).is_some()
    {
        validate_budget_date_range(input.start_date.as_deref(), input.end_date.as_deref())?;
        return Ok(BudgetPeriodRange {
            start_date: input.start_date.clone().unwrap_or_default(),
            end_date: input.end_date.clone().unwrap_or_default(),
        });
    }

    let period_type = input.period_type.as_deref().unwrap_or("monthly").trim();
    let year = input.year.unwrap_or(today.year());
    match period_type {
        "daily" => Ok(BudgetPeriodRange {
            start_date: format_date(today),
            end_date: format_date(today),
        }),
        "weekly" => {
            let start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
            let end = start + Duration::days(6);
            Ok(BudgetPeriodRange {
                start_date: format_date(start),
                end_date: format_date(end),
            })
        }
        "quarterly" => {
            let quarter = input
                .quarter
                .unwrap_or_else(|| ((today.month() - 1) / 3) + 1);
            let start_month = ((quarter - 1) * 3) + 1;
            let end_month = start_month + 2;
            Ok(BudgetPeriodRange {
                start_date: format_date(make_date(year, start_month, 1)?),
                end_date: format_date(last_day_of_month(year, end_month)?),
            })
        }
        "yearly" => Ok(BudgetPeriodRange {
            start_date: format_date(make_date(year, 1, 1)?),
            end_date: format_date(make_date(year, 12, 31)?),
        }),
        _ => {
            let month = input.month.unwrap_or(today.month());
            Ok(BudgetPeriodRange {
                start_date: format_date(make_date(year, month, 1)?),
                end_date: format_date(last_day_of_month(year, month)?),
            })
        }
    }
}

pub fn build_budget_period_scope(
    input: &BudgetPeriodScopeInput,
    today: NaiveDate,
) -> Result<BudgetPeriodScope, String> {
    let period_type = input
        .period_type
        .as_deref()
        .unwrap_or("monthly")
        .trim()
        .to_string();
    let range = resolve_budget_period_range(input, today)?;
    Ok(BudgetPeriodScope {
        budget_type: input.budget_type.unwrap_or(BUDGET_TYPE_EXPENSE),
        period_type,
        start_date: range.start_date,
        end_date: range.end_date,
        year: input.year,
        month: input.month,
        quarter: input.quarter,
    })
}

pub fn parse_budget_csv_int_list(
    raw: Option<&str>,
    field_name: &str,
) -> Result<Option<Vec<i64>>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let mut values = Vec::new();
    for item in raw.split(',') {
        let cleaned = item.trim();
        if cleaned.is_empty() {
            continue;
        }
        values.push(parse_budget_i64(
            cleaned,
            &format!("Invalid integer in {field_name}"),
        )?);
    }

    if values.is_empty() {
        Ok(None)
    } else {
        Ok(Some(values))
    }
}

pub fn parse_budget_json_int_list(
    raw: Option<&Value>,
    field_name: &str,
) -> Result<Vec<i64>, String> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = raw else {
        return Err(format!(
            "Invalid array for {field_name}: expected JSON array"
        ));
    };

    let mut values = Vec::new();
    for item in items {
        let cleaned = json_int_candidate_text(item);
        if cleaned.is_empty() {
            continue;
        }
        values.push(parse_budget_i64(
            &cleaned,
            &format!("Invalid integer in {field_name}"),
        )?);
    }
    Ok(values)
}

pub fn validate_import_budget_item(item: &Value, index: usize) -> Result<(), String> {
    let Value::Object(object) = item else {
        return Err(format!("第{index}条预算格式无效"));
    };

    for required_field in ["period_type", "amount", "start_date"] {
        if !object.contains_key(required_field) || object[required_field].is_null() {
            return Err(format!("第{index}条预算缺少必填字段: {required_field}"));
        }
    }
    if string_field(object, "category").trim().is_empty() {
        return Err(format!("第{index}条预算缺少必填字段: category"));
    }

    let period_type = string_field(object, "period_type");
    if !is_valid_budget_period_type(period_type.trim()) {
        return Err(format!("第{index}条预算 period_type 无效: {period_type}"));
    }
    validate_budget_date_range(
        Some(&string_field(object, "start_date")),
        non_empty_str(Some(&string_field(object, "end_date"))),
    )?;
    Ok(())
}

pub fn calculate_budget_period_progress(
    start_date: &str,
    end_date: &str,
    today: NaiveDate,
) -> Result<BudgetPeriodProgress, String> {
    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    let total_days = (end - start).num_days() + 1;

    if today < start {
        return Ok(BudgetPeriodProgress {
            elapsed_days: 0,
            remaining_days: total_days,
        });
    }
    if today > end {
        return Ok(BudgetPeriodProgress {
            elapsed_days: total_days,
            remaining_days: 0,
        });
    }

    Ok(BudgetPeriodProgress {
        elapsed_days: (today - start).num_days() + 1,
        remaining_days: (end - today).num_days(),
    })
}

pub fn calculate_avg_backtest_mape(items: &[Value]) -> Option<f64> {
    let mut values = Vec::new();
    for item in items {
        if let Some(value) = item.get("backtest_mape").and_then(value_to_f64) {
            values.push(value);
        }
    }
    average(&values).map(round2)
}

pub fn select_budget_detail_items(items: &[Value]) -> Vec<Value> {
    select_budget_items(items, true)
}

pub fn select_budget_summary_items(items: &[Value]) -> Vec<Value> {
    select_budget_items(items, false)
}

pub fn build_budget_execution_summary(items: &[Value]) -> Value {
    let selected = select_budget_summary_items(items);
    let total_budget = selected
        .iter()
        .map(|item| budget_item_amount(item, "budget_amount"))
        .sum::<f64>();
    let total_spent = selected
        .iter()
        .map(|item| budget_item_amount(item, "spent_amount"))
        .sum::<f64>();
    let total_remaining = total_budget - total_spent;
    let overall_execution_rate = if total_budget > 0.0 {
        round2((total_spent / total_budget) * 100.0)
    } else {
        0.0
    };

    json!({
        "total_budget": total_budget,
        "total_spent": total_spent,
        "total_remaining": total_remaining,
        "overall_execution_rate": overall_execution_rate,
        "count": selected.len()
    })
}

pub fn normalize_budget_category_type(raw: Option<i32>) -> Option<TransactionType> {
    match raw {
        Some(LEGACY_EXPENSE_CATEGORY_TYPE) => Some(TransactionType::Expense),
        Some(value) => TransactionType::from_frontend_code(value).ok(),
        None => None,
    }
}

pub fn build_budget_category_context(categories: &[Value]) -> BudgetCategoryContext {
    let mut context = BudgetCategoryContext::default();
    for category in categories {
        let Value::Object(object) = category else {
            continue;
        };

        let raw_name = string_field(object, "name").trim().to_string();
        let raw_parent_name = string_field(object, "parent_name").trim().to_string();
        let main_category = string_field(object, "main_category").trim().to_string();
        let sub_category = string_field(object, "sub_category").trim().to_string();
        let (main_category, sub_category) = if !main_category.is_empty() {
            (main_category, sub_category)
        } else if !raw_parent_name.is_empty() {
            (raw_parent_name, raw_name)
        } else {
            (raw_name, String::new())
        };

        if main_category.is_empty() {
            continue;
        }
        let category_type = object
            .get("type")
            .and_then(value_to_i32)
            .and_then(|value| normalize_budget_category_type(Some(value)))
            .map(TransactionType::code);
        let Some(category_type) = category_type else {
            continue;
        };
        let info = BudgetCategoryInfo {
            id: value_to_i64(object.get("id")),
            name: main_category.clone(),
            parent_name: String::new(),
            main_category: main_category.clone(),
            sub_category: sub_category.clone(),
            category_type: Some(category_type),
            icon: string_field(object, "icon"),
            color: string_field(object, "color"),
        };

        context
            .types_by_name
            .entry(main_category.clone())
            .or_default()
            .insert(category_type);

        if sub_category.is_empty() {
            context
                .primary_by_key
                .insert((category_type, main_category.clone()), info.clone());
            context.primary_by_name.entry(main_category).or_insert(info);
        } else {
            context.sub_by_key.insert(
                (category_type, main_category.clone(), sub_category.clone()),
                info.clone(),
            );
            context
                .sub_by_parent_name
                .insert((main_category.clone(), sub_category), info.clone());
            let fallback_key = (category_type, main_category);
            let should_replace = context
                .fallback_by_key
                .get(&fallback_key)
                .is_none_or(|existing| existing.icon.is_empty() && !info.icon.is_empty());
            if should_replace {
                context.fallback_by_key.insert(fallback_key, info);
            }
        }
    }
    context
}

pub fn resolve_budget_category_type(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    preferred_type: Option<i32>,
) -> Option<TransactionType> {
    let normalized_preferred = preferred_type
        .and_then(|value| normalize_budget_category_type(Some(value)))
        .map(TransactionType::code);
    let category = category.trim();
    let sub_category = sub_category
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(types) = context.types_by_name.get(category) else {
        return normalized_preferred.and_then(|value| normalize_budget_category_type(Some(value)));
    };
    if sub_category.is_none() && types.len() > 1 {
        if let Some(preferred) = normalized_preferred {
            if context
                .primary_by_key
                .contains_key(&(preferred, category.to_string()))
            {
                return normalize_budget_category_type(Some(preferred));
            }
        }
        for candidate_type in types {
            if context
                .primary_by_key
                .contains_key(&(*candidate_type, category.to_string()))
            {
                return normalize_budget_category_type(Some(*candidate_type));
            }
        }
        return None;
    }

    if let Some(preferred) = normalized_preferred {
        if types.contains(&preferred)
            && budget_category_type_matches(context, category, sub_category, preferred)
        {
            return normalize_budget_category_type(Some(preferred));
        }
    }

    for candidate_type in types {
        if budget_category_type_matches(context, category, sub_category, *candidate_type) {
            return normalize_budget_category_type(Some(*candidate_type));
        }
    }

    None
}

fn budget_category_type_matches(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    candidate_type: i32,
) -> bool {
    if let Some(sub_category) = sub_category {
        return context.sub_by_key.contains_key(&(
            candidate_type,
            category.to_string(),
            sub_category.to_string(),
        ));
    }
    context
        .primary_by_key
        .contains_key(&(candidate_type, category.to_string()))
        || context
            .fallback_by_key
            .contains_key(&(candidate_type, category.to_string()))
}

fn budget_category_info_to_json(info: BudgetCategoryInfo) -> Value {
    json!({
        "id": info.id,
        "main_category": info.main_category,
        "sub_category": info.sub_category,
        "type": info.category_type,
        "icon": info.icon,
        "color": info.color
    })
}

fn merge_budget_primary_with_fallback(
    mut primary: BudgetCategoryInfo,
    fallback: Option<&BudgetCategoryInfo>,
) -> BudgetCategoryInfo {
    let Some(fallback) = fallback else {
        return primary;
    };
    if primary.icon.is_empty() {
        primary.icon = fallback.icon.clone();
    }
    if primary.color.is_empty() {
        primary.color = fallback.color.clone();
    }
    primary
}

pub fn resolve_budget_category_info(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    preferred_type: Option<i32>,
) -> Value {
    let category = category.trim();
    if category.is_empty() {
        return Value::Null;
    }

    let budget_type = preferred_type
        .and_then(|value| normalize_budget_category_type(Some(value)))
        .map(TransactionType::code)
        .unwrap_or(BUDGET_TYPE_EXPENSE);
    let sub_category = sub_category
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(sub_category) = sub_category {
        return context
            .sub_by_key
            .get(&(budget_type, category.to_string(), sub_category.to_string()))
            .cloned()
            .map(budget_category_info_to_json)
            .unwrap_or(Value::Null);
    }

    let primary = context
        .primary_by_key
        .get(&(budget_type, category.to_string()))
        .cloned();
    let fallback = context
        .fallback_by_key
        .get(&(budget_type, category.to_string()));

    match primary {
        Some(primary) if !primary.icon.is_empty() || fallback.is_none() => {
            budget_category_info_to_json(primary)
        }
        Some(primary) => {
            budget_category_info_to_json(merge_budget_primary_with_fallback(primary, fallback))
        }
        None => fallback
            .cloned()
            .map(budget_category_info_to_json)
            .unwrap_or(Value::Null),
    }
}

pub fn get_budget_type_name(budget_type: i32) -> &'static str {
    if budget_type == BUDGET_TYPE_EXPENSE {
        "支出"
    } else {
        "投资"
    }
}

pub fn budget_type_matches_category(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    budget_type: i32,
) -> bool {
    resolve_budget_category_type(context, category, sub_category, Some(budget_type))
        .is_some_and(|category_type| category_type.code() == budget_type)
}

pub fn normalize_budget_query_end_date(end_date: Option<&str>) -> Option<String> {
    let end_date = non_empty_str(end_date)?;
    if end_date.len() <= 10 {
        Some(format!("{end_date} 23:59:59"))
    } else {
        Some(end_date.to_string())
    }
}

pub fn expand_forecast_history_window(
    period_type: &str,
    start_date: &str,
    end_date: &str,
    history_periods: i64,
) -> Result<BudgetPeriodRange, String> {
    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    if history_periods <= 1 {
        return Ok(BudgetPeriodRange {
            start_date: start_date.to_string(),
            end_date: format_date(end),
        });
    }
    let periods_to_expand = history_periods - 1;
    let expanded_start = match period_type {
        "daily" => start - Duration::days(periods_to_expand),
        "weekly" => start - Duration::weeks(periods_to_expand),
        "quarterly" => shift_month_start(start, -3 * periods_to_expand)?,
        "yearly" => make_date(
            start.year()
                - i32::try_from(periods_to_expand)
                    .map_err(|_| "Invalid history periods".to_string())?,
            start.month(),
            start.day(),
        )?,
        _ => shift_month_start(start, -periods_to_expand)?,
    };
    Ok(BudgetPeriodRange {
        start_date: format_date(expanded_start),
        end_date: format_date(end),
    })
}

pub fn build_forecast_period_key(period_type: &str, date: NaiveDate) -> String {
    match period_type {
        "daily" => format_date(date),
        "weekly" => date.format("%Y-%W").to_string(),
        "quarterly" => format!("{}-Q{}", date.year(), ((date.month() - 1) / 3) + 1),
        "yearly" => date.year().to_string(),
        _ => date.format("%Y-%m").to_string(),
    }
}

pub fn resolve_parent_budget_period(
    child_period_type: &str,
    child_start_date: &str,
    parent_period_type: &str,
) -> Result<Option<BudgetPeriodRange>, String> {
    let child_start = parse_budget_date_prefix(child_start_date)?;
    match (child_period_type, parent_period_type) {
        ("monthly", "quarterly") => {
            let quarter = ((child_start.month() - 1) / 3) + 1;
            let start_month = ((quarter - 1) * 3) + 1;
            let end_month = start_month + 2;
            Ok(Some(BudgetPeriodRange {
                start_date: format_date(make_date(child_start.year(), start_month, 1)?),
                end_date: format_date(last_day_of_month(child_start.year(), end_month)?),
            }))
        }
        ("monthly", "yearly") | ("quarterly", "yearly") => Ok(Some(BudgetPeriodRange {
            start_date: format_date(make_date(child_start.year(), 1, 1)?),
            end_date: format_date(make_date(child_start.year(), 12, 31)?),
        })),
        _ => Ok(None),
    }
}

pub fn rollup_parent_amount(existing_parent_amount: f64, child_total: f64) -> f64 {
    existing_parent_amount.max(child_total)
}

pub fn rollup_yearly_child_total(
    quarterly_amounts: &BTreeMap<u32, f64>,
    monthly_totals_by_quarter: &BTreeMap<u32, f64>,
) -> f64 {
    let mut total = 0.0;
    for quarter in 1..=4 {
        let quarterly = quarterly_amounts.get(&quarter).copied().unwrap_or_default();
        let monthly = monthly_totals_by_quarter
            .get(&quarter)
            .copied()
            .unwrap_or_default();
        total += quarterly.max(monthly);
    }
    total
}

pub fn build_budget_history_filter_summary(input: &BudgetHistoryFilterSummaryInput) -> String {
    let mut fields = BTreeMap::new();
    fields.insert(
        "account_ids",
        PythonJsonValue::IntArray(sorted_ids(input.account_ids.as_deref())),
    );
    fields.insert("budget_id", PythonJsonValue::OptionalInt(input.budget_id));
    fields.insert(
        "budget_type",
        PythonJsonValue::Int(i64::from(input.budget_type)),
    );
    fields.insert(
        "category_id",
        PythonJsonValue::OptionalInt(input.category_id),
    );
    fields.insert(
        "period_type",
        PythonJsonValue::String(input.period_type.clone().unwrap_or_default()),
    );
    fields.insert(
        "tag_ids",
        PythonJsonValue::IntArray(sorted_ids(input.tag_ids.as_deref())),
    );

    let rendered = fields
        .into_iter()
        .map(|(key, value)| format!("{}: {}", python_json_string(key), value.render()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{rendered}}}")
}

pub fn iter_budget_history_period_ranges(
    period_type: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<BudgetPeriodRange>, String> {
    let start = parse_budget_date_prefix(start_date)?;
    let end = parse_budget_date_prefix(end_date)?;
    if start > end {
        return Ok(Vec::new());
    }

    let mut ranges = Vec::new();
    let mut current = match period_type {
        "weekly" => start - Duration::days(i64::from(start.weekday().num_days_from_monday())),
        "monthly" => make_date(start.year(), start.month(), 1)?,
        "quarterly" => make_date(start.year(), (((start.month() - 1) / 3) * 3) + 1, 1)?,
        "yearly" => make_date(start.year(), 1, 1)?,
        _ => start,
    };

    while current <= end {
        let range = match period_type {
            "weekly" => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(current + Duration::days(6)),
            },
            "monthly" => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(last_day_of_month(current.year(), current.month())?),
            },
            "quarterly" => {
                let end_month = current.month() + 2;
                BudgetPeriodRange {
                    start_date: format_date(current),
                    end_date: format_date(last_day_of_month(current.year(), end_month)?),
                }
            }
            "yearly" => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(make_date(current.year(), 12, 31)?),
            },
            _ => BudgetPeriodRange {
                start_date: format_date(current),
                end_date: format_date(current),
            },
        };
        ranges.push(range);

        current = match period_type {
            "weekly" => current + Duration::weeks(1),
            "monthly" => shift_month_start(current, 1)?,
            "quarterly" => shift_month_start(current, 3)?,
            "yearly" => shift_month_start(current, 12)?,
            _ => current + Duration::days(1),
        };
    }

    Ok(ranges)
}

pub fn budget_overlaps_period(
    budget_start: &str,
    budget_end: Option<&str>,
    period_start: &str,
    period_end: &str,
) -> Result<bool, String> {
    let budget_start = parse_budget_date_prefix(budget_start)?;
    let budget_end = match non_empty_str(budget_end) {
        Some(end_date) => Some(parse_budget_date_prefix(end_date)?),
        None => None,
    };
    let period_start = parse_budget_date_prefix(period_start)?;
    let period_end = parse_budget_date_prefix(period_end)?;

    Ok(budget_start <= period_end && budget_end.is_none_or(|end| end >= period_start))
}

pub fn build_budget_history_item_from_detail(detail: &Value, period: &BudgetPeriodRange) -> Value {
    build_budget_history_item_from_detail_with_context(
        detail,
        period,
        BUDGET_TYPE_EXPENSE,
        "monthly",
        "",
    )
}

pub fn build_budget_history_item_from_detail_with_context(
    detail: &Value,
    period: &BudgetPeriodRange,
    budget_type: i32,
    period_type: &str,
    filter_summary: &str,
) -> Value {
    let id = value_to_i64(detail.get("id")).unwrap_or_default();
    let budget_amount = budget_item_amount(detail, "budget_amount");
    let spent_amount = budget_item_amount(detail, "spent_amount");
    let status = if spent_amount > budget_amount {
        "over_budget"
    } else {
        "within_budget"
    };

    json!({
        "id": format!("{id}_{}_{}", period.start_date, period.end_date),
        "budget_id": id,
        "period_start": period.start_date,
        "period_end": period.end_date,
        "budget_amount": round2(budget_amount),
        "spent_amount": round2(spent_amount),
        "remaining_amount": detail.get("remaining_amount").cloned().unwrap_or_else(|| json!(round2(budget_amount - spent_amount))),
        "execution_rate": detail.get("execution_rate").cloned().unwrap_or_else(|| json!(if budget_amount > 0.0 { round2((spent_amount / budget_amount) * 100.0) } else { 0.0 })),
        "status": status,
        "filter_summary": filter_summary,
        "calculated_at": "",
        "name": detail.get("name").cloned().unwrap_or_else(|| json!("")),
        "category": detail.get("category").cloned().unwrap_or_else(|| json!("")),
        "sub_category": detail.get("sub_category").cloned().unwrap_or_else(|| json!("")),
        "category_id": detail.get("category_id").cloned().unwrap_or_else(|| json!("")),
        "category_info": detail.get("category_info").cloned().unwrap_or(Value::Null),
        "type": detail.get("type").cloned().unwrap_or_else(|| json!(budget_type)),
        "period_type": detail.get("period_type").cloned().unwrap_or_else(|| json!(period_type)),
        "alert_threshold": detail.get("alert_threshold").cloned().unwrap_or_else(|| json!(80)),
        "enabled": detail.get("enabled").cloned().unwrap_or_else(|| json!(1))
    })
}

pub fn calculate_forecast_amount(amounts: &[f64], strategy: &str) -> Option<f64> {
    if amounts.is_empty() {
        return None;
    }
    let values = if strategy == "moving_average" {
        let window_start = amounts.len().saturating_sub(3);
        &amounts[window_start..]
    } else {
        amounts
    };
    average(values).map(round2)
}

pub fn calculate_forecast_backtest_mape(amounts: &[f64], strategy: &str) -> Option<f64> {
    let mut errors = Vec::new();
    for index in 1..amounts.len() {
        let actual = amounts[index];
        if actual <= 0.0 {
            continue;
        }
        let predicted = calculate_forecast_amount(&amounts[..index], strategy)?;
        errors.push(((actual - predicted).abs() / actual) * 100.0);
    }
    average(&errors).map(round2)
}

pub fn resolve_forecast_confidence(backtest_mape: Option<f64>) -> &'static str {
    match backtest_mape {
        Some(value) if value <= 10.0 => "high",
        Some(value) if value <= 20.0 => "medium",
        _ => "low",
    }
}

pub fn resolve_forecast_trend(amounts: &[f64]) -> &'static str {
    if amounts.len() < 2 {
        return "stable";
    }
    let latest = amounts[amounts.len() - 1];
    let baseline = average(&amounts[..amounts.len() - 1]).unwrap_or_default();
    if baseline <= 0.0 {
        return "stable";
    }
    if latest > baseline * 1.05 {
        "up"
    } else if latest < baseline * 0.95 {
        "down"
    } else {
        "stable"
    }
}

pub fn resolve_forecast_budget_amount(primary_amount: f64, sub_total: f64) -> f64 {
    if primary_amount > 0.0 {
        round2(primary_amount)
    } else {
        round2(sub_total)
    }
}

pub fn build_budget_forecast_item(
    category: &str,
    category_info: Value,
    amounts: &[f64],
    current_spent: f64,
    primary_budget_amount: f64,
    sub_budget_total: f64,
    strategy: &str,
) -> Value {
    build_budget_forecast_item_from_input(BudgetForecastItemInput {
        category,
        category_info,
        amounts,
        current_spent,
        primary_budget_amount,
        sub_budget_total,
        strategy,
        period_count: amounts.len(),
        period_labels: None,
    })
}

pub fn build_budget_forecast_item_from_input(input: BudgetForecastItemInput<'_>) -> Value {
    let normalized_strategy = if input.strategy.trim().is_empty() {
        "historical_average"
    } else {
        input.strategy
    };
    let total_amount = round2(input.amounts.iter().sum());
    let average_amount =
        calculate_forecast_amount(input.amounts, "historical_average").unwrap_or(0.0);
    let forecast_amount =
        calculate_forecast_amount(input.amounts, normalized_strategy).unwrap_or(0.0);
    let budget_amount =
        resolve_forecast_budget_amount(input.primary_budget_amount, input.sub_budget_total);
    let backtest_mape = calculate_forecast_backtest_mape(input.amounts, normalized_strategy);
    let periods = input
        .amounts
        .iter()
        .enumerate()
        .map(|(index, amount)| {
            let period = input
                .period_labels
                .and_then(|labels| labels.get(index))
                .cloned()
                .unwrap_or_else(|| (index + 1).to_string());
            json!({"period": period, "amount": round2(*amount)})
        })
        .collect::<Vec<_>>();

    json!({
        "category": input.category,
        "category_info": input.category_info,
        "total_amount": total_amount,
        "average_amount": average_amount,
        "period_count": input.period_count,
        "sample_periods": input.amounts.len(),
        "current_spent": round2(input.current_spent),
        "budget_amount": budget_amount,
        "forecast_amount": forecast_amount,
        "projected_over_budget": forecast_amount > budget_amount && budget_amount > 0.0,
        "forecast_strategy": normalized_strategy,
        "strategy_explanation": forecast_strategy_explanation(normalized_strategy, input.amounts.len()),
        "backtest_mape": backtest_mape,
        "confidence": resolve_forecast_confidence(backtest_mape),
        "trend": resolve_forecast_trend(input.amounts),
        "periods": periods
    })
}

pub fn build_budget_export_item(row: &Value) -> Value {
    json!({
        "name": field_or_null(row, "name"),
        "category": field_or_null(row, "category"),
        "sub_category": field_or_null(row, "sub_category"),
        "period_type": field_or_null(row, "period_type"),
        "amount": field_or_null(row, "amount"),
        "start_date": field_or_null(row, "start_date"),
        "end_date": field_or_null(row, "end_date"),
        "alert_threshold": field_or_null(row, "alert_threshold"),
        "enabled": field_or_null(row, "enabled")
    })
}

pub fn build_budget_export_response(rows: &[Value]) -> Value {
    json!({
        "success": true,
        "result": rows.iter().map(build_budget_export_item).collect::<Vec<_>>()
    })
}

fn select_budget_items(items: &[Value], detail_mode: bool) -> Vec<Value> {
    let mut grouped: Vec<((String, String, String), Vec<Value>)> = Vec::new();
    for item in items {
        let key = (
            value_string(item.get("category")),
            value_string(item.get("period_type")),
            value_string(item.get("start_date")),
        );
        if let Some((_, group)) = grouped
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &key)
        {
            group.push(item.clone());
        } else {
            grouped.push((key, vec![item.clone()]));
        }
    }

    let mut selected = Vec::new();
    for (_, group) in grouped {
        let (primary, secondary): (Vec<Value>, Vec<Value>) = group
            .iter()
            .cloned()
            .partition(|item| value_string(item.get("sub_category")).trim().is_empty());

        if primary.len() > 1 {
            selected.extend(group);
        } else if let Some(primary_item) = primary.first() {
            if detail_mode
                && !secondary.is_empty()
                && primary_is_synchronized_shadow(primary_item, &secondary)
            {
                selected.extend(secondary);
            } else {
                selected.push(primary_item.clone());
            }
        } else {
            selected.extend(secondary);
        }
    }
    selected
}

fn primary_is_synchronized_shadow(primary: &Value, secondary: &[Value]) -> bool {
    let secondary_budget = secondary
        .iter()
        .map(|item| budget_item_amount(item, "budget_amount"))
        .sum::<f64>();
    let secondary_spent = secondary
        .iter()
        .map(|item| budget_item_amount(item, "spent_amount"))
        .sum::<f64>();
    (budget_item_amount(primary, "budget_amount") - secondary_budget).abs()
        <= SYNCHRONIZED_PRIMARY_TOLERANCE
        && (budget_item_amount(primary, "spent_amount") - secondary_spent).abs()
            <= SYNCHRONIZED_PRIMARY_TOLERANCE
}

fn parse_budget_i64(cleaned: &str, prefix: &str) -> Result<i64, String> {
    cleaned
        .parse::<i64>()
        .map_err(|_| format!("{prefix}: {cleaned}"))
}

fn json_int_candidate_text(item: &Value) -> String {
    match item {
        Value::String(value) => value.trim().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "None".to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn string_field(object: &Map<String, Value>, field: &str) -> String {
    value_string(object.get(field))
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn budget_item_amount(item: &Value, field: &str) -> f64 {
    item.get(field).and_then(value_to_f64).unwrap_or_default()
}

fn value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn value_to_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn field_or_null(row: &Value, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn non_empty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn parse_budget_date_prefix(value: &str) -> Result<NaiveDate, String> {
    let date_text = value
        .trim()
        .get(..10)
        .ok_or_else(|| format!("Invalid date: {value}"))?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| format!("Invalid date: {value}"))
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn make_date(year: i32, month: u32, day: u32) -> Result<NaiveDate, String> {
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("Invalid date components: {year}-{month}-{day}"))
}

fn last_day_of_month(year: i32, month: u32) -> Result<NaiveDate, String> {
    let next_month = if month == 12 {
        make_date(year + 1, 1, 1)?
    } else {
        make_date(year, month + 1, 1)?
    };
    Ok(next_month - Duration::days(1))
}

fn shift_month_start(date: NaiveDate, delta_months: i64) -> Result<NaiveDate, String> {
    let absolute_month = i64::from(date.year()) * 12 + i64::from(date.month0()) + delta_months;
    let year = absolute_month.div_euclid(12);
    let month0 = absolute_month.rem_euclid(12);
    let year = i32::try_from(year).map_err(|_| "Invalid shifted date year".to_string())?;
    let month = u32::try_from(month0 + 1).map_err(|_| "Invalid shifted date month".to_string())?;
    make_date(year, month, 1)
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn sorted_ids(values: Option<&[i64]>) -> Vec<i64> {
    let mut values = values.map_or_else(Vec::new, ToOwned::to_owned);
    values.sort_unstable();
    values
}

fn python_json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn forecast_strategy_explanation(strategy: &str, period_count: usize) -> String {
    if strategy == "moving_average" {
        format!("基于最近{}个周期的移动平均", period_count.min(3))
    } else {
        format!("基于最近{period_count}个周期的历史均值")
    }
}

enum PythonJsonValue {
    Int(i64),
    OptionalInt(Option<i64>),
    String(String),
    IntArray(Vec<i64>),
}

impl PythonJsonValue {
    fn render(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::OptionalInt(Some(value)) => value.to_string(),
            Self::OptionalInt(None) => "null".to_string(),
            Self::String(value) => python_json_string(value),
            Self::IntArray(values) => {
                let body = values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{body}]")
            }
        }
    }
}
