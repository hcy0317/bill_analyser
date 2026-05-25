// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
pub fn delete_application_cloud_settings(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "delete_application_cloud_settings", "business operation entered");
    let user_id = user_id_sql(user_id)?;
    connection.execute(
        "DELETE FROM user_application_cloud_settings WHERE user_id = ?1",
        [user_id],
    )?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_user_sessions(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<TokenSessionRow>> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "list_user_sessions", "business operation entered");
    let user_id = user_id_sql(user_id)?;
    let mut statement = connection.prepare(
        r#"
        SELECT id, user_agent, ip_address, expires_at, last_activity_at, created_at
        FROM sessions
        WHERE user_id = ?1 AND is_active = 1
        ORDER BY created_at DESC
        "#,
    )?;
    let rows = statement.query_map([user_id], |row| {
        Ok(TokenSessionRow {
            id: row.get(0)?,
            user_agent: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ip_address: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            expires_at: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            last_activity_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            created_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn create_token_session(
    connection: &Connection,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "create_token_session", "business operation entered");
    let user_id = user_id_sql(draft.user_id)?;
    connection.execute(
        r#"
        INSERT INTO sessions (
            user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, NULL, ?8)
        "#,
        params![
            user_id,
            draft.token_hash,
            draft.refresh_token_hash,
            draft.expires_at,
            draft.refresh_expires_at,
            draft.user_agent,
            draft.ip_address,
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn rotate_refresh_token_session(
    connection: &Connection,
    consumed_session_id: i64,
    consumed_refresh_token_hash: &str,
    draft: &CreateTokenSessionDraft,
) -> DbResult<Option<i64>> {
    let user_id = user_id_sql(draft.user_id)?;
    connection.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| {
        let consumed = connection.execute(
            r#"
            UPDATE sessions
            SET is_active = 0, refresh_token_hash = NULL
            WHERE id = ?1
              AND user_id = ?2
              AND refresh_token_hash = ?3
              AND is_active = 1
            "#,
            params![consumed_session_id, user_id, consumed_refresh_token_hash],
        )?;
        if consumed == 0 {
            return Ok(None);
        }
        create_token_session(connection, draft).map(Some)
    })();

    match result {
        Ok(Some(session_id)) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(Some(session_id))
        }
        Ok(None) => {
            let _ = connection.execute_batch("ROLLBACK");
            Ok(None)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}
