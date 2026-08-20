// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
/// 校验敏感账户操作密码，按用户密码、环境变量、旧 settings 密码的顺序兼容历史配置。
async fn verify_sensitive_account_operation_password_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    password: &str,
) -> bill_analyser_db::DbResult<bool> {
    if password.is_empty() {
        return Ok(false);
    }

    let current_password_matches = get_postgres_login_user_by_id(pool, user_id)
        .await?
        .is_some_and(|user| {
            !user.password_hash.is_empty()
                && bcrypt::verify(password, &user.password_hash).unwrap_or(false)
        });
    if current_password_matches
    {
        return Ok(true);
    }

    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    let stored_password = get_postgres_operation_password(pool, user_id).await?;
    Ok(stored_account_operation_password_matches(
        password,
        stored_password.as_deref(),
    ))
}

fn stored_account_operation_password_matches(password: &str, stored: Option<&str>) -> bool {
    stored
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| password == value)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 以 best-effort 方式写入账户审计日志，审计失败不得阻断原业务操作。
async fn create_account_audit_log_best_effort_postgres(
    pool: &PostgresPool,
    user_id: i64,
    draft: AccountAuditEventDraft,
) {
    let _ = create_postgres_account_audit_event(pool, user_id, draft).await;
}

/// 从代理链优先提取审计 IP，缺失时返回空字符串避免伪造默认值。
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

fn optional_json_body(body: Bytes) -> Option<Value> {
    if body.is_empty() {
        return None;
    }
    serde_json::from_slice(&body).ok()
}

fn value_as_i64_or(value: Option<&Value>, default: i64) -> i64 {
    value.and_then(value_as_i64).unwrap_or(default)
}

#[cfg(test)]
mod sensitive_account_operation_password_tests {
    use super::stored_account_operation_password_matches;

    #[test]
    fn missing_or_blank_operation_password_fails_closed() {
        assert!(!stored_account_operation_password_matches(
            "wrong-current-password",
            None
        ));
        assert!(!stored_account_operation_password_matches(
            "wrong-current-password",
            Some("  ")
        ));
        assert!(stored_account_operation_password_matches(
            "legacy-secret",
            Some("legacy-secret")
        ));
    }
}

/// 解析分类规则列表查询的 enabled_only 默认值，默认只返回启用规则。
fn category_rules_enabled_only(query: &CategoryRulesQuery) -> bool {
    !query
        .enabled_only
        .as_deref()
        .unwrap_or("true")
        .eq_ignore_ascii_case("false")
}
