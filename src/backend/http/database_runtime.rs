// 中文导读：HTTP 数据库运行边界，负责把路由层的仓储运行时选择收拢到统一入口。
// 维护重点：业务路由只通过 HttpAppState 打开当前仓储 runtime，不直接读取 SQLite/Postgres 配置细节。
// 不变式：PostgreSQL 仓储未接管的域必须显式失败，不能静默回退到 SQLite。

use bill_analyser_db::{DbError, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime};
use thiserror::Error;

use crate::config::{DatabaseBackend, HttpShellConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRepositoryBackend {
    SqliteLegacy,
    PostgresPending,
}

impl RouteRepositoryBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SqliteLegacy => "sqlite_legacy",
            Self::PostgresPending => "postgres_pending_repositories",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseRuntimeBoundary {
    pub route_repository_backend: RouteRepositoryBackend,
    pub sqlite_path_configured: bool,
    pub postgres_url_configured: bool,
}

impl DatabaseRuntimeBoundary {
    pub fn from_config(config: &HttpShellConfig) -> Self {
        let route_repository_backend = match config.database_backend {
            DatabaseBackend::Sqlite => RouteRepositoryBackend::SqliteLegacy,
            DatabaseBackend::Postgres => RouteRepositoryBackend::PostgresPending,
        };
        Self {
            route_repository_backend,
            sqlite_path_configured: config.sqlite_db_path.is_some(),
            postgres_url_configured: config.postgres_configured(),
        }
    }

    pub const fn route_repository_backend_str(&self) -> &'static str {
        self.route_repository_backend.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteRepositoryOpenMode {
    CreateIfMissing,
    ExistingOnly,
}

impl SqliteRepositoryOpenMode {
    const fn create_if_missing(self) -> bool {
        matches!(self, Self::CreateIfMissing)
    }
}

#[derive(Debug, Error)]
pub enum RouteRepositoryRuntimeError {
    #[error("Rust {runtime_label} DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH")]
    MissingSqliteDbPath { runtime_label: &'static str },
    #[error(
        "Rust {runtime_label} repository is not wired for PostgreSQL yet; keep BILL_ANALYSER_DATABASE_BACKEND=sqlite until this repository slice is complete"
    )]
    PostgresRepositoryPending { runtime_label: &'static str },
    #[error("{source}")]
    UnsafeSqlitePath { source: DbError },
    #[error("SQLite repository runtime open failed: {source}")]
    SqliteOpen { source: DbError },
}

impl RouteRepositoryRuntimeError {
    pub const fn http_status_code(&self) -> u16 {
        match self {
            Self::MissingSqliteDbPath { .. }
            | Self::PostgresRepositoryPending { .. }
            | Self::UnsafeSqlitePath { .. } => 503,
            Self::SqliteOpen { .. } => 500,
        }
    }

    pub fn public_message(&self, sqlite_open_message: &'static str) -> String {
        match self {
            Self::SqliteOpen { .. } => sqlite_open_message.to_string(),
            Self::MissingSqliteDbPath { .. }
            | Self::PostgresRepositoryPending { .. }
            | Self::UnsafeSqlitePath { .. } => self.to_string(),
        }
    }
}

pub fn open_sqlite_repository_runtime(
    config: &HttpShellConfig,
    runtime_label: &'static str,
    mode: SqliteRepositoryOpenMode,
) -> Result<SqliteRuntime, RouteRepositoryRuntimeError> {
    if config.database_backend.uses_postgres() {
        return Err(RouteRepositoryRuntimeError::PostgresRepositoryPending { runtime_label });
    }

    let db_path = config
        .sqlite_db_path
        .as_deref()
        .ok_or(RouteRepositoryRuntimeError::MissingSqliteDbPath { runtime_label })?;
    let db_path = SqliteDbPath::application_file(db_path)
        .map_err(|source| RouteRepositoryRuntimeError::UnsafeSqlitePath { source })?;

    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: mode.create_if_missing(),
        busy_timeout: config.timeout,
    })
    .map_err(|source| RouteRepositoryRuntimeError::SqliteOpen { source })
}
