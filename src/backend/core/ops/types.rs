use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BACKUP_ZIP_SUFFIX: &str = ".zip";
pub const BACKUP_ENCRYPTED_SUFFIX: &str = ".zip.enc";
pub const BACKUP_PREFIX: &str = "backup_";
pub const BACKUP_DEFAULT_RETENTION_COUNT: usize = 10;
pub const BACKUP_DEFAULT_RETENTION_DAYS: i64 = 30;
pub const BACKUP_MAX_RETENTION_COUNT: i64 = 1_000;
pub const BACKUP_MAX_RETENTION_DAYS: i64 = 3_650;
pub const BACKUP_MAX_JOB_TYPE_LEN: usize = 64;
pub const BACKUP_MAX_SCHEDULE_EXPR_LEN: usize = 256;
pub const DEFAULT_BACKUP_SYNC_PREFIX: &str = "bill_analyser_backups/";
pub const VALID_REPORT_EXPORT_FORMATS: [&str; 3] = ["pdf", "excel", "html"];
pub const SUPPORTED_SYNC_PROVIDERS: [&str; 5] = ["oss", "s3", "cos", "azure", "webdav"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpsContractError {
    pub error: String,
    pub message: String,
    pub status_code: u16,
}

impl OpsContractError {
    #[tracing::instrument(level = "debug", skip_all)]
    /// 构造业务合同层统一错误，保持英文 error、中文/用户消息和 HTTP 状态码一起传递。
    pub fn new(error: impl Into<String>, message: impl Into<String>, status_code: u16) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            status_code,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFilenameResolution {
    pub original: String,
    pub sanitized: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupArchiveSummary {
    pub valid_zip: bool,
    pub contains_data_dir: bool,
    pub entry_count: usize,
    pub top_level_entries: Vec<String>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFileInfoContract {
    pub filename: String,
    pub size: u64,
    pub created_at: String,
    pub path: String,
    pub checksum: String,
    pub encrypted: bool,
    pub valid_zip: bool,
    pub contains_data_dir: bool,
    pub entry_count: usize,
    pub top_level_entries: Vec<String>,
    pub error: String,
    pub metadata_checksum_matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFileInfoInput {
    pub filename: String,
    pub size: u64,
    pub created_at: String,
    pub path: String,
    pub checksum: String,
    pub metadata_checksum: Option<String>,
    pub archive_summary: BackupArchiveSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRecordContract {
    pub id: i64,
    pub backup_name: String,
    pub storage_type: String,
    pub status: String,
    pub created_at: String,
    pub file_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFileCandidate {
    pub filename: String,
    pub modified_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCleanupDecision {
    pub filename: String,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCleanupPlan {
    pub deleted_count: usize,
    pub kept_count: usize,
    pub decisions: Vec<BackupCleanupDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupJobContract {
    pub id: Option<i64>,
    pub job_type: String,
    pub schedule_expr: String,
    pub retention_days: i64,
    pub retention_count: usize,
    pub enabled: bool,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveAuthMode {
    CurrentPassword,
    StepUpToken,
}

impl SensitiveAuthMode {
    pub const fn as_audit_value(self) -> &'static str {
        match self {
            Self::CurrentPassword => "current_password",
            Self::StepUpToken => "step_up_token",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserDataClearKind {
    Transactions,
    All,
}

impl UserDataClearKind {
    pub const fn operation_type(self) -> &'static str {
        match self {
            Self::Transactions => "clear_transactions",
            Self::All => "clear_all_user_data",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDataAuditContract {
    pub operation_type: String,
    pub operation_target: String,
    pub target_id: i64,
    pub details: Value,
    pub affected_count: i64,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDataStatisticsContract {
    #[serde(rename = "billCount")]
    pub bill_count: i64,
    #[serde(rename = "accountCount")]
    pub account_count: i64,
    #[serde(rename = "categoryCount")]
    pub category_count: i64,
    #[serde(rename = "tagCount")]
    pub tag_count: i64,
    #[serde(rename = "templateCount")]
    pub template_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConfigContract {
    pub provider: String,
    pub supported: bool,
    pub endpoint: String,
    pub bucket: String,
    pub prefix: String,
    pub object_key: String,
    pub safe_config: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportExportContract {
    pub format_type: String,
    pub extension: String,
    pub mimetype: String,
}
