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
    let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
        return Ok(false);
    };
    let old_mutation = prepare_postgres_bill_mutation(pool, user_id, &existing).await?;
    let mut merged = existing;
    for (key, value) in &draft.fields {
        merged.insert(key.clone(), value.clone());
    }
    if update_requires_hash_recalculation(&draft.fields) && !draft.fields.contains_key("hash") {
        let hash = calculate_bill_hash_from_record(&merged)?;
        merged.insert("hash".to_string(), Value::String(hash));
    }
    let new_mutation = prepare_postgres_bill_mutation(pool, user_id, &merged).await?;

    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
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
    .bind(new_mutation.occurred_at)
    .bind(new_mutation.amount_cents)
    .bind(&new_mutation.direction)
    .bind(&new_mutation.transaction_type)
    .bind(new_mutation.source_account_id)
    .bind(new_mutation.destination_account_id)
    .bind(new_mutation.category_id)
    .bind(&new_mutation.merchant)
    .bind(&new_mutation.description)
    .bind(&new_mutation.payment_method)
    .bind(&new_mutation.source_hash)
    .bind(&new_mutation.standard_payload)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Ok(false);
    }
    if let Some(tag_ids) = &draft.tag_ids {
        replace_postgres_bill_tags(&mut tx, user_id, bill_id, tag_ids).await?;
    }
    let mut deltas = old_mutation
        .balance_deltas()
        .into_iter()
        .map(|(account_id, amount)| (account_id, -amount))
        .collect::<Vec<_>>();
    deltas.extend(new_mutation.balance_deltas());
    apply_postgres_balance_deltas(&mut tx, user_id, &deltas).await?;
    tx.commit().await?;
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

    let mut prepared = Vec::new();
    let mut failed_ids = Vec::new();
    for bill_id in bill_ids {
        let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
            failed_ids.push(bill_id);
            continue;
        };
        let old_mutation = prepare_postgres_bill_mutation(pool, user_id, &existing).await?;
        let mut merged = existing;
        for (key, value) in fields {
            merged.insert(key.clone(), value.clone());
        }
        if update_requires_hash_recalculation(fields) && !fields.contains_key("hash") {
            let hash = calculate_bill_hash_from_record(&merged)?;
            merged.insert("hash".to_string(), Value::String(hash));
        }
        let new_mutation = prepare_postgres_bill_mutation(pool, user_id, &merged).await?;
        prepared.push(PreparedPostgresBillUpdate {
            bill_id,
            old_mutation,
            new_mutation,
        });
    }

    let mut tx = pool.begin().await?;
    let mut success_count = 0_usize;
    let mut balance_deltas = Vec::new();
    for prepared_bill in prepared {
        let updated = update_postgres_bill_on_tx(
            &mut tx,
            user_id,
            prepared_bill.bill_id,
            &prepared_bill.new_mutation,
        )
        .await?;
        if updated == 0 {
            failed_ids.push(prepared_bill.bill_id);
            continue;
        }
        balance_deltas.extend(
            prepared_bill
                .old_mutation
                .balance_deltas()
                .into_iter()
                .map(|(account_id, amount)| (account_id, -amount)),
        );
        balance_deltas.extend(prepared_bill.new_mutation.balance_deltas());
        success_count += 1;
    }
    apply_postgres_balance_deltas(&mut tx, user_id, &balance_deltas).await?;
    tx.commit().await?;
    Ok(BatchUpdateBillsResult {
        success_count,
        failed_count: failed_ids.len(),
        failed_ids,
    })
}
