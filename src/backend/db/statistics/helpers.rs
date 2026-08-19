fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn canonical_postgres_transaction_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => "income",
        "支出" | "expense" | "3" => "expense",
        "转账" | "transfer" | "4" => "transfer",
        "投资" | "investment" | "5" => "investment",
        other => other,
    }
    .to_string()
}

fn postgres_category_names_from_path(path: Option<&str>, name: &str) -> (String, String) {
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

fn postgres_value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn postgres_value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn signed_postgres_statistics_amount_cents(
    transaction_type: &str,
    amount_cents: i64,
) -> DbResult<i64> {
    let normalized = transaction_type.trim().to_ascii_lowercase();
    let amount = amount_cents
        .checked_abs()
        .ok_or_else(|| DbError::InvalidOperation("invalid statistics amount_cents".to_string()))?;
    Ok(match normalized.as_str() {
        "income" | "收入" | "2" => amount,
        "expense" | "支出" | "3" | "transfer" | "转账" | "4" | "investment" | "投资" | "5" => {
            -amount
        }
        _ => amount_cents,
    })
}

fn postgres_initial_balance_cents(metadata: &Value, balance_cents: i64) -> i64 {
    metadata
        .get("initial_balance_cents")
        .and_then(postgres_value_to_i64)
        .unwrap_or(balance_cents)
}

fn postgres_timestamp_text(timestamp: DateTime<Utc>) -> String {
    timestamp
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn positive_i64(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

fn decimal_number_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

fn effective_date_timestamp(value: &str) -> Option<i64> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(timestamp) = text.parse::<i64>() {
        return Some(timestamp);
    }
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(datetime.timestamp());
    }
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S") {
        return Some(datetime.and_utc().timestamp());
    }
    NaiveDate::parse_from_str(text.get(0..10)?, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|datetime| datetime.and_utc().timestamp())
}
