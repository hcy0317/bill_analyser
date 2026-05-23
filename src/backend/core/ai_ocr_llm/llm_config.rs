// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Map, Value};

use super::llm_provider::llm_available_providers;
use super::secret_redaction::{object_has_non_empty_secret, redact_secrets_in_map};
use super::types::AiRouteResponse;
use super::value_helpers::string_field_or;

const LLM_PROMPT_TEXT_LIMIT: usize = 12_000;

pub fn normalize_llm_advanced_settings(settings: Option<&Value>) -> Map<String, Value> {
    let loaded = decode_settings_object(settings);
    let mut normalized = Map::new();

    if let Some(reasoning_depth) = loaded
        .get("reasoning_depth")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !matches!(value.as_str(), "" | "auto" | "default" | "none"))
        .filter(|value| matches!(value.as_str(), "low" | "medium" | "high"))
    {
        normalized.insert("reasoning_depth".to_string(), json!(reasoning_depth));
    }

    if let Some(temperature) = normalize_float_setting(loaded.get("temperature"), 0.0, 2.0) {
        normalized.insert("temperature".to_string(), json!(temperature));
    }

    if let Some(max_tokens) = normalize_int_setting(loaded.get("max_tokens"), 1, 200_000) {
        normalized.insert("max_tokens".to_string(), json!(max_tokens));
    }

    for key in [
        "system_prompt",
        "classification_prompt_template",
        "rule_prompt_template",
    ] {
        if let Some(prompt) = loaded
            .get(key)
            .and_then(Value::as_str)
            .map(normalize_prompt_text)
            .filter(|value| !value.is_empty())
        {
            normalized.insert(key.to_string(), json!(prompt));
        }
    }

    normalized
}

pub fn safe_llm_config_payload(config: &Value) -> Value {
    let mut safe_config = config.as_object().cloned().unwrap_or_default();
    safe_config.insert(
        "advanced_settings".to_string(),
        Value::Object(normalize_llm_advanced_settings(
            safe_config.get("advanced_settings"),
        )),
    );
    let has_api_key = object_has_non_empty_secret(&safe_config);
    redact_secrets_in_map(&mut safe_config);
    safe_config.insert("has_api_key".to_string(), json!(has_api_key));
    safe_config
        .entry("api_key".to_string())
        .or_insert_with(|| json!(""));
    Value::Object(safe_config)
}

pub fn build_runtime_llm_config_from_saved_config(config: &Value) -> Value {
    let object = config.as_object();
    json!({
        "enabled": true,
        "provider": string_field_or(object, "provider", "openai"),
        "advanced_settings": Value::Object(normalize_llm_advanced_settings(
            object.and_then(|item| item.get("advanced_settings")),
        )),
        "provider_config": {
            "api_key": string_field_or(object, "api_key", ""),
            "base_url": string_field_or(object, "base_url", ""),
            "model": string_field_or(object, "model", ""),
        },
    })
}

pub fn copy_runtime_llm_config(config: &Value) -> Value {
    let object = config.as_object();
    let provider_config = object
        .and_then(|item| item.get("provider_config"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let advanced_source = object
        .and_then(|item| item.get("advanced_settings"))
        .filter(|value| !is_falsy_settings_value(value))
        .or_else(|| provider_config.get("advanced_settings"));

    json!({
        "enabled": object
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "provider": string_field_or(object, "provider", "openai"),
        "provider_config": provider_config,
        "advanced_settings": Value::Object(normalize_llm_advanced_settings(advanced_source)),
    })
}

pub fn build_llm_config_get_response(config: &Value) -> AiRouteResponse {
    let copied = copy_runtime_llm_config(config);
    let copied_object = copied.as_object();
    let provider_config = copied_object
        .and_then(|item| item.get("provider_config"))
        .and_then(Value::as_object);
    AiRouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": {
                "enabled": copied_object
                    .and_then(|item| item.get("enabled"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "provider": string_field_or(copied_object, "provider", "openai"),
                "model": string_field_or(provider_config, "model", ""),
                "advanced_settings": copied
                    .get("advanced_settings")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
                "available_providers": llm_available_providers(),
            },
        }),
    }
}

fn decode_settings_object(settings: Option<&Value>) -> Map<String, Value> {
    match settings {
        Some(Value::Object(object)) => object.clone(),
        Some(Value::String(text)) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default(),
        _ => Map::new(),
    }
}

fn is_falsy_settings_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(false) => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
        _ => false,
    }
}

fn normalize_float_setting(value: Option<&Value>, minimum: f64, maximum: f64) -> Option<f64> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.parse::<f64>().ok(),
        _ => None,
    }?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

fn normalize_int_setting(value: Option<&Value>, minimum: i64, maximum: i64) -> Option<i64> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse::<i64>().ok(),
        _ => None,
    }?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

fn normalize_prompt_text(value: &str) -> String {
    value.trim().chars().take(LLM_PROMPT_TEXT_LIMIT).collect()
}
