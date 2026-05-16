pub fn delete_application_cloud_settings(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    connection.execute(
        "DELETE FROM user_application_cloud_settings WHERE user_id = ?1",
        [user_id],
    )?;
    Ok(true)
}

pub fn list_user_sessions(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<TokenSessionRow>> {
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

pub fn create_token_session(
    connection: &Connection,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
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
