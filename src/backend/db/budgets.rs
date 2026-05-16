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
include!("budgets/crud_import.rs");
include!("budgets/execution.rs");
include!("budgets/history.rs");
include!("budgets/import_rows.rs");
include!("budgets/forecast.rs");
include!("budgets/execution_candidates.rs");
include!("budgets/listing_mutation.rs");
include!("budgets/hierarchy.rs");
include!("budgets/rows_helpers.rs");
