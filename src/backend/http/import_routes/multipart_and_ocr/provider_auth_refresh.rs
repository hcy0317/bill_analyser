use super::*;

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
    validate_provider_token_endpoint(token_endpoint, provider_base_url)
        .map_err(|_| ProviderAuthRefreshError)?;

    let mut url = Url::parse(token_endpoint).map_err(|_| ProviderAuthRefreshError)?;
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

#[tracing::instrument(level = "debug", skip_all)]
fn validate_provider_token_endpoint(
    token_endpoint: &str,
    provider_base_url: &str,
) -> Result<(), String> {
    let parsed = parse_provider_runtime_url(token_endpoint)?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("token endpoint must not contain credentials".to_string());
    }
    if provider_url_is_same_origin(&parsed, provider_base_url)
        || provider_url_is_allowlisted(&parsed, "BILL_ANALYSER_LLM_TOKEN_URL_ALLOWLIST")
    {
        if parsed.scheme() == "https" || provider_url_is_local_http(&parsed) {
            return Ok(());
        }
        return Err("token endpoint must use https unless it is local".to_string());
    }
    Err("token endpoint is not allowed".to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_provider_runtime_url(value: &str) -> Result<Url, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('\\') || trimmed.chars().any(char::is_control) {
        return Err("provider url is not allowed".to_string());
    }
    let parsed = Url::parse(trimmed).map_err(|_| "provider url must be valid".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("provider url must use http or https with a host".to_string());
    }
    Ok(parsed)
}

fn provider_url_is_same_origin(parsed: &Url, other_url: &str) -> bool {
    Url::parse(other_url)
        .map(|other| {
            parsed.scheme() == other.scheme()
                && parsed.host_str().map(str::to_ascii_lowercase)
                    == other.host_str().map(str::to_ascii_lowercase)
                && parsed.port_or_known_default() == other.port_or_known_default()
        })
        .unwrap_or(false)
}

fn provider_url_is_allowlisted(parsed: &Url, env_key: &str) -> bool {
    let origin = provider_url_origin(parsed);
    let full = parsed.as_str().trim_end_matches('/').to_ascii_lowercase();
    env::var(env_key)
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.trim_end_matches('/').to_ascii_lowercase())
        .any(|entry| entry == full || entry == origin)
}

fn provider_url_is_local_http(parsed: &Url) -> bool {
    parsed.scheme() == "http"
        && parsed.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host.ends_with(".localhost")
                || host == "127.0.0.1"
                || host == "::1"
        })
}

fn provider_url_origin(parsed: &Url) -> String {
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    match parsed.port() {
        Some(port) => format!("{}://{}:{port}", parsed.scheme(), host),
        None => format!("{}://{}", parsed.scheme(), host),
    }
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
