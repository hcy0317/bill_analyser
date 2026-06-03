// 中文导读：PostgreSQL backup metadata DTO。实际仓储读写在 `backup_postgres`。
// 维护重点：文件备份 I/O 留在 HTTP 层；这里不保留 non-Postgres backup_ops schema 或查询。

use bill_analyser_core::ops::BackupJobContract;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupJobDraft {
    pub id: Option<i64>,
    pub job_type: String,
    pub schedule_expr: String,
    pub retention_days: i64,
    pub retention_count: usize,
    pub enabled: bool,
    pub last_status: Option<String>,
}

impl From<BackupJobContract> for BackupJobDraft {
    fn from(value: BackupJobContract) -> Self {
        Self {
            id: value.id,
            job_type: value.job_type,
            schedule_expr: value.schedule_expr,
            retention_days: value.retention_days,
            retention_count: value.retention_count,
            enabled: value.enabled,
            last_status: value.last_status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupJobRow {
    pub id: i64,
    pub job_type: String,
    pub schedule_expr: String,
    pub retention_days: i64,
    pub retention_count: i64,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct BackupAuditLogDraft {
    pub operation_type: String,
    pub details: Value,
    pub affected_count: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRecordDraft {
    pub backup_name: String,
    pub file_path: String,
    pub checksum: String,
    pub encrypted: bool,
    pub status: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupRecordRow {
    pub id: i64,
    pub backup_name: String,
    pub storage_type: String,
    pub file_path: String,
    pub checksum: Option<String>,
    pub encrypted: bool,
    pub status: String,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}
