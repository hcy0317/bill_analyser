// 中文导读：PostgreSQL budgets DTO 与预算执行/历史纯 helper。实际仓储读写在 `budgets::postgres_reads`。
// 维护重点：保留预算聚合、筛选和 payload 规范化逻辑，不保留 non-Postgres 查询/事务 runtime。

use std::collections::{BTreeMap, BTreeSet};

pub(crate) use bill_analyser_core::budgets::normalize_budget_query_end_date;
use bill_analyser_core::budgets::{
    budget_overlaps_period, build_budget_history_filter_summary, resolve_budget_category_info,
    resolve_budget_category_type, BudgetHistoryFilterSummaryInput,
};
use chrono::Utc;
use serde_json::{json, Map, Number, Value};

use crate::{DbError, DbResult};

pub mod postgres_reads;

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
pub(crate) struct BudgetGroupKey {
    category: String,
    period_type: String,
    start_date: String,
    user_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) struct BudgetPeriodGroupKey {
    category: String,
    sub_category: String,
    period_type: String,
    start_date: String,
    user_id: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct BudgetForecastBudgetAmount {
    pub(crate) primary_cents: i64,
    pub(crate) sub_total_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct BudgetForecastCategoryTotals {
    pub(crate) periods: Vec<BudgetForecastPeriodAmount>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BudgetForecastPeriodAmount {
    pub(crate) period: String,
    pub(crate) amount_cents: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BudgetForecastRow {
    period: String,
    category: String,
    amount_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportBudgetRowAction {
    Created,
    Updated,
}

const BUDGET_UPDATE_COLUMNS: &[&str] = &[
    "name",
    "category",
    "sub_category",
    "period_type",
    "amount_cents",
    "start_date",
    "end_date",
    "alert_threshold",
    "enabled",
    "updated_at",
];

#[tracing::instrument(level = "debug", skip_all)]
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
    for field in ["category", "period_type", "amount_cents", "start_date"] {
        if missing_required_field(&payload, field) {
            return Err(DbError::InvalidOperation(format!(
                "missing required budget field: {field}"
            )));
        }
    }
    validate_budget_amount_cents(&payload)?;
    Ok(payload)
}

#[tracing::instrument(level = "debug", skip_all)]
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
    if payload.contains_key("amount_cents") {
        validate_budget_amount_cents(&payload)?;
    }
    Ok(payload)
}

fn validate_budget_amount_cents(payload: &BudgetRecord) -> DbResult<()> {
    record_i64(payload, "amount_cents")
        .map(|_| ())
        .ok_or_else(|| DbError::InvalidOperation("invalid budget amount_cents".to_string()))
}

fn should_copy_existing_sub_category(payload: &BudgetRecord) -> bool {
    [
        "category",
        "period_type",
        "amount_cents",
        "start_date",
        "end_date",
    ]
    .iter()
    .any(|key| payload.contains_key(*key))
}

fn budget_import_has_required_name_and_amount(budget: &BudgetRecord) -> bool {
    !missing_required_field(budget, "name") && !missing_required_field(budget, "amount_cents")
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

#[tracing::instrument(level = "debug", skip_all)]
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
                record_i64(&budget, "amount_cents").unwrap_or_default(),
                record_i64(&budget, "id").unwrap_or_default(),
            ) >= (
                record_i64(current, "amount_cents").unwrap_or_default(),
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
fn build_budget_execution_item(
    budget: &BudgetRecord,
    spent_cents: i64,
    category_context: &bill_analyser_core::budgets::BudgetCategoryContext,
    fallback_budget_type: i32,
) -> Value {
    let budget_amount_cents = record_i64(budget, "amount_cents").unwrap_or_default();
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
    let execution_rate = if budget_amount_cents > 0 {
        round2((spent_cents as f64 / budget_amount_cents as f64) * 100.0)
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
        "budget_amount_cents": budget_amount_cents,
        "spent_amount_cents": spent_cents,
        "remaining_amount_cents": budget_amount_cents - spent_cents,
        "execution_rate": execution_rate,
        "type": resolved_budget_type,
        "alert_threshold": record_i64(budget, "alert_threshold").unwrap_or(80),
        "start_date": field_or_null(budget, "start_date"),
        "end_date": field_or_null(budget, "end_date"),
        "enabled": record_i64(budget, "enabled").unwrap_or(1)
    })
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

fn normalize_sub_category(value: Option<&Value>) -> String {
    value_string(value).trim().to_string()
}

fn missing_required_field(payload: &BudgetRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn record_text(record: &BudgetRecord, key: &str) -> String {
    value_string(record.get(key))
}

fn record_i64(record: &BudgetRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
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

fn value_field_string(value: &Value, key: &str) -> String {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .map(|value| value_string(Some(value)))
        .unwrap_or_default()
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn field_or_null(row: &BudgetRecord, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
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

    fn budget_record(entries: impl IntoIterator<Item = (&'static str, Value)>) -> BudgetRecord {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    #[test]
    fn budget_create_update_and_import_helpers_require_explicit_cents() {
        let payload = budget_record([
            ("category", json!("餐饮")),
            ("sub_category", json!(" 午餐 ")),
            ("period_type", json!("monthly")),
            ("amount_cents", json!(12345)),
            ("start_date", json!("2026-06-01")),
        ]);

        let normalized =
            normalize_create_payload(&payload, "2026-06-12 12:00:00").expect("create payload");
        assert_eq!(normalized.get("amount_cents"), Some(&json!(12345)));
        assert_eq!(normalized.get("sub_category"), Some(&json!("午餐")));
        assert_eq!(normalized.get("enabled"), Some(&json!(true)));
        assert!(budget_import_has_required_name_and_amount(&budget_record(
            [("name", json!("餐饮预算")), ("amount_cents", json!(12345)),]
        )));
        assert!(!budget_import_has_required_name_and_amount(&budget_record(
            [("name", json!("餐饮预算")), ("amount", json!(123.45)),]
        )));

        let existing = budget_record([("sub_category", json!("旧午餐"))]);
        let update =
            normalize_update_payload(&existing, &budget_record([("amount_cents", json!(23456))]))
                .expect("update payload");
        assert_eq!(update.get("amount_cents"), Some(&json!(23456)));
        assert_eq!(update.get("sub_category"), Some(&json!("旧午餐")));

        assert!(normalize_create_payload(
            &budget_record([
                ("category", json!("餐饮")),
                ("period_type", json!("monthly")),
                ("amount", json!(123.45)),
                ("start_date", json!("2026-06-01")),
            ]),
            "2026-06-12 12:00:00",
        )
        .is_err());
        assert!(normalize_create_payload(
            &budget_record([
                ("category", json!("餐饮")),
                ("period_type", json!("monthly")),
                ("amount_cents", json!(true)),
                ("start_date", json!("2026-06-01")),
            ]),
            "2026-06-12 12:00:00",
        )
        .is_err());
        assert_eq!(
            record_i64(
                &budget_record([("amount_cents", json!(true))]),
                "amount_cents"
            ),
            None
        );
        assert!(normalize_update_payload(
            &BudgetRecord::new(),
            &budget_record([("amount_cents", json!(12.34))])
        )
        .is_err());
    }

    #[test]
    fn budget_execution_helpers_use_cents_for_selection_and_output() {
        let lower = budget_record([
            ("id", json!(1)),
            ("category", json!("餐饮")),
            ("sub_category", json!("午餐")),
            ("period_type", json!("monthly")),
            ("amount_cents", json!(1000)),
            ("start_date", json!("2026-06-01")),
        ]);
        let higher = budget_record([
            ("id", json!(2)),
            ("category", json!("餐饮")),
            ("sub_category", json!("午餐")),
            ("period_type", json!("monthly")),
            ("amount_cents", json!(2000)),
            ("start_date", json!("2026-06-01")),
            ("_resolved_budget_type", json!(3)),
        ]);

        let deduped = dedupe_budget_execution_candidates(vec![lower, higher.clone()]);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].get("id"), Some(&json!(2)));

        let categories = vec![json!({
            "id": 9,
            "main_category": "餐饮",
            "sub_category": "午餐",
            "type": 3,
            "icon": "meal",
            "color": "#f80"
        })];
        let context = bill_analyser_core::budgets::build_budget_category_context(&categories);
        let item = build_budget_execution_item(&higher, 1500, &context, 3);

        assert_eq!(item["budget_amount_cents"], json!(2000));
        assert_eq!(item["spent_amount_cents"], json!(1500));
        assert_eq!(item["remaining_amount_cents"], json!(500));
        assert_eq!(item["execution_rate"], json!(75.0));
        assert_eq!(item["category_id"], json!("9"));
    }
}
