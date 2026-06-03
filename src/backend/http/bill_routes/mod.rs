// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{fmt::Write as _, fs, path::Path as FsPath};

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::adapters::transaction::{
    apply_create_category_contract, apply_manual_create_defaults,
    batch_create_persist_error_route_response, batch_create_prepare_error_route_response,
    batch_create_success_route_response, batch_create_transaction_items,
    batch_delete_success_payload, batch_update_response, build_reconciliation_transactions,
    calculate_reconciliation_summary, delete_bill_success_payload,
    frontend_transaction_from_backend, frontend_transaction_mutation_to_backend,
    frontend_transaction_type_from_backend, invalid_transaction_picture_file_response,
    is_allowed_transaction_picture_filename, missing_transaction_picture_file_response,
    missing_unused_transaction_picture_id_response, parse_reconciliation_query,
    reconciliation_account_not_found_response, reconciliation_category_filters,
    reconciliation_internal_error_response, reconciliation_opening_balance,
    reconciliation_result_payload, reconciliation_type_filter,
    remove_unused_transaction_picture_success_response, serialize_optional_export_cell,
    transaction_list_type_filter, transaction_picture_data_url_from_base64,
    transaction_picture_delete_path, transaction_picture_internal_error_response,
    transaction_picture_upload_id, transaction_picture_upload_success_response,
    unsupported_transaction_picture_type_response, BackendTransactionView, FrontendTransactionTag,
    ReconciliationBill, ReconciliationOpeningBalanceSnapshot, ReconciliationQueryParams,
    RouteResponseContract, EXPORT_COLUMNS,
};
use bill_analyser_core::category_rules::{escape_rule_expression_term, match_rule_expression};
use bill_analyser_core::{Money, RuntimeError, UserId, UtcOffsetMinutes};
use bill_analyser_db::{
    batch_create_postgres_bills, batch_delete_postgres_bills, batch_update_postgres_bills,
    bind_postgres_bill_to_recurring, create_postgres_bill, create_postgres_category_rule,
    delete_postgres_bill, get_first_postgres_account_id, get_postgres_bill_by_id,
    get_postgres_bill_recurring_candidates, get_postgres_bill_tags, get_postgres_category_by_name,
    get_postgres_reconciliation_account, list_postgres_category_rules,
    list_postgres_reconciliation_categories, postgres_category_filters_for_ids,
    query_postgres_bills, resolve_postgres_category_by_id, unbind_postgres_bill_from_recurring,
    update_postgres_bill, BillCategoryFilter, BillCreateDraft, BillFilters, BillRecord,
    BillUpdateDraft, CategoryRuleRecord, PostgresPool,
};
use chrono::{DateTime, Local};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const DEFAULT_UTC_OFFSET_MINUTES: i32 = 480;

type RouteResult<T> = Result<T, Box<Response>>;

pub const BILL_CRUD_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/bills"),
    ("GET", "/api/bills/"),
    ("POST", "/api/bills"),
    ("POST", "/api/bills/"),
    ("GET", "/api/bills/by-month"),
    ("GET", "/api/bills/get"),
    ("GET", "/api/bills/{bill_id}"),
    ("PUT", "/api/bills/{bill_id}"),
    ("DELETE", "/api/bills/{bill_id}"),
    ("POST", "/api/bills/batch"),
    ("PUT", "/api/bills/batch/update"),
    ("DELETE", "/api/bills/batch/delete"),
    ("GET", "/api/bills/export"),
    ("GET", "/api/bills/reconciliation_statements"),
    ("POST", "/api/bills/category/quick-add-rule"),
    ("POST", "/api/bills/category/refresh"),
    ("GET", "/api/bills/{bill_id}/recurring-candidates"),
    ("PUT", "/api/bills/{bill_id}/recurring-match"),
    ("DELETE", "/api/bills/{bill_id}/recurring-match"),
    ("POST", "/api/bills/pictures"),
    ("POST", "/api/bills/pictures/unused"),
];

#[tracing::instrument(level = "debug", skip_all)]
pub fn bill_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route("/api/bills/export", get(export_bills_handler))
        .route(
            "/api/bills/pictures",
            post(upload_transaction_picture_handler),
        )
        .route(
            "/api/bills/pictures/unused",
            post(remove_unused_transaction_picture_handler),
        )
        .route(
            "/api/bills/reconciliation_statements",
            get(reconciliation_statements_handler),
        )
        .route(
            "/api/bills/category/quick-add-rule",
            post(quick_add_category_rule_handler),
        )
        .route(
            "/api/bills/category/refresh",
            post(refresh_bill_categories_handler),
        )
        .route(
            "/api/bills/:bill_id/recurring-candidates",
            get(recurring_candidates_handler),
        )
        .route(
            "/api/bills/:bill_id/recurring-match",
            put(bind_recurring_match_handler).delete(unbind_recurring_match_handler),
        )
        .route(
            "/api/bills",
            get(list_bills_handler).post(create_bill_handler),
        )
        .route(
            "/api/bills/",
            get(list_bills_handler).post(create_bill_handler),
        )
        .route("/api/bills/by-month", get(bills_by_month_handler))
        .route("/api/bills/get", get(get_bill_query_handler))
        .route("/api/bills/batch", post(batch_create_bills_handler))
        .route("/api/bills/batch/update", put(batch_update_bills_handler))
        .route(
            "/api/bills/batch/delete",
            delete(batch_delete_bills_handler),
        )
        .route(
            "/api/bills/:bill_id",
            get(get_bill_path_handler)
                .put(update_bill_path_handler)
                .delete(delete_bill_path_handler),
        )
}

include!("query.rs");
include!("crud_handlers.rs");
include!("batch_handlers.rs");
include!("export_handlers.rs");
include!("picture_handlers.rs");
include!("recurring_handlers.rs");
include!("reconciliation_handlers.rs");
include!("category_action_handlers.rs");
include!("payload_helpers.rs");
include!("record_presenters.rs");
include!("response_helpers.rs");
include!("value_helpers.rs");
