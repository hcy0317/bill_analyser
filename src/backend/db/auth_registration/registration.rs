// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

pub fn auth_username_exists(connection: &Connection, username: &str) -> DbResult<bool> {
    exists_by_text(
        connection,
        "SELECT 1 FROM users WHERE username = ?1 LIMIT 1",
        username,
    )
}

pub fn auth_email_exists(connection: &Connection, email: &str) -> DbResult<bool> {
    exists_by_text(
        connection,
        "SELECT 1 FROM users WHERE email = ?1 LIMIT 1",
        email,
    )
}

pub fn create_registered_user_with_defaults(
    connection: &Connection,
    draft: &RegisterUserDraft,
    preset_categories: &[RegisterPresetCategory],
    auth_log: &AuthLogDraft,
) -> DbResult<RegisterUserResult> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let user_id = insert_registered_user(connection, draft)?;
        let preset_categories_saved = insert_register_preset_categories(
            connection,
            user_id,
            preset_categories,
            &draft.created_at,
        )?;
        let default_seed = ensure_default_category_seed(connection, user_id, &draft.created_at)?;
        let account_result = create_register_default_accounts(
            connection,
            user_id,
            &draft.language,
            &draft.created_at,
        )?;
        create_auth_log(
            connection,
            &AuthLogDraft {
                user_id: Some(
                    bill_analyser_core::UserId::new(user_id as u64).map_err(|_| {
                        DbError::InvalidOperation("registered user id must be positive".to_string())
                    })?,
                ),
                username: auth_log.username.clone(),
                event_type: auth_log.event_type.clone(),
                ip_address: auth_log.ip_address.clone(),
                user_agent: auth_log.user_agent.clone(),
                success: auth_log.success,
                error_message: auth_log.error_message.clone(),
                metadata: auth_log.metadata.clone(),
                created_at: auth_log.created_at.clone(),
            },
        )?;
        Ok(RegisterUserResult {
            user_id,
            preset_categories_saved,
            preset_accounts_saved: account_result.success,
            cash_account_id: account_result.cash_account_id,
            default_account_id: account_result.default_account_id,
            default_seed,
        })
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

fn exists_by_text(connection: &Connection, sql: &str, value: &str) -> DbResult<bool> {
    connection
        .query_row(sql, [value], |_| Ok(()))
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn insert_registered_user(connection: &Connection, draft: &RegisterUserDraft) -> DbResult<i64> {
    connection.execute(
        r#"
        INSERT INTO users (
            username, email, password_hash, nickname, language,
            default_currency, first_day_of_week, is_active, email_verified,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9)
        "#,
        params![
            draft.username,
            draft.email,
            draft.password_hash,
            draft.nickname,
            draft.language,
            draft.default_currency,
            draft.first_day_of_week,
            if draft.email_verified { 1 } else { 0 },
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}
