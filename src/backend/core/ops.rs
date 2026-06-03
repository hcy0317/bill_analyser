// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::collections::BTreeSet;

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

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

#[tracing::instrument(level = "debug", skip_all)]
pub fn secure_backup_filename(filename: &str) -> String {
    let normalized = filename.nfkd().collect::<String>();
    let mut cleaned = String::new();
    for ch in normalized.chars() {
        if matches!(ch, '/' | '\\') || ch.is_whitespace() {
            cleaned.push(' ');
        } else if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            cleaned.push(ch);
        }
    }

    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .trim_matches(|ch| matches!(ch, '.' | '_'))
        .to_string()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_backup_filename(
    filename: &str,
) -> Result<BackupFilenameResolution, OpsContractError> {
    let raw_filename = filename.trim();
    let bytes = raw_filename.as_bytes();
    let has_windows_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    if raw_filename.is_empty()
        || raw_filename.starts_with('/')
        || has_windows_drive
        || raw_filename.contains('/')
        || raw_filename.contains('\\')
        || raw_filename.contains(':')
        || raw_filename.chars().any(char::is_control)
    {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }

    let sanitized = secure_backup_filename(raw_filename);
    if sanitized != raw_filename {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }

    if !is_allowed_backup_filename(&sanitized) {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }

    Ok(BackupFilenameResolution {
        original: filename.to_string(),
        encrypted: sanitized.ends_with(BACKUP_ENCRYPTED_SUFFIX),
        sanitized,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn is_allowed_backup_filename(filename: &str) -> bool {
    filename.starts_with(BACKUP_PREFIX)
        && (filename.ends_with(BACKUP_ZIP_SUFFIX) || filename.ends_with(BACKUP_ENCRYPTED_SUFFIX))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn is_safe_backup_archive_member(member_name: &str) -> bool {
    let normalized = member_name.replace('\\', "/").trim().to_string();
    if normalized.is_empty() || normalized.starts_with('/') {
        return false;
    }

    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return false;
    }

    !normalized.split('/').any(|part| part == "..")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn backup_archive_summary_from_entries<I, S>(entries: I) -> BackupArchiveSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let raw_names = entries
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>();
    if let Some(unsafe_name) = raw_names
        .iter()
        .find(|name| !is_safe_backup_archive_member(name))
    {
        return invalid_backup_archive_summary(&format!("备份文件包含不安全路径: {unsafe_name}"));
    }

    let names = raw_names
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let contains_data_dir = names.iter().any(|name| name.starts_with("data/"));
    let top_level_entries = names
        .iter()
        .filter_map(|name| name.split('/').next())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    BackupArchiveSummary {
        valid_zip: true,
        contains_data_dir,
        entry_count: names.len(),
        top_level_entries,
        error: String::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn invalid_backup_archive_summary(error: &str) -> BackupArchiveSummary {
    BackupArchiveSummary {
        valid_zip: false,
        contains_data_dir: false,
        entry_count: 0,
        top_level_entries: Vec::new(),
        error: error.to_string(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_backup_file_info(input: BackupFileInfoInput) -> BackupFileInfoContract {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "build_backup_file_info",
        "business operation entered"
    );
    let encrypted = input.filename.ends_with(BACKUP_ENCRYPTED_SUFFIX);
    let metadata_checksum_matched = input
        .metadata_checksum
        .as_deref()
        .is_some_and(|checksum| !checksum.is_empty() && checksum == input.checksum);

    BackupFileInfoContract {
        filename: input.filename,
        size: input.size,
        created_at: input.created_at,
        path: input.path,
        checksum: input.checksum,
        encrypted,
        valid_zip: input.archive_summary.valid_zip,
        contains_data_dir: input.archive_summary.contains_data_dir,
        entry_count: input.archive_summary.entry_count,
        top_level_entries: input.archive_summary.top_level_entries,
        error: input.archive_summary.error,
        metadata_checksum_matched,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn plan_backup_cleanup(
    records: &[BackupRecordContract],
    stray_files: &[BackupFileCandidate],
    keep_count: usize,
) -> BackupCleanupPlan {
    let mut decisions = Vec::new();
    let mut active_records = records
        .iter()
        .filter(|record| record.storage_type.trim().is_empty() || record.storage_type == "local")
        .filter(|record| record.status != "deleted")
        .collect::<Vec<_>>();

    for record in active_records
        .iter()
        .filter(|record| !record.file_exists)
        .copied()
    {
        decisions.push(BackupCleanupDecision {
            filename: record.backup_name.clone(),
            action: "mark_deleted".to_string(),
            reason: "missing_file".to_string(),
        });
    }

    active_records.retain(|record| record.file_exists);
    active_records.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id))
    });

    let kept_record_names = active_records
        .iter()
        .take(keep_count)
        .map(|record| record.backup_name.clone())
        .collect::<BTreeSet<_>>();
    let active_record_names = active_records
        .iter()
        .map(|record| record.backup_name.clone())
        .collect::<BTreeSet<_>>();

    let mut deleted_count = 0_usize;
    for record in active_records.iter().skip(keep_count) {
        deleted_count += 1;
        decisions.push(BackupCleanupDecision {
            filename: record.backup_name.clone(),
            action: "delete_file_and_mark_deleted".to_string(),
            reason: "retention_cleanup".to_string(),
        });
    }

    let mut stray = stray_files
        .iter()
        .filter(|candidate| !kept_record_names.contains(&candidate.filename))
        .filter(|candidate| !active_record_names.contains(&candidate.filename))
        .collect::<Vec<_>>();
    stray.sort_by_key(|candidate| std::cmp::Reverse(candidate.modified_at));

    let remaining_slots = keep_count.saturating_sub(kept_record_names.len());
    for candidate in stray.iter().skip(remaining_slots) {
        deleted_count += 1;
        decisions.push(BackupCleanupDecision {
            filename: candidate.filename.clone(),
            action: "delete_stray_file".to_string(),
            reason: "retention_cleanup".to_string(),
        });
    }

    let kept_count = kept_record_names.len() + stray.len().min(remaining_slots);
    BackupCleanupPlan {
        deleted_count,
        kept_count: kept_count.min(keep_count),
        decisions,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_backup_job_payload(
    payload: &Value,
) -> Result<BackupJobContract, OpsContractError> {
    let object = payload.as_object();
    let job_type = string_field(object, "job_type").trim().to_string();
    if job_type.is_empty() {
        return Err(OpsContractError::new(
            "job_type is required",
            "job_type is required",
            400,
        ));
    }
    if job_type.len() > BACKUP_MAX_JOB_TYPE_LEN
        || !job_type.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err(OpsContractError::new(
            "job_type is invalid",
            "job_type is invalid",
            400,
        ));
    }

    let retention_days = optional_i64_field_with_default(
        object,
        "retention_days",
        BACKUP_DEFAULT_RETENTION_DAYS,
        "retention_days must be an integer",
    )?;
    if !(0..=BACKUP_MAX_RETENTION_DAYS).contains(&retention_days) {
        return Err(OpsContractError::new(
            "retention_days must be between 0 and 3650",
            "retention_days must be between 0 and 3650",
            400,
        ));
    }
    let retention_count_i64 = optional_i64_field_with_default(
        object,
        "retention_count",
        BACKUP_DEFAULT_RETENTION_COUNT as i64,
        "retention_count must be an integer",
    )?;
    if retention_count_i64 < 0 {
        return Err(OpsContractError::new(
            "retention_count must be greater than or equal to 0",
            "retention_count must be greater than or equal to 0",
            400,
        ));
    }
    if retention_count_i64 > BACKUP_MAX_RETENTION_COUNT {
        return Err(OpsContractError::new(
            "retention_count must be less than or equal to 1000",
            "retention_count must be less than or equal to 1000",
            400,
        ));
    }

    let job_id = optional_positive_i64_field(object, "id", "id must be a positive integer")?;
    let schedule_expr = string_field(object, "schedule_expr").trim().to_string();
    if schedule_expr.len() > BACKUP_MAX_SCHEDULE_EXPR_LEN
        || schedule_expr.chars().any(char::is_control)
    {
        return Err(OpsContractError::new(
            "schedule_expr is invalid",
            "schedule_expr is invalid",
            400,
        ));
    }

    Ok(BackupJobContract {
        id: job_id,
        job_type,
        schedule_expr,
        retention_days,
        retention_count: retention_count_i64 as usize,
        enabled: object
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        last_status: non_empty_string_field(object, "last_status"),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn backup_encryption_secret_configured(secret: Option<&str>) -> bool {
    secret.is_some_and(|value| !value.trim().is_empty())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn derive_backup_fernet_key(secret: &str) -> Option<String> {
    let secret = secret.trim();
    if secret.is_empty() {
        return None;
    }
    let digest = Sha256::digest(secret.as_bytes());
    Some(base64_urlsafe_padded(&digest))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_comma_separated_ints(raw_value: &str) -> Vec<i64> {
    raw_value
        .split(',')
        .filter_map(|item| item.trim().parse::<i64>().ok())
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_export_timestamp_millis(raw_value: &str) -> Option<String> {
    let cleaned = raw_value.trim();
    if cleaned.is_empty() || cleaned == "0" {
        return None;
    }
    let timestamp = cleaned.parse::<i64>().ok()?;
    Local
        .timestamp_millis_opt(timestamp)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_sensitive_auth_mode(
    has_current_password: bool,
    has_step_up_token: bool,
) -> Result<SensitiveAuthMode, OpsContractError> {
    if has_step_up_token {
        return Ok(SensitiveAuthMode::StepUpToken);
    }
    if has_current_password {
        return Ok(SensitiveAuthMode::CurrentPassword);
    }
    Err(OpsContractError::new(
        "Invalid request",
        "Current password or stepUpToken is required",
        400,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_user_data_audit_contract(
    kind: UserDataClearKind,
    user_id: i64,
    auth_mode: SensitiveAuthMode,
    result: &Value,
) -> UserDataAuditContract {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "build_user_data_audit_contract",
        "business operation entered"
    );
    let success = result
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let affected_count = if kind == UserDataClearKind::Transactions {
        result
            .get("deleted_count")
            .and_then(Value::as_i64)
            .unwrap_or_default()
    } else {
        0
    };
    let details = if kind == UserDataClearKind::Transactions {
        json!({
            "deleted_count": affected_count,
            "auth_mode": auth_mode.as_audit_value(),
        })
    } else {
        let mut details = result
            .get("counts")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        details.insert("auth_mode".to_string(), json!(auth_mode.as_audit_value()));
        Value::Object(details)
    };

    UserDataAuditContract {
        operation_type: kind.operation_type().to_string(),
        operation_target: "user_data".to_string(),
        target_id: user_id,
        details,
        affected_count,
        status: if success { "success" } else { "failed" }.to_string(),
        error_message: if success {
            None
        } else {
            non_empty_value_string(result.get("message"))
        },
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_user_data_statistics(value: &Value) -> UserDataStatisticsContract {
    UserDataStatisticsContract {
        bill_count: field_i64_with_aliases(value, &["billCount", "bill_count", "bills"]),
        account_count: field_i64_with_aliases(
            value,
            &["accountCount", "account_count", "accounts"],
        ),
        category_count: field_i64_with_aliases(
            value,
            &["categoryCount", "category_count", "categories"],
        ),
        tag_count: field_i64_with_aliases(value, &["tagCount", "tag_count", "tags"]),
        template_count: field_i64_with_aliases(
            value,
            &["templateCount", "template_count", "templates"],
        ),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn user_data_statistics_response(statistics: &UserDataStatisticsContract) -> Value {
    json!({
        "success": true,
        "result": statistics,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_sync_provider(provider: &str) -> Option<String> {
    let normalized = provider.trim().to_lowercase();
    if SUPPORTED_SYNC_PROVIDERS.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_report_export_format(
    format_type: &str,
) -> Result<ReportExportContract, OpsContractError> {
    let normalized = format_type.trim().to_lowercase();
    match normalized.as_str() {
        "pdf" => Ok(ReportExportContract {
            format_type: normalized,
            extension: "pdf".to_string(),
            mimetype: "application/pdf".to_string(),
        }),
        "excel" | "xlsx" => Ok(ReportExportContract {
            format_type: "excel".to_string(),
            extension: "xlsx".to_string(),
            mimetype: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .to_string(),
        }),
        "html" => Ok(ReportExportContract {
            format_type: normalized,
            extension: "html".to_string(),
            mimetype: "text/html".to_string(),
        }),
        _ => Err(OpsContractError::new(
            "Unsupported report format",
            format!("不支持的格式: {format_type}"),
            400,
        )),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn secure_report_filename(filename: &str) -> String {
    let fallback = "report";
    let trimmed = filename.trim();
    if trimmed.is_empty() || matches!(trimmed, "." | "..") {
        return fallback.to_string();
    }

    let leaf = trimmed
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .map(str::trim)
        .unwrap_or(fallback);
    if leaf.is_empty() || matches!(leaf, "." | "..") {
        return fallback.to_string();
    }

    let mut cleaned = String::new();
    let mut previous_underscore = false;
    for ch in leaf.chars() {
        let replacement = if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || ch.is_control()
        {
            '_'
        } else {
            ch
        };
        if replacement == '_' {
            if !previous_underscore {
                cleaned.push('_');
            }
            previous_underscore = true;
        } else {
            cleaned.push(replacement);
            previous_underscore = false;
        }
    }

    let cleaned = cleaned.trim_matches(|ch| matches!(ch, ' ' | '.' | '_'));
    let reserved_name = cleaned
        .split_once('.')
        .map(|(prefix, _)| prefix)
        .unwrap_or(cleaned)
        .trim_end_matches([' ', '.'])
        .to_lowercase();

    if cleaned.is_empty()
        || matches!(cleaned, "." | "..")
        || is_windows_reserved_report_name(&reserved_name)
    {
        fallback.to_string()
    } else {
        cleaned.to_string()
    }
}

fn is_windows_reserved_report_name(name: &str) -> bool {
    matches!(name, "con" | "prn" | "aux" | "nul")
        || (name.len() == 4
            && (name.starts_with("com") || name.starts_with("lpt"))
            && name.as_bytes()[3].is_ascii_digit()
            && name.as_bytes()[3] != b'0')
}

fn optional_positive_i64_field(
    object: Option<&Map<String, Value>>,
    key: &str,
    error_message: &'static str,
) -> Result<Option<i64>, OpsContractError> {
    let Some(value) = object.and_then(|item| item.get(key)) else {
        return Ok(None);
    };
    let parsed = optional_i64_value(value)
        .ok_or_else(|| OpsContractError::new(error_message, error_message, 400))?;
    match parsed {
        None => Ok(None),
        Some(item) if item > 0 => Ok(Some(item)),
        Some(_) => Err(OpsContractError::new(error_message, error_message, 400)),
    }
}

fn optional_i64_field_with_default(
    object: Option<&Map<String, Value>>,
    key: &str,
    default_value: i64,
    error_message: &'static str,
) -> Result<i64, OpsContractError> {
    let Some(value) = object.and_then(|item| item.get(key)) else {
        return Ok(default_value);
    };
    optional_i64_value(value)
        .map(|item| item.unwrap_or(default_value))
        .ok_or_else(|| OpsContractError::new(error_message, error_message, 400))
}

fn optional_i64_value(value: &Value) -> Option<Option<i64>> {
    if value.is_null() {
        return Some(None);
    }
    match value {
        Value::Number(number) => number.as_i64().map(Some),
        Value::String(text) if text.trim().is_empty() => Some(None),
        Value::String(text) => text.trim().parse::<i64>().ok().map(Some),
        _ => None,
    }
}

fn string_field(object: Option<&Map<String, Value>>, key: &str) -> String {
    object
        .and_then(|item| item.get(key))
        .and_then(value_to_string)
        .unwrap_or_default()
}

fn non_empty_string_field(object: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    let value = string_field(object, key);
    (!value.trim().is_empty()).then_some(value)
}

fn non_empty_value_string(value: Option<&Value>) -> Option<String> {
    let text = value.and_then(value_to_string)?;
    (!text.trim().is_empty()).then_some(text)
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn field_i64_with_aliases(value: &Value, aliases: &[&str]) -> i64 {
    aliases
        .iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(|item| item.as_i64().or_else(|| item.as_str()?.parse().ok()))
        })
        .unwrap_or_default()
}

fn redact_ops_secrets_in_map(object: &mut Map<String, Value>) {
    for (key, value) in object.iter_mut() {
        if is_ops_secret_key(key) {
            let replacement = if secret_value_present(value) {
                "********"
            } else {
                ""
            };
            *value = json!(replacement);
        } else {
            redact_ops_secrets_in_value(value);
        }
    }
}

fn redact_ops_secrets_in_value(value: &mut Value) {
    match value {
        Value::Object(object) => redact_ops_secrets_in_map(object),
        Value::Array(items) => {
            for item in items {
                redact_ops_secrets_in_value(item);
            }
        }
        _ => {}
    }
}

fn secret_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

fn is_ops_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "accesskey"
            | "secretkey"
            | "password"
            | "token"
            | "credential"
            | "credentials"
            | "authorization"
            | "apikey"
            | "clientsecret"
    ) || [
        "secretkey",
        "accesskey",
        "password",
        "token",
        "credential",
        "credentials",
        "authorization",
        "apikey",
        "clientsecret",
    ]
    .iter()
    .any(|suffix| normalized.len() > suffix.len() && normalized.ends_with(suffix))
}

fn base64_urlsafe_padded(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        let triple = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);

        encoded.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() >= 2 {
            encoded.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() == 3 {
            encoded.push(ALPHABET[(triple & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}
