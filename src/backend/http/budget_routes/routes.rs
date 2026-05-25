// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。


pub const BUDGET_CRUD_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/budgets/execution"),
    ("GET", "/api/budgets/forecast"),
    ("GET", "/api/budgets/history"),
    ("POST", "/api/budgets/history/snapshot"),
    ("POST", "/api/budgets/import"),
    ("GET", "/api/budgets"),
    ("GET", "/api/budgets/"),
    ("POST", "/api/budgets"),
    ("POST", "/api/budgets/"),
    ("GET", "/api/budgets/{budget_id}"),
    ("PUT", "/api/budgets/{budget_id}"),
    ("DELETE", "/api/budgets/{budget_id}"),
    ("GET", "/api/budgets/export"),
];

#[tracing::instrument(level = "debug", skip_all)]
pub fn budget_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route("/api/budgets/execution", get(get_budget_execution_handler))
        .route("/api/budgets/forecast", get(get_budget_forecast_handler))
        .route("/api/budgets/history", get(get_budget_history_handler))
        .route(
            "/api/budgets/history/snapshot",
            post(create_budget_history_snapshot_handler),
        )
        .route("/api/budgets/import", post(import_budgets_handler))
        .route("/api/budgets/export", get(export_budgets_handler))
        .route(
            "/api/budgets",
            get(list_budgets_handler).post(create_budget_handler),
        )
        .route(
            "/api/budgets/",
            get(list_budgets_handler).post(create_budget_handler),
        )
        .route(
            "/api/budgets/:budget_id",
            get(get_budget_handler)
                .put(update_budget_handler)
                .delete(delete_budget_handler),
        )
}

#[derive(Debug, Default, Deserialize)]
struct BudgetListQuery {
    period_type: Option<String>,
    enabled: Option<String>,
    category: Option<String>,
    budget_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BudgetExecutionQuery {
    budget_type: Option<String>,
    period_type: Option<String>,
    year: Option<String>,
    month: Option<String>,
    quarter: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    budget_id: Option<String>,
    category_id: Option<String>,
    account_ids: Option<String>,
    tag_ids: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BudgetForecastQuery {
    budget_type: Option<String>,
    period_type: Option<String>,
    year: Option<String>,
    month: Option<String>,
    quarter: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    forecast_strategy: Option<String>,
    months_history: Option<String>,
}
