fn pg_record_text(record: &BillRecord, key: &str) -> String {
    pg_value_string(record.get(key).unwrap_or(&Value::Null)).unwrap_or_default()
}

fn pg_record_optional_i64(record: &BillRecord, key: &str) -> Option<i64> {
    pg_value_i64(record.get(key)?)
}

fn pg_recurring_text(record: &BillRecord, key: &str) -> String {
    pg_record_text(record, key)
}

fn pg_recurring_i64(record: &BillRecord, key: &str) -> Option<i64> {
    pg_record_optional_i64(record, key)
}

fn pg_value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
    .filter(|value| !value.is_empty())
}

fn pg_value_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_f64().map(|value| value as i64)),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        Value::Bool(value) => Some(i64::from(*value)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn pg_normalize_template_transaction_type(value: Option<&Value>) -> i64 {
    let text = match value {
        Some(Value::String(value)) => value.trim().to_ascii_lowercase(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => return 3,
    };
    match text.as_str() {
        "2" | "income" | "收入" => 2,
        "3" | "expense" | "支出" => 3,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn pg_recurring_optional_text_json(record: &BillRecord, key: &str) -> Value {
    pg_value_string(record.get(key).unwrap_or(&Value::Null))
        .map(Value::String)
        .unwrap_or(Value::Null)
}

fn optional_i64_json(value: Option<i64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn pg_optional_string_json(value: Option<String>) -> Value {
    value
        .filter(|value| !value.trim().is_empty())
        .map(Value::String)
        .unwrap_or(Value::Null)
}

fn non_empty_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}

fn optional_i64_from_text(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|value| *value > 0)
}

fn utc_today_text() -> String {
    Utc::now().date_naive().to_string()
}
