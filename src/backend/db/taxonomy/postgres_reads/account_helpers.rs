fn account_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, name, account_type, payment_method, currency,
            balance_cents, is_active, display_order, metadata, created_at, updated_at
        FROM accounts
        {where_clause}
        "#
    )
}

fn account_metadata_from_payload(existing: Option<&Value>, payload: &Value) -> DbResult<Value> {
    let mut metadata = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for key in [
        "category",
        "icon",
        "color",
        "comment",
        "parent_id",
        "credit_card_statement_date",
    ] {
        if let Some(value) = payload.get(key) {
            metadata.insert(key.to_string(), value.clone());
        }
    }
    if let Some(value) = payload
        .get("initialBalanceCents")
        .or_else(|| payload.get("initial_balance_cents"))
    {
        let initial_balance_cents = strict_minor_units_value(value, "initialBalanceCents")?;
        metadata.insert(
            "initial_balance_cents".to_string(),
            Value::Number(Number::from(initial_balance_cents)),
        );
    }
    Ok(Value::Object(metadata))
}

fn metadata_with_parent_id(metadata: Value, parent_id: Option<i64>) -> Value {
    let Some(parent_id) = parent_id else {
        return metadata;
    };
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    object.insert(
        "parent_id".to_string(),
        Value::Number(Number::from(parent_id)),
    );
    Value::Object(object)
}
