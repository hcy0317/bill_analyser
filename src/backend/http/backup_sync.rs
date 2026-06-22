// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{path::Path, time::Duration};

use bill_analyser_core::SyncConfigContract;
use reqwest::Client;
use serde_json::Value;

mod azure;
mod body_hash;
mod config;
mod cos;
mod endpoint;
mod oss;
mod s3;
mod types;
mod webdav;

use azure::upload_azure;
pub use config::validate_sync_upload_config;
use cos::upload_cos;
use oss::upload_oss;
use s3::upload_s3;
pub use types::{CloudBackupUploadError, CloudBackupUploadResult};
use webdav::upload_webdav;

/// 编排云备份上传：先校验配置和 endpoint，再按 provider 调用具体签名/上传实现并统一失败投影。
pub async fn upload_backup_to_cloud(
    config: &Value,
    contract: &SyncConfigContract,
    file_path: &Path,
    timeout: Duration,
) -> Result<CloudBackupUploadResult, CloudBackupUploadError> {
    validate_sync_upload_config(config, contract)?;
    let client = Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| CloudBackupUploadError::provider(error.to_string()))?;
    let response = match contract.provider.as_str() {
        "webdav" => upload_webdav(&client, config, contract, file_path).await?,
        "oss" => upload_oss(&client, config, contract, file_path).await?,
        "s3" => upload_s3(&client, config, contract, file_path).await?,
        "cos" => upload_cos(&client, config, contract, file_path).await?,
        "azure" => upload_azure(&client, config, contract, file_path).await?,
        _ => {
            return Err(CloudBackupUploadError::config(format!(
                "Unsupported backup sync provider: {}",
                contract.provider
            )));
        }
    };
    let status = response.status();
    if !status.is_success() {
        return Err(CloudBackupUploadError::provider_status(
            &contract.provider,
            status,
        ));
    }
    Ok(CloudBackupUploadResult {
        provider: contract.provider.clone(),
        object_key: contract.object_key.clone(),
        status_code: status.as_u16(),
    })
}

#[cfg(test)]
mod tests;
