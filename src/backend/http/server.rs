// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
};

use axum::serve;
use thiserror::Error;
use tokio::net::TcpListener;

use crate::{build_router, HttpAppState};

pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:5000";

#[tracing::instrument(level = "debug", skip_all)]
pub fn bind_addr_from_env() -> Result<SocketAddr, HttpServerConfigError> {
    bind_addr_from_env_with(|name| env::var(name).ok())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn bind_addr_from_env_with(
    mut lookup: impl FnMut(&'static str) -> Option<String>,
) -> Result<SocketAddr, HttpServerConfigError> {
    let raw = lookup("BILL_ANALYSER_HTTP_BIND").unwrap_or_else(|| DEFAULT_HTTP_BIND.to_string());
    resolve_bind_address(&raw)
}

fn resolve_bind_address(raw: &str) -> Result<SocketAddr, HttpServerConfigError> {
    let raw = raw.trim();
    if let Ok(address) = raw.parse::<SocketAddr>() {
        return (address.port() > 0)
            .then_some(address)
            .ok_or(HttpServerConfigError::InvalidBindAddress);
    }

    let (host, port) = raw
        .rsplit_once(':')
        .ok_or(HttpServerConfigError::InvalidBindAddress)?;
    if !host.trim().eq_ignore_ascii_case("localhost") {
        return Err(HttpServerConfigError::InvalidBindAddress);
    }
    let port = port
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or(HttpServerConfigError::InvalidBindAddress)?;
    ("localhost", port)
        .to_socket_addrs()
        .map_err(|_| HttpServerConfigError::InvalidBindAddress)?
        .find(|address| address.ip().is_loopback())
        .ok_or(HttpServerConfigError::InvalidBindAddress)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn run_http_server(
    listener: TcpListener,
    state: HttpAppState,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_parser_supports_default_custom_ipv4_and_localhost() {
        assert_eq!(
            bind_addr_from_env_with(|_| None).expect("default bind"),
            "127.0.0.1:5000".parse().unwrap()
        );
        assert_eq!(
            bind_addr_from_env_with(|_| Some("127.0.0.1:5123".to_string()))
                .expect("custom IPv4 bind"),
            "127.0.0.1:5123".parse().unwrap()
        );
        let localhost = bind_addr_from_env_with(|_| Some("localhost:5124".to_string()))
            .expect("localhost is an allowed bind hostname");
        assert!(localhost.ip().is_loopback());
        assert_eq!(localhost.port(), 5124);
    }

    #[test]
    fn bind_parser_rejects_unresolved_or_missing_ports() {
        for value in ["example.invalid:5123", "127.0.0.1", "localhost:0"] {
            assert_eq!(
                bind_addr_from_env_with(|_| Some(value.to_string())),
                Err(HttpServerConfigError::InvalidBindAddress),
                "{value}"
            );
        }
    }
}
