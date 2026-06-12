// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::TransactionType;

pub const BUDGET_TYPE_EXPENSE: i32 = 3;
pub const BUDGET_TYPE_INVESTMENT: i32 = 5;
pub const CURRENT_EXPENSE_CATEGORY_TYPE: i32 = 1;
pub const VALID_BUDGET_PERIOD_TYPES: [&str; 5] =
    ["daily", "weekly", "monthly", "quarterly", "yearly"];

const SYNCHRONIZED_PRIMARY_TOLERANCE_CENTS: i64 = 1;

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
    pub amounts_cents: &'a [i64],
    pub current_spent_cents: i64,
    pub primary_budget_amount_cents: i64,
    pub sub_budget_total_cents: i64,
    pub strategy: &'a str,
    pub period_count: usize,
    pub period_labels: Option<&'a [String]>,
}
include!("budgets/period_validation.rs");
include!("budgets/execution_summary.rs");
include!("budgets/category_context.rs");
include!("budgets/history_periods.rs");
include!("budgets/forecast_export.rs");
include!("budgets/helpers.rs");
