// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const FEATURE_SCHEMA_VERSION: &str = "import-learning-features-v1";
pub const DEFAULT_FEATURE_DIMENSION: usize = 96;
pub const MODEL_KEY: &str = "import-learning-dual-head";
pub const MODEL_FAMILY: &str = "shared-hidden-dual-softmax-v1";
pub const MIN_TRAINING_SAMPLES: usize = 3;
pub const HIDDEN_DIMENSION: usize = 16;
pub const POLICY_VERSION: &str = "learning-green-blue-policy-v1";
pub const GREEN_CONFIDENCE_THRESHOLD: f64 = 0.52;
pub const GREEN_MARGIN_THRESHOLD: f64 = 0.02;
pub const BLUE_CONFIDENCE_THRESHOLD: f64 = 0.70;
pub const BLUE_MARGIN_THRESHOLD: f64 = 0.05;
pub const BLUE_ACCEPT_CONFIRMATION_THRESHOLD: i64 = 2;

mod features;
mod llm_memory;
mod model_registry;
mod policy;
mod route_response;

pub use features::*;
pub use llm_memory::*;
pub use model_registry::*;
pub use policy::*;
pub use route_response::*;

fn iter_text_tokens(field: &str, value: &str) -> Vec<String> {
    let normalized_value = normalize_learning_text(Some(&Value::String(value.to_string())));
    if normalized_value.is_empty() {
        return Vec::new();
    }
    let mut tokens = vec![format!("{field}={normalized_value}")];
    for part in split_learning_token_parts(&normalized_value) {
        tokens.push(format!("{field}:tok={part}"));
        let chars: Vec<char> = part.chars().collect();
        if chars.len() >= 2 {
            for index in 0..(chars.len() - 1) {
                tokens.push(format!("{}:bi={}{}", field, chars[index], chars[index + 1]));
            }
        }
    }
    tokens
}

fn split_learning_token_parts(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '|' | ','
                        | '，'
                        | '/'
                        | '、'
                        | '_'
                        | '-'
                        | ':'
                        | '：'
                        | ';'
                        | '；'
                        | '('
                        | ')'
                        | '（'
                        | '）'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                )
        })
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn snapshot_payload(value: Option<&Value>) -> Map<String, Value> {
    match value {
        Some(Value::Object(object)) => object.clone(),
        Some(Value::String(text)) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| match value {
                Value::Object(object) => Some(object),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Map::new(),
    }
}

fn value_to_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn optional_positive_i64(value: Option<&Value>) -> Option<i64> {
    optional_nonnegative_i64(value).filter(|value| *value > 0)
}

fn optional_nonnegative_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.trim().is_empty() || text.trim() == "0" => None,
        Some(Value::Number(number)) if number.as_i64() == Some(0) || number.as_u64() == Some(0) => {
            None
        }
        Some(value) => Some(value_to_i64(value).max(0)),
    }
}

fn value_to_i64(value: &Value) -> i64 {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or_default(),
        Value::String(text) => text.trim().parse::<i64>().unwrap_or_default(),
        Value::Bool(true) => 1,
        _ => 0,
    }
}

fn coerce_json_int(value: &Value) -> Result<i64, String> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .ok_or_else(|| "cannot convert value to int".to_string()),
        Value::String(text) => text
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("invalid literal for int() with base 10: '{}'", text)),
        Value::Bool(value) => Ok(if *value { 1 } else { 0 }),
        Value::Null => Err(
            "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
                .to_string(),
        ),
        Value::Array(_) => Err(
            "int() argument must be a string, a bytes-like object or a real number, not 'list'"
                .to_string(),
        ),
        Value::Object(_) => Err(
            "int() argument must be a string, a bytes-like object or a real number, not 'dict'"
                .to_string(),
        ),
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}
