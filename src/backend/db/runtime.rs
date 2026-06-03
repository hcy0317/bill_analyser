// 中文导读：PostgreSQL 仓储运行边界，负责统一构造 repository runtime。
// 维护重点：HTTP/业务层通过 provider 选择仓储运行时；不要把 Postgres 连接逻辑散落到 handler。
// 不变式：业务仓储只接受 PostgreSQL runtime，不存在 non-Postgres 回退。

use std::{fmt, time::Duration};

use sqlx::postgres::PgPoolOptions;

use crate::{DbError, DbResult, PostgresPool};

#[derive(Clone)]
pub struct DatabaseRuntimeConfig {
    pub postgres_url: Option<String>,
    pub busy_timeout: Duration,
    pub postgres_max_connections: u32,
}

impl DatabaseRuntimeConfig {
    pub fn postgres(postgres_url: impl Into<String>, busy_timeout: Duration) -> Self {
        Self {
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
            .field("postgres_url_configured", &self.postgres_url.is_some())
            .field("busy_timeout", &self.busy_timeout)
            .field("postgres_max_connections", &self.postgres_max_connections)
            .finish()
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
        if config.postgres_url.is_none() {
            return Err(DbError::InvalidOperation(
                "PostgreSQL repository runtime requires a Postgres URL".to_string(),
            ));
        }

        Ok(Self { config })
    }

    pub fn postgres_runtime(&self) -> DbResult<PostgresRepositoryRuntime> {
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
            .field(
                "postgres_runtime_configured",
                &self.config.postgres_url.is_some(),
            )
            .finish()
    }
}
