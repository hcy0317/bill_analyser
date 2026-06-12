// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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
    create_postgres_budget, create_postgres_budget_execution_snapshots, delete_postgres_budget,
    export_postgres_budgets, get_postgres_budget_by_id, import_postgres_budgets,
    query_postgres_budget_execution_details, query_postgres_budget_execution_history,
    query_postgres_budget_forecast, query_postgres_budgets_for_listing, update_postgres_budget,
    BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastFilters, BudgetRecord,
    BudgetUpdateDraft, PostgresRepositoryRuntime,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_payloads_accept_explicit_camel_cents_and_normalize_to_db_field() {
        let imported = import_budget_records_from_payload(&json!([
            {
                "name": "餐饮月预算",
                "category": "餐饮",
                "period_type": "monthly",
                "amountCents": 12345,
                "start_date": "2026-06-01"
            }
        ]))
        .expect("import payload");
        assert_eq!(imported[0].get("amount_cents"), Some(&json!(12345)));
        assert!(imported[0].get("amountCents").is_none());

        let created = create_fields_from_payload(&json!({
            "category": "餐饮",
            "period_type": "monthly",
            "amountCents": 23456,
            "start_date": "2026-06-01"
        }))
        .expect("create fields");
        assert_eq!(created.get("amount_cents"), Some(&json!(23456)));
        assert_eq!(created.get("enabled"), Some(&json!(true)));
        assert_eq!(created.get("alert_threshold"), Some(&json!(80)));

        let mut existing = BudgetRecord::new();
        existing.insert("start_date".to_string(), json!("2026-06-01"));
        let updated = update_fields_from_payload(&json!({"amountCents": 34567}), &existing)
            .expect("update fields");
        assert_eq!(updated.get("amount_cents"), Some(&json!(34567)));
        assert!(updated.get("updated_at").is_some());
    }

    #[test]
    fn budget_payloads_require_explicit_amount_cents_after_normalization() {
        assert!(validate_import_budget_record(
            &Map::from_iter([
                ("category".to_string(), json!("餐饮")),
                ("period_type".to_string(), json!("monthly")),
                ("start_date".to_string(), json!("2026-06-01")),
            ]),
            0,
        )
        .is_err());

        assert_eq!(
            forecast_amount_cents(&json!({"forecast_amount_cents": 9876})),
            9876
        );
        assert_eq!(forecast_amount_cents(&json!({})), 0);
    }
}
