// 中文导读：核心业务合同层，负责 LLM/OCR provider 凭据 JSON 的解析、脱敏和刷新判定。
// 维护重点：这里不执行网络请求，只产出运行时可消费的规范化 auth profile。
// 不变式：所有普通读出必须通过 redaction helper；新增 secret key 要同步覆盖嵌套 JSON。

use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Map, Value};

use super::secret_redaction::redact_secrets_in_value;

const DEFAULT_EXPIRY_SKEW_SECONDS: i64 = 60;

pub fn normalize_provider_auth_config(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return json!({});
    };
    let parsed = credential_source_value(value);
    let source = parsed.as_ref().unwrap_or(value);
    let root_object = value.as_object();
    let object = source.as_object();
    let mut normalized = Map::new();

    if let Some(mode) = first_text(root_object, &["credential_mode", "credentialMode", "mode"])
        .or_else(|| first_text(object, &["credential_mode", "credentialMode", "mode"]))
    {
        normalized.insert(
            "credential_mode".to_string(),
            json!(normalize_credential_mode(&mode)),
        );
    } else {
        normalized.insert("credential_mode".to_string(), json!("api_key"));
    }

    if let Some(raw) = credential_json_value(value) {
        normalized.insert("credential_json".to_string(), raw);
    } else if value.is_object() {
        normalized.insert("credential_json".to_string(), value.clone());
    }

    if let Some(access_token) = first_deep_text(
        source,
        &[
            "access_token",
            "accessToken",
            "access-token",
            "bearer_token",
            "bearerToken",
            "id_token",
            "idToken",
            "token",
            "api_key",
            "apiKey",
        ],
    ) {
        normalized.insert("access_token".to_string(), json!(access_token));
    } else if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        normalized.insert("access_token".to_string(), json!(text));
    }

    if let Some(refresh_token) = first_deep_text(
        source,
        &["refresh_token", "refreshToken", "refresh-token", "refresh"],
    ) {
        normalized.insert("refresh_token".to_string(), json!(refresh_token));
    }

    if let Some(expires_at) = first_deep_text(
        source,
        &["expires_at", "expiresAt", "expiry", "expiration", "expires"],
    ) {
        normalized.insert("expires_at".to_string(), json!(expires_at));
    } else if let Some(expires_in) = first_deep_i64(source, &["expires_in", "expiresIn"]) {
        let expires_at = Utc::now() + Duration::seconds(expires_in.max(0));
        normalized.insert("expires_at".to_string(), json!(expires_at.to_rfc3339()));
    }

    if let Some(token_endpoint) = first_text(
        root_object,
        &[
            "token_endpoint",
            "tokenEndpoint",
            "refresh_endpoint",
            "refreshEndpoint",
            "refresh_url",
            "refreshUrl",
        ],
    )
    .or_else(|| {
        first_deep_text(
            source,
            &[
                "token_endpoint",
                "tokenEndpoint",
                "refresh_endpoint",
                "refreshEndpoint",
                "refresh_url",
                "refreshUrl",
            ],
        )
    }) {
        normalized.insert("token_endpoint".to_string(), json!(token_endpoint));
    }

    for (output_key, aliases) in [
        (
            "refresh_headers",
            &[
                "refresh_headers",
                "refreshHeaders",
                "token_headers",
                "tokenHeaders",
            ][..],
        ),
        (
            "refresh_body",
            &["refresh_body", "refreshBody", "token_body", "tokenBody"][..],
        ),
        (
            "refresh_params",
            &[
                "refresh_params",
                "refreshParams",
                "token_params",
                "tokenParams",
            ][..],
        ),
        (
            "request_headers",
            &["request_headers", "requestHeaders", "headers"][..],
        ),
        (
            "request_params",
            &["request_params", "requestParams", "params", "parameters"][..],
        ),
    ] {
        if let Some(value) =
            first_object_value(root_object, aliases).or_else(|| first_object_value(object, aliases))
        {
            normalized.insert(output_key.to_string(), value);
        }
    }

    if normalized.iter().all(|(key, value)| {
        key == "credential_mode" || (key == "credential_json" && is_empty_jsonish(value))
    }) {
        return json!({});
    }

    Value::Object(normalized)
}

pub fn redact_provider_auth_config(value: &Value) -> Value {
    let mut redacted = normalize_provider_auth_config(Some(value));
    redact_secrets_in_value(&mut redacted);
    redacted
}

pub fn provider_auth_access_token(value: &Value) -> Option<String> {
    value
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty() && *text != "********")
        .map(ToString::to_string)
}

pub fn provider_auth_refresh_token(value: &Value) -> Option<String> {
    value
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty() && *text != "********")
        .map(ToString::to_string)
}

pub fn provider_auth_has_refresh_credential(value: &Value) -> bool {
    provider_auth_refresh_token(value).is_some()
}

pub fn provider_auth_is_expired(value: &Value, now: DateTime<Utc>) -> bool {
    let Some(expires_at) = value
        .get("expires_at")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    else {
        return false;
    };
    let parsed = DateTime::parse_from_rfc3339(expires_at)
        .map(|value| value.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|value| value.and_utc())
        });
    parsed
        .map(|expires_at| expires_at <= now + Duration::seconds(DEFAULT_EXPIRY_SKEW_SECONDS))
        .unwrap_or(false)
}

fn credential_source_value(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    for key in [
        "credential_json",
        "credentialJson",
        "session_json",
        "sessionJson",
        "auth_json",
        "authJson",
        "account_json",
        "accountJson",
        "sub2api_json",
        "sub2apiJson",
        "session",
        "auth",
        "account",
        "sub2api",
    ] {
        if let Some(value) = object.get(key).and_then(parse_jsonish_value) {
            if !is_empty_jsonish(&value) {
                return Some(value);
            }
        }
    }
    None
}

fn credential_json_value(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    for key in [
        "credential_json",
        "credentialJson",
        "session_json",
        "sessionJson",
        "auth_json",
        "authJson",
        "account_json",
        "accountJson",
        "sub2api_json",
        "sub2apiJson",
    ] {
        if let Some(value) = object.get(key) {
            return parse_jsonish_value(value);
        }
    }
    None
}

fn parse_jsonish_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .or_else(|| Some(Value::String(text.clone()))),
        Value::Object(_) | Value::Array(_) => Some(value.clone()),
        _ => None,
    }
}

fn is_empty_jsonish(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
        _ => false,
    }
}

fn normalize_credential_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "" => "api_key".to_string(),
        "apikey" | "api_key" => "api_key".to_string(),
        "session" | "session_json" => "session_json".to_string(),
        "auth" | "auth_json" => "auth_json".to_string(),
        "account" | "account_json" => "account_json".to_string(),
        "sub2api" | "sub2api_json" => "sub2api_json".to_string(),
        "access" | "access_token" => "access_token".to_string(),
        "refresh" | "refresh_token" => "refresh_token".to_string(),
        other => other.to_string(),
    }
}

fn first_text(object: Option<&Map<String, Value>>, keys: &[&str]) -> Option<String> {
    let object = object?;
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(value_to_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn first_object_value(object: Option<&Map<String, Value>>, keys: &[&str]) -> Option<Value> {
    let object = object?;
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(parse_jsonish_value)
            .filter(Value::is_object)
    })
}

fn first_deep_text(value: &Value, keys: &[&str]) -> Option<String> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(text) = object.get(*key).and_then(value_to_text) {
                let text = text.trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
        for child in object.values() {
            if let Some(text) = first_deep_text(child, keys) {
                return Some(text);
            }
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            if let Some(text) = first_deep_text(child, keys) {
                return Some(text);
            }
        }
    }
    None
}

fn first_deep_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(value) = object.get(*key).and_then(value_to_i64) {
                return Some(value);
            }
        }
        for child in object.values() {
            if let Some(value) = first_deep_i64(child, keys) {
                return Some(value);
            }
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            if let Some(value) = first_deep_i64(child, keys) {
                return Some(value);
            }
        }
    }
    None
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}
