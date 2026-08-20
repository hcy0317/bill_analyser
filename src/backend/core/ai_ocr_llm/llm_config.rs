// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Map, Value};

use super::provider_auth::{normalize_provider_auth_config, provider_auth_access_token};
use super::secret_redaction::{object_has_non_empty_secret, redact_secrets_in_map};
use super::value_helpers::string_field_or;

const LLM_PROMPT_TEXT_LIMIT: usize = 12_000;

/// 归一化 LLM 高级参数，只保留受支持且有边界限制的推理深度、温度、token 和 prompt 模板。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 生成可返回给前端的安全 LLM 配置，递归脱敏密钥并保留 has_api_key 状态。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 将数据库保存配置投影成运行时配置，合并 credential_config 与旧 api_key 字段。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_runtime_llm_config_from_saved_config(config: &Value) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_runtime_llm_config_from_saved_config",
        "business operation entered"
    );
    let object = config.as_object();
    let auth_profile =
        normalize_provider_auth_config(object.and_then(|item| item.get("credential_config")));
    let direct_api_key = string_field_or(object, "api_key", "");
    let resolved_token = provider_auth_access_token(&auth_profile).unwrap_or(direct_api_key);
    json!({
        "enabled": true,
        "id": object.and_then(|item| item.get("id")).cloned().unwrap_or(Value::Null),
        "user_id": object.and_then(|item| item.get("user_id")).cloned().unwrap_or(Value::Null),
        "provider": string_field_or(object, "provider", "openai"),
        "advanced_settings": Value::Object(normalize_llm_advanced_settings(
            object.and_then(|item| item.get("advanced_settings")),
        )),
        "credential_config": auth_profile,
        "provider_config": {
            "api_key": resolved_token,
            "base_url": string_field_or(object, "base_url", ""),
            "model": string_field_or(object, "model", ""),
        },
    })
}

/// 复制并规范化运行时 LLM 配置，兼容 provider_config 内旧 advanced_settings 位置。
#[tracing::instrument(level = "debug", skip_all)]
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
        "id": object.and_then(|item| item.get("id")).cloned().unwrap_or(Value::Null),
        "user_id": object.and_then(|item| item.get("user_id")).cloned().unwrap_or(Value::Null),
        "provider": string_field_or(object, "provider", "openai"),
        "provider_config": provider_config,
        "credential_config": object
            .and_then(|item| item.get("credential_config"))
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new())),
        "advanced_settings": Value::Object(normalize_llm_advanced_settings(advanced_source)),
    })
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

/// 判断高级配置值是否等价为空，避免旧 provider_config 中的空值覆盖有效设置。
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

/// 解析并限制浮点型高级参数，非法或越界值直接忽略。
#[tracing::instrument(level = "debug", skip_all)]
fn normalize_float_setting(value: Option<&Value>, minimum: f64, maximum: f64) -> Option<f64> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.parse::<f64>().ok(),
        _ => None,
    }?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

/// 解析并限制整数型高级参数，避免外部 provider 请求使用异常 token 上限。
#[tracing::instrument(level = "debug", skip_all)]
fn normalize_int_setting(value: Option<&Value>, minimum: i64, maximum: i64) -> Option<i64> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse::<i64>().ok(),
        _ => None,
    }?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

/// 截断自定义 prompt 模板，避免配置层把超长文本送入运行时请求。
#[tracing::instrument(level = "debug", skip_all)]
fn normalize_prompt_text(value: &str) -> String {
    value.trim().chars().take(LLM_PROMPT_TEXT_LIMIT).collect()
}
