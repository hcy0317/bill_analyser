async fn apply_postgres_balance_deltas(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    deltas: &[(i64, i64)],
) -> DbResult<()> {
    let mut combined: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for (account_id, delta) in deltas {
        if *account_id > 0 && *delta != 0 {
            *combined.entry(*account_id).or_default() += *delta;
        }
    }
    for (account_id, delta) in combined {
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance_cents = balance_cents + $3,
                updated_at = now(),
                version = version + 1
            WHERE user_id = $1 AND id = $2
            "#,
        )
        .bind(user_id)
        .bind(account_id)
        .bind(delta)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn insert_postgres_bill_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    mutation: &PostgresBillMutation,
) -> DbResult<i64> {
    sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, amount_cents, direction, transaction_type,
            account_id, source_account_id, target_account_id, transfer_target_account_id,
            category_id, merchant, description, payment_method, source_hash, standard_payload
        )
        VALUES ($1, $2, $3, $4, $5, $6, $6, $7, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(mutation.occurred_at)
    .bind(mutation.amount_cents)
    .bind(&mutation.direction)
    .bind(&mutation.transaction_type)
    .bind(mutation.source_account_id)
    .bind(mutation.destination_account_id)
    .bind(mutation.category_id)
    .bind(&mutation.merchant)
    .bind(&mutation.description)
    .bind(&mutation.payment_method)
    .bind(&mutation.source_hash)
    .bind(&mutation.standard_payload)
    .fetch_one(&mut **tx)
    .await?
    .try_get("id")
    .map_err(DbError::from)
}

async fn update_postgres_bill_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    bill_id: i64,
    mutation: &PostgresBillMutation,
) -> DbResult<u64> {
    sqlx::query(
        r#"
        UPDATE bills
        SET occurred_at = $3,
            amount_cents = $4,
            direction = $5,
            transaction_type = $6,
            account_id = $7,
            source_account_id = $7,
            target_account_id = $8,
            transfer_target_account_id = $8,
            category_id = $9,
            merchant = $10,
            description = $11,
            payment_method = $12,
            source_hash = $13,
            standard_payload = $14,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id)
    .bind(bill_id)
    .bind(mutation.occurred_at)
    .bind(mutation.amount_cents)
    .bind(&mutation.direction)
    .bind(&mutation.transaction_type)
    .bind(mutation.source_account_id)
    .bind(mutation.destination_account_id)
    .bind(mutation.category_id)
    .bind(&mutation.merchant)
    .bind(&mutation.description)
    .bind(&mutation.payment_method)
    .bind(&mutation.source_hash)
    .bind(&mutation.standard_payload)
    .execute(&mut **tx)
    .await
    .map(|result| result.rows_affected())
    .map_err(DbError::from)
}
