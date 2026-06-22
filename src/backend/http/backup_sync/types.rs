use reqwest::StatusCode;

pub(super) const CONTENT_TYPE: &str = "application/octet-stream";
pub(super) const AZURE_BLOB_VERSION: &str = "2023-11-03";
pub(super) const BACKUP_SYNC_ENDPOINT_ALLOWLIST_ENV: &str =
    "BILL_ANALYSER_BACKUP_SYNC_ENDPOINT_ALLOWLIST";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudBackupUploadResult {
    pub provider: String,
    pub object_key: String,
    pub status_code: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudBackupUploadError {
    pub status_code: u16,
    pub message: String,
    pub response_status: Option<u16>,
}

impl CloudBackupUploadError {
    /// 构造用户配置错误，固定映射为 400，避免和外部 provider 失败混淆。
    pub(super) fn config(message: impl ToString) -> Self {
        Self {
            status_code: 400,
            message: message.to_string(),
            response_status: None,
        }
    }

    /// 构造上传链路错误，固定映射为 502 表示外部 provider 或本地 I/O 失败。
    pub(super) fn provider(message: impl ToString) -> Self {
        Self {
            status_code: 502,
            message: message.to_string(),
            response_status: None,
        }
    }

    /// 构造 provider 非 2xx 响应错误，同时保留外部响应状态方便审计和前端提示。
    pub(super) fn provider_status(provider: &str, status: StatusCode) -> Self {
        Self {
            status_code: 502,
            message: format!("{provider} upload failed with status {}", status.as_u16()),
            response_status: Some(status.as_u16()),
        }
    }
}
