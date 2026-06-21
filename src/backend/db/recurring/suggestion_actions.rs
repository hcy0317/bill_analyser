pub async fn accept_postgres_recurring_suggestion(
    pool: &PostgresPool,
    user_id: UserId,
    suggestion_id: i64,
) -> DbResult<Option<Value>> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await?;
    let suggestion = sqlx::query(
        r#"
        SELECT id, name, description, type, amount_cents, source_account_id,
               destination_account_id, counterparty, frequency, first_occurrence,
               suggested_next_date
        FROM recurring_suggestions
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(suggestion) = suggestion else {
        return Ok(None);
    };

    let changed = sqlx::query(
        r#"
        UPDATE recurring_suggestions
        SET status = 'accepted', updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed == 0 {
        return Ok(None);
    }

    let name: String = suggestion.try_get("name")?;
    let description: Option<String> = suggestion.try_get("description")?;
    let transaction_type: String = suggestion.try_get("type")?;
    let amount_cents: i64 = suggestion.try_get("amount_cents")?;
    let source_account_id: Option<i64> = suggestion.try_get("source_account_id")?;
    let destination_account_id: Option<i64> = suggestion.try_get("destination_account_id")?;
    let counterparty: Option<String> = suggestion.try_get("counterparty")?;
    let frequency: String = suggestion.try_get("frequency")?;
    let first_occurrence: Option<String> = suggestion.try_get("first_occurrence")?;
    let suggested_next_date: Option<String> = suggestion.try_get("suggested_next_date")?;
    let recurring_id: i64 = sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, description, transaction_type,
            source_account_id, destination_account_id, source_amount_minor_units,
            destination_amount_minor_units, comment, scheduled_frequency,
            scheduled_start_date, scheduled_next_date, enabled, auto_create
        )
        VALUES ($1, 2, $2, $3, $4, $5, $6, $7, 0, $8, $9, $10, $11, true, false)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(description.unwrap_or_default())
    .bind(transaction_type)
    .bind(
        source_account_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    )
    .bind(
        destination_account_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    )
    .bind(amount_cents)
    .bind(counterparty.unwrap_or_default())
    .bind(frequency)
    .bind(first_occurrence.unwrap_or_else(utc_today_text))
    .bind(suggested_next_date.unwrap_or_else(utc_today_text))
    .fetch_one(&mut *tx)
    .await?
    .try_get("id")?;

    tx.commit().await?;
    Ok(Some(json!({
        "recurring_id": recurring_id,
        "suggestion_id": suggestion_id,
        "status": "accepted",
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn reject_postgres_recurring_suggestion(
    pool: &PostgresPool,
    user_id: UserId,
    suggestion_id: i64,
) -> DbResult<bool> {
    let user_id = user_id_i64(user_id)?;
    let changed = sqlx::query(
        r#"
        UPDATE recurring_suggestions
        SET status = 'rejected', updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}
