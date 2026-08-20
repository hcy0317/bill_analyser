// 中文导读：PostgreSQL budgets DTO 与预算执行/历史纯 helper。实际仓储读写在 `budgets::postgres_reads`。
// 维护重点：保留预算聚合、筛选和 payload 规范化逻辑，不保留 non-Postgres 查询/事务 runtime。

use std::collections::{BTreeMap, BTreeSet};

pub(crate) use bill_analyser_core::budgets::normalize_budget_query_end_date;
use bill_analyser_core::budgets::{
    budget_overlaps_period, build_budget_history_filter_summary, resolve_budget_category_info,
    resolve_budget_category_type, BudgetHistoryFilterSummaryInput, BudgetPeriodKind,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BudgetGroupKey {
    category: String,
    period_kind: BudgetPeriodKind,
    start_date: String,
    user_id: i64,
}

impl BudgetGroupKey {
    fn period_type(&self) -> &'static str {
        self.period_kind.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BudgetPeriodGroupKey {
    category: String,
    sub_category: String,
    period_kind: BudgetPeriodKind,
    start_date: String,
    user_id: i64,
}

impl BudgetPeriodGroupKey {
    fn period_type(&self) -> &'static str {
        self.period_kind.as_str()
    }
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

include!("budgets/payloads.rs");
include!("budgets/execution.rs");
include!("budgets/history.rs");
include!("budgets/record_helpers.rs");
include!("budgets/tests.rs");
