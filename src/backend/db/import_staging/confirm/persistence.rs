fn confirm_transaction_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "income" | "收入" | "2" => "income",
        "transfer" | "转账" | "4" => "transfer",
        "investment" | "投资" | "5" => "investment",
        _ => "expense",
    }
    .to_string()
}

fn confirm_balance_deltas(
    transaction_type: &str,
    direction: &str,
    amount_cents: i64,
    destination_amount_cents: Option<i64>,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
) -> Vec<(i64, i64)> {
    let amount = amount_cents.saturating_abs();
    let destination_amount = destination_amount_cents
        .unwrap_or(amount)
        .saturating_abs();
    match transaction_type {
        "income" => source_account_id
            .filter(|value| *value > 0)
            .map(|account_id| vec![(account_id, amount)])
            .unwrap_or_default(),
        "transfer" | "investment" => {
            let mut deltas = Vec::new();
            if let Some(account_id) = source_account_id.filter(|value| *value > 0) {
                deltas.push((account_id, -amount));
            }
            if let Some(account_id) = destination_account_id.filter(|value| *value > 0) {
                deltas.push((account_id, destination_amount));
            }
            deltas
        }
        _ => source_account_id
            .filter(|value| *value > 0)
            .map(|account_id| {
                vec![(
                    account_id,
                    if direction == "income" { amount } else { -amount },
                )]
            })
            .unwrap_or_default(),
    }
}

async fn apply_confirm_balance_delta_difference(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    old_deltas: &[(i64, i64)],
    new_deltas: &[(i64, i64)],
) -> DbResult<()> {
    let mut combined = BTreeMap::<i64, i64>::new();
    for (account_id, delta) in old_deltas {
        *combined.entry(*account_id).or_default() -= *delta;
    }
    for (account_id, delta) in new_deltas {
        *combined.entry(*account_id).or_default() += *delta;
    }
    for (account_id, delta) in combined {
        if delta == 0 {
            continue;
        }
        let changed = sqlx::query(
            r#"
            UPDATE accounts
            SET balance_cents = balance_cents + $3,
                updated_at = now(),
                version = version + 1
            WHERE user_id = $1 AND id = $2 AND is_active = true
            "#,
        )
        .bind(user_id)
        .bind(account_id)
        .bind(delta)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(DbError::InvalidOperation(format!(
                "history account CAS mismatch: {account_id}"
            )));
        }
    }
    Ok(())
}

async fn persist_confirm_receipt(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    request_session_version: i64,
    receipt: &StoredConfirmReceipt,
    confirmed_count: usize,
) -> DbResult<()> {
    let receipt = serde_json::to_value(receipt).map_err(|error| {
        DbError::InvalidOperation(format!("failed to serialize confirm receipt: {error}"))
    })?;
    let changed = sqlx::query(
        r#"
        UPDATE import_sessions
        SET status = 'confirmed',
            total_confirmed = $4,
            metadata = jsonb_set(metadata, '{confirm_receipt}', $5::jsonb, true),
            updated_at = now(),
            version = version + 1
        WHERE id = $1 AND user_id = $2 AND version = $3 AND status <> 'confirmed'
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .bind(request_session_version)
    .bind(i64::try_from(confirmed_count).unwrap_or(i64::MAX))
    .bind(receipt.to_string())
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(DbError::InvalidOperation(
            "import session confirm CAS mismatch".to_string(),
        ));
    }
    Ok(())
}

async fn load_selected_preview_rows_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<Vec<ImportPreviewRow>> {
    let rows = sqlx::query(
        r#"
        SELECT p.*, s.session_key
        FROM import_preview_rows p
        JOIN import_sessions s ON s.id = p.session_id
        WHERE p.session_id = $1 AND p.user_id = $2 AND p.selected = true
        ORDER BY p.occurred_at ASC, p.id ASC
        FOR UPDATE OF p
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(preview_from_pg_row).collect()
}

async fn load_import_identity_maps_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(&mut **tx)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut maps = ImportIdentityMaps::default();
    for row in account_rows {
        maps.active_accounts.insert(row.try_get("id")?);
    }
    for row in category_rows {
        let id = row.try_get::<i64, _>("id")?;
        let category_type = row
            .try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .and_then(preview_category_type_code);
        maps.active_categories.insert(id, category_type);
    }
    Ok(maps)
}

fn bill_create_fields_from_preview(preview: &ImportPreviewRow) -> BillRecord {
    let mut fields = Map::new();
    fields.insert("date".to_string(), json!(preview.preview_date));
    fields.insert("type".to_string(), json!(preview.preview_type));
    fields.insert(
        "amount_cents".to_string(),
        json!(preview.preview_amount_cents),
    );
    fields.insert(
        "destination_amount_cents".to_string(),
        json!(preview.preview_destination_amount_cents),
    );
    fields.insert(
        "counterparty".to_string(),
        json!(preview.preview_counterparty),
    );
    fields.insert(
        "description".to_string(),
        json!(preview.preview_description),
    );
    fields.insert(
        "payment_method".to_string(),
        json!(preview.preview_payment_method),
    );
    if let Some(value) = preview.category_id {
        fields.insert("category_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_source_account_id {
        fields.insert("source_account_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_destination_account_id {
        fields.insert("destination_account_id".to_string(), json!(value));
    }
    fields
}
