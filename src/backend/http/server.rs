// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{env, net::SocketAddr};

use axum::serve;
use thiserror::Error;
use tokio::net::TcpListener;

use crate::{build_router, HttpAppState};

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
