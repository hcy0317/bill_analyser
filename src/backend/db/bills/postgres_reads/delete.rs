/// 删除当前用户的一笔正式账单，并同步相关账户余额。
pub async fn delete_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<bool> {
    let mut tx = pool.begin().await?;
    if !delete_postgres_bill_in_transaction(&mut tx, user_id, bill_id).await? {
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

async fn delete_postgres_bill_in_transaction(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    bill_id: i64,
) -> DbResult<bool> {
    let Some(locked) = get_postgres_bill_for_update_on_tx(tx, user_id, bill_id).await? else {
        return Ok(false);
    };
    let old_mutation = prepare_postgres_bill_mutation(&locked.record)?;
    lock_and_validate_postgres_bill_mutations_on_tx(tx, user_id, &[&old_mutation]).await?;
    delete_locked_postgres_bill_on_tx(tx, user_id, &locked, &old_mutation).await
}

async fn delete_locked_postgres_bill_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    locked: &LockedPostgresBill,
    old_mutation: &PostgresBillMutation,
) -> DbResult<bool> {
    let deleted = sqlx::query(
        "DELETE FROM bills WHERE user_id = $1 AND id = $2 AND version = $3 AND is_deleted = false",
    )
    .bind(user_id)
    .bind(locked.bill_id)
    .bind(locked.version)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if deleted != 1 {
        return Ok(false);
    }
    let deltas = old_mutation
        .balance_deltas()?
        .into_iter()
        .map(|(account_id, amount)| (account_id, -amount))
        .collect::<Vec<_>>();
    apply_postgres_balance_deltas(tx, user_id, &deltas).await?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 批量删除当前用户的正式账单，并按受影响账户去重重算余额。
pub async fn batch_delete_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<usize> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    let locked = get_postgres_bills_for_update_on_tx(&mut tx, user_id, &bill_ids).await?;
    let prepared = locked
        .into_iter()
        .map(|locked| {
            prepare_postgres_bill_mutation(&locked.record).map(|mutation| (locked, mutation))
        })
        .collect::<DbResult<Vec<_>>>()?;
    let mutations = prepared
        .iter()
        .map(|(_, mutation)| mutation)
        .collect::<Vec<_>>();
    lock_and_validate_postgres_bill_mutations_on_tx(&mut tx, user_id, &mutations).await?;

    let mut deleted_count = 0_usize;
    for (locked, mutation) in prepared {
        if delete_locked_postgres_bill_on_tx(&mut tx, user_id, &locked, &mutation).await? {
            deleted_count = deleted_count.saturating_add(1);
        }
    }
    tx.commit().await?;
    Ok(deleted_count)
}
