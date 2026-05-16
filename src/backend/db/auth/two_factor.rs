pub fn hash_two_factor_recovery_code(recovery_code: &str) -> Option<String> {
    recovery_code_hash_input(recovery_code)
        .map(|value| format!("{:x}", Sha256::digest(value.as_bytes())))
}

pub fn replace_two_factor_recovery_codes(
    connection: &Connection,
    user_id: UserId,
    recovery_codes: &[&str],
    now: &str,
) -> DbResult<usize> {
    let user_id = user_id_sql(user_id)?;
    let code_hashes = normalized_two_factor_recovery_code_hashes(recovery_codes);

    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result =
        replace_two_factor_recovery_code_hashes_in_tx(connection, user_id, &code_hashes, now);

    match result {
        Ok(count) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(count)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn enable_two_factor_with_recovery_codes_and_session(
    connection: &Connection,
    user_id: UserId,
    secret: &str,
    recovery_codes: &[&str],
    session_draft: &CreateTokenSessionDraft,
    now: &str,
) -> DbResult<(usize, i64)> {
    let target_user_id = user_id;
    if session_draft.user_id != target_user_id {
        return Err(DbError::InvalidOperation(
            "session user mismatch".to_string(),
        ));
    }
    let user_id = user_id_sql(target_user_id)?;
    let code_hashes = normalized_two_factor_recovery_code_hashes(recovery_codes);

    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let changed = connection.execute(
            r#"
            UPDATE users
            SET two_factor_enabled = 1,
                two_factor_secret = ?1,
                updated_at = ?2
            WHERE id = ?3
              AND COALESCE(two_factor_enabled, 0) = 0
            "#,
            params![secret, now, user_id],
        )?;
        if changed == 0 {
            let enabled = connection
                .query_row(
                    "SELECT COALESCE(two_factor_enabled, 0) FROM users WHERE id = ?1",
                    [user_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if enabled.is_some_and(|value| value != 0) {
                return Err(DbError::InvalidOperation(
                    "two-factor authentication is already enabled".to_string(),
                ));
            }
            return Err(DbError::InvalidOperation("user not found".to_string()));
        }
        let stored_count =
            replace_two_factor_recovery_code_hashes_in_tx(connection, user_id, &code_hashes, now)?;
        let session_id = create_token_session(connection, session_draft)?;
        Ok((stored_count, session_id))
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

pub fn disable_two_factor_and_clear_recovery_codes(
    connection: &Connection,
    user_id: UserId,
    now: &str,
) -> DbResult<usize> {
    let user_id = user_id_sql(user_id)?;

    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let changed = connection.execute(
            r#"
            UPDATE users
            SET two_factor_enabled = 0,
                two_factor_secret = '',
                updated_at = ?1
            WHERE id = ?2
            "#,
            params![now, user_id],
        )?;
        if changed == 0 {
            return Err(DbError::InvalidOperation("user not found".to_string()));
        }
        let cleared = connection.execute(
            "DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?1",
            [user_id],
        )?;
        Ok(cleared)
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

pub fn consume_two_factor_recovery_code(
    connection: &Connection,
    user_id: UserId,
    recovery_code: &str,
    now: &str,
) -> DbResult<bool> {
    let Some(code_hash) = hash_two_factor_recovery_code(recovery_code) else {
        return Ok(false);
    };
    let user_id = user_id_sql(user_id)?;
    let changed = connection.execute(
        r#"
        UPDATE user_two_factor_recovery_codes
        SET used_at = ?1, updated_at = ?1
        WHERE user_id = ?2 AND code_hash = ?3 AND used_at IS NULL
        "#,
        params![now, user_id, code_hash],
    )?;
    Ok(changed > 0)
}

pub fn clear_two_factor_recovery_codes(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<usize> {
    let user_id = user_id_sql(user_id)?;
    Ok(connection.execute(
        "DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?1",
        [user_id],
    )?)
}

pub fn count_active_two_factor_recovery_codes(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<i64> {
    let user_id = user_id_sql(user_id)?;
    connection
        .query_row(
            "SELECT COUNT(*) FROM user_two_factor_recovery_codes WHERE user_id = ?1 AND used_at IS NULL",
            [user_id],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

fn normalized_two_factor_recovery_code_hashes(recovery_codes: &[&str]) -> Vec<String> {
    let mut seen_hashes = HashSet::new();
    let mut code_hashes = Vec::new();
    for recovery_code in recovery_codes {
        let Some(code_hash) = hash_two_factor_recovery_code(recovery_code) else {
            continue;
        };
        if seen_hashes.insert(code_hash.clone()) {
            code_hashes.push(code_hash);
        }
    }
    code_hashes
}

fn replace_two_factor_recovery_code_hashes_in_tx(
    connection: &Connection,
    user_id: i64,
    code_hashes: &[String],
    now: &str,
) -> DbResult<usize> {
    connection.execute(
        "DELETE FROM user_two_factor_recovery_codes WHERE user_id = ?1",
        [user_id],
    )?;
    if !code_hashes.is_empty() {
        let mut statement = connection.prepare(
            r#"
            INSERT INTO user_two_factor_recovery_codes (
                user_id, code_hash, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4)
            "#,
        )?;
        for code_hash in code_hashes {
            statement.execute(params![user_id, code_hash, now, now])?;
        }
    }
    Ok(code_hashes.len())
}
