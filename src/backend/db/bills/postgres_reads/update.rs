/// 更新当前用户的一笔正式账单，并重新同步标签与受影响账户余额。
pub async fn update_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
    draft: &BillUpdateDraft,
) -> DbResult<bool> {
    if draft.fields.is_empty() && draft.tag_ids.is_none() {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    if !update_postgres_bill_cas_in_transaction(
        &mut tx,
        user_id,
        bill_id,
        &draft.fields,
        draft.tag_ids.as_deref(),
    )
    .await?
    {
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

/// 在已有事务内锁定并 CAS 更新账单；读取、身份校验与余额差额共享同一快照。
pub(crate) async fn update_postgres_bill_cas_in_transaction(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    bill_id: i64,
    fields: &BillRecord,
    tag_ids: Option<&[i64]>,
) -> DbResult<bool> {
    let Some(locked) = get_postgres_bill_for_update_on_tx(tx, user_id, bill_id).await? else {
        return Ok(false);
    };
    let prepared = prepare_postgres_bill_update(locked, fields)?;
    lock_and_validate_postgres_bill_mutations_on_tx(
        tx,
        user_id,
        &[&prepared.old_mutation, &prepared.new_mutation],
    )
    .await?;
    apply_prepared_postgres_bill_update_on_tx(tx, user_id, &prepared, tag_ids).await
}

fn prepare_postgres_bill_update(
    locked: LockedPostgresBill,
    fields: &BillRecord,
) -> DbResult<PreparedPostgresBillUpdate> {
    let old_mutation = prepare_postgres_bill_mutation(&locked.record)?;
    let mut merged = locked.record;
    for (key, value) in fields {
        merged.insert(key.clone(), value.clone());
    }
    if update_requires_hash_recalculation(fields) && !fields.contains_key("hash") {
        let hash = calculate_bill_hash_from_record(&merged)?;
        merged.insert("hash".to_string(), Value::String(hash));
    }
    let new_mutation = prepare_postgres_bill_mutation(&merged)?;
    Ok(PreparedPostgresBillUpdate {
        bill_id: locked.bill_id,
        expected_version: locked.version,
        old_mutation,
        new_mutation,
    })
}

async fn apply_prepared_postgres_bill_update_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    prepared: &PreparedPostgresBillUpdate,
    tag_ids: Option<&[i64]>,
) -> DbResult<bool> {
    let updated = update_postgres_bill_on_tx(
        tx,
        user_id,
        prepared.bill_id,
        prepared.expected_version,
        &prepared.new_mutation,
    )
    .await?;
    if updated != 1 {
        return Ok(false);
    }
    if let Some(tag_ids) = tag_ids {
        replace_postgres_bill_tags(tx, user_id, prepared.bill_id, tag_ids).await?;
    }
    let mut deltas = prepared
        .old_mutation
        .balance_deltas()?
        .into_iter()
        .map(|(account_id, amount)| (account_id, -amount))
        .collect::<Vec<_>>();
    deltas.extend(prepared.new_mutation.balance_deltas()?);
    apply_postgres_balance_deltas(tx, user_id, &deltas).await?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 批量更新正式账单，确保每条更新的身份校验和余额同步在事务内完成。
pub async fn batch_update_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    bill_ids: &[i64],
    fields: &BillRecord,
) -> DbResult<BatchUpdateBillsResult> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() || fields.is_empty() {
        return Ok(BatchUpdateBillsResult::default());
    }
    validate_batch_route_update_fields(fields.keys().map(String::as_str))
        .map_err(|error| DbError::InvalidOperation(error.to_string()))?;

    let mut tx = pool.begin().await?;
    let locked = get_postgres_bills_for_update_on_tx(&mut tx, user_id, &bill_ids).await?;
    let mut locked_by_id = locked
        .into_iter()
        .map(|locked| (locked.bill_id, locked))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut failed_ids = Vec::new();
    let mut prepared = Vec::with_capacity(locked_by_id.len());
    for bill_id in bill_ids {
        let Some(locked) = locked_by_id.remove(&bill_id) else {
            failed_ids.push(bill_id);
            continue;
        };
        prepared.push(prepare_postgres_bill_update(locked, fields)?);
    }
    let mutations = prepared
        .iter()
        .flat_map(|prepared| [&prepared.old_mutation, &prepared.new_mutation])
        .collect::<Vec<_>>();
    lock_and_validate_postgres_bill_mutations_on_tx(&mut tx, user_id, &mutations).await?;

    let mut success_count = 0_usize;
    for prepared in prepared {
        let bill_id = prepared.bill_id;
        let updated =
            apply_prepared_postgres_bill_update_on_tx(&mut tx, user_id, &prepared, None).await?;
        if !updated {
            failed_ids.push(bill_id);
            continue;
        }
        success_count += 1;
    }
    tx.commit().await?;
    Ok(BatchUpdateBillsResult {
        success_count,
        failed_count: failed_ids.len(),
        failed_ids,
    })
}
