fn bill_record_from_postgres_row(row: PgRow) -> DbResult<BillRecord> {
    let standard_payload: Value = row.try_get("standard_payload")?;
    let category_path: Option<String> = row.try_get("category_path")?;
    let category_name: Option<String> = row.try_get("category_name")?;
    let (main_category, sub_category) = bill_category_names(
        &standard_payload,
        category_path.as_deref(),
        category_name.as_deref().unwrap_or_default(),
    );
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
        optional_string_value(Some(main_category)),
    );
    record.insert(
        "sub_category".to_string(),
        optional_string_value(Some(sub_category)),
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

fn bill_category_names(
    standard_payload: &Value,
    category_path: Option<&str>,
    category_name: &str,
) -> (String, String) {
    let (path_main, path_sub) =
        category_names_from_postgres_path(category_path, category_name);
    let main_category = standard_payload
        .get("main_category")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map_or(path_main, ToOwned::to_owned);
    let sub_category = standard_payload
        .get("sub_category")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map_or(path_sub, ToOwned::to_owned);
    (main_category, sub_category)
}

fn bill_page_from_postgres_rows((rows, total): (Vec<PgRow>, i64)) -> DbResult<BillPage> {
    let bills = rows
        .into_iter()
        .map(bill_record_from_postgres_row)
        .collect::<DbResult<Vec<_>>>()?;
    Ok(BillPage { bills, total })
}

fn reconciliation_bill_page_from_postgres_rows(
    (rows, total): (Vec<PgRow>, i64),
) -> DbResult<PostgresReconciliationBillPage> {
    let rows = rows
        .into_iter()
        .map(|row| {
            let ledger_bill = reconciliation_bill_from_postgres_row(&row)?;
            let bill_id = row.try_get("id")?;
            let frontend_record = bill_record_from_postgres_row(row)?;
            Ok(PostgresReconciliationBillRow {
                bill_id,
                ledger_bill,
                frontend_record,
            })
        })
        .collect::<DbResult<Vec<_>>>()?;
    Ok(PostgresReconciliationBillPage { rows, total })
}

fn reconciliation_bill_from_postgres_row(row: &PgRow) -> DbResult<ReconciliationBill> {
    let standard_payload: Value = row.try_get("standard_payload")?;
    let occurred_at: DateTime<Utc> = row.try_get("occurred_at")?;
    let transaction_type = row
        .try_get::<Option<String>, _>("transaction_type")?
        .as_deref()
        .and_then(|value| TransactionType::from_backend_name(value).ok());
    let source_account_id = first_positive([
        row.try_get::<Option<i64>, _>("source_account_id")?,
        row.try_get::<Option<i64>, _>("account_id")?,
    ]);
    let destination_account_id = first_positive([
        row.try_get::<Option<i64>, _>("target_account_id")?,
        row.try_get::<Option<i64>, _>("transfer_target_account_id")?,
    ]);
    Ok(ReconciliationBill {
        id: row.try_get::<i64, _>("id")?.to_string(),
        date: occurred_at
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        transaction_type,
        amount: Money::from_cents(row.try_get("amount_cents")?),
        destination_amount: destination_amount_cents(&standard_payload).map(Money::from_cents),
        source_account_id,
        destination_account_id,
    })
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
