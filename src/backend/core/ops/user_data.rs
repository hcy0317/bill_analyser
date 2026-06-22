use chrono::{Local, TimeZone};
use serde_json::{json, Value};

use super::helpers::{field_i64_with_aliases, non_empty_value_string};
use super::types::{
    OpsContractError, SensitiveAuthMode, UserDataAuditContract, UserDataClearKind,
    UserDataStatisticsContract,
};

#[tracing::instrument(level = "debug", skip_all)]
/// 解析逗号分隔的整型参数，供用户数据导出/清理接口复用兼容旧查询格式。
pub fn parse_comma_separated_ints(raw_value: &str) -> Vec<i64> {
    raw_value
        .split(',')
        .filter_map(|item| item.trim().parse::<i64>().ok())
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将前端毫秒时间戳转为本地时间展示字符串，空值和 0 保持为未指定时间。
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
/// 解析敏感操作认证方式，保证当前密码和 step-up token 至少提供一种才允许继续。
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
/// 根据清理结果生成审计合同，保留 auth_mode、影响数量和失败消息供 route 层写入审计表。
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
/// 兼容 camelCase、snake_case 和旧别名，规范化用户数据统计响应。
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
/// 生成用户数据统计接口的统一 success/result envelope。
pub fn user_data_statistics_response(statistics: &UserDataStatisticsContract) -> Value {
    json!({
        "success": true,
        "result": statistics,
    })
}
