// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::collections::BTreeMap;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use bill_analyser_core::{category_rules::match_rule_expression, UserId};
use bill_analyser_db::{
    get_postgres_login_user_by_id, get_postgres_operation_password, list_postgres_llm_configs,
    load_postgres_ocr_config_setting, sync_all_postgres_account_balances,
    taxonomy::{
        account_rules::AccountRuleRecord,
        accounts::{AccountDisplayOrder, AccountRecord},
        categories::{CategoryRecord, CategoryStatistic},
        category_rules::CategoryRuleRecord,
        postgres_reads::{
            clear_postgres_account_transactions, create_postgres_account,
            create_postgres_account_audit_event, create_postgres_account_rule,
            create_postgres_category, create_postgres_category_rule, create_postgres_tag,
            create_postgres_template, delete_postgres_account, delete_postgres_account_rule,
            delete_postgres_categories_by_main_category, delete_postgres_category,
            delete_postgres_category_rule, delete_postgres_tag, delete_postgres_template,
            ensure_postgres_category_rule_defaults, get_postgres_account_by_id,
            get_postgres_account_rule, get_postgres_category_by_id, get_postgres_category_by_name,
            get_postgres_category_rule, get_postgres_sub_accounts, get_postgres_tag,
            get_postgres_template_by_id, list_postgres_account_rules, list_postgres_accounts,
            list_postgres_categories, list_postgres_category_rules, list_postgres_tags,
            list_postgres_templates, move_all_postgres_account_transactions,
            query_postgres_category_statistics, query_postgres_rules_overview_payload,
            reorder_postgres_account_rules, reorder_postgres_category_rules,
            test_postgres_account_rule_match, update_postgres_account,
            update_postgres_account_display_orders, update_postgres_account_rule,
            update_postgres_category, update_postgres_category_display_order,
            update_postgres_category_rule, update_postgres_main_category_name, update_postgres_tag,
            update_postgres_tag_display_orders, update_postgres_template,
            update_postgres_template_display_orders,
        },
        settings_bundle::{export_taxonomy_sections, import_postgres_settings_bundle},
        tags::{TagDisplayOrder, TagRecord},
        templates::{TemplateDisplayOrder, TemplateRecord},
    },
    AccountAuditEventDraft, AccountBalanceDiscrepancy, PostgresPool, SyncAllAccountBalancesResult,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use crate::{
    auth::resolve_user_id_from_headers,
    bill_routes::recategorize_bills_with_category_rules_postgres, config::HttpShellConfig,
    state::HttpAppState,
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const SETTINGS_BUNDLE_SCHEMA_VERSION: i64 = 1;
const SETTINGS_BUNDLE_SECTION_KEYS: &[&str] = &[
    "accounts",
    "transactionCategories",
    "transactionTags",
    "transactionTemplates",
    "scheduledTransactions",
    "categoryRecognitionRules",
    "accountRecognitionRules",
    "llmConfigs",
    "ocrConfig",
];
const SENSITIVE_EXPORT_SECTIONS: &[&str] = &["llmConfigs", "ocrConfig"];

type RouteResult<T> = Result<T, Box<Response>>;

pub const TAXONOMY_ACCOUNT_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/accounts"),
    ("GET", "/api/accounts/"),
    ("POST", "/api/accounts"),
    ("POST", "/api/accounts/"),
    ("GET", "/api/accounts/{account_id}"),
    ("PUT", "/api/accounts/{account_id}"),
    ("DELETE", "/api/accounts/{account_id}"),
    ("PUT", "/api/accounts/display-orders"),
    ("POST", "/api/accounts/sync-balances"),
    ("POST", "/api/accounts/{account_id}/transactions/clear"),
    ("POST", "/api/accounts/{account_id}/transactions/move"),
];

pub const TAXONOMY_TAG_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/tags"),
    ("GET", "/api/tags/"),
    ("POST", "/api/tags"),
    ("POST", "/api/tags/"),
    ("POST", "/api/tags/batch"),
    ("GET", "/api/tags/{tag_id}"),
    ("PUT", "/api/tags/{tag_id}"),
    ("DELETE", "/api/tags/{tag_id}"),
    ("PUT", "/api/tags/display-orders"),
];

pub const TAXONOMY_TEMPLATE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/templates"),
    ("GET", "/api/templates/"),
    ("POST", "/api/templates"),
    ("POST", "/api/templates/"),
    ("GET", "/api/templates/{template_id}"),
    ("PUT", "/api/templates/{template_id}"),
    ("DELETE", "/api/templates/{template_id}"),
    ("PUT", "/api/templates/display-orders"),
];

pub const TAXONOMY_CATEGORY_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/categories"),
    ("GET", "/api/categories/"),
    ("POST", "/api/categories"),
    ("POST", "/api/categories/"),
    ("GET", "/api/categories/all"),
    ("PUT", "/api/categories/all"),
    ("POST", "/api/categories/batch"),
    ("GET", "/api/categories/export"),
    ("GET", "/api/categories/flat"),
    ("POST", "/api/categories/import"),
    ("POST", "/api/categories/move"),
    ("GET", "/api/categories/statistics"),
    ("GET", "/api/categories/tree"),
    ("POST", "/api/categories/update-all"),
    ("GET", "/api/categories/{category_id}"),
    ("PUT", "/api/categories/{category_id}"),
    ("DELETE", "/api/categories/{category_id}"),
];

pub const TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/category-rules/"),
    ("POST", "/api/category-rules/"),
    ("DELETE", "/api/category-rules/{rule_id}"),
    ("PUT", "/api/category-rules/{rule_id}"),
    ("POST", "/api/category-rules/{rule_id}/test"),
    ("POST", "/api/category-rules/defaults"),
    ("POST", "/api/category-rules/reorder"),
];

pub const TAXONOMY_ACCOUNT_RULE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/account-rules/"),
    ("POST", "/api/account-rules/"),
    ("DELETE", "/api/account-rules/{rule_id}"),
    ("PUT", "/api/account-rules/{rule_id}"),
    ("POST", "/api/account-rules/{rule_id}/test"),
    ("POST", "/api/account-rules/reorder"),
];

pub const TAXONOMY_RULE_CENTER_ROUTE_PATTERNS: &[(&str, &str)] = &[("GET", "/api/rules/overview")];

pub const TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/settings/bundle/export"),
    ("POST", "/api/settings/bundle/import"),
    ("POST", "/api/settings/bundle/import/preview"),
    ("GET", "/api/settings/bundle/sections/{section_key}/export"),
    ("POST", "/api/settings/bundle/sections/{section_key}/export"),
    ("POST", "/api/settings/bundle/sections/{section_key}/import"),
    (
        "POST",
        "/api/settings/bundle/sections/{section_key}/import/preview",
    ),
];

/// 中文说明：注册 taxonomy/settings 运行态路由，保持账户、分类、标签、模板、规则和设置包 API 主链。
#[tracing::instrument(level = "debug", skip_all)]
pub fn taxonomy_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route("/api/rules/overview", get(rules_overview_handler))
        .route(
            "/api/settings/bundle/export",
            get(export_settings_bundle_handler),
        )
        .route(
            "/api/settings/bundle/import",
            axum::routing::post(import_settings_bundle_handler),
        )
        .route(
            "/api/settings/bundle/import/preview",
            axum::routing::post(preview_import_settings_bundle_handler),
        )
        .route(
            "/api/settings/bundle/sections/:section_key/export",
            get(export_settings_bundle_section_get_handler)
                .post(export_settings_bundle_section_post_handler),
        )
        .route(
            "/api/settings/bundle/sections/:section_key/import",
            axum::routing::post(import_settings_bundle_section_handler),
        )
        .route(
            "/api/settings/bundle/sections/:section_key/import/preview",
            axum::routing::post(preview_import_settings_bundle_section_handler),
        )
        .route(
            "/api/accounts",
            get(list_accounts_handler).post(create_account_handler),
        )
        .route(
            "/api/accounts/",
            get(list_accounts_handler).post(create_account_handler),
        )
        .route(
            "/api/accounts/display-orders",
            put(update_account_display_orders_handler),
        )
        .route(
            "/api/accounts/sync-balances",
            post(sync_account_balances_handler),
        )
        .route(
            "/api/accounts/:account_id/transactions/move",
            post(move_account_transactions_handler),
        )
        .route(
            "/api/accounts/:account_id/transactions/clear",
            post(clear_account_transactions_handler),
        )
        .route(
            "/api/accounts/:account_id",
            get(get_account_handler)
                .put(update_account_handler)
                .delete(delete_account_handler),
        )
        .route("/api/tags", get(list_tags_handler).post(create_tag_handler))
        .route(
            "/api/tags/",
            get(list_tags_handler).post(create_tag_handler),
        )
        .route(
            "/api/tags/display-orders",
            put(update_tag_display_orders_handler),
        )
        .route(
            "/api/tags/batch",
            axum::routing::post(batch_create_tags_handler),
        )
        .route(
            "/api/tags/:tag_id",
            get(get_tag_handler)
                .put(update_tag_handler)
                .delete(delete_tag_handler),
        )
        .route(
            "/api/templates",
            get(list_templates_handler).post(create_template_handler),
        )
        .route(
            "/api/templates/",
            get(list_templates_handler).post(create_template_handler),
        )
        .route(
            "/api/templates/display-orders",
            put(update_template_display_orders_handler),
        )
        .route(
            "/api/templates/:template_id",
            get(get_template_handler)
                .put(update_template_handler)
                .delete(delete_template_handler),
        )
        .route(
            "/api/categories",
            get(list_categories_handler).post(create_category_handler),
        )
        .route(
            "/api/categories/",
            get(list_categories_handler).post(create_category_handler),
        )
        .route("/api/categories/tree", get(list_categories_handler))
        .route("/api/categories/flat", get(flat_categories_handler))
        .route(
            "/api/categories/all",
            get(all_categories_handler).put(update_all_categories_handler),
        )
        .route(
            "/api/categories/batch",
            axum::routing::post(batch_create_categories_handler),
        )
        .route("/api/categories/export", get(export_categories_handler))
        .route(
            "/api/categories/import",
            axum::routing::post(import_categories_handler),
        )
        .route(
            "/api/categories/move",
            axum::routing::post(move_categories_handler),
        )
        .route(
            "/api/categories/statistics",
            get(category_statistics_handler),
        )
        .route(
            "/api/categories/update-all",
            axum::routing::post(recategorize_all_bills_handler),
        )
        .route(
            "/api/category-rules/",
            get(list_category_rules_handler).post(create_category_rule_handler),
        )
        .route(
            "/api/category-rules/reorder",
            axum::routing::post(reorder_category_rules_handler),
        )
        .route(
            "/api/category-rules/defaults",
            axum::routing::post(ensure_category_rule_defaults_handler),
        )
        .route(
            "/api/category-rules/:rule_id",
            put(update_category_rule_handler).delete(delete_category_rule_handler),
        )
        .route(
            "/api/category-rules/:rule_id/test",
            axum::routing::post(test_category_rule_handler),
        )
        .route(
            "/api/account-rules/",
            get(list_account_rules_handler).post(create_account_rule_handler),
        )
        .route(
            "/api/account-rules/reorder",
            axum::routing::post(reorder_account_rules_handler),
        )
        .route(
            "/api/account-rules/:rule_id",
            put(update_account_rule_handler).delete(delete_account_rule_handler),
        )
        .route(
            "/api/account-rules/:rule_id/test",
            axum::routing::post(test_account_rule_handler),
        )
        .route(
            "/api/categories/:category_id",
            get(get_category_handler)
                .put(update_category_handler)
                .delete(delete_category_handler),
        )
}

#[derive(Debug, Default, Deserialize)]
struct CategoryStatisticsQuery {
    period: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    #[serde(rename = "type")]
    category_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CategoryRulesQuery {
    category_id: Option<i64>,
    enabled_only: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AccountRulesQuery {
    account_id: Option<i64>,
    enabled_only: Option<String>,
    account_role_scope: Option<String>,
    transaction_type_scope: Option<String>,
}

include!("account_handlers.rs");
include!("tag_template_handlers.rs");
include!("category_handlers.rs");
include!("category_rule_handlers.rs");
include!("account_rule_handlers.rs");
include!("settings_bundle_handlers.rs");
include!("account_category_formatters.rs");
include!("settings_serialization.rs");
include!("audit_and_rules_helpers.rs");
include!("common_helpers.rs");
