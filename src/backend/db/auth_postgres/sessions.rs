#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：清理已过期 token session，保持 refresh/session 表不会无限累积历史记录。
pub async fn cleanup_postgres_expired_sessions(pool: &PostgresPool, now: &str) -> DbResult<u64> {
    sqlx::query(
        r#"
        DELETE FROM token_sessions
        WHERE (refresh_expires_at IS NULL AND expires_at < $1::timestamptz)
           OR (refresh_expires_at IS NOT NULL AND refresh_expires_at < $1::timestamptz)
        "#,
    )
    .bind(now)
    .execute(pool)
    .await
    .map(|result| result.rows_affected())
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按 access token hash 读取当前有效 session 用户，作为 Bearer token 认证主入口。
pub async fn get_postgres_auth_token_user(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<AuthTokenUserRow>> {
    sqlx::query(
        r#"
        SELECT id, username, COALESCE(password_hash, '') AS password_hash
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| {
        let raw_id: i64 = row.try_get("id").map_err(postgres_auth_error)?;
        Ok(AuthTokenUserRow {
            id: user_id_from_i64(raw_id)?,
            username: row.try_get("username").map_err(postgres_auth_error)?,
            password_hash: row.try_get("password_hash").map_err(postgres_auth_error)?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：列出用户所有 token session，并投影当前 session、设备和最近使用时间给前端安全页。
pub async fn list_postgres_user_sessions(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<TokenSessionRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, COALESCE(user_agent, '') AS user_agent,
               COALESCE(ip_address, '') AS ip_address,
               to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS') AS expires_at,
               COALESCE(to_char(last_activity_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS'), '') AS last_activity_at,
               to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS') AS created_at
        FROM token_sessions
        WHERE user_id = $1 AND is_active = TRUE
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_all(pool)
    .await
    .map_err(postgres_auth_error)?;

    rows.into_iter()
        .map(|row| {
            Ok(TokenSessionRow {
                id: row.try_get("id").map_err(postgres_auth_error)?,
                user_agent: row.try_get("user_agent").map_err(postgres_auth_error)?,
                ip_address: row.try_get("ip_address").map_err(postgres_auth_error)?,
                expires_at: row.try_get("expires_at").map_err(postgres_auth_error)?,
                last_activity_at: row
                    .try_get("last_activity_at")
                    .map_err(postgres_auth_error)?,
                created_at: row.try_get("created_at").map_err(postgres_auth_error)?,
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：根据 token hash 查询仍有效的 session id，用于撤销当前 token 或校验会话存在性。
pub async fn get_postgres_active_session_id_by_token_hash(
    pool: &PostgresPool,
    token_hash: &str,
) -> DbResult<Option<i64>> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM token_sessions
        WHERE token_hash = $1 AND is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按 access token hash、用户、active 与过期时间读取唯一授权 session。
pub async fn get_postgres_authoritative_access_session_id(
    pool: &PostgresPool,
    token_hash: &str,
    user_id: UserId,
) -> DbResult<Option<i64>> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM token_sessions
        WHERE token_hash = $1
          AND user_id = $2
          AND is_active = TRUE
          AND expires_at > now()
        LIMIT 1
        "#,
    )
    .bind(token_hash)
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：读取仍有效的 refresh session，确保 refresh token 轮换只发生在当前可用会话内。
pub async fn get_postgres_active_refresh_session(
    pool: &PostgresPool,
    refresh_token_hash: &str,
) -> DbResult<Option<AuthRefreshSessionRow>> {
    sqlx::query(
        r#"
        SELECT s.id, s.user_id, u.username,
               COALESCE(to_char(s.refresh_expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS'), '') AS refresh_expires_at,
               COALESCE((u.metadata->>'is_active')::boolean, TRUE) AS user_is_active
        FROM token_sessions s
        JOIN users u ON s.user_id = u.id
        WHERE s.refresh_token_hash = $1 AND s.is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(refresh_token_hash)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| {
        let raw_user_id: i64 = row.try_get("user_id").map_err(postgres_auth_error)?;
        Ok(AuthRefreshSessionRow {
            id: row.try_get("id").map_err(postgres_auth_error)?,
            user_id: user_id_from_i64(raw_user_id)?,
            username: row.try_get("username").map_err(postgres_auth_error)?,
            refresh_expires_at: row
                .try_get("refresh_expires_at")
                .map_err(postgres_auth_error)?,
            user_is_active: row.try_get("user_is_active").map_err(postgres_auth_error)?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：创建新的 token session，保存 access/refresh hash 与设备信息供后续撤销和轮换。
pub async fn create_postgres_token_session(
    pool: &PostgresPool,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let session_id = create_postgres_token_session_in_tx(&mut transaction, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(session_id)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：轮换 refresh token session，原子更新 access/refresh hash 和过期时间。
pub async fn rotate_postgres_refresh_token_session(
    pool: &PostgresPool,
    consumed_session_id: i64,
    consumed_refresh_token_hash: &str,
    draft: &CreateTokenSessionDraft,
) -> DbResult<Option<i64>> {
    let user_id = user_id_i64(draft.user_id)?;
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let consumed = sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE,
            refresh_token_hash = NULL,
            updated_at = now(),
            version = version + 1
        WHERE id = $1
          AND user_id = $2
          AND refresh_token_hash = $3
          AND is_active = TRUE
        "#,
    )
    .bind(consumed_session_id)
    .bind(user_id)
    .bind(consumed_refresh_token_hash)
    .execute(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    if consumed.rows_affected() == 0 {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Ok(None);
    }
    let session_id = create_postgres_token_session_in_tx(&mut transaction, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(Some(session_id))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按 session id 失效当前用户的指定会话，避免跨用户撤销其他人的 token。
pub async fn invalidate_postgres_session_by_id(
    pool: &PostgresPool,
    session_id: i64,
    user_id: UserId,
) -> DbResult<bool> {
    sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE, updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(session_id)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map(|result| result.rows_affected() > 0)
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按 token hash 失效当前会话，服务于 logout 和旧 refresh token best-effort 撤销。
pub async fn invalidate_postgres_session_by_token_hash(
    pool: &PostgresPool,
    token_hash: &str,
) -> DbResult<bool> {
    sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE, updated_at = now(), version = version + 1
        WHERE token_hash = $1 AND is_active = TRUE
        "#,
    )
    .bind(token_hash)
    .execute(pool)
    .await
    .map(|result| result.rows_affected() > 0)
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：撤销当前用户除指定 session 外的其他会话，支持改密后踢出其他设备。
pub async fn invalidate_other_postgres_user_sessions(
    pool: &PostgresPool,
    user_id: UserId,
    current_session_id: i64,
) -> DbResult<u64> {
    sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE, updated_at = now(), version = version + 1
        WHERE user_id = $1 AND id != $2 AND is_active = TRUE
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(current_session_id)
    .execute(pool)
    .await
    .map(|result| result.rows_affected())
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：统计近期 token 密码校验失败次数，作为敏感操作限流和锁定依据。
pub async fn count_postgres_recent_token_password_failures(
    pool: &PostgresPool,
    user_id: UserId,
    since: &str,
) -> DbResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'auth'
          AND metadata->>'success' = 'false'
          AND action IN ('api_token_generate_failed', 'mcp_token_generate_failed')
          AND created_at >= $2::timestamptz
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(since)
    .fetch_one(pool)
    .await
    .map_err(postgres_auth_error)
}
