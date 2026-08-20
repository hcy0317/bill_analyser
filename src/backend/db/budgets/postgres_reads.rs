use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::{
    budgets::{
        build_budget_category_context, build_budget_forecast_item_from_input,
        build_budget_history_item_from_detail_with_context, expand_forecast_history_window,
        get_budget_type_name, iter_budget_history_period_ranges, resolve_budget_category_info,
        resolve_budget_category_type, resolve_parent_budget_period, BudgetForecastItemInput,
        BudgetPeriodKind,
    },
    UserId,
};
use chrono::{DateTime, Datelike, NaiveDate, SecondsFormat, Utc};
use serde_json::{json, Map, Number, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row, Transaction};

use crate::category_path::{
    category_names_from_postgres_path, push_postgres_bill_main_category_expr,
    push_postgres_bill_sub_category_expr,
};
use crate::{DbError, DbResult, PostgresPool};

use super::{
    BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastBudgetAmount,
    BudgetForecastCategoryTotals, BudgetForecastFilters, BudgetForecastPeriodAmount,
    BudgetForecastRow, BudgetGroupKey, BudgetPeriodGroupKey, BudgetRecord, BudgetUpdateDraft,
    ImportBudgetRowAction, BUDGET_UPDATE_COLUMNS,
};

include!("postgres_reads/public_api.rs");
include!("postgres_reads/listing.rs");
include!("postgres_reads/execution.rs");
include!("postgres_reads/forecast.rs");
include!("postgres_reads/history.rs");
include!("postgres_reads/mapping.rs");
include!("postgres_reads/mutation_sql.rs");
include!("postgres_reads/period_sync.rs");
include!("postgres_reads/value_helpers.rs");
include!("postgres_reads/tests.rs");
