//! Opt-in Rust HTTP ingress shell for the backend migration.
//!
//! This crate is the Rust-primary HTTP ingress. Unmigrated business routes are
//! still proxied to Python, while opted-in domains can be served directly by
//! Rust runtime handlers.

pub mod auth;
pub mod auth_routes;
pub mod bill_routes;
pub mod budget_routes;
pub mod config;
pub mod import_routes;
pub mod proxy;
pub mod router;
pub mod runtime;
pub mod server;
pub mod statistics_routes;
pub mod taxonomy_routes;

pub use auth::{
    resolve_authenticated_user_from_headers, resolve_user_id_from_headers, AuthenticatedUser,
    RustRouteAuthError,
};
pub use auth_routes::{
    auth_token_runtime_router, AUTH_PROXIED_ROUTE_PATTERNS, AUTH_TOKEN_ROUTE_PATTERNS,
};
pub use bill_routes::{
    bill_runtime_router, BILL_CRUD_PROXIED_ROUTE_PATTERNS, BILL_CRUD_ROUTE_PATTERNS,
};
pub use budget_routes::{
    budget_runtime_router, BUDGET_CRUD_ROUTE_PATTERNS, BUDGET_PROXIED_ROUTE_PATTERNS,
};
pub use config::{HttpShellConfig, HttpShellConfigError, ImportRouteMode};
pub use import_routes::{
    import_skeleton_router, not_yet_owned_response, IMPORT_SKELETON_ROUTE_PATTERNS,
};
pub use proxy::{
    build_upstream_url, filter_proxy_request_headers, filter_proxy_response_headers,
    is_hop_by_hop_header, is_manifest_python_proxied_route, ownership_aware_proxy_handler,
    proxy_allowed_for_request, ProxyErrorBody, ProxyState, REQUEST_ID_HEADER,
};
pub use router::{build_router, health_handler, metadata_handler};
pub use runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity, ProxyFallback};
pub use server::{bind_addr_from_env, bind_addr_from_env_with, run_http_server, DEFAULT_HTTP_BIND};
pub use statistics_routes::{
    statistics_runtime_router, STATISTICS_PROXIED_ROUTE_PATTERNS, STATISTICS_ROUTE_PATTERNS,
};
pub use taxonomy_routes::{
    taxonomy_runtime_router, TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS,
    TAXONOMY_ACCOUNT_ROUTE_PATTERNS, TAXONOMY_CATEGORY_PROXIED_ROUTE_PATTERNS,
    TAXONOMY_CATEGORY_ROUTE_PATTERNS, TAXONOMY_TAG_PROXIED_ROUTE_PATTERNS,
    TAXONOMY_TAG_ROUTE_PATTERNS,
};
