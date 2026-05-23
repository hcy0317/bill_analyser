// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

fn user_id_sql(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get()).map_err(|_| {
        DbError::InvalidOperation("user id is outside SQLite INTEGER range".to_string())
    })
}

fn user_id_from_sql(raw_id: i64, column: usize) -> rusqlite::Result<UserId> {
    let raw_id = u64::try_from(raw_id)
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(column, raw_id))?;
    UserId::new(raw_id).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(column, raw_id as i64))
}

fn external_auth_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExternalAuthRow> {
    Ok(ExternalAuthRow {
        external_auth_category: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
        external_auth_type: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        external_username: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        created_at: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
    })
}

fn apply_user_profile_update(
    connection: &Connection,
    user_id: i64,
    update: &AuthUserProfileUpdate,
) -> DbResult<bool> {
    let changed = match update {
        AuthUserProfileUpdate::Nickname(value) => {
            update_text_field(connection, "nickname", value, user_id)?
        }
        AuthUserProfileUpdate::Email(value) => update_email_field(connection, value, user_id)?,
        AuthUserProfileUpdate::Avatar(value) => {
            update_text_field(connection, "avatar", value, user_id)?
        }
        AuthUserProfileUpdate::Language(value) => {
            update_text_field(connection, "language", value, user_id)?
        }
        AuthUserProfileUpdate::DefaultCurrency(value) => {
            update_text_field(connection, "default_currency", value, user_id)?
        }
        AuthUserProfileUpdate::FirstDayOfWeek(value) => {
            update_i64_field(connection, "first_day_of_week", *value, user_id)?
        }
        AuthUserProfileUpdate::DefaultAccountId(value) => {
            update_optional_i64_field(connection, "default_account_id", *value, user_id)?
        }
        AuthUserProfileUpdate::TransactionEditScope(value) => {
            update_i64_field(connection, "transaction_edit_scope", *value, user_id)?
        }
        AuthUserProfileUpdate::FiscalYearStart(value) => {
            update_i64_field(connection, "fiscal_year_start", *value, user_id)?
        }
        AuthUserProfileUpdate::CalendarDisplayType(value) => {
            update_i64_field(connection, "calendar_display_type", *value, user_id)?
        }
        AuthUserProfileUpdate::DateDisplayType(value) => {
            update_i64_field(connection, "date_display_type", *value, user_id)?
        }
        AuthUserProfileUpdate::LongDateFormat(value) => {
            update_i64_field(connection, "long_date_format", *value, user_id)?
        }
        AuthUserProfileUpdate::ShortDateFormat(value) => {
            update_i64_field(connection, "short_date_format", *value, user_id)?
        }
        AuthUserProfileUpdate::LongTimeFormat(value) => {
            update_i64_field(connection, "long_time_format", *value, user_id)?
        }
        AuthUserProfileUpdate::ShortTimeFormat(value) => {
            update_i64_field(connection, "short_time_format", *value, user_id)?
        }
        AuthUserProfileUpdate::FiscalYearFormat(value) => {
            update_i64_field(connection, "fiscal_year_format", *value, user_id)?
        }
        AuthUserProfileUpdate::CurrencyDisplayType(value) => {
            update_i64_field(connection, "currency_display_type", *value, user_id)?
        }
        AuthUserProfileUpdate::NumeralSystem(value) => {
            update_i64_field(connection, "numeral_system", *value, user_id)?
        }
        AuthUserProfileUpdate::DecimalSeparator(value) => {
            update_i64_field(connection, "decimal_separator", *value, user_id)?
        }
        AuthUserProfileUpdate::DigitGroupingSymbol(value) => {
            update_i64_field(connection, "digit_grouping_symbol", *value, user_id)?
        }
        AuthUserProfileUpdate::DigitGrouping(value) => {
            update_i64_field(connection, "digit_grouping", *value, user_id)?
        }
        AuthUserProfileUpdate::CoordinateDisplayType(value) => {
            update_i64_field(connection, "coordinate_display_type", *value, user_id)?
        }
        AuthUserProfileUpdate::ExpenseAmountColor(value) => {
            update_i64_field(connection, "expense_amount_color", *value, user_id)?
        }
        AuthUserProfileUpdate::IncomeAmountColor(value) => {
            update_i64_field(connection, "income_amount_color", *value, user_id)?
        }
        AuthUserProfileUpdate::CashAccountId(value) => {
            update_optional_i64_field(connection, "cash_account_id", *value, user_id)?
        }
        AuthUserProfileUpdate::CashTransferCategoryId(value) => {
            update_optional_i64_field(connection, "cash_transfer_category_id", *value, user_id)?
        }
        AuthUserProfileUpdate::ImportLearningEnabled(value) => update_i64_field(
            connection,
            "import_learning_enabled",
            i64::from(*value),
            user_id,
        )?,
        AuthUserProfileUpdate::InvestmentPlatformKeywords(value) => {
            update_text_field(connection, "investment_platform_keywords", value, user_id)?
        }
        AuthUserProfileUpdate::InvestmentProductKeywords(value) => {
            update_text_field(connection, "investment_product_keywords", value, user_id)?
        }
        AuthUserProfileUpdate::InvestmentExcludeKeywords(value) => {
            update_text_field(connection, "investment_exclude_keywords", value, user_id)?
        }
    };
    Ok(changed)
}

fn update_text_field(
    connection: &Connection,
    field_name: &str,
    value: &str,
    user_id: i64,
) -> DbResult<bool> {
    let sql = format!("UPDATE users SET {field_name} = ?1 WHERE id = ?2");
    Ok(connection.execute(&sql, params![value, user_id])? > 0)
}

fn update_i64_field(
    connection: &Connection,
    field_name: &str,
    value: i64,
    user_id: i64,
) -> DbResult<bool> {
    let sql = format!("UPDATE users SET {field_name} = ?1 WHERE id = ?2");
    Ok(connection.execute(&sql, params![value, user_id])? > 0)
}

fn update_optional_i64_field(
    connection: &Connection,
    field_name: &str,
    value: Option<i64>,
    user_id: i64,
) -> DbResult<bool> {
    let sql = format!("UPDATE users SET {field_name} = ?1 WHERE id = ?2");
    Ok(connection.execute(&sql, params![value, user_id])? > 0)
}

fn list_application_cloud_setting_keys(
    connection: &Connection,
    user_id: i64,
) -> DbResult<Vec<String>> {
    let mut statement = connection.prepare(
        r#"
        SELECT setting_key
        FROM user_application_cloud_settings
        WHERE user_id = ?1
        "#,
    )?;
    let rows = statement.query_map([user_id], |row| {
        Ok(row.get::<_, Option<String>>(0)?.unwrap_or_default())
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn scoped_id_exists(
    connection: &Connection,
    table_name: &str,
    user_id: UserId,
    record_id: i64,
) -> DbResult<bool> {
    if record_id <= 0 {
        return Ok(false);
    }
    let user_id = user_id_sql(user_id)?;
    let sql = format!("SELECT 1 FROM {table_name} WHERE id = ?1 AND user_id = ?2 LIMIT 1");
    connection
        .query_row(&sql, params![record_id, user_id], |_| Ok(()))
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn update_email_field(connection: &Connection, value: &str, user_id: i64) -> DbResult<bool> {
    Ok(connection.execute(
        r#"
        UPDATE users
        SET email = ?1,
            email_verified = CASE WHEN email = ?1 THEN email_verified ELSE 0 END
        WHERE id = ?2
        "#,
        params![value, user_id],
    )? > 0)
}

fn auth_user_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuthUserProfileRow> {
    let raw_id: i64 = row.get(0)?;
    Ok(AuthUserProfileRow {
        id: user_id_from_sql(raw_id, 0)?,
        username: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        email: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        nickname: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        avatar: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        default_account_id: row.get(5)?,
        transaction_edit_scope: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
        language: row
            .get::<_, Option<String>>(7)?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "zh_Hans".to_string()),
        default_currency: row
            .get::<_, Option<String>>(8)?
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "CNY".to_string()),
        first_day_of_week: row.get::<_, Option<i64>>(9)?.unwrap_or(1),
        fiscal_year_start: row.get::<_, Option<i64>>(10)?.unwrap_or(1),
        calendar_display_type: row.get::<_, Option<i64>>(11)?.unwrap_or(0),
        date_display_type: row.get::<_, Option<i64>>(12)?.unwrap_or(0),
        long_date_format: row.get::<_, Option<i64>>(13)?.unwrap_or(0),
        short_date_format: row.get::<_, Option<i64>>(14)?.unwrap_or(0),
        long_time_format: row.get::<_, Option<i64>>(15)?.unwrap_or(0),
        short_time_format: row.get::<_, Option<i64>>(16)?.unwrap_or(0),
        fiscal_year_format: row.get::<_, Option<i64>>(17)?.unwrap_or(0),
        currency_display_type: row.get::<_, Option<i64>>(18)?.unwrap_or(0),
        numeral_system: row.get::<_, Option<i64>>(19)?.unwrap_or(0),
        decimal_separator: row.get::<_, Option<i64>>(20)?.unwrap_or(0),
        digit_grouping_symbol: row.get::<_, Option<i64>>(21)?.unwrap_or(0),
        digit_grouping: row.get::<_, Option<i64>>(22)?.unwrap_or(0),
        coordinate_display_type: row.get::<_, Option<i64>>(23)?.unwrap_or(0),
        expense_amount_color: row.get::<_, Option<i64>>(24)?.unwrap_or(0),
        income_amount_color: row.get::<_, Option<i64>>(25)?.unwrap_or(0),
        cash_account_id: row.get(26)?,
        cash_transfer_category_id: row.get(27)?,
        import_learning_enabled: row.get::<_, Option<i64>>(28)?.unwrap_or(1) != 0,
        investment_platform_keywords: row.get(29)?,
        investment_product_keywords: row.get(30)?,
        investment_exclude_keywords: row.get(31)?,
        email_verified: row.get::<_, Option<i64>>(32)?.unwrap_or(0) != 0,
    })
}

fn auth_login_user_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuthLoginUserRow> {
    Ok(AuthLoginUserRow {
        profile: auth_user_profile_from_row(row)?,
        password_hash: row.get::<_, Option<String>>(33)?.unwrap_or_default(),
        is_active: row.get::<_, Option<i64>>(34)?.unwrap_or(0) != 0,
        two_factor_enabled: row.get::<_, Option<i64>>(35)?.unwrap_or(0) != 0,
        two_factor_secret: row.get::<_, Option<String>>(36)?.unwrap_or_default(),
        failed_login_attempts: row.get::<_, Option<i64>>(37)?.unwrap_or(0),
        locked_until: row.get::<_, Option<String>>(38)?.unwrap_or_default(),
    })
}
