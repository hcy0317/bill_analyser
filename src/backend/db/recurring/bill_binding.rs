#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_bill_recurring_candidates(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
    tolerance_days: i64,
) -> DbResult<Option<BillRecurringCandidates>> {
    let user_id_i64 = user_id_i64(user_id)?;
    let Some(bill) = get_postgres_bill_by_id(pool, user_id_i64, bill_id).await? else {
        return Ok(None);
    };
    let linked_recurring_id = pg_record_optional_i64(&bill, "created_from_recurring");
    let linked_recurring_name = match linked_recurring_id {
        Some(recurring_id) => get_postgres_recurring_template_name(pool, user_id_i64, recurring_id)
            .await?
            .unwrap_or_default(),
        None => String::new(),
    };
    let recurring_rows = list_enabled_postgres_recurring_templates(pool, user_id_i64).await?;
    let candidates = build_postgres_recurring_candidates_for_bill_data(
        &bill,
        &recurring_rows,
        linked_recurring_id,
        tolerance_days.clamp(0, 31),
    );
    Ok(Some(BillRecurringCandidates {
        linked_recurring_id,
        linked_recurring_name,
        candidates,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn bind_postgres_bill_to_recurring(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
    recurring_id: i64,
) -> DbResult<Option<BillRecurringBindResult>> {
    let user_id_i64 = user_id_i64(user_id)?;
    let Some(bill) = get_postgres_bill_by_id(pool, user_id_i64, bill_id).await? else {
        return Ok(None);
    };
    let previous_recurring_id = pg_record_optional_i64(&bill, "created_from_recurring");
    let mut tx = pool.begin().await?;
    let Some(recurring) =
        get_postgres_recurring_template_on_tx(&mut tx, user_id_i64, recurring_id).await?
    else {
        return Ok(None);
    };
    let bill_date_text = pg_record_text(&bill, "date");
    let next_occurrence = pg_parse_date_value(&bill_date_text)
        .and_then(|bill_date| pg_next_recurring_occurrence_after(&recurring, bill_date, 370))
        .map(|date| date.to_string());
    let fallback_next_date = pg_recurring_text(&recurring, "next_date");
    let stored_next_date = next_occurrence
        .or_else(|| non_empty_text(Some(fallback_next_date.as_str())).map(ToOwned::to_owned));
    sqlx::query(
        r#"
        UPDATE bills
        SET standard_payload = jsonb_set(
                COALESCE(standard_payload, '{}'::jsonb),
                '{created_from_recurring}',
                to_jsonb($3::bigint),
                true
            ),
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id_i64)
    .bind(bill_id)
    .bind(recurring_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE transaction_templates
        SET scheduled_next_date = $3,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND template_type = 2
        "#,
    )
    .bind(user_id_i64)
    .bind(recurring_id)
    .bind(stored_next_date.as_deref())
    .execute(&mut *tx)
    .await?;
    if previous_recurring_id.is_some_and(|previous| previous != recurring_id) {
        pg_recalculate_recurring_next_date_on_tx(
            &mut tx,
            user_id_i64,
            previous_recurring_id.unwrap_or_default(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Some(BillRecurringBindResult {
        bill_id,
        recurring_id,
        next_scheduled_date: stored_next_date,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn unbind_postgres_bill_from_recurring(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
) -> DbResult<Option<bool>> {
    let user_id_i64 = user_id_i64(user_id)?;
    let Some(bill) = get_postgres_bill_by_id(pool, user_id_i64, bill_id).await? else {
        return Ok(None);
    };
    let recurring_id = pg_record_optional_i64(&bill, "created_from_recurring");
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        r#"
        UPDATE bills
        SET standard_payload = COALESCE(standard_payload, '{}'::jsonb) - 'created_from_recurring',
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id_i64)
    .bind(bill_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if let Some(recurring_id) = recurring_id {
        pg_recalculate_recurring_next_date_on_tx(&mut tx, user_id_i64, recurring_id).await?;
    }
    tx.commit().await?;
    Ok(Some(updated > 0))
}
