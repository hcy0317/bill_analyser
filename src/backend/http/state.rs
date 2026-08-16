// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde_json::Value;

use crate::{
    config::HttpShellConfig,
    database_runtime::{DatabaseRuntimeBoundary, RouteRepositoryRuntimeError},
};
use bill_analyser_db::{DbError, PostgresLedgerQueries, PostgresRepositoryRuntime};

#[derive(Debug, Clone)]
pub struct HttpAppState {
    pub config: HttpShellConfig,
    llm_runtime_configs: Arc<Mutex<HashMap<i64, Value>>>,
    postgres_runtime: Arc<Mutex<Option<PostgresRepositoryRuntime>>>,
}

impl HttpAppState {
    pub fn new(config: HttpShellConfig) -> Result<Self, DbError> {
        Ok(Self {
            config,
            llm_runtime_configs: Arc::new(Mutex::new(HashMap::new())),
            postgres_runtime: Arc::new(Mutex::new(None)),
        })
    }

    pub fn get_llm_runtime_config(&self, user_id: i64) -> Option<Value> {
        self.llm_runtime_configs
            .lock()
            .ok()
            .and_then(|configs| configs.get(&user_id).cloned())
    }

    pub fn set_llm_runtime_config(&self, user_id: i64, config: Value) {
        if let Ok(mut configs) = self.llm_runtime_configs.lock() {
            configs.insert(user_id, config);
        }
    }

    pub fn clear_llm_runtime_config(&self, user_id: i64) {
        if let Ok(mut configs) = self.llm_runtime_configs.lock() {
            configs.remove(&user_id);
        }
    }

    pub fn database_runtime_boundary(&self) -> DatabaseRuntimeBoundary {
        DatabaseRuntimeBoundary::from_config(&self.config)
    }

    pub fn open_postgres_repository_runtime(
        &self,
        runtime_label: &'static str,
    ) -> Result<PostgresRepositoryRuntime, RouteRepositoryRuntimeError> {
        let postgres_url = self.config.postgres_url.as_deref().ok_or_else(|| {
            RouteRepositoryRuntimeError::PostgresOpen {
                runtime_label,
                reason: "PostgreSQL repository runtime requires BILL_ANALYSER_POSTGRES_URL"
                    .to_string(),
            }
        })?;

        let mut cached_runtime = self.postgres_runtime.lock().map_err(|_| {
            RouteRepositoryRuntimeError::PostgresOpen {
                runtime_label,
                reason: "PostgreSQL repository runtime lock poisoned".to_string(),
            }
        })?;
        if let Some(runtime) = cached_runtime.clone() {
            return Ok(runtime);
        }

        let runtime = PostgresRepositoryRuntime::lazy(postgres_url, 5).map_err(|source| {
            RouteRepositoryRuntimeError::PostgresOpen {
                runtime_label,
                reason: source.to_string(),
            }
        })?;
        *cached_runtime = Some(runtime.clone());
        Ok(runtime)
    }

    pub fn ledger_queries(&self) -> Result<PostgresLedgerQueries, RouteRepositoryRuntimeError> {
        let runtime = self.open_postgres_repository_runtime("bills")?;
        Ok(PostgresLedgerQueries::new(runtime.pool()))
    }

    pub fn invalidate_postgres_repository_runtime(&self) {
        if let Ok(mut cached_runtime) = self.postgres_runtime.lock() {
            *cached_runtime = None;
        }
    }
}
