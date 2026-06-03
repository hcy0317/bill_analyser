// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use bill_analyser_http::init_runtime_tracing;
use bill_analyser_http::{bind_addr_from_env, run_http_server, HttpAppState, HttpShellConfig};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_runtime_tracing();
    let config = HttpShellConfig::from_env()?;
    let bind_addr = bind_addr_from_env()?;
    let state = HttpAppState::new(config)?;
    let listener = TcpListener::bind(bind_addr).await?;

    tracing::info!(
        domain = "runtime",
        operation = "http_server_start",
        bind_addr = %bind_addr,
        "Bill Analyser Rust HTTP listening with Rust-only backend runtime"
    );
    run_http_server(listener, state).await?;
    Ok(())
}
