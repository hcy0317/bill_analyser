pub fn cleanup_expired_sessions(connection: &Connection, now: &str) -> DbResult<usize> {
    Ok(connection.execute(
        r#"
        DELETE FROM sessions
        WHERE (refresh_expires_at IS NULL AND expires_at < ?1)
           OR (refresh_expires_at IS NOT NULL AND refresh_expires_at < ?1)
        "#,
        [now],
    )?)
}

pub fn get_auth_token_user(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Option<AuthTokenUserRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT id, username, password_hash
            FROM users
            WHERE id = ?1
            "#,
            [user_id_sql],
            |row| {
                let raw_id: i64 = row.get(0)?;
                let raw_id = u64::try_from(raw_id)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, raw_id))?;
                let id = UserId::new(raw_id)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, raw_id as i64))?;
                Ok(AuthTokenUserRow {
                    id,
                    username: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    password_hash: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_login_user_by_login_name(
    connection: &Connection,
    login_name: &str,
) -> DbResult<Option<AuthLoginUserRow>> {
    connection
        .query_row(
            r#"
            SELECT
                id, username, email, nickname, avatar, default_account_id,
                transaction_edit_scope, language, default_currency, first_day_of_week,
                fiscal_year_start, calendar_display_type, date_display_type,
                long_date_format, short_date_format, long_time_format, short_time_format,
                fiscal_year_format, currency_display_type, numeral_system, decimal_separator,
                digit_grouping_symbol, digit_grouping, coordinate_display_type,
                expense_amount_color, income_amount_color, cash_account_id,
                cash_transfer_category_id, import_learning_enabled,
                investment_platform_keywords, investment_product_keywords,
                investment_exclude_keywords, email_verified,
                password_hash, is_active, two_factor_enabled, two_factor_secret,
                failed_login_attempts, locked_until
            FROM users
            WHERE username = ?1 OR email = ?1
            ORDER BY CASE WHEN username = ?1 THEN 0 ELSE 1 END, id ASC
            LIMIT 1
            "#,
            [login_name],
            auth_login_user_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_login_user_by_email(
    connection: &Connection,
    email: &str,
) -> DbResult<Option<AuthLoginUserRow>> {
    connection
        .query_row(
            r#"
            SELECT
                id, username, email, nickname, avatar, default_account_id,
                transaction_edit_scope, language, default_currency, first_day_of_week,
                fiscal_year_start, calendar_display_type, date_display_type,
                long_date_format, short_date_format, long_time_format, short_time_format,
                fiscal_year_format, currency_display_type, numeral_system, decimal_separator,
                digit_grouping_symbol, digit_grouping, coordinate_display_type,
                expense_amount_color, income_amount_color, cash_account_id,
                cash_transfer_category_id, import_learning_enabled,
                investment_platform_keywords, investment_product_keywords,
                investment_exclude_keywords, email_verified,
                password_hash, is_active, two_factor_enabled, two_factor_secret,
                failed_login_attempts, locked_until
            FROM users
            WHERE email = ?1
            ORDER BY id ASC
            LIMIT 1
            "#,
            [email],
            auth_login_user_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_login_user_by_id(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Option<AuthLoginUserRow>> {
    let user_id = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT
                id, username, email, nickname, avatar, default_account_id,
                transaction_edit_scope, language, default_currency, first_day_of_week,
                fiscal_year_start, calendar_display_type, date_display_type,
                long_date_format, short_date_format, long_time_format, short_time_format,
                fiscal_year_format, currency_display_type, numeral_system, decimal_separator,
                digit_grouping_symbol, digit_grouping, coordinate_display_type,
                expense_amount_color, income_amount_color, cash_account_id,
                cash_transfer_category_id, import_learning_enabled,
                investment_platform_keywords, investment_product_keywords,
                investment_exclude_keywords, email_verified,
                password_hash, is_active, two_factor_enabled, two_factor_secret,
                failed_login_attempts, locked_until
            FROM users
            WHERE id = ?1
            "#,
            [user_id],
            auth_login_user_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_active_refresh_session(
    connection: &Connection,
    refresh_token_hash: &str,
) -> DbResult<Option<AuthRefreshSessionRow>> {
    connection
        .query_row(
            r#"
            SELECT s.id, s.user_id, u.username, s.refresh_expires_at, u.is_active
            FROM sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.refresh_token_hash = ?1 AND s.is_active = 1
            "#,
            [refresh_token_hash],
            |row| {
                let raw_user_id: i64 = row.get(1)?;
                Ok(AuthRefreshSessionRow {
                    id: row.get(0)?,
                    user_id: user_id_from_sql(raw_user_id, 1)?,
                    username: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    refresh_expires_at: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    user_is_active: row.get::<_, i64>(4)? == 1,
                })
            },
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_active_logout_session_by_token_hash(
    connection: &Connection,
    token_hash: &str,
) -> DbResult<Option<AuthLogoutSessionRow>> {
    connection
        .query_row(
            r#"
            SELECT s.id, s.user_id, u.username
            FROM sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.token_hash = ?1 AND s.is_active = 1
            "#,
            [token_hash],
            |row| {
                let raw_user_id: i64 = row.get(1)?;
                Ok(AuthLogoutSessionRow {
                    id: row.get(0)?,
                    user_id: user_id_from_sql(raw_user_id, 1)?,
                    username: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_auth_user_profile(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Option<AuthUserProfileRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT
                id, username, email, nickname, avatar, default_account_id,
                transaction_edit_scope, language, default_currency, first_day_of_week,
                fiscal_year_start, calendar_display_type, date_display_type,
                long_date_format, short_date_format, long_time_format, short_time_format,
                fiscal_year_format, currency_display_type, numeral_system, decimal_separator,
                digit_grouping_symbol, digit_grouping, coordinate_display_type,
                expense_amount_color, income_amount_color, cash_account_id,
                cash_transfer_category_id, import_learning_enabled,
                investment_platform_keywords, investment_product_keywords,
                investment_exclude_keywords, email_verified
            FROM users
            WHERE id = ?1
            "#,
            [user_id_sql],
            auth_user_profile_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_auth_user_two_factor_enabled(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Option<bool>> {
    let user_id_sql = user_id_sql(user_id)?;
    connection
        .query_row(
            "SELECT COALESCE(two_factor_enabled, 0) FROM users WHERE id = ?1",
            [user_id_sql],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()
        .map_err(DbError::from)
}
