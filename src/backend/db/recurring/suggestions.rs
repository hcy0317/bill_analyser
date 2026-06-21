#[tracing::instrument(level = "debug", skip_all)]
/// 统计当前用户周期建议数量，供分页和 badge 使用。
pub async fn count_postgres_recurring_suggestions(
    pool: &PostgresPool,
    user_id: UserId,
    status: Option<&str>,
) -> DbResult<i64> {
    let user_id = user_id_i64(user_id)?;
    let status = non_empty_text(status);
    let count = if let Some(status) = status {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM recurring_suggestions WHERE user_id = $1 AND status = $2",
        )
        .bind(user_id)
        .bind(status)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM recurring_suggestions WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?
    };
    Ok(count)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 分页读取当前用户周期建议列表，并保持检测结果的排序合同。
pub async fn list_postgres_recurring_suggestions(
    pool: &PostgresPool,
    user_id: UserId,
    status: Option<&str>,
    limit: usize,
    offset: usize,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let limit = i64::try_from(limit)
        .map_err(|_| DbError::InvalidOperation("invalid recurring limit".to_string()))?;
    let offset = i64::try_from(offset)
        .map_err(|_| DbError::InvalidOperation("invalid recurring offset".to_string()))?;
    let status = non_empty_text(status);
    let rows = if let Some(status) = status {
        let sql = recurring_suggestion_select_sql(
            "WHERE user_id = $1 AND status = $2",
            "LIMIT $3 OFFSET $4",
        );
        sqlx::query(&sql)
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
    } else {
        let sql = recurring_suggestion_select_sql("WHERE user_id = $1", "LIMIT $2 OFFSET $3");
        sqlx::query(&sql)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
    };
    rows.iter()
        .map(postgres_recurring_suggestion_from_row)
        .collect()
}
