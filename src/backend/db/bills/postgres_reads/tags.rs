async fn replace_postgres_bill_tags(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    bill_id: i64,
    tag_ids: &[i64],
) -> DbResult<()> {
    sqlx::query("DELETE FROM bill_tags WHERE user_id = $1 AND bill_id = $2")
        .bind(user_id)
        .bind(bill_id)
        .execute(&mut **tx)
        .await?;
    for tag_id in normalize_ids(tag_ids) {
        let exists = sqlx::query("SELECT 1 FROM tags WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(tag_id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some();
        if !exists {
            return Err(DbError::InvalidOperation(format!(
                "tag not found: {tag_id}"
            )));
        }
        sqlx::query(
            "INSERT INTO bill_tags (user_id, bill_id, tag_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(bill_id)
        .bind(tag_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
