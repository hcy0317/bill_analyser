#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：写入认证审计日志，记录登录、敏感操作和 profile 变更等安全事件。
pub async fn create_postgres_auth_log(pool: &PostgresPool, draft: &AuthLogDraft) -> DbResult<i64> {
    let metadata = auth_log_metadata(draft);
    let row = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor, metadata, created_at
        ) VALUES ($1, 'auth', $2, $3, $4, $5, $6::timestamptz)
        RETURNING id
        "#,
    )
    .bind(draft.user_id.map(user_id_i64).transpose()?)
    .bind(
        draft
            .user_id
            .map(|value| value.get().to_string())
            .unwrap_or_else(|| draft.username.clone()),
    )
    .bind(&draft.event_type)
    .bind(&draft.username)
    .bind(Json(metadata))
    .bind(&draft.created_at)
    .fetch_one(pool)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：在事件限流窗口内写入认证日志，超过阈值时拒绝以保护敏感操作入口。
pub async fn create_postgres_auth_log_under_event_limit(
    pool: &PostgresPool,
    user_id: UserId,
    event_type: &str,
    since: &str,
    limit: i64,
    draft: &AuthLogDraft,
) -> DbResult<bool> {
    if draft.user_id != Some(user_id) {
        return Err(DbError::InvalidOperation(
            "auth log user does not match event limit user".to_string(),
        ));
    }
    if draft.event_type != event_type {
        return Err(DbError::InvalidOperation(
            "auth log event type does not match event limit type".to_string(),
        ));
    }

    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'auth'
          AND action = $2
          AND created_at >= $3::timestamptz
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(event_type)
    .bind(since)
    .fetch_one(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    if count >= limit {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Ok(false);
    }

    insert_postgres_auth_log_in_transaction(&mut transaction, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：统计指定事件在时间窗口内的次数，服务于登录失败和敏感操作限流。
pub async fn count_postgres_auth_events_since(
    pool: &PostgresPool,
    user_id: UserId,
    event_type: &str,
    window_start: &str,
) -> DbResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'auth'
          AND action = $2
          AND metadata->>'success' = 'false'
          AND created_at >= $3::timestamptz
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(event_type)
    .bind(window_start)
    .fetch_one(pool)
    .await
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：读取用户操作密码 hash，供 step-up 或敏感数据操作校验当前密码。
pub async fn get_postgres_operation_password(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<String>> {
    let value: Option<Value> =
        sqlx::query_scalar("SELECT value FROM settings WHERE user_id = $1 AND key = $2")
            .bind(user_id_i64(user_id)?)
            .bind("operation_password")
            .fetch_optional(pool)
            .await
            .map_err(postgres_auth_error)?;
    Ok(value.as_ref().and_then(json_setting_string))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：更新用户邮箱验证状态，支持邮箱验证完成和邮箱变更后的状态重置。
pub async fn set_postgres_user_email_verified(
    pool: &PostgresPool,
    user_id: UserId,
    verified: bool,
    updated_at: &str,
) -> DbResult<bool> {
    let row = sqlx::query("SELECT metadata FROM users WHERE id = $1")
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map_err(postgres_auth_error)?;
    let Some(row) = row else {
        return Ok(false);
    };
    let Json(mut metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    metadata_set_bool(&mut metadata, "email_verified", verified);
    let result = sqlx::query(
        r#"
        UPDATE users
        SET metadata = $1,
            updated_at = COALESCE(NULLIF($2, '')::timestamptz, now()),
            version = version + 1
        WHERE id = $3
        "#,
    )
    .bind(Json(metadata))
    .bind(updated_at)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map_err(postgres_auth_error)?;
    Ok(result.rows_affected() > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：更新用户密码 hash，并同步修改时间、失败计数和可选 token 会话清理策略。
pub async fn update_postgres_user_password_hash(
    pool: &PostgresPool,
    user_id: UserId,
    password_hash: &str,
    updated_at: &str,
) -> DbResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET password_hash = $1,
            updated_at = COALESCE(NULLIF($2, '')::timestamptz, now()),
            version = version + 1
        WHERE id = $3
        "#,
    )
    .bind(password_hash)
    .bind(updated_at)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map_err(postgres_auth_error)?;
    Ok(result.rows_affected() > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：记录用户最近登录时间并清零登录失败计数，供登录成功后的状态维护使用。
pub async fn update_postgres_user_last_login(
    pool: &PostgresPool,
    user_id: UserId,
    now: &str,
    ip_address: &str,
) -> DbResult<bool> {
    let mut metadata = load_user_metadata(pool, user_id).await?;
    metadata_set_string(&mut metadata, "last_login_at", now);
    metadata_set_string(&mut metadata, "last_login_ip", ip_address);
    metadata_set_i64(&mut metadata, "failed_login_attempts", 0);
    metadata_set_string(&mut metadata, "locked_until", "");
    update_user_metadata(pool, user_id, metadata, now).await
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：递增用户登录失败次数，供登录锁定和安全审计判断使用。
pub async fn increment_postgres_failed_login(
    pool: &PostgresPool,
    user_id: UserId,
    max_attempts: i64,
    lockout_until: &str,
    expired_locked_until: Option<&str>,
) -> DbResult<LoginFailureUpdate> {
    let mut metadata = load_user_metadata(pool, user_id).await?;
    if expired_locked_until.is_some() {
        metadata_set_i64(&mut metadata, "failed_login_attempts", 0);
        metadata_set_string(&mut metadata, "locked_until", "");
    }
    let failed_attempts = metadata_i64(&metadata, &["failed_login_attempts"], 0) + 1;
    let locked = failed_attempts >= max_attempts;
    metadata_set_i64(&mut metadata, "failed_login_attempts", failed_attempts);
    if locked {
        metadata_set_string(&mut metadata, "locked_until", lockout_until);
    }
    update_user_metadata(pool, user_id, metadata, "").await?;
    Ok(LoginFailureUpdate {
        failed_attempts,
        locked,
    })
}
