// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn sync_config_validation_audit_details(config: &Value) -> Value {
    json!({
        "provider": audit_safe_json_field(config, "provider"),
        "endpoint": audit_safe_endpoint_json_field(config, "endpoint"),
        "bucket": audit_safe_json_field(config, "bucket"),
        "prefix": audit_safe_json_field(config, "prefix"),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn sync_config_audit_details(contract: &SyncConfigContract) -> Value {
    json!({
        "provider": contract.provider.clone(),
        "supported": contract.supported,
        "endpoint": audit_safe_endpoint_text(&contract.endpoint),
        "bucket": contract.bucket.clone(),
        "prefix": contract.prefix.clone(),
        "object_key": contract.object_key.clone(),
        "safe_config": audit_safe_sync_config(&contract.safe_config),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn validation_audit_details(payload: &Value) -> Value {
    json!({
        "job_type": audit_safe_json_field(payload, "job_type"),
        "id": audit_safe_json_field(payload, "id"),
        "retention_days": audit_safe_json_field(payload, "retention_days"),
        "retention_count": audit_safe_json_field(payload, "retention_count"),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn write_backup_sync_audit(
    runtime: &BackupOpsRuntime,
    user_id: UserId,
    headers: &HeaderMap,
    details: Value,
    affected_count: i64,
    status: &str,
    error_message: Option<String>,
) {
    write_backup_audit_event(
        runtime,
        user_id,
        headers,
        "backup_cloud_synced",
        details,
        affected_count,
        status,
        error_message,
    );
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn write_backup_job_audit(
    runtime: &BackupOpsRuntime,
    user_id: UserId,
    headers: &HeaderMap,
    details: Value,
    affected_count: i64,
    status: &str,
    error_message: Option<String>,
) {
    write_backup_audit_event(
        runtime,
        user_id,
        headers,
        "backup_job_saved",
        details,
        affected_count,
        status,
        error_message,
    );
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn write_backup_audit_event(
    runtime: &BackupOpsRuntime,
    user_id: UserId,
    headers: &HeaderMap,
    operation_type: &str,
    details: Value,
    affected_count: i64,
    status: &str,
    error_message: Option<String>,
) {
    let details = with_audit_actor(details, user_id);
    let draft = BackupAuditLogDraft {
        operation_type: operation_type.to_string(),
        details,
        affected_count,
        ip_address: client_ip(headers),
        user_agent: header_text(headers, "user-agent"),
        status: status.to_string(),
        error_message,
    };
    match runtime {
        BackupOpsRuntime::Postgres(runtime) => {
            let _ = block_on_backup_db(create_postgres_backup_audit_log_best_effort(
                runtime.pool(),
                draft,
            ));
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn with_audit_actor(mut details: Value, user_id: UserId) -> Value {
    let Value::Object(object) = &mut details else {
        return json!({ "user_id": user_id.get(), "details": details });
    };
    object.insert("user_id".to_string(), json!(user_id.get()));
    details
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn client_ip(headers: &HeaderMap) -> Option<String> {
    let forwarded_for = header_text(headers, "x-forwarded-for");
    if let Some(value) = forwarded_for {
        let first = value.split(',').next().unwrap_or_default().trim();
        if !first.is_empty() {
            return Some(first.to_string());
        }
    }
    header_text(headers, "x-real-ip")
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_safe_json_field(payload: &Value, key: &str) -> String {
    payload.get(key).map(audit_safe_value).unwrap_or_default()
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_safe_endpoint_json_field(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .map(audit_safe_endpoint_value)
        .unwrap_or_default()
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_safe_endpoint_value(value: &Value) -> String {
    value
        .as_str()
        .map(audit_safe_endpoint_text)
        .unwrap_or_else(|| "<invalid-url>".to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_safe_endpoint_text(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let Ok(mut url) = Url::parse(raw) else {
        return "<invalid-url>".to_string();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    audit_safe_value(&Value::String(
        url.as_str().trim_end_matches('/').to_string(),
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_safe_sync_config(config: &Value) -> Value {
    let mut config = config.clone();
    audit_sanitize_endpoint_keys(&mut config);
    config
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_sanitize_endpoint_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if key.eq_ignore_ascii_case("endpoint") {
                    *value = Value::String(audit_safe_endpoint_value(value));
                } else {
                    audit_sanitize_endpoint_keys(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                audit_sanitize_endpoint_keys(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn audit_safe_value(value: &Value) -> String {
    let raw = value.as_str().map(str::to_string).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    });
    raw.trim().chars().take(128).collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn json_string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
