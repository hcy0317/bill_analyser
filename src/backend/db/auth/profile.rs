// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

pub fn auth_email_exists_for_other_user(
    connection: &Connection,
    user_id: UserId,
    email: &str,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    connection
        .query_row(
            "SELECT 1 FROM users WHERE email = ?1 AND id != ?2 LIMIT 1",
            params![email, user_id],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

pub fn auth_account_belongs_to_user(
    connection: &Connection,
    user_id: UserId,
    account_id: i64,
) -> DbResult<bool> {
    scoped_id_exists(connection, "accounts", user_id, account_id)
}

pub fn auth_category_belongs_to_user(
    connection: &Connection,
    user_id: UserId,
    category_id: i64,
) -> DbResult<bool> {
    scoped_id_exists(connection, "categories", user_id, category_id)
}

pub fn count_auth_events_since(
    connection: &Connection,
    user_id: UserId,
    event_type: &str,
    since: &str,
) -> DbResult<i64> {
    let user_id = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM auth_logs
            WHERE user_id = ?1 AND event_type = ?2 AND created_at >= ?3
            "#,
            params![user_id, event_type, since],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

pub fn create_auth_log_under_event_limit(
    connection: &Connection,
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
    connection.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| {
        let count = count_auth_events_since(connection, user_id, event_type, since)?;
        if count >= limit {
            return Ok(false);
        }
        create_auth_log(connection, draft)?;
        Ok(true)
    })();

    match result {
        Ok(true) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(true)
        }
        Ok(false) => {
            let _ = connection.execute_batch("ROLLBACK");
            Ok(false)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn update_auth_user_profile(
    connection: &Connection,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
) -> DbResult<bool> {
    if updates.is_empty() {
        return Ok(false);
    }
    let user_id = user_id_sql(user_id)?;
    connection.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| {
        let mut touched = false;
        for update in updates {
            touched |= apply_user_profile_update(connection, user_id, update)?;
        }
        let updated = connection.execute(
            "UPDATE users SET updated_at = ?1 WHERE id = ?2",
            params![updated_at, user_id],
        )?;
        Ok(touched && updated > 0)
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

pub fn update_auth_user_profile_with_auth_log(
    connection: &Connection,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
    auth_log: &AuthLogDraft,
) -> DbResult<bool> {
    if updates.is_empty() {
        return Ok(false);
    }
    let user_id = user_id_sql(user_id)?;
    connection.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| {
        let mut touched = false;
        for update in updates {
            touched |= apply_user_profile_update(connection, user_id, update)?;
        }
        let updated = connection.execute(
            "UPDATE users SET updated_at = ?1 WHERE id = ?2",
            params![updated_at, user_id],
        )?;
        let changed = touched && updated > 0;
        if changed {
            create_auth_log(connection, auth_log)?;
        }
        Ok(changed)
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

pub fn update_application_cloud_settings(
    connection: &Connection,
    user_id: UserId,
    settings: &[ApplicationCloudSettingDraft],
    full_update: bool,
    updated_at: &str,
) -> DbResult<bool> {
    let user_id = user_id_sql(user_id)?;
    connection.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| {
        let normalized_settings = settings
            .iter()
            .filter(|setting| !setting.setting_key.trim().is_empty())
            .collect::<Vec<_>>();
        if full_update {
            if normalized_settings.is_empty() {
                connection.execute(
                    "DELETE FROM user_application_cloud_settings WHERE user_id = ?1",
                    [user_id],
                )?;
            } else {
                let keep_keys = normalized_settings
                    .iter()
                    .map(|setting| setting.setting_key.trim().to_string())
                    .collect::<HashSet<_>>();
                let existing_keys = list_application_cloud_setting_keys(connection, user_id)?;
                for existing_key in existing_keys {
                    if !keep_keys.contains(&existing_key) {
                        connection.execute(
                            "DELETE FROM user_application_cloud_settings WHERE user_id = ?1 AND setting_key = ?2",
                            params![user_id, existing_key],
                        )?;
                    }
                }
            }
        }

        for setting in normalized_settings {
            connection.execute(
                r#"
                INSERT INTO user_application_cloud_settings (
                    user_id, setting_key, setting_value, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?4)
                ON CONFLICT(user_id, setting_key) DO UPDATE SET
                    setting_value = excluded.setting_value,
                    updated_at = excluded.updated_at
                "#,
                params![
                    user_id,
                    setting.setting_key.trim(),
                    setting.setting_value,
                    updated_at,
                ],
            )?;
        }
        Ok(true)
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
