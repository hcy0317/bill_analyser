// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

mod backup_archive;
mod backup_cleanup;
mod backup_crypto;
mod backup_filename;
mod backup_jobs;
mod backup_sync;
mod helpers;
mod report_export;
mod types;
mod user_data;

pub use backup_archive::{
    backup_archive_summary_from_entries, build_backup_file_info, invalid_backup_archive_summary,
    is_safe_backup_archive_member,
};
pub use backup_cleanup::plan_backup_cleanup;
pub use backup_crypto::{backup_encryption_secret_configured, derive_backup_fernet_key};
pub use backup_filename::{
    is_allowed_backup_filename, resolve_backup_filename, secure_backup_filename,
};
pub use backup_jobs::normalize_backup_job_payload;
pub use backup_sync::{
    build_cloud_backup_object_key, build_sync_config_contract, normalize_backup_sync_prefix,
    normalize_sync_provider,
};
pub use report_export::{normalize_report_export_format, secure_report_filename};
pub use types::{
    BackupArchiveSummary, BackupCleanupDecision, BackupCleanupPlan, BackupFileCandidate,
    BackupFileInfoContract, BackupFileInfoInput, BackupFilenameResolution, BackupJobContract,
    BackupRecordContract, OpsContractError, ReportExportContract, SensitiveAuthMode,
    SyncConfigContract, UserDataAuditContract, UserDataClearKind, UserDataStatisticsContract,
    BACKUP_DEFAULT_RETENTION_COUNT, BACKUP_DEFAULT_RETENTION_DAYS, BACKUP_ENCRYPTED_SUFFIX,
    BACKUP_PREFIX, BACKUP_ZIP_SUFFIX, DEFAULT_BACKUP_SYNC_PREFIX, SUPPORTED_SYNC_PROVIDERS,
    VALID_REPORT_EXPORT_FORMATS,
};
pub use user_data::{
    build_user_data_audit_contract, normalize_user_data_statistics, parse_comma_separated_ints,
    parse_export_timestamp_millis, resolve_sensitive_auth_mode, user_data_statistics_response,
};
