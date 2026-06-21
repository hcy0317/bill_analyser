#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountTransactionsMoveResult {
    success: bool,
    message: String,
    moved_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountTransactionsClearResult {
    success: bool,
    message: String,
    deleted_count: i64,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn sync_all_postgres_account_balances(
    pool: &PostgresPool,
    user_id: UserId,
) -> bill_analyser_db::DbResult<SyncAllAccountBalancesResult> {
    let user_id_value = db_user_id(user_id);
    let account_rows = sqlx::query(
        "SELECT id, name, balance_cents, metadata FROM accounts WHERE user_id = $1",
    )
    .bind(user_id_value)
    .fetch_all(pool)
    .await?;
    let bill_rows = sqlx::query(
        r#"
        SELECT transaction_type, amount_cents, source_account_id, target_account_id,
               transfer_target_account_id, standard_payload
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
        "#,
    )
    .bind(user_id_value)
    .fetch_all(pool)
    .await?;

    let mut deltas = std::collections::BTreeMap::<i64, i64>::new();
    for row in bill_rows {
        let transaction_type: String = row.try_get("transaction_type")?;
        let amount_cents: i64 = row.try_get("amount_cents")?;
        let source_account_id: Option<i64> = row.try_get("source_account_id")?;
        let target_account_id: Option<i64> = row.try_get("target_account_id")?;
        let transfer_target_account_id: Option<i64> = row.try_get("transfer_target_account_id")?;
        let standard_payload: Value = row.try_get("standard_payload")?;
        for (account_id, delta) in postgres_bill_balance_deltas(
            &transaction_type,
            amount_cents,
            source_account_id,
            target_account_id.or(transfer_target_account_id),
            &standard_payload,
        ) {
            *deltas.entry(account_id).or_default() += delta;
        }
    }

    let mut discrepancies = Vec::new();
    let mut synced_accounts = 0_usize;
    for row in &account_rows {
        let account_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let old_balance_cents: i64 = row.try_get("balance_cents")?;
        let metadata: Value = row.try_get("metadata")?;
        let initial_balance_cents =
            metadata_initial_balance_cents(&metadata, old_balance_cents);
        let new_balance_cents = initial_balance_cents + deltas.get(&account_id).copied().unwrap_or(0);
        if new_balance_cents != old_balance_cents {
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance_cents = $3,
                    updated_at = now(),
                    version = version + 1
                WHERE user_id = $1 AND id = $2
                "#,
            )
            .bind(user_id_value)
            .bind(account_id)
            .bind(new_balance_cents)
            .execute(pool)
            .await?;
            synced_accounts += 1;
            discrepancies.push(AccountBalanceDiscrepancy {
                account_id,
                name,
                old_balance_cents,
                new_balance_cents,
                diff_cents: new_balance_cents - old_balance_cents,
            });
        }
    }

    Ok(SyncAllAccountBalancesResult {
        total_accounts: account_rows.len(),
        synced_accounts,
        discrepancies,
        errors: Vec::new(),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn move_all_postgres_account_transactions(
    pool: &PostgresPool,
    from_account_id: i64,
    to_account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<AccountTransactionsMoveResult> {
    if from_account_id == to_account_id {
        return Ok(account_move_failure(
            "Source and target accounts must be different",
        ));
    }
    let mut transaction = pool.begin().await?;
    if !postgres_account_exists(&mut transaction, from_account_id, user_id).await? {
        return Ok(account_move_failure("Source account not found"));
    }
    if !postgres_account_exists(&mut transaction, to_account_id, user_id).await? {
        return Ok(account_move_failure("Target account not found"));
    }
    let moved_count = sqlx::query(
        r#"
        UPDATE bills
        SET account_id = CASE WHEN account_id = $2 THEN $3 ELSE account_id END,
            source_account_id = CASE WHEN source_account_id = $2 THEN $3 ELSE source_account_id END,
            target_account_id = CASE WHEN target_account_id = $2 THEN $3 ELSE target_account_id END,
            transfer_target_account_id = CASE WHEN transfer_target_account_id = $2 THEN $3 ELSE transfer_target_account_id END,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(from_account_id)
    .bind(to_account_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected() as i64;
    transaction.commit().await?;
    Ok(AccountTransactionsMoveResult {
        success: true,
        message: "Transactions moved successfully".to_string(),
        moved_count,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_postgres_account_transactions(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<AccountTransactionsClearResult> {
    let mut transaction = pool.begin().await?;
    if !postgres_account_exists(&mut transaction, account_id, user_id).await? {
        return Ok(account_clear_failure("Account not found"));
    }
    let affected = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM bills
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        DELETE FROM bill_tags
        WHERE user_id = $1
          AND bill_id IN (
              SELECT id FROM bills
              WHERE user_id = $1
                AND is_deleted = false
                AND (
                    account_id = $2 OR source_account_id = $2 OR target_account_id = $2
                    OR transfer_target_account_id = $2
                )
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE bills
        SET is_deleted = true,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(AccountTransactionsClearResult {
        success: true,
        message: "Transactions deleted successfully".to_string(),
        deleted_count: affected,
    })
}

async fn postgres_account_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<bool> {
    Ok(sqlx::query("SELECT 1 FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&mut **transaction)
        .await?
        .is_some())
}

fn account_move_failure(message: &str) -> AccountTransactionsMoveResult {
    AccountTransactionsMoveResult {
        success: false,
        message: message.to_string(),
        moved_count: 0,
    }
}

fn account_clear_failure(message: &str) -> AccountTransactionsClearResult {
    AccountTransactionsClearResult {
        success: false,
        message: message.to_string(),
        deleted_count: 0,
    }
}

fn postgres_bill_balance_deltas(
    transaction_type: &str,
    amount_cents: i64,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
    standard_payload: &Value,
) -> Vec<(i64, i64)> {
    let mut deltas = Vec::new();
    let amount = amount_cents.abs();
    let destination_amount_cents = standard_payload
        .get("destination_amount_cents")
        .and_then(value_to_cents)
        .unwrap_or(amount)
        .abs();
    match transaction_type {
        "income" => push_account_delta(&mut deltas, source_account_id, amount),
        "transfer" | "investment" => {
            push_account_delta(&mut deltas, source_account_id, -amount);
            push_account_delta(&mut deltas, destination_account_id, destination_amount_cents);
        }
        _ => push_account_delta(&mut deltas, source_account_id, -amount),
    }
    deltas
}

fn push_account_delta(deltas: &mut Vec<(i64, i64)>, account_id: Option<i64>, delta: i64) {
    if let Some(account_id) = account_id.filter(|value| *value > 0) {
        deltas.push((account_id, delta));
    }
}

fn value_to_cents(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn metadata_initial_balance_cents(metadata: &Value, fallback_cents: i64) -> i64 {
    metadata
        .get("initial_balance_cents")
        .and_then(value_to_cents)
        .unwrap_or(fallback_cents)
}
