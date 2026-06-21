fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}

fn record_i64(record: &BudgetRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn value_field_i64(value: &Value, key: &str) -> Option<i64> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(value_to_i64)
}

fn value_field_f64(value: &Value, key: &str) -> Option<f64> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(value_to_f64)
}

fn record_amount_cents(record: &BudgetRecord) -> Option<i64> {
    record.get("amount_cents").and_then(value_to_i64)
}

fn amount_cents_from_record(record: &BudgetRecord, key: &str) -> DbResult<i64> {
    let value = record.get(key).ok_or_else(|| {
        DbError::InvalidOperation(format!("missing required budget field: {key}"))
    })?;
    amount_cents_from_value(value)
}

fn amount_cents_from_value(value: &Value) -> DbResult<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| DbError::InvalidOperation("invalid budget amount".to_string())),
        Value::String(value) => value
            .trim()
            .parse::<i64>()
            .map_err(|_| DbError::InvalidOperation("invalid budget amount".to_string())),
        Value::Bool(_) => Err(DbError::InvalidOperation(
            "invalid budget amount".to_string(),
        )),
        _ => Err(DbError::InvalidOperation(
            "invalid budget amount".to_string(),
        )),
    }
}

fn push_postgres_i64_bind_list(builder: &mut QueryBuilder<'_, Postgres>, values: &[i64]) {
    let mut separated = builder.separated(", ");
    for value in values {
        separated.push_bind(*value);
    }
}

fn required_text(record: &BudgetRecord, key: &str) -> DbResult<String> {
    optional_text(record.get(key))
        .ok_or_else(|| DbError::InvalidOperation(format!("missing required budget field: {key}")))
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .map(|value| value_string(Some(value)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_date(record: &BudgetRecord, key: &str) -> DbResult<NaiveDate> {
    parse_date_prefix(&required_text(record, key)?)
}

fn optional_date(value: Option<&Value>) -> DbResult<Option<NaiveDate>> {
    optional_text(value)
        .map(|value| parse_date_prefix(&value))
        .transpose()
}

fn parse_date_prefix(value: &str) -> DbResult<NaiveDate> {
    let date_text = value
        .trim()
        .get(..10)
        .ok_or_else(|| DbError::InvalidOperation(format!("invalid date: {value}")))?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|_| DbError::InvalidOperation(format!("invalid date: {value}")))
}

fn i32_value(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(value_to_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn bool_value(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Number(value)) => value.as_i64().map(|value| value != 0),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn field_or_null(row: &BudgetRecord, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
