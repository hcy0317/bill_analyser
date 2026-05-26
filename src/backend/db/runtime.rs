// 中文导读：数据库仓储运行边界，负责描述 SQLite legacy 与 PostgreSQL repository runtime 的选择。
// 维护重点：HTTP/业务层通过 provider 选择仓储运行时；不要把 Postgres 切换逻辑散落到 handler。
// 不变式：PostgreSQL runtime 可以 lazy 构造，但未迁移的 SQLite 仓储请求不能静默回退。

use std::{fmt, time::Duration};

use sqlx::postgres::PgPoolOptions;

use crate::{DbError, DbResult, PostgresPool, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseRuntimeBackend {
    Sqlite,
    Postgres,
}

impl DatabaseRuntimeBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
        }
    }
}

#[derive(Clone)]
pub struct DatabaseRuntimeConfig {
    pub backend: DatabaseRuntimeBackend,
    pub sqlite_path: Option<SqliteDbPath>,
    pub postgres_url: Option<String>,
    pub busy_timeout: Duration,
    pub postgres_max_connections: u32,
}

impl DatabaseRuntimeConfig {
    pub fn sqlite(sqlite_path: SqliteDbPath, busy_timeout: Duration) -> Self {
        Self {
            backend: DatabaseRuntimeBackend::Sqlite,
            sqlite_path: Some(sqlite_path),
            postgres_url: None,
            busy_timeout,
            postgres_max_connections: 5,
        }
    }

    pub fn postgres(postgres_url: impl Into<String>, busy_timeout: Duration) -> Self {
        Self {
            backend: DatabaseRuntimeBackend::Postgres,
            sqlite_path: None,
            postgres_url: Some(postgres_url.into()),
            busy_timeout,
            postgres_max_connections: 5,
        }
    }

    pub fn with_postgres_max_connections(mut self, postgres_max_connections: u32) -> Self {
        self.postgres_max_connections = postgres_max_connections.max(1);
        self
    }
}

impl fmt::Debug for DatabaseRuntimeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseRuntimeConfig")
            .field("backend", &self.backend)
            .field("sqlite_path_configured", &self.sqlite_path.is_some())
            .field("postgres_url_configured", &self.postgres_url.is_some())
            .field("busy_timeout", &self.busy_timeout)
            .field("postgres_max_connections", &self.postgres_max_connections)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteRuntimeOpenMode {
    CreateIfMissing,
    ExistingOnly,
}

impl SqliteRuntimeOpenMode {
    const fn create_if_missing(self) -> bool {
        matches!(self, Self::CreateIfMissing)
    }
}

#[derive(Clone)]
pub struct PostgresRepositoryRuntime {
    pool: PostgresPool,
}

impl PostgresRepositoryRuntime {
    pub fn lazy(postgres_url: &str, max_connections: u32) -> DbResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections.max(1))
            .connect_lazy(postgres_url)?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PostgresPool {
        &self.pool
    }
}

impl fmt::Debug for PostgresRepositoryRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresRepositoryRuntime")
            .field("pool_configured", &true)
            .finish()
    }
}

#[derive(Clone)]
pub struct DatabaseRuntimeProvider {
    config: DatabaseRuntimeConfig,
}

impl DatabaseRuntimeProvider {
    pub fn new(config: DatabaseRuntimeConfig) -> DbResult<Self> {
        if config.backend == DatabaseRuntimeBackend::Postgres && config.postgres_url.is_none() {
            return Err(DbError::InvalidOperation(
                "PostgreSQL repository runtime requires a Postgres URL".to_string(),
            ));
        }

        Ok(Self { config })
    }

    pub fn backend(&self) -> DatabaseRuntimeBackend {
        self.config.backend
    }

    pub fn open_sqlite_runtime(&self, mode: SqliteRuntimeOpenMode) -> DbResult<SqliteRuntime> {
        if self.config.backend != DatabaseRuntimeBackend::Sqlite {
            return Err(DbError::InvalidOperation(
                "PostgreSQL repository runtime is selected; SQLite repository requests must not silently fall back".to_string(),
            ));
        }

        let sqlite_path = self.config.sqlite_path.clone().ok_or_else(|| {
            DbError::InvalidOperation(
                "SQLite repository runtime requires a configured SQLite path".to_string(),
            )
        })?;
        SqliteRuntime::open(SqliteConnectionConfig {
            path: sqlite_path,
            create_if_missing: mode.create_if_missing(),
            busy_timeout: self.config.busy_timeout,
        })
    }

    pub fn postgres_runtime(&self) -> DbResult<PostgresRepositoryRuntime> {
        if self.config.backend != DatabaseRuntimeBackend::Postgres {
            return Err(DbError::InvalidOperation(
                "PostgreSQL repository runtime is not configured for the selected backend"
                    .to_string(),
            ));
        }
        let postgres_url = self
            .config
            .postgres_url
            .as_deref()
            .expect("PostgreSQL provider construction validates postgres_url");
        PostgresRepositoryRuntime::lazy(postgres_url, self.config.postgres_max_connections)
    }
}

impl fmt::Debug for DatabaseRuntimeProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseRuntimeProvider")
            .field("backend", &self.config.backend)
            .field(
                "postgres_runtime_configured",
                &(self.config.backend == DatabaseRuntimeBackend::Postgres
                    && self.config.postgres_url.is_some()),
            )
            .finish()
    }
}
