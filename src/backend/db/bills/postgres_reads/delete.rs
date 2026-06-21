/// 删除当前用户的一笔正式账单，并同步相关账户余额。
pub async fn delete_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<bool> {
    let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
        return Ok(false);
    };
    let old_mutation = prepare_postgres_bill_mutation(pool, user_id, &existing).await?;
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query("DELETE FROM bills WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(bill_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Ok(false);
    }
    let deltas = old_mutation
        .balance_deltas()
        .into_iter()
        .map(|(account_id, amount)| (account_id, -amount))
        .collect::<Vec<_>>();
    apply_postgres_balance_deltas(&mut tx, user_id, &deltas).await?;
    tx.commit().await?;
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
    let mut prepared = Vec::new();
    for bill_id in bill_ids {
        let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
            continue;
        };
        prepared.push((
            bill_id,
            prepare_postgres_bill_mutation(pool, user_id, &existing).await?,
        ));
    }
    if prepared.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;
    let mut deleted_count = 0_usize;
    let mut balance_deltas = Vec::new();
    for (bill_id, old_mutation) in prepared {
        let deleted = sqlx::query("DELETE FROM bills WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(bill_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if deleted == 0 {
            continue;
        }
        deleted_count += usize::try_from(deleted).unwrap_or(usize::MAX);
        balance_deltas.extend(
            old_mutation
                .balance_deltas()
                .into_iter()
                .map(|(account_id, amount)| (account_id, -amount)),
        );
    }
    apply_postgres_balance_deltas(&mut tx, user_id, &balance_deltas).await?;
    tx.commit().await?;
    Ok(deleted_count)
}
