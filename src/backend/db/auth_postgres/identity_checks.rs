#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_username_exists(pool: &PostgresPool, username: &str) -> DbResult<bool> {
    postgres_exists_by_text(pool, "username", username).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_email_exists(pool: &PostgresPool, email: &str) -> DbResult<bool> {
    postgres_exists_by_text(pool, "email", email).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_email_exists_for_other_user(
    pool: &PostgresPool,
    user_id: UserId,
    email: &str,
) -> DbResult<bool> {
    sqlx::query("SELECT 1 FROM users WHERE email = $1 AND id != $2 LIMIT 1")
        .bind(email)
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map(|row| row.is_some())
        .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_account_belongs_to_user(
    pool: &PostgresPool,
    user_id: UserId,
    account_id: i64,
) -> DbResult<bool> {
    postgres_scoped_id_exists(pool, "accounts", user_id, account_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_category_belongs_to_user(
    pool: &PostgresPool,
    user_id: UserId,
    category_id: i64,
) -> DbResult<bool> {
    postgres_scoped_id_exists(pool, "categories", user_id, category_id).await
}

async fn postgres_exists_by_text(
    pool: &PostgresPool,
    column: &'static str,
    value: &str,
) -> DbResult<bool> {
    let sql = format!("SELECT 1 FROM users WHERE {column} = $1 LIMIT 1");
    sqlx::query(&sql)
        .bind(value)
        .fetch_optional(pool)
        .await
        .map(|row| row.is_some())
        .map_err(postgres_auth_error)
}

async fn postgres_scoped_id_exists(
    pool: &PostgresPool,
    table_name: &'static str,
    user_id: UserId,
    id: i64,
) -> DbResult<bool> {
    let sql = format!("SELECT 1 FROM {table_name} WHERE id = $1 AND user_id = $2 LIMIT 1");
    sqlx::query(&sql)
        .bind(id)
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map(|row| row.is_some())
        .map_err(postgres_auth_error)
}
