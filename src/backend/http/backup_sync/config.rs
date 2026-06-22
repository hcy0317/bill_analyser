use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::SyncConfigContract;
use serde_json::Value;

use super::endpoint::endpoint_url;
use super::types::CloudBackupUploadError;

/// 校验云备份上传配置，固定 provider 必填项、endpoint 安全策略和 Azure secret base64 合同。
pub fn validate_sync_upload_config(
    config: &Value,
    contract: &SyncConfigContract,
) -> Result<(), CloudBackupUploadError> {
    if contract.provider.trim().is_empty() {
        return Err(CloudBackupUploadError::config("provider is required"));
    }
    if !contract.supported {
        return Err(CloudBackupUploadError::config(format!(
            "Unsupported backup sync provider: {}",
            contract.provider
        )));
    }
    let endpoint = endpoint_url(config, &contract.provider)?;
    match contract.provider.as_str() {
        "webdav" => {
            let _ = endpoint;
        }
        "oss" | "s3" | "cos" => {
            require_non_empty(config, "bucket")?;
            require_non_empty(config, "access_key")?;
            require_non_empty(config, "secret_key")?;
        }
        "azure" => {
            require_non_empty(config, "bucket")?;
            require_non_empty(config, "access_key")?;
            let secret_key = require_non_empty(config, "secret_key")?;
            general_purpose::STANDARD
                .decode(secret_key)
                .map_err(|_| CloudBackupUploadError::config("azure secret_key must be base64"))?;
        }
        _ => {
            return Err(CloudBackupUploadError::config(format!(
                "Unsupported backup sync provider: {}",
                contract.provider
            )));
        }
    }
    Ok(())
}

/// 读取 provider 配置中的标量字符串，兼容旧配置里 number/bool 被序列化成标量的情况。
pub(super) fn string_config(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(value_to_string)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

/// 读取非空 provider 配置字段，空白值按缺失处理，供可选配置如 S3 region 使用。
pub(super) fn non_empty_config(config: &Value, key: &str) -> Option<String> {
    let value = string_config(config, key);
    (!value.is_empty()).then_some(value)
}

/// 读取 provider 必填字符串字段，统一把缺失、空白和非字符串值投影为配置错误。
pub(super) fn require_non_empty<'a>(
    config: &'a Value,
    key: &str,
) -> Result<&'a str, CloudBackupUploadError> {
    config
        .get(key)
        .and_then(value_to_string_ref)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CloudBackupUploadError::config(format!("{key} is required")))
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_string_ref(value: &Value) -> Option<&str> {
    match value {
        Value::String(text) => Some(text.as_str()),
        _ => None,
    }
}
