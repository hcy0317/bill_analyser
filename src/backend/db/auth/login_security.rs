pub fn increment_failed_login(
    connection: &Connection,
    user_id: UserId,
    max_login_attempts: i64,
    lockout_until: &str,
    reset_locked_until: Option<&str>,
) -> DbResult<Option<LoginFailureUpdate>> {
    let user_id = user_id_sql(user_id)?;
    let max_login_attempts = max_login_attempts.max(1);
    connection.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| {
        let login_state = connection
            .query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = ?1",
                [user_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional()?;
        let Some((failed_attempts, current_locked_until)) = login_state else {
            return Ok(None);
        };
        let reset_expired_lock = reset_locked_until
            .filter(|value| !value.trim().is_empty())
            .is_some_and(|value| current_locked_until.as_deref() == Some(value));
        let failed_attempts = if reset_expired_lock {
            0
        } else {
            failed_attempts.unwrap_or(0)
        };
        let failed_attempts = failed_attempts + 1;
        let locked = failed_attempts >= max_login_attempts;
        if locked {
            connection.execute(
                "UPDATE users SET failed_login_attempts = ?1, locked_until = ?2 WHERE id = ?3",
                params![failed_attempts, lockout_until, user_id],
            )?;
        } else if reset_expired_lock {
            connection.execute(
                "UPDATE users SET failed_login_attempts = ?1, locked_until = NULL WHERE id = ?2",
                params![failed_attempts, user_id],
            )?;
        } else {
            connection.execute(
                "UPDATE users SET failed_login_attempts = ?1 WHERE id = ?2",
                params![failed_attempts, user_id],
            )?;
        }
        Ok(Some(LoginFailureUpdate {
            failed_attempts,
            locked,
        }))
    })();

    match result {
        Ok(value) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(value)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn clear_expired_login_lock(connection: &Connection, user_id: UserId) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        "UPDATE users SET locked_until = NULL, failed_login_attempts = 0 WHERE id = ?1",
        [user_id],
    )?;
    Ok(changed > 0)
}

pub fn update_user_last_login(
    connection: &Connection,
    user_id: UserId,
    last_login_at: &str,
    ip_address: &str,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        r#"
        UPDATE users
        SET last_login_at = ?1, last_login_ip = ?2, failed_login_attempts = 0, locked_until = NULL
        WHERE id = ?3
        "#,
        params![last_login_at, ip_address, user_id],
    )?;
    Ok(changed > 0)
}

pub fn set_user_email_verified(
    connection: &Connection,
    user_id: UserId,
    verified: bool,
    updated_at: &str,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        "UPDATE users SET email_verified = ?1, updated_at = ?2 WHERE id = ?3",
        params![if verified { 1 } else { 0 }, updated_at, user_id],
    )?;
    Ok(changed > 0)
}

pub fn update_user_password_hash(
    connection: &Connection,
    user_id: UserId,
    password_hash: &str,
    updated_at: &str,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
        params![password_hash, updated_at, user_id],
    )?;
    Ok(changed > 0)
}

pub fn count_recent_token_password_failures(
    connection: &Connection,
    user_id: UserId,
    since: &str,
) -> DbResult<i64> {
    let user_id = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM auth_logs
            WHERE user_id = ?1
              AND success = 0
              AND event_type IN ('api_token_generate_failed', 'mcp_token_generate_failed')
              AND created_at >= ?2
            "#,
            params![user_id, since],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

pub fn invalidate_session_by_id(
    connection: &Connection,
    session_id: i64,
    user_id: UserId,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        "UPDATE sessions SET is_active = 0 WHERE id = ?1 AND user_id = ?2",
        params![session_id, user_id],
    )?;
    Ok(changed > 0)
}

pub fn invalidate_session_by_token_hash(
    connection: &Connection,
    token_hash: &str,
) -> DbResult<bool> {
    let changed = connection.execute(
        "UPDATE sessions SET is_active = 0 WHERE token_hash = ?1 AND is_active = 1",
        [token_hash],
    )?;
    Ok(changed > 0)
}

pub fn invalidate_other_user_sessions(
    connection: &Connection,
    user_id: UserId,
    current_session_id: i64,
) -> DbResult<usize> {
    let user_id = user_id_sql(user_id)?;
    Ok(connection.execute(
        "UPDATE sessions SET is_active = 0 WHERE user_id = ?1 AND id != ?2 AND is_active = 1",
        params![user_id, current_session_id],
    )?)
}
