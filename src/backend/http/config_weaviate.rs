// 中文导读：HTTP Weaviate 配置层，负责解析和脱敏必需的派生向量索引配置。
// 维护重点：运行态默认启用本地 Weaviate；endpoint/API key 只来自运行环境，不通过请求输入动态改写。
// 不变式：健康输出只暴露 endpoint 和 key 是否配置，不能泄露 API key。

use std::time::Duration;

use bill_analyser_core::{
    validate_weaviate_collection_prefix, WEAVIATE_DEFAULT_COLLECTION_PREFIX,
    WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
};
use url::Url;

use crate::config::HttpShellConfigError;

pub const DEFAULT_WEAVIATE_TIMEOUT_MS: u64 = 2_000;
pub const DEFAULT_WEAVIATE_ENDPOINT: &str = "http://127.0.0.1:8088";
pub const DEFAULT_WEAVIATE_RETRY_ATTEMPTS: usize = 2;
pub const DEFAULT_WEAVIATE_BATCH_SIZE: usize = 64;
pub const MAX_WEAVIATE_TIMEOUT_MS: u64 = 60_000;
pub const MAX_WEAVIATE_RETRY_ATTEMPTS: usize = 10;
pub const MAX_WEAVIATE_BATCH_SIZE: usize = 1_000;
pub const MAX_WEAVIATE_VECTOR_DIMENSIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaviateRuntimeConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub collection_prefix: String,
    pub timeout: Duration,
    pub retry_attempts: usize,
    pub batch_size: usize,
    pub vector_dimensions: usize,
}

impl WeaviateRuntimeConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            api_key: None,
            collection_prefix: WEAVIATE_DEFAULT_COLLECTION_PREFIX.to_string(),
            timeout: Duration::from_millis(DEFAULT_WEAVIATE_TIMEOUT_MS),
            retry_attempts: DEFAULT_WEAVIATE_RETRY_ATTEMPTS,
            batch_size: DEFAULT_WEAVIATE_BATCH_SIZE,
            vector_dimensions: WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
        }
    }

    pub fn from_env_with(
        lookup: &mut impl FnMut(&'static str) -> Option<String>,
    ) -> Result<Self, HttpShellConfigError> {
        let enabled = parse_env_bool_value(
            "BILL_ANALYSER_WEAVIATE_ENABLED",
            lookup("BILL_ANALYSER_WEAVIATE_ENABLED").filter(|value| !value.trim().is_empty()),
            true,
        )?;
        let endpoint = normalize_weaviate_endpoint(
            lookup("BILL_ANALYSER_WEAVIATE_ENDPOINT")
                .filter(|value| !value.trim().is_empty())
                .or_else(|| Some(DEFAULT_WEAVIATE_ENDPOINT.to_string())),
        )?;
        let api_key = lookup("BILL_ANALYSER_WEAVIATE_API_KEY")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let collection_prefix = lookup("BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| WEAVIATE_DEFAULT_COLLECTION_PREFIX.to_string());
        if !validate_weaviate_collection_prefix(&collection_prefix) {
            return Err(HttpShellConfigError::InvalidWeaviateCollectionPrefix);
        }
        let timeout_ms = parse_env_u64_range_value(
            "BILL_ANALYSER_WEAVIATE_TIMEOUT_MS",
            lookup("BILL_ANALYSER_WEAVIATE_TIMEOUT_MS"),
            DEFAULT_WEAVIATE_TIMEOUT_MS,
            1,
            MAX_WEAVIATE_TIMEOUT_MS,
        )?;
        let retry_attempts = parse_env_usize_range_value(
            "BILL_ANALYSER_WEAVIATE_RETRY_ATTEMPTS",
            lookup("BILL_ANALYSER_WEAVIATE_RETRY_ATTEMPTS"),
            DEFAULT_WEAVIATE_RETRY_ATTEMPTS,
            0,
            MAX_WEAVIATE_RETRY_ATTEMPTS,
        )?;
        let batch_size = parse_env_usize_range_value(
            "BILL_ANALYSER_WEAVIATE_BATCH_SIZE",
            lookup("BILL_ANALYSER_WEAVIATE_BATCH_SIZE"),
            DEFAULT_WEAVIATE_BATCH_SIZE,
            1,
            MAX_WEAVIATE_BATCH_SIZE,
        )?;
        let vector_dimensions = parse_env_usize_range_value(
            "BILL_ANALYSER_WEAVIATE_VECTOR_DIMENSIONS",
            lookup("BILL_ANALYSER_WEAVIATE_VECTOR_DIMENSIONS"),
            WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
            1,
            MAX_WEAVIATE_VECTOR_DIMENSIONS,
        )?;

        Ok(Self {
            enabled,
            endpoint,
            api_key,
            collection_prefix,
            timeout: Duration::from_millis(timeout_ms),
            retry_attempts,
            batch_size,
            vector_dimensions,
        })
    }

    pub fn api_key_configured(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn redacted_endpoint(&self) -> String {
        self.endpoint
            .as_deref()
            .map(redact_weaviate_endpoint)
            .unwrap_or_else(|| "unconfigured".to_string())
    }

    pub fn status_without_probe(&self) -> &'static str {
        if !self.enabled {
            "disabled"
        } else if self.endpoint.is_none() {
            "degraded:missing_endpoint"
        } else {
            "configured"
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_weaviate_endpoint(
    value: Option<String>,
) -> Result<Option<String>, HttpShellConfigError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = Url::parse(trimmed).map_err(|_| HttpShellConfigError::InvalidWeaviateEndpoint)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(HttpShellConfigError::InvalidWeaviateEndpoint);
    }
    Ok(Some(trimmed.to_string()))
}

pub fn redact_weaviate_endpoint(endpoint: &str) -> String {
    let Ok(mut parsed) = Url::parse(endpoint.trim()) else {
        return "<invalid-weaviate-endpoint>".to_string();
    };
    parsed.set_query(None);
    parsed.set_fragment(None);
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.to_string().trim_end_matches('/').to_string()
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_bool_value(
    name: &'static str,
    value: Option<String>,
    default_value: bool,
) -> Result<bool, HttpShellConfigError> {
    let Some(value) = value else {
        return Ok(default_value);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(HttpShellConfigError::InvalidBoolean(name)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_u64_value(
    name: &'static str,
    value: Option<String>,
    default_value: u64,
) -> Result<u64, HttpShellConfigError> {
    match value {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| HttpShellConfigError::InvalidInteger(name)),
        None => Ok(default_value),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_u64_range_value(
    name: &'static str,
    value: Option<String>,
    default_value: u64,
    min_value: u64,
    max_value: u64,
) -> Result<u64, HttpShellConfigError> {
    let parsed = parse_env_u64_value(name, value, default_value)?;
    if parsed < min_value || parsed > max_value {
        return Err(HttpShellConfigError::InvalidInteger(name));
    }
    Ok(parsed)
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_usize_value(
    name: &'static str,
    value: Option<String>,
    default_value: usize,
) -> Result<usize, HttpShellConfigError> {
    match value {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| HttpShellConfigError::InvalidInteger(name)),
        None => Ok(default_value),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_usize_range_value(
    name: &'static str,
    value: Option<String>,
    default_value: usize,
    min_value: usize,
    max_value: usize,
) -> Result<usize, HttpShellConfigError> {
    let parsed = parse_env_usize_value(name, value, default_value)?;
    if parsed < min_value || parsed > max_value {
        return Err(HttpShellConfigError::InvalidInteger(name));
    }
    Ok(parsed)
}
