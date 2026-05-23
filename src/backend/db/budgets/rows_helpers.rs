// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


fn budget_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.get::<_, i64>("id")?));
    record.insert(
        "user_id".to_string(),
        json_i64(row.get::<_, i64>("user_id")?),
    );
    for key in [
        "name",
        "category",
        "sub_category",
        "period_type",
        "start_date",
        "end_date",
        "created_at",
        "updated_at",
    ] {
        record.insert(
            key.to_string(),
            optional_string_value(row.get::<_, Option<String>>(key)?),
        );
    }
    record.insert(
        "amount".to_string(),
        json_real(row.get::<_, f64>("amount")?),
    );
    record.insert(
        "alert_threshold".to_string(),
        json_i64(row.get::<_, Option<i64>>("alert_threshold")?.unwrap_or(80)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(row.get::<_, Option<i64>>("enabled")?.unwrap_or(1)),
    );
    Ok(record)
}

fn budget_history_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.get::<_, i64>("id")?));
    record.insert(
        "budget_id".to_string(),
        json_i64(row.get::<_, i64>("budget_id")?),
    );
    for key in [
        "period_start",
        "period_end",
        "status",
        "filter_summary",
        "calculated_at",
        "name",
        "category",
        "sub_category",
        "period_type",
    ] {
        record.insert(
            key.to_string(),
            optional_string_value(row.get::<_, Option<String>>(key)?),
        );
    }
    for key in [
        "budget_amount",
        "spent_amount",
        "remaining_amount",
        "execution_rate",
    ] {
        record.insert(
            key.to_string(),
            row.get::<_, Option<f64>>(key)?
                .map(json_real)
                .unwrap_or(Value::Null),
        );
    }
    record.insert(
        "alert_threshold".to_string(),
        json_i64(row.get::<_, Option<i64>>("alert_threshold")?.unwrap_or(80)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(row.get::<_, Option<i64>>("enabled")?.unwrap_or(1)),
    );
    Ok(record)
}

fn table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn normalize_sub_category(value: Option<&Value>) -> String {
    value_string(value).trim().to_string()
}

fn missing_required_field(payload: &BudgetRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn where_suffix(conditions: &[String]) -> String {
    if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    }
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn record_value<'a>(record: &'a BudgetRecord, key: &str) -> Option<&'a Value> {
    record.get(key)
}

fn record_text(record: &BudgetRecord, key: &str) -> String {
    value_string(record.get(key))
}

fn record_i64(record: &BudgetRecord, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn record_f64(record: &BudgetRecord, key: &str) -> Option<f64> {
    record.get(key).and_then(value_to_f64)
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
        Value::Bool(value) => Some(i64::from(*value)),
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

fn value_field_string(value: &Value, key: &str) -> String {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .map(|value| value_string(Some(value)))
        .unwrap_or_default()
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

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_date_prefix(value: &str) -> DbResult<NaiveDate> {
    let date_text = value
        .trim()
        .get(..10)
        .ok_or_else(|| DbError::InvalidOperation(format!("invalid date: {value}")))?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|_| DbError::InvalidOperation(format!("invalid date: {value}")))
}

fn field_or_null(row: &BudgetRecord, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn json_real(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::String)
}

fn json_to_sql_value(value: Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(value)),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_u64().and_then(|value| i64::try_from(value).ok())
            {
                SqlValue::Integer(value)
            } else {
                SqlValue::Real(number.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(value) => SqlValue::Text(value),
        Value::Array(_) | Value::Object(_) => SqlValue::Text(value.to_string()),
    }
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
