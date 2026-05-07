use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::HttpShellConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyFallback {
    Python,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpShellIdentity {
    pub crate_name: String,
    pub version: String,
    pub runtime_boundary: String,
    pub business_migration: String,
    pub api_takeover: bool,
    pub proxy_fallback: ProxyFallback,
}

impl HttpShellIdentity {
    pub fn current() -> Self {
        Self {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_boundary: "rust-http-shell:proxy-only".to_string(),
            business_migration: "none".to_string(),
            api_takeover: false,
            proxy_fallback: ProxyFallback::Python,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpShellHealth {
    pub status: String,
    pub identity: HttpShellIdentity,
    pub details: BTreeMap<String, String>,
}

pub fn http_shell_health(config: &HttpShellConfig) -> HttpShellHealth {
    let mut details = BTreeMap::new();
    details.insert(
        "owned_routes".to_string(),
        "/api/health,/api/runtime".to_string(),
    );
    details.insert("proxied_routes".to_string(), "unowned /api/*".to_string());
    details.insert(
        "python_upstream".to_string(),
        config.python_upstream.clone(),
    );
    details.insert("business_api".to_string(), "not-migrated".to_string());

    HttpShellHealth {
        status: "ok".to_string(),
        identity: HttpShellIdentity::current(),
        details,
    }
}
