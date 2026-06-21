fn parse_postgres_bill_datetime(value: String) -> DbResult<DateTime<Utc>> {
    let parsed = parse_bill_datetime(&value)
        .ok_or_else(|| DbError::InvalidOperation("invalid bill date time".to_string()))?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(
        parsed.inner(),
        Utc,
    ))
}

fn required_text(record: &BillRecord, key: &str) -> DbResult<String> {
    optional_value_string(record.get(key))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DbError::InvalidOperation(format!("missing required bill field: {key}")))
}

fn amount_cents_from_record(record: &BillRecord, key: &str) -> DbResult<i64> {
    let value = record
        .get(key)
        .and_then(value_to_i64)
        .ok_or_else(|| DbError::InvalidOperation(format!("missing required bill field: {key}")))?;
    Ok(value)
}

fn postgres_direction_for_type(transaction_type: &str) -> &'static str {
    if transaction_type == "income" {
        "income"
    } else {
        "expense"
    }
}

fn update_requires_hash_recalculation(fields: &BillRecord) -> bool {
    [
        "date",
        "type",
        "amount_cents",
        "counterparty",
        "description",
    ]
    .iter()
    .any(|key| fields.contains_key(*key))
}
