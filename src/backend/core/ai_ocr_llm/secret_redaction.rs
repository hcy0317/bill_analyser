// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Map, Value};

/// 判断对象及其嵌套值中是否存在非空 secret，用于前端 has_api_key 状态。
pub(super) fn object_has_non_empty_secret(object: &Map<String, Value>) -> bool {
    object.iter().any(|(key, value)| {
        let current_key_has_secret = is_secret_key(key) && secret_value_present(value);
        current_key_has_secret || value_has_non_empty_secret(value)
    })
}

fn value_has_non_empty_secret(value: &Value) -> bool {
    match value {
        Value::Object(object) => object_has_non_empty_secret(object),
        Value::Array(items) => items.iter().any(value_has_non_empty_secret),
        _ => false,
    }
}

/// 递归脱敏对象中的 secret 字段，非空 secret 用统一占位符替代。
pub(super) fn redact_secrets_in_map(object: &mut Map<String, Value>) {
    for (key, value) in object.iter_mut() {
        if is_secret_key(key) {
            let replacement = if secret_value_present(value) {
                "********"
            } else {
                ""
            };
            *value = json!(replacement);
        } else {
            redact_secrets_in_value(value);
        }
    }
}

/// 递归脱敏 JSON 值中的 secret 字段，覆盖数组和对象嵌套结构。
pub(super) fn redact_secrets_in_value(value: &mut Value) {
    match value {
        Value::Object(object) => redact_secrets_in_map(object),
        Value::Array(items) => {
            for item in items {
                redact_secrets_in_value(item);
            }
        }
        _ => {}
    }
}

fn secret_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let exact_alias = matches!(
        normalized.as_str(),
        "apikey"
            | "authorization"
            | "xapikey"
            | "apisecret"
            | "secretkey"
            | "credential"
            | "credentials"
            | "proxyauthorization"
            | "subscriptionkey"
            | "ocpapimsubscriptionkey"
            | "accesstoken"
            | "refreshtoken"
            | "bearertoken"
            | "idtoken"
            | "privatekey"
            | "token"
            | "password"
            | "clientsecret"
    );
    if exact_alias {
        return true;
    }

    const SECRET_KEY_SUFFIXES: [&str; 15] = [
        "apikey",
        "xapikey",
        "apisecret",
        "secretkey",
        "authorizationheader",
        "proxyauthorization",
        "subscriptionkey",
        "accesstoken",
        "refreshtoken",
        "bearertoken",
        "idtoken",
        "privatekey",
        "password",
        "clientsecret",
        "credentials",
    ];

    SECRET_KEY_SUFFIXES
        .iter()
        .any(|suffix| normalized.len() > suffix.len() && normalized.ends_with(suffix))
}
