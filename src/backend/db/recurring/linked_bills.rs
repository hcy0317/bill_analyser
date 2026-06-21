pub async fn get_postgres_bills_linked_to_recurring(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<BTreeSet<i64>> {
    let user_id = user_id_i64(user_id)?;
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM bills
        WHERE user_id = $1
          AND COALESCE(
              NULLIF(standard_payload->>'created_from_recurring', ''),
              NULLIF(raw_payload->>'created_from_recurring', '')
          ) IS NOT NULL
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| row.try_get::<i64, _>("id").map_err(DbError::from))
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_recent_bills_for_recurring_detection(
    pool: &PostgresPool,
    user_id: UserId,
    limit: usize,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let rows = sqlx::query(
        r#"
        SELECT
            b.id,
            b.occurred_at,
            b.transaction_type,
            b.amount_cents,
            b.merchant,
            b.description,
            b.source_account_id,
            b.target_account_id,
            b.transfer_target_account_id,
            b.standard_payload,
            c.path AS category_path,
            c.name AS category_name
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = $1 AND b.is_deleted = false
        ORDER BY b.occurred_at DESC, b.id DESC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await?;
    rows.iter().map(postgres_recent_bill_from_row).collect()
}
