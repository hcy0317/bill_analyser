//! Opt-in Rust HTTP ingress shell for the backend migration.
//!
//! This crate is proxy-only for unmigrated business routes. It does not switch
//! the default backend startup, write the database, or claim business API
//! ownership for proxied Python responses.

pub mod config;
pub mod proxy;
pub mod router;
pub mod runtime;

pub use config::{HttpShellConfig, HttpShellConfigError};
pub use proxy::{
    build_upstream_url, filter_proxy_request_headers, filter_proxy_response_headers,
    is_hop_by_hop_header, ProxyErrorBody, ProxyState, REQUEST_ID_HEADER,
};
pub use router::{build_router, health_handler, metadata_handler};
pub use runtime::{http_shell_health, HttpShellHealth, HttpShellIdentity, ProxyFallback};
