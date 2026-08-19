#[tracing::instrument(level = "debug", skip_all)]
/// 按当前用户的正式账单重新计算全部账户余额，并返回发生变化的账户。
pub async fn sync_all_postgres_account_balances(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<SyncAllAccountBalancesResult> {
    let account_rows = sqlx::query(
        "SELECT id, name, balance_cents, metadata FROM accounts WHERE user_id = $1",
    )
    .bind(user_id)
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
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut deltas = BTreeMap::<i64, i64>::new();
    for row in bill_rows {
        let transaction_type: String = row.try_get("transaction_type")?;
        let amount_cents: i64 = row.try_get("amount_cents")?;
        let source_account_id: Option<i64> = row.try_get("source_account_id")?;
        let target_account_id: Option<i64> = row.try_get("target_account_id")?;
        let transfer_target_account_id: Option<i64> =
            row.try_get("transfer_target_account_id")?;
        let standard_payload: Value = row.try_get("standard_payload")?;
        let destination_amount_cents = standard_payload
            .get("destination_amount_cents")
            .and_then(value_to_i64);
        for (account_id, delta) in project_postgres_bill_balance_deltas(
            &transaction_type,
            amount_cents,
            source_account_id,
            target_account_id.or(transfer_target_account_id),
            destination_amount_cents,
        )? {
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
            sync_initial_balance_cents(&metadata, old_balance_cents);
        let new_balance_cents =
            initial_balance_cents + deltas.get(&account_id).copied().unwrap_or(0);
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
            .bind(user_id)
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

fn sync_initial_balance_cents(metadata: &Value, fallback_cents: i64) -> i64 {
    metadata
        .get("initial_balance_cents")
        .and_then(value_to_i64)
        .unwrap_or(fallback_cents)
}
