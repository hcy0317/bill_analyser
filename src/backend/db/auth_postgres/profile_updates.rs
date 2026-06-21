#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_auth_user_profile(
    pool: &PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
) -> DbResult<bool> {
    update_postgres_auth_user_profile_internal(pool, user_id, updates, updated_at, None).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_auth_user_profile_with_auth_log(
    pool: &PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
    auth_log: &AuthLogDraft,
) -> DbResult<bool> {
    update_postgres_auth_user_profile_internal(pool, user_id, updates, updated_at, Some(auth_log))
        .await
}

async fn update_postgres_auth_user_profile_internal(
    pool: &PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
    auth_log: Option<&AuthLogDraft>,
) -> DbResult<bool> {
    if updates.is_empty() {
        return Ok(false);
    }
    let user_id_sql = user_id_i64(user_id)?;
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let row =
        sqlx::query("SELECT email, display_name, metadata FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id_sql)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(postgres_auth_error)?;
    let Some(row) = row else {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Ok(false);
    };
    let current_email: String = row.try_get("email").map_err(postgres_auth_error)?;
    let mut display_name: String = row.try_get("display_name").map_err(postgres_auth_error)?;
    let Json(mut metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    let mut email = current_email.clone();

    for update in updates {
        apply_postgres_profile_update(
            update,
            &mut email,
            &current_email,
            &mut display_name,
            &mut metadata,
        );
    }

    let changed = sqlx::query(
        r#"
        UPDATE users
        SET email = $1,
            display_name = $2,
            metadata = $3,
            updated_at = $4::timestamptz
        WHERE id = $5
        "#,
    )
    .bind(email)
    .bind(display_name)
    .bind(Json(metadata))
    .bind(updated_at)
    .bind(user_id_sql)
    .execute(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?
    .rows_affected()
        > 0;

    if changed {
        if let Some(auth_log) = auth_log {
            insert_postgres_auth_log_in_transaction(&mut transaction, auth_log).await?;
        }
    }
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(changed)
}
