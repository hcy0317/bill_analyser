fn canonical_transaction_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => "income",
        "支出" | "expense" | "3" => "expense",
        "转账" | "transfer" | "4" => "transfer",
        "投资" | "investment" | "5" => "investment",
        other => other,
    }
    .to_string()
}

fn category_names_from_path(path: Option<&str>, name: &str) -> (String, String) {
    let parts = path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

fn destination_amount_cents(payload: &Value) -> Option<i64> {
    payload
        .get("destination_amount_cents")
        .or_else(|| payload.get("destinationAmountCents"))
        .and_then(value_to_i64)
}

fn payload_string_value(payload: &Value, key: &str) -> Value {
    payload
        .get(key)
        .and_then(value_string)
        .map_or(Value::Null, Value::String)
}

fn payload_i64_value(payload: &Value, key: &str) -> Value {
    payload
        .get(key)
        .and_then(value_to_i64)
        .map_or(Value::Null, |value| Value::Number(Number::from(value)))
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn optional_value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(value_string)
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn first_positive(values: [Option<i64>; 2]) -> Option<i64> {
    values.into_iter().flatten().find(|value| *value > 0)
}

fn normalize_ids(values: &[i64]) -> Vec<i64> {
    let mut values = values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn postgres_initial_balance_money(metadata: &Value, balance_cents: i64) -> DbResult<Money> {
    let fallback = Money::from_cents(balance_cents);
    if let Some(value) = metadata.get("initial_balance_cents") {
        return match value {
            Value::Number(number) => number
                .as_i64()
                .map(Money::from_cents)
                .map_or(Ok(fallback), Ok),
            Value::String(text) if !text.trim().is_empty() => text
                .trim()
                .parse::<i64>()
                .map(Money::from_cents)
                .map_err(|error| DbError::InvalidOperation(error.to_string())),
            _ => Ok(fallback),
        };
    }
    Ok(fallback)
}

fn optional_string_value(value: Option<String>) -> Value {
    value
        .filter(|value| !value.trim().is_empty())
        .map_or(Value::Null, Value::String)
}

fn json_i64(value: i64) -> Value {
    json!(value)
}

fn insert_timestamp(record: &mut Map<String, Value>, key: &str, timestamp: DateTime<Utc>) {
    record.insert(
        key.to_string(),
        Value::String(
            timestamp
                .naive_utc()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        ),
    );
}
