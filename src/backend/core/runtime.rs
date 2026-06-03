// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatus {
    Ok,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeIdentity {
    pub crate_name: String,
    pub version: String,
    pub runtime_boundary: String,
}

impl RuntimeIdentity {
    pub fn current() -> Self {
        Self {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_boundary: "rust-core:postgres-authority+weaviate-required".to_string(),
        }
    }
}

impl Default for RuntimeIdentity {
    fn default() -> Self {
        Self::current()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHealth {
    pub status: RuntimeStatus,
    pub identity: RuntimeIdentity,
    pub details: BTreeMap<String, String>,
}

pub fn runtime_health() -> RuntimeHealth {
    let mut details = BTreeMap::new();
    details.insert("http_runtime".to_string(), "rust".to_string());
    details.insert("database".to_string(), "postgres".to_string());
    details.insert("vector_index".to_string(), "weaviate".to_string());

    RuntimeHealth {
        status: RuntimeStatus::Ok,
        identity: RuntimeIdentity::current(),
        details,
    }
}

pub fn runtime_identity_json() -> Result<String, RuntimeError> {
    serde_json::to_string(&RuntimeIdentity::current()).map_err(RuntimeError::from)
}
