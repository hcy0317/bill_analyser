fn bill_record_from_postgres_row(row: PgRow) -> DbResult<BillRecord> {
    let standard_payload: Value = row.try_get("standard_payload")?;
    let amount_cents: i64 = row.try_get("amount_cents")?;
    let destination_amount_cents = destination_amount_cents(&standard_payload);
    let source_account_id = first_positive([
        row.try_get::<Option<i64>, _>("source_account_id")?,
        row.try_get::<Option<i64>, _>("account_id")?,
    ]);
    let destination_account_id = first_positive([
        row.try_get::<Option<i64>, _>("target_account_id")?,
        row.try_get::<Option<i64>, _>("transfer_target_account_id")?,
    ]);

    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.try_get("id")?));
    record.insert("user_id".to_string(), json_i64(row.try_get("user_id")?));
    insert_timestamp(&mut record, "date", row.try_get("occurred_at")?);
    record.insert(
        "type".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("transaction_type")?),
    );
    record.insert("amount_cents".to_string(), json_i64(amount_cents));
    record.insert(
        "counterparty".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("merchant")?),
    );
    record.insert(
        "description".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("description")?),
    );
    record.insert(
        "payment_method".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("payment_method")?),
    );
    record.insert(
        "main_category".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("main_category")?),
    );
    record.insert(
        "sub_category".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("sub_category")?),
    );
    record.insert(
        "batch_id".to_string(),
        payload_string_value(&standard_payload, "batch_id"),
    );
    record.insert(
        "hash".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("source_hash")?),
    );
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    record.insert(
        "source_account_id".to_string(),
        json_i64(source_account_id.unwrap_or_default()),
    );
    record.insert(
        "destination_account_id".to_string(),
        json_i64(destination_account_id.unwrap_or_default()),
    );
    record.insert(
        "destination_amount_cents".to_string(),
        json_i64(destination_amount_cents.unwrap_or_default()),
    );
    record.insert(
        "category_id".to_string(),
        row.try_get::<Option<i64>, _>("category_id")?
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );
    for key in [
        "created_from_template",
        "created_from_recurring",
        "import_history_id",
    ] {
        record.insert(key.to_string(), payload_i64_value(&standard_payload, key));
    }
    Ok(record)
}

fn tag_value_from_postgres_row(row: PgRow) -> DbResult<Value> {
    let metadata: Value = row.try_get("metadata")?;
    let mut tag = Map::new();
    tag.insert(
        "id".to_string(),
        row.try_get::<i64, _>("id")?.to_string().into(),
    );
    tag.insert("name".to_string(), row.try_get::<String, _>("name")?.into());
    tag.insert(
        "color".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("color")?),
    );
    tag.insert(
        "icon".to_string(),
        metadata
            .get("icon")
            .and_then(Value::as_str)
            .map_or(Value::Null, |value| Value::String(value.to_string())),
    );
    Ok(Value::Object(tag))
}
