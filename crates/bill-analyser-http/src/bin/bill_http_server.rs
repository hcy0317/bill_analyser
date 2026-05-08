use bill_analyser_http::{bind_addr_from_env, run_http_server, HttpShellConfig, ProxyState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = HttpShellConfig::from_env()?;
    let bind_addr = bind_addr_from_env()?;
    let state = ProxyState::new(config.clone())?;
    let listener = TcpListener::bind(bind_addr).await?;

    println!(
        "Bill Analyser Rust HTTP listening on http://{}; Python fallback {}",
        bind_addr, config.python_upstream
    );
    run_http_server(listener, state).await?;
    Ok(())
}
