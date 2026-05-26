// 中文导读：HTTP 数据库配置 helper，负责 PostgreSQL URL 校验和健康输出脱敏。
// 维护重点：只保留配置字符串处理，不打开连接、不决定 repository 切换。
// 不变式：健康输出不得包含数据库密码或 query 参数。

use url::Url;

use crate::config::HttpShellConfigError;

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn normalize_postgres_url(
    value: Option<String>,
) -> Result<Option<String>, HttpShellConfigError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = Url::parse(trimmed).map_err(|_| HttpShellConfigError::InvalidPostgresUrl)?;
    if !matches!(parsed.scheme(), "postgres" | "postgresql") || parsed.host_str().is_none() {
        return Err(HttpShellConfigError::InvalidPostgresUrl);
    }
    Ok(Some(parsed.to_string()))
}

pub fn redact_postgres_url(postgres_url: &str) -> String {
    let Ok(mut parsed) = Url::parse(postgres_url.trim()) else {
        return "<invalid-postgres-url>".to_string();
    };

    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("***"));
    }
    parsed.set_query(None);
    parsed.to_string()
}
