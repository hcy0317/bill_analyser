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
    pub business_migration: String,
    pub api_takeover: bool,
}

impl RuntimeIdentity {
    pub fn current() -> Self {
        Self {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_boundary: "flask-rest-shell".to_string(),
            business_migration: "none".to_string(),
            api_takeover: false,
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
    details.insert("rest_shell".to_string(), "flask".to_string());
    details.insert("rust_boundary".to_string(), "internal-library".to_string());
    details.insert("business_api".to_string(), "not-migrated".to_string());

    RuntimeHealth {
        status: RuntimeStatus::Ok,
        identity: RuntimeIdentity::current(),
        details,
    }
}

pub fn runtime_identity_json() -> Result<String, RuntimeError> {
    serde_json::to_string(&RuntimeIdentity::current()).map_err(RuntimeError::from)
}
