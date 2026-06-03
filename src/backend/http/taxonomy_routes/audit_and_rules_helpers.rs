// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

struct AccountAuditLogDraft {
    operation_type: &'static str,
    target_id: i64,
    details: Value,
    affected_count: i64,
    status: &'static str,
    error_message: Option<String>,
    ip_address: String,
    user_agent: String,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn verify_sensitive_account_operation_password_postgres(
    pool: &PostgresPool,
    user_id: i64,
    password: &str,
) -> bill_analyser_db::DbResult<bool> {
    if password.is_empty() {
        return Ok(false);
    }

    let password_hash: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    if password_hash
        .filter(|value| !value.is_empty())
        .is_some_and(|hash| bcrypt::verify(password, &hash).unwrap_or(false))
    {
        return Ok(true);
    }

    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    let stored_password: Option<Value> =
        sqlx::query_scalar("SELECT value FROM settings WHERE user_id = $1 AND key = $2")
            .bind(user_id)
            .bind("operation_password")
            .fetch_optional(pool)
            .await?;
    let stored_password = stored_password.as_ref().and_then(json_setting_string);
    Ok(match stored_password.as_deref().filter(|value| !value.is_empty()) {
        Some(value) => password == value,
        None => true,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_account_audit_log_best_effort_postgres(
    pool: &PostgresPool,
    user_id: i64,
    draft: AccountAuditLogDraft,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor,
            before_payload, after_payload, metadata
        )
        VALUES ($1, 'account', $2, $3, 'runtime', '{}'::jsonb, '{}'::jsonb, $4)
        "#,
    )
    .bind(user_id)
    .bind(draft.target_id.to_string())
    .bind(draft.operation_type)
    .bind(json!({
        "details": draft.details,
        "affected_count": draft.affected_count,
        "status": draft.status,
        "error_message": draft.error_message,
        "ip_address": draft.ip_address,
        "user_agent": draft.user_agent,
    }))
    .execute(pool)
    .await;
}

fn audit_ip_address(headers: &HeaderMap) -> String {
    header_string(headers, "x-forwarded-for")
        .split(',')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            let value = header_string(headers, "x-real-ip");
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .unwrap_or_default()
}

fn audit_user_agent(headers: &HeaderMap) -> String {
    header_string(headers, "user-agent")
}

fn header_string(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn json_setting_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(object) => object
            .get("value")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn optional_json_body(body: Bytes) -> Option<Value> {
    if body.is_empty() {
        return None;
    }
    serde_json::from_slice(&body).ok()
}

fn value_as_i64_or(value: Option<&Value>, default: i64) -> i64 {
    value.and_then(value_as_i64).unwrap_or(default)
}

fn category_rules_enabled_only(query: &CategoryRulesQuery) -> bool {
    !query
        .enabled_only
        .as_deref()
        .unwrap_or("true")
        .eq_ignore_ascii_case("false")
}
