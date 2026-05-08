use std::{env, net::SocketAddr};

use axum::serve;
use thiserror::Error;
use tokio::net::TcpListener;

use crate::{build_router, ProxyState};

pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:5000";

pub fn bind_addr_from_env() -> Result<SocketAddr, HttpServerConfigError> {
    bind_addr_from_env_with(|name| env::var(name).ok())
}

pub fn bind_addr_from_env_with(
    mut lookup: impl FnMut(&'static str) -> Option<String>,
) -> Result<SocketAddr, HttpServerConfigError> {
    let raw = lookup("BILL_ANALYSER_HTTP_BIND").unwrap_or_else(|| DEFAULT_HTTP_BIND.to_string());
    raw.trim()
        .parse::<SocketAddr>()
        .map_err(|_| HttpServerConfigError::InvalidBindAddress)
}

pub async fn run_http_server(
    listener: TcpListener,
    state: ProxyState,
) -> Result<(), std::io::Error> {
    serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HttpServerConfigError {
    #[error("invalid Rust HTTP bind address")]
    InvalidBindAddress,
}
