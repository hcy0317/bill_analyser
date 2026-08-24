use super::*;
use bill_analyser_core::{OutboundHostClass, OutboundHttpUrl, OutboundHttpUrlParseError};

/// 刷新 provider auth profile，先校验 token endpoint，再发起受限 JSON refresh 请求。
pub(super) async fn refresh_provider_auth_profile(
    client: &reqwest::Client,
    credential_config: &Value,
    provider_base_url: &str,
) -> Result<Value, ProviderAuthRefreshError> {
    let token_endpoint = credential_config
        .get("token_endpoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ProviderAuthRefreshError)?;
    let mut url = validate_provider_token_endpoint(token_endpoint, provider_base_url)
        .map_err(|_| ProviderAuthRefreshError)?;
    if let Some(params) = credential_config
        .get("refresh_params")
        .and_then(Value::as_object)
        .or_else(|| {
            credential_config
                .get("request_params")
                .and_then(Value::as_object)
        })
    {
        for (key, value) in params {
            if let Some(value) = header_value_text(value) {
                url.query_pairs_mut().append_pair(key, &value);
            }
        }
    }
    let mut request = client.post(url);
    if let Some(headers) = credential_config
        .get("refresh_headers")
        .and_then(Value::as_object)
    {
        for (key, value) in headers {
            if let Some(value) = header_value_text(value) {
                request = request.header(key, value);
            }
        }
    }

    let mut body = credential_config
        .get("refresh_body")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if !body.contains_key("grant_type") {
        body.insert("grant_type".to_string(), json!("refresh_token"));
    }
    if !body.contains_key("refresh_token") {
        let refresh_token =
            provider_auth_refresh_token(credential_config).ok_or(ProviderAuthRefreshError)?;
        body.insert("refresh_token".to_string(), json!(refresh_token));
    }

    let response = request
        .header("content-type", "application/json")
        .body(Value::Object(body).to_string())
        .send()
        .await
        .map_err(|_| ProviderAuthRefreshError)?;
    if !response.status().is_success() {
        return Err(ProviderAuthRefreshError);
    }
    let raw_text = read_limited_llm_provider_body(response)
        .await
        .map_err(|_| ProviderAuthRefreshError)?;
    let response_json =
        serde_json::from_str::<Value>(&raw_text).map_err(|_| ProviderAuthRefreshError)?;
    merge_refreshed_provider_auth_profile(credential_config, &response_json)
        .ok_or(ProviderAuthRefreshError)
}

/// 合并刷新响应和原 profile，保留 refresh token/token endpoint 并重新归一化。
#[tracing::instrument(level = "debug", skip_all)]
fn merge_refreshed_provider_auth_profile(current: &Value, response: &Value) -> Option<Value> {
    let mut merged = current.as_object().cloned().unwrap_or_default();
    let refreshed = normalize_provider_auth_config(Some(response));
    let refreshed_object = refreshed.as_object()?;
    for key in [
        "access_token",
        "refresh_token",
        "expires_at",
        "credential_mode",
        "credential_json",
    ] {
        if let Some(value) = refreshed_object.get(key) {
            merged.insert(key.to_string(), value.clone());
        }
    }
    if !merged.contains_key("refresh_token") {
        if let Some(refresh_token) = provider_auth_refresh_token(current) {
            merged.insert("refresh_token".to_string(), json!(refresh_token));
        }
    }
    if !merged.contains_key("token_endpoint") {
        if let Some(token_endpoint) = current.get("token_endpoint").cloned() {
            merged.insert("token_endpoint".to_string(), token_endpoint);
        }
    }
    let normalized = normalize_provider_auth_config(Some(&Value::Object(merged)));
    provider_auth_access_token(&normalized)?;
    Some(normalized)
}

/// 校验 token endpoint 的 SSRF 边界，只允许同源、allowlist 或安全本地 https/http 端点。
#[tracing::instrument(level = "debug", skip_all)]
fn validate_provider_token_endpoint(
    token_endpoint: &str,
    provider_base_url: &str,
) -> Result<Url, String> {
    let parsed = parse_provider_runtime_url(token_endpoint)?;
    if provider_url_host_is_never_allowed(&parsed) {
        return Err("token endpoint is not allowed".to_string());
    }
    if parsed.same_origin(provider_base_url)
        || provider_url_is_allowlisted(&parsed, "BILL_ANALYSER_LLM_TOKEN_URL_ALLOWLIST")
    {
        if parsed.is_https() || provider_url_is_local_http(&parsed) {
            return Ok(parsed.into_url());
        }
        return Err("token endpoint must use https unless it is local".to_string());
    }
    Err("token endpoint is not allowed".to_string())
}

/// 解析 provider runtime URL，拒绝空值、控制字符、反斜杠、非 http/https 或无 host 输入。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_provider_runtime_url(value: &str) -> Result<OutboundHttpUrl, String> {
    OutboundHttpUrl::parse(value).map_err(|error| match error {
        OutboundHttpUrlParseError::Empty | OutboundHttpUrlParseError::UnsafeText => {
            "provider url is not allowed".to_string()
        }
        OutboundHttpUrlParseError::Invalid => "provider url must be valid".to_string(),
        OutboundHttpUrlParseError::UnsupportedScheme
        | OutboundHttpUrlParseError::MissingHost => {
            "provider url must use http or https with a host".to_string()
        }
        OutboundHttpUrlParseError::Credentials => {
            "token endpoint must not contain credentials".to_string()
        }
    })
}

/// 判断 token endpoint 是否位于环境变量 allowlist 中，支持完整 URL 或 origin。
fn provider_url_is_allowlisted(parsed: &OutboundHttpUrl, env_key: &str) -> bool {
    parsed.matches_exact_or_origin_allowlist(&env::var(env_key).unwrap_or_default())
}

/// 仅允许本地 HTTP 例外，其他非 HTTPS token endpoint 必须被拒绝。
fn provider_url_is_local_http(parsed: &OutboundHttpUrl) -> bool {
    !parsed.is_https()
        && matches!(
            parsed.host_class(),
            OutboundHostClass::Localhost | OutboundHostClass::Loopback
        )
}

fn provider_url_host_is_never_allowed(parsed: &OutboundHttpUrl) -> bool {
    matches!(
        parsed.host_class(),
        OutboundHostClass::Metadata
            | OutboundHostClass::Unspecified
            | OutboundHostClass::LinkLocal
            | OutboundHostClass::Multicast
    )
}

fn header_value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "provider_auth_refresh_tests.rs"]
mod tests;
