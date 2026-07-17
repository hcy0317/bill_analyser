// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use bill_analyser_db::run_postgres_migrations;
use bill_analyser_http::init_runtime_tracing;
use bill_analyser_http::{bind_addr_from_env, run_http_server, HttpAppState, HttpShellConfig};
use tokio::net::TcpListener;

async fn run_startup_postgres_migrations_if_configured(
    state: &HttpAppState,
    postgres_configured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !postgres_configured {
        return Ok(());
    }

    let runtime = state.open_postgres_repository_runtime("startup migrations")?;
    run_postgres_migrations(runtime.pool()).await?;
    record_startup_postgres_migrations_complete();
    Ok(())
}

fn record_startup_postgres_migrations_complete() {
    tracing::info!(
        domain = "runtime",
        operation = "postgres_migrations_complete",
        "PostgreSQL migrations completed before HTTP listen"
    );
}

async fn prepare_http_state(
    config: HttpShellConfig,
) -> Result<HttpAppState, Box<dyn std::error::Error>> {
    let postgres_configured = config.postgres_configured();
    let state = HttpAppState::new(config)?;
    run_startup_postgres_migrations_if_configured(&state, postgres_configured).await?;
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_runtime_tracing();
    let bind_addr = bind_addr_from_env()?;
    let config = HttpShellConfig::from_env()?;
    let state = prepare_http_state(config).await?;
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

#[cfg(test)]
mod tests {
    use std::{env, ffi::OsString, sync::Mutex, time::Duration};

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn base_test_config() -> HttpShellConfig {
        HttpShellConfig::new("", Duration::from_millis(100), 1024).expect("test config")
    }

    fn restore_env_var(name: &str, previous: Option<OsString>) {
        if let Some(value) = previous {
            env::set_var(name, value);
        } else {
            env::remove_var(name);
        }
    }

    fn postgres_timeout_state() -> HttpAppState {
        let config = base_test_config()
            .with_postgres_url("postgres://bill_analyser:invalid@127.0.0.1:1/bill_analyser")
            .expect("test postgres url");
        HttpAppState::new(config).expect("test state")
    }

    #[tokio::test]
    async fn startup_migrations_skip_when_postgres_is_not_configured() {
        run_startup_postgres_migrations_if_configured(&postgres_timeout_state(), false)
            .await
            .expect("disabled startup migrations do not touch PostgreSQL");
    }

    #[tokio::test]
    async fn startup_migrations_open_runtime_when_postgres_is_configured() {
        if let Ok(Ok(())) = tokio::time::timeout(
            Duration::from_millis(50),
            run_startup_postgres_migrations_if_configured(&postgres_timeout_state(), true),
        )
        .await
        {
            panic!(
                "test PostgreSQL URL should not complete migrations; it only proves runtime entry"
            );
        }
    }

    #[tokio::test]
    async fn prepare_http_state_skips_startup_migrations_without_postgres_url() {
        let mut config = base_test_config();
        config.postgres_url = None;
        let state = prepare_http_state(config)
            .await
            .expect("state prepares when PostgreSQL is absent");

        assert!(!state.config.postgres_configured());
    }

    #[test]
    fn startup_migration_completion_log_is_safe_to_record() {
        record_startup_postgres_migrations_complete();
    }

    #[test]
    fn restore_env_var_restores_existing_value() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let name = "BILL_ANALYSER_TEST_RESTORE_ENV";
        env::set_var(name, "original");
        restore_env_var(name, Some(OsString::from("restored")));
        assert_eq!(env::var(name).as_deref(), Ok("restored"));
        env::remove_var(name);
    }

    #[test]
    fn main_rejects_invalid_bind_before_state_or_database_startup() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_bind = env::var_os("BILL_ANALYSER_HTTP_BIND");
        env::set_var("BILL_ANALYSER_HTTP_BIND", "not-a-socket-address");

        let error = main().expect_err("invalid bind exits before state/database startup");

        restore_env_var("BILL_ANALYSER_HTTP_BIND", previous_bind);
        assert!(error.to_string().contains("invalid Rust HTTP bind address"));
    }
}
