fn prepare_postgres_bill_mutation(fields: &BillRecord) -> DbResult<PostgresBillMutation> {
    let occurred_at = parse_postgres_bill_datetime(required_text(fields, "date")?)?;
    let transaction_type = canonical_transaction_type(&required_text(fields, "type")?);
    let direction = postgres_direction_for_type(&transaction_type).to_string();
    let amount_cents = amount_cents_from_record(fields, "amount_cents")?
        .checked_abs()
        .ok_or_else(|| DbError::InvalidOperation("invalid bill amount_cents".to_string()))?;
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
    let category_id = explicit_category_id_from_fields(fields)?;
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
        let destination_amount_cents = match fields.get("destination_amount_cents") {
            Some(value) => value_to_i64(value).ok_or_else(|| {
                DbError::InvalidOperation("invalid bill destination_amount_cents".to_string())
            })?,
            None => 0,
        }
            .checked_abs()
            .ok_or_else(|| {
                DbError::InvalidOperation("invalid bill destination_amount_cents".to_string())
            })?;
        payload.insert(
            "destination_amount_cents".to_string(),
            json_i64(destination_amount_cents),
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

async fn lock_and_validate_postgres_bill_mutations_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    mutations: &[&PostgresBillMutation],
) -> DbResult<()> {
    let category_ids = mutations
        .iter()
        .filter_map(|mutation| mutation.category_id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut locked_categories = std::collections::BTreeMap::new();
    if !category_ids.is_empty() {
        let rows = sqlx::query(
            r#"
            SELECT id, category_type
            FROM categories
            WHERE user_id = $1 AND id = ANY($2) AND is_active = true
            ORDER BY id
            FOR UPDATE
            "#,
        )
        .bind(user_id)
        .bind(&category_ids)
        .fetch_all(&mut **tx)
        .await?;
        for row in rows {
            let category_id: i64 = row.try_get("id")?;
            let category_type = row
                .try_get::<Option<String>, _>("category_type")?
                .as_deref()
                .and_then(postgres_category_type_code);
            locked_categories.insert(category_id, category_type);
        }
    }

    let account_ids = mutations
        .iter()
        .flat_map(|mutation| [mutation.source_account_id, mutation.destination_account_id])
        .flatten()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut locked_accounts = std::collections::BTreeSet::new();
    if !account_ids.is_empty() {
        let rows = sqlx::query(
            r#"
            SELECT id
            FROM accounts
            WHERE user_id = $1 AND id = ANY($2) AND is_active = true
            ORDER BY id
            FOR UPDATE
            "#,
        )
        .bind(user_id)
        .bind(&account_ids)
        .fetch_all(&mut **tx)
        .await?;
        for row in rows {
            locked_accounts.insert(row.try_get::<i64, _>("id")?);
        }
    }

    for mutation in mutations {
        if let Some(category_id) = mutation.category_id {
            let Some(category_type) = locked_categories.get(&category_id).copied() else {
                return Err(DbError::InvalidOperation(format!(
                    "category not found: {category_id}"
                )));
            };
            if !postgres_category_type_matches_transaction_type(
                category_type,
                &mutation.transaction_type,
            ) {
                return Err(DbError::InvalidOperation(format!(
                    "category type mismatch: {category_id}"
                )));
            }
        }
        for (account_id, field) in [
            (mutation.source_account_id, "source_account_id"),
            (
                mutation.destination_account_id,
                "destination_account_id",
            ),
        ] {
            let Some(account_id) = account_id else {
                continue;
            };
            if locked_accounts.contains(&account_id) {
                continue;
            }
            return Err(DbError::InvalidOperation(format!(
                "{field} not found: {account_id}"
            )));
        }
        if bill_type_uses_destination_account(&mutation.transaction_type)
            && mutation.source_account_id.is_some()
            && mutation.source_account_id == mutation.destination_account_id
        {
            return Err(DbError::InvalidOperation(
                "source and destination accounts must differ".to_string(),
            ));
        }
    }
    Ok(())
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

fn bill_type_uses_destination_account(transaction_type: &str) -> bool {
    matches!(transaction_type, "transfer" | "investment")
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
