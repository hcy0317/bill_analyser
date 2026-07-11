fn normalize_id_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) if text.is_empty() || text == "0" => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) if json_number_is_zero(number) => String::new(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Bool(true)) => "true".to_string(),
        Some(value) => value.to_string(),
    }
}

fn integer_lookup_key(value: Option<&Value>) -> Option<i64> {
    let text = normalize_id_text(value);
    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        text.parse::<i64>().ok()
    } else {
        None
    }
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a String>) -> Option<&'a str> {
    values
        .into_iter()
        .map(|value| value.as_str())
        .find(|value| !value.is_empty())
}

fn integer_field(value: &Value, key: &str) -> i64 {
    value
        .as_object()
        .map(|object| integer_field_from_map(object, key))
        .unwrap_or_default()
}

fn integer_field_from_map(map: &Map<String, Value>, key: &str) -> i64 {
    match map.get(key) {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1,
        _ => 0,
    }
}

fn float_field_from_map(map: &Map<String, Value>, key: &str) -> f64 {
    match map.get(key) {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .as_object()
        .map(|object| string_field_from_map(object, key))
        .unwrap_or_default()
}

fn string_field_from_map(map: &Map<String, Value>, key: &str) -> String {
    match map.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) if !json_number_is_zero(number) => number.to_string(),
        Some(Value::Bool(true)) => "true".to_string(),
        _ => String::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn list_field_from_map(map: &Map<String, Value>, key: &str) -> Vec<Value> {
    match map.get(key) {
        Some(Value::Array(values)) => values.clone(),
        _ => Vec::new(),
    }
}

fn object_field_from_map<'a>(
    map: &'a Map<String, Value>,
    key: &str,
) -> Option<&'a Map<String, Value>> {
    object_field(map.get(key))
}

fn object_field(value: Option<&Value>) -> Option<&Map<String, Value>> {
    match value {
        Some(Value::Object(object)) => Some(object),
        _ => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：把 dedup source ids 的逗号字符串或数组表示归一为 JSON 数组，供筛选索引和 matching payload 复用。
fn parse_dedup_source_ids(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::String(text)) => text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                if part.bytes().all(|byte| byte.is_ascii_digit()) {
                    part.parse::<i64>().map_or_else(
                        |_| Value::String(part.to_string()),
                        |number| Value::Number(Number::from(number)),
                    )
                } else {
                    Value::String(part.to_string())
                }
            })
            .collect(),
        Some(Value::Array(values)) => values.clone(),
        _ => Vec::new(),
    }
}

fn value_to_trimmed_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
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

fn json_value_is_truthy(value: &Value) -> bool {
    // 中文说明：统一导入预览旧字段中的 bool/number/string/list/object 真值判断，避免 selected 与 signal 状态分歧。
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => !json_number_is_zero(number),
        Value::String(text) => !text.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn json_number_is_zero(number: &Number) -> bool {
    number.as_i64() == Some(0) || number.as_u64() == Some(0) || number.as_f64() == Some(0.0)
}
