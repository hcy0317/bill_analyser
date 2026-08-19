fn postgres_budget_forecast_group_expr(period_type: &str) -> &'static str {
    match period_type {
        "daily" => "to_char(b.occurred_at, 'YYYY-MM-DD')",
        "weekly" => "to_char(b.occurred_at, 'YYYY-WW')",
        "quarterly" => {
            "to_char(b.occurred_at, 'YYYY') || '-Q' || EXTRACT(QUARTER FROM b.occurred_at)::INT"
        }
        "monthly" => "to_char(b.occurred_at, 'YYYY-MM')",
        _ => "to_char(b.occurred_at, 'YYYY')",
    }
}

fn canonical_budget_transaction_type(type_name: &str) -> String {
    match type_name.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => "income".to_string(),
        "支出" | "expense" | "3" => "expense".to_string(),
        "投资" | "investment" | "5" => "investment".to_string(),
        value => value.to_string(),
    }
}

fn budget_record_from_postgres_row(row: PgRow) -> DbResult<BudgetRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.try_get("id")?));
    record.insert("user_id".to_string(), json_i64(row.try_get("user_id")?));
    for key in ["name", "category", "sub_category", "period_type"] {
        record.insert(
            key.to_string(),
            optional_string_value(row.try_get::<Option<String>, _>(key)?),
        );
    }
    record.insert(
        "start_date".to_string(),
        row.try_get::<NaiveDate, _>("start_date")?
            .to_string()
            .into(),
    );
    record.insert(
        "end_date".to_string(),
        row.try_get::<Option<NaiveDate>, _>("end_date")?
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );
    let amount_cents: i64 = row.try_get("amount_cents")?;
    record.insert("amount_cents".to_string(), json_i64(amount_cents));
    record.insert(
        "alert_threshold".to_string(),
        json_i64(i64::from(row.try_get::<i32, _>("alert_threshold")?)),
    );
    record.insert(
        "enabled".to_string(),
        json_i64(i64::from(row.try_get::<bool, _>("enabled")?)),
    );
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    Ok(record)
}

fn record_value<'a>(record: &'a BudgetRecord, key: &str) -> Option<&'a Value> {
    record.get(key)
}

fn record_text(record: &BudgetRecord, key: &str) -> String {
    value_string(record.get(key))
}

fn normalize_sub_category(value: Option<&Value>) -> String {
    value_string(value).trim().to_string()
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

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::String)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn json_real(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn insert_timestamp(record: &mut BudgetRecord, key: &str, value: DateTime<Utc>) {
    record.insert(
        key.to_string(),
        Value::String(value.to_rfc3339_opts(SecondsFormat::Secs, true)),
    );
}
