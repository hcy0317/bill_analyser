// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bill_analyser_core::budgets::{
    build_budget_execution_summary, build_budget_period_scope, calculate_avg_backtest_mape,
    calculate_budget_period_progress, parse_budget_csv_int_list, parse_budget_json_int_list,
    validate_budget_date_range, validate_budget_period_args, BudgetPeriodScopeInput,
};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_budget, create_budget_execution_snapshots, create_postgres_budget,
    create_postgres_budget_execution_snapshots, delete_budget, delete_postgres_budget,
    export_budgets, export_postgres_budgets, get_budget_by_id, get_postgres_budget_by_id,
    import_budgets, import_postgres_budgets, query_budget_execution_details,
    query_budget_execution_history, query_budget_forecast, query_budgets_for_listing,
    query_postgres_budget_execution_details, query_postgres_budget_execution_history,
    query_postgres_budget_forecast, query_postgres_budgets_for_listing, update_budget,
    update_postgres_budget, BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters,
    BudgetForecastFilters, BudgetRecord, BudgetUpdateDraft, PostgresRepositoryRuntime,
    SqliteRuntime,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

type RouteResult<T> = Result<T, Box<Response>>;
include!("budget_routes/routes.rs");
include!("budget_routes/handlers.rs");
include!("budget_routes/payloads.rs");
include!("budget_routes/runtime_helpers.rs");
