fn matching_history_pair_key(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let pair_id = first_value(object, &["id"])
        .and_then(value_to_i64)
        .unwrap_or_default();
    if pair_id > 0 {
        return Some(format!("pair:{pair_id}"));
    }
    let left = first_value(object, &["leftBillId", "left_bill_id"]).and_then(value_to_i64)?;
    let right = first_value(object, &["rightBillId", "right_bill_id"]).and_then(value_to_i64)?;
    Some(format!("bills:{}:{}", left.min(right), left.max(right)))
}

fn first_value<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn non_empty_value_string(value: &Value) -> Option<String> {
    value_to_text(value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<f64>().ok()
            }
        }
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_preview_type_text(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "expense" => "支出".to_string(),
        "income" => "收入".to_string(),
        "transfer" => "转账".to_string(),
        "investment" => "投资".to_string(),
        _ => value.to_string(),
    }
}

fn response_mode_is_preview_item(object: &Map<String, Value>) -> bool {
    first_value(object, &["responseMode", "response_mode"]).is_some_and(|value| {
        value
            .as_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("preview-item"))
    })
}

fn match_reasons_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(value_to_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect(),
        value => value_to_text(value)
            .unwrap_or_default()
            .split(['|', ','])
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
    }
}
