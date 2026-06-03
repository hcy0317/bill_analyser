// 中文导读：HTTP 数据库运行边界，负责把路由层的 PostgreSQL 仓储运行时选择收拢到统一入口。
// 维护重点：业务路由只通过 HttpAppState 打开当前仓储 runtime，不直接读取 Postgres 配置细节。
// 不变式：HTTP 业务仓储只允许 PostgreSQL，不存在 non-Postgres 回退。

use thiserror::Error;

use crate::config::HttpShellConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRepositoryBackend {
    PostgresConfigurationMissing,
    PostgresAuthority,
}

impl RouteRepositoryBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PostgresConfigurationMissing => "postgres_configuration_missing",
            Self::PostgresAuthority => "postgres_authority",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseRuntimeBoundary {
    pub route_repository_backend: RouteRepositoryBackend,
    pub postgres_url_configured: bool,
}

impl DatabaseRuntimeBoundary {
    pub fn from_config(config: &HttpShellConfig) -> Self {
        let route_repository_backend = if config.postgres_configured() {
            RouteRepositoryBackend::PostgresAuthority
        } else {
            RouteRepositoryBackend::PostgresConfigurationMissing
        };
        Self {
            route_repository_backend,
            postgres_url_configured: config.postgres_configured(),
        }
    }

    pub const fn route_repository_backend_str(&self) -> &'static str {
        self.route_repository_backend.as_str()
    }

    pub fn postgres_authority_status(config: &HttpShellConfig) -> &'static str {
        if !config.postgres_configured() {
            return "blocked:postgres_url_unconfigured";
        }
        "complete:postgres_authority"
    }

    pub fn postgres_authority_is_healthy(config: &HttpShellConfig) -> bool {
        !Self::postgres_authority_status(config).starts_with("blocked:")
    }

    pub fn route_repository_runtime_is_healthy(config: &HttpShellConfig) -> bool {
        matches!(
            Self::from_config(config).route_repository_backend,
            RouteRepositoryBackend::PostgresAuthority
        )
    }
}

#[derive(Debug, Error)]
pub enum RouteRepositoryRuntimeError {
    #[error("Rust {runtime_label} PostgreSQL repository runtime open failed: {reason}")]
    PostgresOpen {
        runtime_label: &'static str,
        reason: String,
    },
}

impl RouteRepositoryRuntimeError {
    pub const fn http_status_code(&self) -> u16 {
        match self {
            Self::PostgresOpen { .. } => 503,
        }
    }

    pub fn public_message(&self) -> String {
        self.to_string()
    }
}
