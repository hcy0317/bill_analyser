use serde_json::{json, Map, Value};

use super::types::OpsContractError;

pub(super) fn optional_positive_i64_field(
    object: Option<&Map<String, Value>>,
    key: &str,
    error_message: &'static str,
) -> Result<Option<i64>, OpsContractError> {
    let Some(value) = object.and_then(|item| item.get(key)) else {
        return Ok(None);
    };
    let parsed = optional_i64_value(value)
        .ok_or_else(|| OpsContractError::new(error_message, error_message, 400))?;
    match parsed {
        None => Ok(None),
        Some(item) if item > 0 => Ok(Some(item)),
        Some(_) => Err(OpsContractError::new(error_message, error_message, 400)),
    }
}

pub(super) fn optional_i64_field_with_default(
    object: Option<&Map<String, Value>>,
    key: &str,
    default_value: i64,
    error_message: &'static str,
) -> Result<i64, OpsContractError> {
    let Some(value) = object.and_then(|item| item.get(key)) else {
        return Ok(default_value);
    };
    optional_i64_value(value)
        .map(|item| item.unwrap_or(default_value))
        .ok_or_else(|| OpsContractError::new(error_message, error_message, 400))
}

pub(super) fn optional_i64_value(value: &Value) -> Option<Option<i64>> {
    if value.is_null() {
        return Some(None);
    }
    match value {
        Value::Number(number) => number.as_i64().map(Some),
        Value::String(text) if text.trim().is_empty() => Some(None),
        Value::String(text) => text.trim().parse::<i64>().ok().map(Some),
        _ => None,
    }
}

pub(super) fn string_field(object: Option<&Map<String, Value>>, key: &str) -> String {
    object
        .and_then(|item| item.get(key))
        .and_then(value_to_string)
        .unwrap_or_default()
}

pub(super) fn non_empty_string_field(
    object: Option<&Map<String, Value>>,
    key: &str,
) -> Option<String> {
    let value = string_field(object, key);
    (!value.trim().is_empty()).then_some(value)
}

pub(super) fn non_empty_value_string(value: Option<&Value>) -> Option<String> {
    let text = value.and_then(value_to_string)?;
    (!text.trim().is_empty()).then_some(text)
}

pub(super) fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn field_i64_with_aliases(value: &Value, aliases: &[&str]) -> i64 {
    aliases
        .iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(|item| item.as_i64().or_else(|| item.as_str()?.parse().ok()))
        })
        .unwrap_or_default()
}

pub(super) fn redact_ops_secrets_in_map(object: &mut Map<String, Value>) {
    for (key, value) in object.iter_mut() {
        if is_ops_secret_key(key) {
            let replacement = if secret_value_present(value) {
                "********"
            } else {
                ""
            };
            *value = json!(replacement);
        } else {
            redact_ops_secrets_in_value(value);
        }
    }
}

pub(super) fn redact_ops_secrets_in_value(value: &mut Value) {
    match value {
        Value::Object(object) => redact_ops_secrets_in_map(object),
        Value::Array(items) => {
            for item in items {
                redact_ops_secrets_in_value(item);
            }
        }
        _ => {}
    }
}

pub(super) fn secret_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

pub(super) fn is_ops_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "accesskey"
            | "secretkey"
            | "password"
            | "token"
            | "credential"
            | "credentials"
            | "authorization"
            | "apikey"
            | "clientsecret"
    ) || [
        "secretkey",
        "accesskey",
        "password",
        "token",
        "credential",
        "credentials",
        "authorization",
        "apikey",
        "clientsecret",
    ]
    .iter()
    .any(|suffix| normalized.len() > suffix.len() && normalized.ends_with(suffix))
}
