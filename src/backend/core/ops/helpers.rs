use serde_json::{json, Map, Value};

use super::types::OpsContractError;

/// 读取可选正整数配置字段，允许缺失/null，拒绝非数字或非正数。
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

/// 读取可选整数配置字段，缺失或 null 时使用默认值，非法类型返回业务合同错误。
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

/// 将 JSON 值解析为可空整数，兼容数字、数字字符串和空字符串/null。
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

/// 从 JSON 对象字段读取字符串表示，缺失或不可转换时返回空字符串。
pub(super) fn string_field(object: Option<&Map<String, Value>>, key: &str) -> String {
    object
        .and_then(|item| item.get(key))
        .and_then(value_to_string)
        .unwrap_or_default()
}

/// 从 JSON 对象字段读取非空字符串，空白值按缺失处理。
pub(super) fn non_empty_string_field(
    object: Option<&Map<String, Value>>,
    key: &str,
) -> Option<String> {
    let value = string_field(object, key);
    (!value.trim().is_empty()).then_some(value)
}

/// 从可选 JSON 值读取非空字符串，供多个 ops 合同解析器复用。
pub(super) fn non_empty_value_string(value: Option<&Value>) -> Option<String> {
    let text = value.and_then(value_to_string)?;
    (!text.trim().is_empty()).then_some(text)
}

/// 将标量 JSON 值转换为字符串，避免合同层重复处理 string/number/bool。
pub(super) fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

/// 按多个历史别名读取整数统计字段，缺失或非法时保持旧响应的 0 默认值。
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

/// 递归脱敏 ops 合同对象中的 secret 字段，保留字段存在性但移除真实密钥。
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

/// 递归脱敏任意 JSON 值中的 ops secret 字段，兼容嵌套对象和数组。
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

/// 判断 secret 字段是否实际有值，用于决定审计/响应中显示空串还是掩码。
pub(super) fn secret_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

/// 识别 ops 合同中的敏感 key，支持大小写、分隔符和后缀形式的密钥字段。
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
