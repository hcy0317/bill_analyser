//! Rust HTTP ingress for Bill Analyser.
//!
//! This crate owns the backend HTTP runtime directly. All routable `/api/...`
//! business endpoints are served by Rust handlers.

// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

pub mod auth;
pub mod auth_routes;
pub mod backup_routes;
mod backup_sync;
pub mod bill_routes;
pub mod budget_routes;
pub mod config;
pub mod import_routes;
pub mod matching_routes;
pub mod router;
pub mod runtime;
pub mod server;
pub mod state;
pub mod statistics_routes;
pub mod taxonomy_routes;

pub use auth::{
    resolve_authenticated_user_from_headers, resolve_user_id_from_headers, AuthenticatedUser,
    RustRouteAuthError,
};
pub use auth_routes::{auth_token_runtime_router, AUTH_TOKEN_ROUTE_PATTERNS};
pub use backup_routes::{backup_ops_runtime_router, BACKUP_OPS_ROUTE_PATTERNS};
pub use bill_routes::{bill_runtime_router, BILL_CRUD_ROUTE_PATTERNS};
pub use budget_routes::{budget_runtime_router, BUDGET_CRUD_ROUTE_PATTERNS};
pub use config::{HttpShellConfig, HttpShellConfigError, ImportRouteMode};
pub use import_routes::IMPORT_SKELETON_ROUTE_PATTERNS;
pub use matching_routes::{
    matching_recurring_calendar_networth_runtime_router,
    MATCHING_RECURRING_CALENDAR_NETWORTH_ROUTE_PATTERNS,
};
pub use router::{build_router, health_handler, metadata_handler};
pub use runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity};
pub use server::{bind_addr_from_env, bind_addr_from_env_with, run_http_server, DEFAULT_HTTP_BIND};
pub use state::HttpAppState;
pub use statistics_routes::{statistics_runtime_router, STATISTICS_ROUTE_PATTERNS};
pub use taxonomy_routes::{
    taxonomy_runtime_router, TAXONOMY_ACCOUNT_ROUTE_PATTERNS, TAXONOMY_CATEGORY_ROUTE_PATTERNS,
    TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS, TAXONOMY_RULE_CENTER_ROUTE_PATTERNS,
    TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS, TAXONOMY_TAG_ROUTE_PATTERNS,
    TAXONOMY_TEMPLATE_ROUTE_PATTERNS,
};
