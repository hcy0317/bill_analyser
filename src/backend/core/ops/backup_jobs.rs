use serde_json::Value;

use super::helpers::{
    non_empty_string_field, optional_i64_field_with_default, optional_positive_i64_field,
    string_field,
};
use super::types::{
    BackupJobContract, OpsContractError, BACKUP_DEFAULT_RETENTION_COUNT,
    BACKUP_DEFAULT_RETENTION_DAYS, BACKUP_MAX_JOB_TYPE_LEN, BACKUP_MAX_RETENTION_COUNT,
    BACKUP_MAX_RETENTION_DAYS, BACKUP_MAX_SCHEDULE_EXPR_LEN,
};

#[tracing::instrument(level = "debug", skip_all)]
/// 规范化备份任务保存 payload，集中校验 job_type、schedule、保留天数/数量和启用状态默认值。
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
