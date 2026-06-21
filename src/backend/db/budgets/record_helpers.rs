fn normalize_sub_category(value: Option<&Value>) -> String {
    value_string(value).trim().to_string()
}

fn missing_required_field(payload: &BudgetRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn record_text(record: &BudgetRecord, key: &str) -> String {
    value_string(record.get(key))
}

fn record_i64(record: &BudgetRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        Value::Bool(_) => None,
        _ => None,
    }
}

fn value_field_string(value: &Value, key: &str) -> String {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .map(|value| value_string(Some(value)))
        .unwrap_or_default()
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn field_or_null(row: &BudgetRecord, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
