async fn prepare_postgres_bill_mutation(
    pool: &PostgresPool,
    user_id: i64,
    fields: &BillRecord,
) -> DbResult<PostgresBillMutation> {
    let occurred_at = parse_postgres_bill_datetime(required_text(fields, "date")?)?;
    let transaction_type = canonical_transaction_type(&required_text(fields, "type")?);
    let direction = postgres_direction_for_type(&transaction_type).to_string();
    let amount_cents = amount_cents_from_record(fields, "amount_cents")?.abs();
    let source_account_id =
        optional_positive_id_from_fields(fields, &["source_account_id", "sourceAccountId"])?;
    let destination_account_id = if bill_type_uses_destination_account(&transaction_type) {
        optional_positive_id_from_fields(
            fields,
            &["destination_account_id", "destinationAccountId"],
        )?
    } else {
        None
    };
    validate_postgres_bill_account_identity(
        pool,
        user_id,
        &transaction_type,
        source_account_id,
        destination_account_id,
    )
    .await?;
    let category_id =
        resolve_postgres_category_id_for_fields(pool, user_id, fields, &transaction_type).await?;
    let mut standard_payload = Value::Object(fields.clone());
    if let Value::Object(payload) = &mut standard_payload {
        payload.insert(
            "main_category".to_string(),
            fields
                .get("main_category")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        payload.insert(
            "sub_category".to_string(),
            fields
                .get("sub_category")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        payload.insert(
            "destination_amount_cents".to_string(),
            fields
                .get("destination_amount_cents")
                .cloned()
                .unwrap_or_else(|| json_i64(0)),
        );
        if !bill_type_uses_destination_account(&transaction_type) {
            payload.remove("destination_account_id");
            payload.remove("destinationAccountId");
        }
    }
    let source_hash = match optional_value_string(fields.get("hash")) {
        Some(value) => Some(value),
        None => Some(calculate_bill_hash_from_record(fields)?),
    };
    Ok(PostgresBillMutation {
        occurred_at,
        amount_cents,
        direction,
        transaction_type,
        source_account_id,
        destination_account_id,
        category_id,
        merchant: optional_value_string(fields.get("counterparty")),
        description: optional_value_string(fields.get("description")),
        payment_method: optional_value_string(fields.get("payment_method")),
        source_hash,
        standard_payload,
    })
}

async fn resolve_postgres_category_id_for_fields(
    pool: &PostgresPool,
    user_id: i64,
    fields: &BillRecord,
    transaction_type: &str,
) -> DbResult<Option<i64>> {
    if let Some(category_id) = explicit_category_id_from_fields(fields)? {
        return resolve_postgres_category_id_by_id(pool, user_id, category_id, transaction_type)
            .await
            .map(Some);
    }
    Ok(None)
}

async fn resolve_postgres_category_id_by_id(
    pool: &PostgresPool,
    user_id: i64,
    category_id: i64,
    transaction_type: &str,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT id, category_type
        FROM categories
        WHERE user_id = $1 AND id = $2 AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::InvalidOperation(format!("category not found: {category_id}")))?;
    let category_type = row
        .try_get::<Option<String>, _>("category_type")?
        .as_deref()
        .and_then(postgres_category_type_code);
    if !postgres_category_type_matches_transaction_type(category_type, transaction_type) {
        return Err(DbError::InvalidOperation(format!(
            "category type mismatch: {category_id}"
        )));
    }
    row.try_get("id").map_err(DbError::from)
}

fn explicit_category_id_from_fields(fields: &BillRecord) -> DbResult<Option<i64>> {
    optional_positive_id_from_fields(fields, &["category_id", "categoryId"])
}

fn optional_positive_id_from_fields(fields: &BillRecord, keys: &[&str]) -> DbResult<Option<i64>> {
    for key in keys {
        let Some(value) = fields.get(*key) else {
            continue;
        };
        if value.is_null() {
            return Ok(None);
        }
        let Some(id) = value_to_i64(value) else {
            return Err(DbError::InvalidOperation(format!(
                "invalid identifier field: {key}"
            )));
        };
        if id <= 0 {
            return Err(DbError::InvalidOperation(format!(
                "invalid non-positive identifier field: {key}"
            )));
        }
        return Ok(Some(id));
    }
    Ok(None)
}

async fn validate_postgres_bill_account_identity(
    pool: &PostgresPool,
    user_id: i64,
    transaction_type: &str,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
) -> DbResult<()> {
    if let Some(account_id) = source_account_id {
        ensure_active_postgres_account_id(pool, user_id, account_id, "source_account_id").await?;
    }
    if !bill_type_uses_destination_account(transaction_type) && destination_account_id.is_some() {
        return Err(DbError::InvalidOperation(format!(
            "destination_account_id is not allowed for transaction type: {transaction_type}"
        )));
    }
    if let Some(account_id) = destination_account_id {
        ensure_active_postgres_account_id(pool, user_id, account_id, "destination_account_id")
            .await?;
    }
    if bill_type_uses_destination_account(transaction_type)
        && source_account_id.is_some()
        && destination_account_id.is_some()
        && source_account_id == destination_account_id
    {
        return Err(DbError::InvalidOperation(
            "source and destination accounts must differ".to_string(),
        ));
    }
    Ok(())
}

fn bill_type_uses_destination_account(transaction_type: &str) -> bool {
    matches!(transaction_type, "transfer" | "investment")
}

async fn ensure_active_postgres_account_id(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
    field: &str,
) -> DbResult<()> {
    let exists = sqlx::query(
        "SELECT 1 FROM accounts WHERE user_id = $1 AND id = $2 AND is_active = true LIMIT 1",
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await?
    .is_some();
    if exists {
        return Ok(());
    }
    Err(DbError::InvalidOperation(format!(
        "{field} not found: {account_id}"
    )))
}

fn postgres_category_type_matches_transaction_type(
    category_type: Option<i64>,
    transaction_type: &str,
) -> bool {
    let Some(category_type) = category_type else {
        return true;
    };
    if matches!(category_type, 0 | 1) {
        return true;
    }
    postgres_transaction_type_code(transaction_type).is_none_or(|value| value == category_type)
}

fn postgres_category_type_code(value: &str) -> Option<i64> {
    match value.trim().to_ascii_lowercase().as_str() {
        "2" | "income" | "收入" => Some(2),
        "3" | "expense" | "支出" => Some(3),
        "4" | "transfer" | "转账" => Some(4),
        "5" | "investment" | "投资" => Some(5),
        _ => None,
    }
}

fn postgres_transaction_type_code(value: &str) -> Option<i64> {
    match value.trim().to_ascii_lowercase().as_str() {
        "income" | "收入" | "2" => Some(2),
        "expense" | "支出" | "3" => Some(3),
        "transfer" | "转账" | "4" => Some(4),
        "investment" | "投资" | "5" => Some(5),
        _ => None,
    }
}
