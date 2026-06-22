use serde_json::Value;

use super::backup_filename::resolve_backup_filename;
use super::helpers::{redact_ops_secrets_in_map, string_field};
use super::types::{
    OpsContractError, SyncConfigContract, DEFAULT_BACKUP_SYNC_PREFIX, SUPPORTED_SYNC_PROVIDERS,
};

#[tracing::instrument(level = "debug", skip_all)]
/// 规范化云备份 provider 名称，并只接受当前上传实现明确支持的 provider。
pub fn normalize_sync_provider(provider: &str) -> Option<String> {
    let normalized = provider.trim().to_lowercase();
    if SUPPORTED_SYNC_PROVIDERS.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将用户配置的对象前缀规范为安全相对目录，拒绝盘符、反斜杠、控制字符和路径穿越。
pub fn normalize_backup_sync_prefix(prefix: Option<&str>) -> Result<String, OpsContractError> {
    let raw_prefix = prefix.map(str::trim).unwrap_or_default();
    if raw_prefix.is_empty() {
        return Ok(DEFAULT_BACKUP_SYNC_PREFIX.to_string());
    }

    let bytes = raw_prefix.as_bytes();
    let has_windows_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    if raw_prefix.starts_with('/')
        || has_windows_drive
        || raw_prefix.contains('\\')
        || raw_prefix.chars().any(char::is_control)
    {
        return Err(OpsContractError::new(
            "Invalid backup sync prefix",
            "backup sync prefix must be a safe relative object prefix",
            400,
        ));
    }

    let normalized = raw_prefix.trim_matches('/');
    if normalized.is_empty() {
        return Ok(DEFAULT_BACKUP_SYNC_PREFIX.to_string());
    }

    if normalized
        .split('/')
        .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(OpsContractError::new(
            "Invalid backup sync prefix",
            "backup sync prefix must be a safe relative object prefix",
            400,
        ));
    }

    Ok(format!("{normalized}/"))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 基于安全前缀和备份文件名构造云端对象 key，保证最终 key 不会逃逸备份命名空间。
pub fn build_cloud_backup_object_key(
    prefix: Option<&str>,
    filename: &str,
) -> Result<String, OpsContractError> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "build_cloud_backup_object_key",
        "business operation entered"
    );
    let safe_filename = resolve_cloud_backup_object_filename(filename)?;
    Ok(format!(
        "{}{}",
        normalize_backup_sync_prefix(prefix)?,
        safe_filename
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 复用本地备份文件名校验规则校验云端对象叶子名，避免把任意文件名拼入对象 key。
fn resolve_cloud_backup_object_filename(filename: &str) -> Result<String, OpsContractError> {
    let trimmed = filename.trim();
    let resolution = resolve_backup_filename(trimmed)?;
    if resolution.sanitized != trimmed {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }
    Ok(resolution.sanitized)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 构造云同步配置合同，输出 provider 支持状态、对象 key 和已脱敏的安全配置快照。
pub fn build_sync_config_contract(
    config: &Value,
    filename: &str,
) -> Result<SyncConfigContract, OpsContractError> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "build_sync_config_contract",
        "business operation entered"
    );
    let object = config.as_object();
    let provider = string_field(object, "provider").trim().to_lowercase();
    let normalized_provider = normalize_sync_provider(&provider);
    let prefix = string_field(object, "prefix");
    let prefix = normalize_backup_sync_prefix(Some(&prefix))?;
    let mut safe_config = object.cloned().unwrap_or_default();
    redact_ops_secrets_in_map(&mut safe_config);

    Ok(SyncConfigContract {
        provider,
        supported: normalized_provider.is_some(),
        endpoint: string_field(object, "endpoint"),
        bucket: string_field(object, "bucket"),
        object_key: build_cloud_backup_object_key(Some(&prefix), filename)?,
        prefix,
        safe_config: Value::Object(safe_config),
    })
}
