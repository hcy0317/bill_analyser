// 中文导读：HTTP 应用层的敏感操作密码策略唯一所有者。
// 维护重点：只负责既有密码来源的优先级与 fail-closed 判定，不拥有 step-up、TOTP、审计或业务写入。
// 不变式：当前用户密码优先；非空环境变量覆盖存储密码；缺失、空白或无法识别的配置均拒绝。

use bill_analyser_db::{get_postgres_operation_password, AuthLoginUserRow, DbResult, PostgresPool};

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) async fn verify_sensitive_operation_password(
    pool: &PostgresPool,
    user: &AuthLoginUserRow,
    password: &str,
) -> DbResult<bool> {
    if password.is_empty() {
        return Ok(false);
    }

    if !user.password_hash.is_empty()
        && bcrypt::verify(password, &user.password_hash).unwrap_or(false)
    {
        return Ok(true);
    }

    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    let stored_password = get_postgres_operation_password(pool, user.profile.id).await?;
    Ok(stored_operation_password_matches(
        password,
        stored_password.as_deref(),
    ))
}

fn stored_operation_password_matches(password: &str, stored_password: Option<&str>) -> bool {
    stored_password
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| password == value)
}

#[cfg(test)]
mod tests {
    use super::stored_operation_password_matches;

    #[test]
    fn stored_password_fails_closed_when_missing_or_blank() {
        assert!(!stored_operation_password_matches("wrong", None));
        assert!(!stored_operation_password_matches("wrong", Some("  ")));
    }

    #[test]
    fn stored_password_uses_trimmed_configured_value() {
        assert!(stored_operation_password_matches(
            "operation-secret",
            Some("  operation-secret  ")
        ));
        assert!(!stored_operation_password_matches(
            "wrong-current-password",
            Some("operation-secret")
        ));
    }
}
