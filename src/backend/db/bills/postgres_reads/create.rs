/// 在当前用户范围内创建单笔正式账单，并同步标签、账户余额等副作用。
pub async fn create_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    draft: &BillCreateDraft,
) -> DbResult<i64> {
    let mutation = prepare_postgres_bill_mutation(pool, user_id, &draft.fields).await?;
    let mut tx = pool.begin().await?;
    let bill_id: i64 = sqlx::query(
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
    .fetch_one(&mut *tx)
    .await?
    .try_get("id")?;
    replace_postgres_bill_tags(&mut tx, user_id, bill_id, &draft.tag_ids).await?;
    apply_postgres_balance_deltas(&mut tx, user_id, &mutation.balance_deltas()).await?;
    tx.commit().await?;
    Ok(bill_id)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 批量创建正式账单，保持全部条目在同一事务中的原子性。
pub async fn batch_create_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    drafts: &[BillCreateDraft],
) -> DbResult<Vec<i64>> {
    let mut tx = pool.begin().await?;
    let bill_ids =
        batch_create_postgres_bills_in_transaction(pool, &mut tx, user_id, drafts).await?;
    tx.commit().await?;
    Ok(bill_ids)
}

pub(crate) async fn batch_create_postgres_bills_in_transaction(
    pool: &PostgresPool,
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    drafts: &[BillCreateDraft],
) -> DbResult<Vec<i64>> {
    if drafts.is_empty() {
        return Ok(Vec::new());
    }
    let mut prepared = Vec::with_capacity(drafts.len());
    for draft in drafts {
        prepared.push(PreparedPostgresBillMutation {
            mutation: prepare_postgres_bill_mutation(pool, user_id, &draft.fields).await?,
            tag_ids: draft.tag_ids.clone(),
        });
    }

    let mut bill_ids = Vec::with_capacity(prepared.len());
    let mut balance_deltas = Vec::new();
    for prepared_bill in prepared {
        let bill_id = insert_postgres_bill_on_tx(tx, user_id, &prepared_bill.mutation).await?;
        replace_postgres_bill_tags(tx, user_id, bill_id, &prepared_bill.tag_ids).await?;
        balance_deltas.extend(prepared_bill.mutation.balance_deltas());
        bill_ids.push(bill_id);
    }
    apply_postgres_balance_deltas(tx, user_id, &balance_deltas).await?;
    Ok(bill_ids)
}
