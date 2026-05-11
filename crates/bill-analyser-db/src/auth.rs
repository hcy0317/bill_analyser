use bill_analyser_core::{auth::recovery_code_hash_input, UserId};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSessionRow {
    pub id: i64,
    pub user_agent: String,
    pub ip_address: String,
    pub expires_at: String,
    pub last_activity_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTokenUserRow {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLoginUserRow {
    pub profile: AuthUserProfileRow,
    pub password_hash: String,
    pub is_active: bool,
    pub two_factor_enabled: bool,
    pub two_factor_secret: String,
    pub failed_login_attempts: i64,
    pub locked_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRefreshSessionRow {
    pub id: i64,
    pub user_id: UserId,
    pub username: String,
    pub refresh_expires_at: String,
    pub user_is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogoutSessionRow {
    pub id: i64,
    pub user_id: UserId,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUserProfileRow {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub nickname: String,
    pub avatar: String,
    pub default_account_id: Option<i64>,
    pub transaction_edit_scope: i64,
    pub language: String,
    pub default_currency: String,
    pub first_day_of_week: i64,
    pub fiscal_year_start: i64,
    pub calendar_display_type: i64,
    pub date_display_type: i64,
    pub long_date_format: i64,
    pub short_date_format: i64,
    pub long_time_format: i64,
    pub short_time_format: i64,
    pub fiscal_year_format: i64,
    pub currency_display_type: i64,
    pub numeral_system: i64,
    pub decimal_separator: i64,
    pub digit_grouping_symbol: i64,
    pub digit_grouping: i64,
    pub coordinate_display_type: i64,
    pub expense_amount_color: i64,
    pub income_amount_color: i64,
    pub cash_account_id: Option<i64>,
    pub cash_transfer_category_id: Option<i64>,
    pub import_learning_enabled: bool,
    pub investment_platform_keywords: Option<String>,
    pub investment_product_keywords: Option<String>,
    pub investment_exclude_keywords: Option<String>,
    pub email_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCloudSettingRow {
    pub setting_key: String,
    pub setting_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAuthRow {
    pub external_auth_category: String,
    pub external_auth_type: String,
    pub external_username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCloudSettingDraft {
    pub setting_key: String,
    pub setting_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthUserProfileUpdate {
    Nickname(String),
    Email(String),
    Avatar(String),
    Language(String),
    DefaultCurrency(String),
    FirstDayOfWeek(i64),
    DefaultAccountId(Option<i64>),
    TransactionEditScope(i64),
    FiscalYearStart(i64),
    CalendarDisplayType(i64),
    DateDisplayType(i64),
    LongDateFormat(i64),
    ShortDateFormat(i64),
    LongTimeFormat(i64),
    ShortTimeFormat(i64),
    FiscalYearFormat(i64),
    CurrencyDisplayType(i64),
    NumeralSystem(i64),
    DecimalSeparator(i64),
    DigitGroupingSymbol(i64),
    DigitGrouping(i64),
    CoordinateDisplayType(i64),
    ExpenseAmountColor(i64),
    IncomeAmountColor(i64),
    CashAccountId(Option<i64>),
    CashTransferCategoryId(Option<i64>),
    ImportLearningEnabled(bool),
    InvestmentPlatformKeywords(String),
    InvestmentProductKeywords(String),
    InvestmentExcludeKeywords(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTokenSessionDraft {
    pub user_id: UserId,
    pub token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub expires_at: String,
    pub refresh_expires_at: Option<String>,
    pub user_agent: String,
    pub ip_address: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLogDraft {
    pub user_id: Option<UserId>,
    pub username: String,
    pub event_type: String,
    pub ip_address: String,
    pub user_agent: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginFailureUpdate {
    pub failed_attempts: i64,
    pub locked: bool,
}

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

pub fn list_application_cloud_settings(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<ApplicationCloudSettingRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    let mut statement = connection.prepare(
        r#"
        SELECT setting_key, setting_value
        FROM user_application_cloud_settings
        WHERE user_id = ?1
        ORDER BY created_at ASC, id ASC
        "#,
    )?;
    let rows = statement.query_map([user_id_sql], |row| {
        Ok(ApplicationCloudSettingRow {
            setting_key: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            setting_value: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn list_user_external_auths(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<ExternalAuthRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    let mut statement = connection.prepare(
        r#"
        SELECT external_auth_category, external_auth_type, external_username, created_at
        FROM user_external_auths
        WHERE user_id = ?1
        ORDER BY created_at DESC, id DESC
        "#,
    )?;
    let rows = statement.query_map([user_id_sql], external_auth_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn get_user_external_auth(
    connection: &Connection,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<Option<ExternalAuthRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT external_auth_category, external_auth_type, external_username, created_at
            FROM user_external_auths
            WHERE user_id = ?1 AND external_auth_type = ?2
            LIMIT 1
            "#,
            params![user_id_sql, external_auth_type],
            external_auth_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn delete_user_external_auth(
    connection: &Connection,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<bool> {
    let user_id_sql = user_id_sql(user_id)?;
    let changed = connection.execute(
        "DELETE FROM user_external_auths WHERE user_id = ?1 AND external_auth_type = ?2",
        params![user_id_sql, external_auth_type],
    )?;
    Ok(changed > 0)
}

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

pub fn create_auth_log(connection: &Connection, draft: &AuthLogDraft) -> DbResult<i64> {
    let user_id = draft.user_id.map(user_id_sql).transpose()?;
    connection.execute(
        r#"
        INSERT INTO auth_logs (
            user_id, username, event_type, ip_address, user_agent,
            success, error_message, metadata, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        params![
            user_id,
            draft.username,
            draft.event_type,
            draft.ip_address,
            draft.user_agent,
            if draft.success { 1 } else { 0 },
            draft.error_message,
            draft.metadata,
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

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

    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
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
            for code_hash in &code_hashes {
                statement.execute(params![user_id, code_hash, now, now])?;
            }
        }
        Ok(code_hashes.len())
    })();

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

#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(value: u64) -> UserId {
        UserId::new(value).expect("positive user id")
    }

    #[test]
    fn token_session_primitives_match_python_session_scope() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                token_hash TEXT NOT NULL,
                refresh_token_hash TEXT,
                expires_at TEXT NOT NULL,
                refresh_expires_at TEXT,
                user_agent TEXT,
                ip_address TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                last_activity_at TEXT,
                created_at TEXT
            );
            CREATE TABLE auth_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER,
                username TEXT,
                event_type TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                success INTEGER NOT NULL,
                error_message TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL
            );
            INSERT INTO users(id, username, password_hash) VALUES (42, 'alice', 'hash');
            INSERT INTO sessions (
                id, user_id, token_hash, expires_at, refresh_expires_at,
                user_agent, ip_address, is_active, last_activity_at, created_at
            ) VALUES
                (1, 42, 'current', '2099-01-01T00:00:00', NULL, 'Mozilla Chrome', '127.0.0.1', 1, '2026-01-02T00:00:00', '2026-01-01T00:00:00'),
                (2, 42, 'api', '2099-01-01T00:00:00', NULL, 'Bill Analyser API Token', '127.0.0.2', 1, '2026-01-03T00:00:00', '2026-01-03T00:00:00'),
                (3, 42, 'inactive', '2099-01-01T00:00:00', NULL, 'inactive', '127.0.0.3', 0, '2026-01-04T00:00:00', '2026-01-04T00:00:00'),
                (4, 7, 'other-user', '2099-01-01T00:00:00', NULL, 'other', '127.0.0.4', 1, '2026-01-05T00:00:00', '2026-01-05T00:00:00'),
                (5, 42, 'expired', '2020-01-01T00:00:00', NULL, 'expired', '127.0.0.5', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00'),
                (7, 42, 'refresh-backed', '2020-01-01T00:00:00', '2099-01-01T00:00:00', 'refresh', '127.0.0.7', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00'),
                (8, 42, 'refresh-expired', '2099-01-01T00:00:00', '2020-01-01T00:00:00', 'refresh expired', '127.0.0.8', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00');
            "#,
        )?;

        assert_eq!(
            cleanup_expired_sessions(&connection, "2026-01-01T00:00:00")?,
            2
        );
        let sessions = list_user_sessions(&connection, user_id(42))?;
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![2, 1, 7]
        );

        assert!(invalidate_session_by_id(&connection, 2, user_id(42))?);
        assert!(!invalidate_session_by_id(&connection, 4, user_id(42))?);
        assert_eq!(
            list_user_sessions(&connection, user_id(42))?
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![1, 7]
        );

        connection.execute(
            "INSERT INTO sessions(id, user_id, token_hash, expires_at, is_active, created_at) VALUES (6, 42, 'new', '2099-01-01T00:00:00', 1, '2026-01-06T00:00:00')",
            [],
        )?;
        assert_eq!(
            invalidate_other_user_sessions(&connection, user_id(42), 1)?,
            2
        );
        assert_eq!(list_user_sessions(&connection, user_id(42))?.len(), 1);
        assert_eq!(list_user_sessions(&connection, user_id(7))?.len(), 1);

        let user = get_auth_token_user(&connection, user_id(42))?.expect("token user");
        assert_eq!(user.username, "alice");
        assert_eq!(user.password_hash, "hash");
        let session_id = create_token_session(
            &connection,
            &CreateTokenSessionDraft {
                user_id: user_id(42),
                token_hash: "issued-token".to_string(),
                refresh_token_hash: None,
                expires_at: "2099-01-01T00:00:00".to_string(),
                refresh_expires_at: None,
                user_agent: "Bill Analyser API Token".to_string(),
                ip_address: "127.0.0.9".to_string(),
                created_at: "2026-01-07T00:00:00".to_string(),
            },
        )?;
        assert!(session_id > 0);
        assert_eq!(
            create_auth_log(
                &connection,
                &AuthLogDraft {
                    user_id: Some(user_id(42)),
                    username: "alice".to_string(),
                    event_type: "api_token_generate_success".to_string(),
                    ip_address: "127.0.0.9".to_string(),
                    user_agent: "Bill Analyser API Token".to_string(),
                    success: true,
                    error_message: None,
                    metadata: Some(format!(r#"{{"session_id":{session_id}}}"#)),
                    created_at: "2026-01-07T00:00:00".to_string(),
                },
            )?,
            1
        );
        assert_eq!(
            count_recent_token_password_failures(&connection, user_id(42), "2026-01-06T23:59:00")?,
            0
        );
        for created_at in [
            "2026-01-07T00:01:00",
            "2026-01-07T00:02:00",
            "2026-01-07T00:03:00",
        ] {
            create_auth_log(
                &connection,
                &AuthLogDraft {
                    user_id: Some(user_id(42)),
                    username: "alice".to_string(),
                    event_type: "api_token_generate_failed".to_string(),
                    ip_address: "127.0.0.9".to_string(),
                    user_agent: "Mozilla".to_string(),
                    success: false,
                    error_message: Some("Invalid password".to_string()),
                    metadata: None,
                    created_at: created_at.to_string(),
                },
            )?;
        }
        create_auth_log(
            &connection,
            &AuthLogDraft {
                user_id: Some(user_id(42)),
                username: "alice".to_string(),
                event_type: "api_token_generate_failed".to_string(),
                ip_address: "127.0.0.8".to_string(),
                user_agent: "Mozilla".to_string(),
                success: false,
                error_message: Some("Invalid password".to_string()),
                metadata: None,
                created_at: "2026-01-07T00:04:00".to_string(),
            },
        )?;
        assert_eq!(
            count_recent_token_password_failures(&connection, user_id(42), "2026-01-07T00:00:00")?,
            4
        );
        connection.execute(
            "INSERT INTO sessions(id, user_id, token_hash, expires_at, is_active, created_at) VALUES (10, 42, 'logout-token', '2099-01-01T00:00:00', 1, '2026-01-08T00:00:00')",
            [],
        )?;
        let logout_session = get_active_logout_session_by_token_hash(&connection, "logout-token")?
            .expect("logout session");
        assert_eq!(logout_session.id, 10);
        assert_eq!(logout_session.user_id, user_id(42));
        assert_eq!(logout_session.username, "alice");
        assert!(invalidate_session_by_token_hash(
            &connection,
            "logout-token"
        )?);
        assert!(get_active_logout_session_by_token_hash(&connection, "logout-token")?.is_none());
        assert!(!invalidate_session_by_token_hash(
            &connection,
            "logout-token"
        )?);
        Ok(())
    }

    #[test]
    fn profile_and_cloud_setting_primitives_preserve_user_scope() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                nickname TEXT,
                avatar TEXT,
                default_account_id INTEGER,
                transaction_edit_scope INTEGER DEFAULT 0,
                language TEXT DEFAULT 'zh_Hans',
                default_currency TEXT DEFAULT 'CNY',
                first_day_of_week INTEGER DEFAULT 1,
                fiscal_year_start INTEGER DEFAULT 1,
                calendar_display_type INTEGER DEFAULT 0,
                date_display_type INTEGER DEFAULT 0,
                long_date_format INTEGER DEFAULT 0,
                short_date_format INTEGER DEFAULT 0,
                long_time_format INTEGER DEFAULT 0,
                short_time_format INTEGER DEFAULT 0,
                fiscal_year_format INTEGER DEFAULT 0,
                currency_display_type INTEGER DEFAULT 0,
                numeral_system INTEGER DEFAULT 0,
                decimal_separator INTEGER DEFAULT 0,
                digit_grouping_symbol INTEGER DEFAULT 0,
                digit_grouping INTEGER DEFAULT 0,
                coordinate_display_type INTEGER DEFAULT 0,
                expense_amount_color INTEGER DEFAULT 0,
                income_amount_color INTEGER DEFAULT 0,
                cash_account_id INTEGER,
                cash_transfer_category_id INTEGER,
                import_learning_enabled INTEGER DEFAULT 1,
                investment_platform_keywords TEXT,
                investment_product_keywords TEXT,
                investment_exclude_keywords TEXT,
                email_verified INTEGER DEFAULT 0,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE user_application_cloud_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                setting_key TEXT NOT NULL,
                setting_value TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, setting_key)
            );
            CREATE TABLE user_external_auths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                external_auth_category TEXT NOT NULL,
                external_auth_type TEXT NOT NULL,
                external_user_id TEXT,
                external_username TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, external_auth_type)
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                name TEXT NOT NULL
            );
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                main_category TEXT NOT NULL,
                sub_category TEXT NOT NULL
            );
            CREATE TABLE auth_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER,
                username TEXT,
                event_type TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                success INTEGER NOT NULL,
                error_message TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            );
            INSERT INTO users(id, username, email, nickname, email_verified, updated_at)
            VALUES (42, 'alice', 'alice@example.test', '', 1, '2026-01-01T00:00:00');
            INSERT INTO users(id, username, email, nickname, updated_at)
            VALUES (7, 'bob', 'bob@example.test', 'Bob', '2026-01-01T00:00:00');
            INSERT INTO accounts(id, user_id, name) VALUES (100, 42, 'Cash'), (101, 7, 'Other Cash');
            INSERT INTO categories(id, user_id, main_category, sub_category)
            VALUES (200, 42, 'Transfer', ''), (201, 7, 'Other Transfer', '');
            INSERT INTO auth_logs(user_id, username, event_type, ip_address, user_agent, success, created_at)
            VALUES
                (42, 'alice', 'verification_email_resend_requested', '127.0.0.1', '', 1, '2026-01-03T00:00:00'),
                (42, 'alice', 'verification_email_resend_requested', '127.0.0.1', '', 1, '2026-01-03T00:04:00'),
                (7, 'bob', 'verification_email_resend_requested', '127.0.0.1', '', 1, '2026-01-03T00:04:00');
            INSERT INTO user_application_cloud_settings(user_id, setting_key, setting_value, created_at, updated_at)
            VALUES
                (42, 'showAmountInHomePage', 'true', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
                (42, 'autoSaveTransactionDraft', 'old', '2026-01-01T00:00:00', '2026-01-01T00:00:00'),
                (7, 'showAmountInHomePage', 'false', '2026-01-01T00:00:00', '2026-01-01T00:00:00');
            INSERT INTO user_external_auths(
                user_id, external_auth_category, external_auth_type,
                external_user_id, external_username, created_at, updated_at
            ) VALUES
                (42, 'oauth2', 'github', 'gh-42', 'alice-gh', '2026-01-02T00:00:00', '2026-01-02T00:00:00'),
                (42, 'oauth2', 'google', 'gg-42', NULL, '2026-01-03T00:00:00', '2026-01-03T00:00:00'),
                (7, 'oauth2', 'github', 'gh-7', 'bob-gh', '2026-01-04T00:00:00', '2026-01-04T00:00:00');
            "#,
        )?;

        assert!(update_auth_user_profile(
            &connection,
            user_id(42),
            &[
                AuthUserProfileUpdate::Nickname("Alice".to_string()),
                AuthUserProfileUpdate::Email("new-alice@example.test".to_string()),
                AuthUserProfileUpdate::Avatar("data:text/plain;base64,YQ==".to_string()),
                AuthUserProfileUpdate::Language("en".to_string()),
                AuthUserProfileUpdate::DefaultCurrency("USD".to_string()),
                AuthUserProfileUpdate::FirstDayOfWeek(2),
                AuthUserProfileUpdate::DefaultAccountId(Some(100)),
                AuthUserProfileUpdate::TransactionEditScope(3),
                AuthUserProfileUpdate::FiscalYearStart(4),
                AuthUserProfileUpdate::CalendarDisplayType(5),
                AuthUserProfileUpdate::DateDisplayType(6),
                AuthUserProfileUpdate::LongDateFormat(7),
                AuthUserProfileUpdate::ShortDateFormat(8),
                AuthUserProfileUpdate::LongTimeFormat(9),
                AuthUserProfileUpdate::ShortTimeFormat(10),
                AuthUserProfileUpdate::FiscalYearFormat(11),
                AuthUserProfileUpdate::CurrencyDisplayType(12),
                AuthUserProfileUpdate::NumeralSystem(13),
                AuthUserProfileUpdate::DecimalSeparator(14),
                AuthUserProfileUpdate::DigitGroupingSymbol(15),
                AuthUserProfileUpdate::DigitGrouping(16),
                AuthUserProfileUpdate::CoordinateDisplayType(17),
                AuthUserProfileUpdate::ExpenseAmountColor(18),
                AuthUserProfileUpdate::IncomeAmountColor(19),
                AuthUserProfileUpdate::CashAccountId(None),
                AuthUserProfileUpdate::CashTransferCategoryId(Some(200)),
                AuthUserProfileUpdate::ImportLearningEnabled(false),
                AuthUserProfileUpdate::InvestmentPlatformKeywords(
                    r#"["蚂蚁财富","雪球"]"#.to_string(),
                ),
                AuthUserProfileUpdate::InvestmentProductKeywords(r#"["基金"]"#.to_string()),
                AuthUserProfileUpdate::InvestmentExcludeKeywords(r#"["还款"]"#.to_string()),
            ],
            "2026-01-02T00:00:00",
        )?);
        assert!(!update_auth_user_profile(
            &connection,
            user_id(42),
            &[],
            "2026-01-02T00:00:00",
        )?);
        assert!(!update_auth_user_profile(
            &connection,
            user_id(999),
            &[AuthUserProfileUpdate::Nickname("Missing".to_string())],
            "2026-01-02T00:00:00",
        )?);
        let profile = get_auth_user_profile(&connection, user_id(42))?.expect("profile");
        assert_eq!(profile.nickname, "Alice");
        assert_eq!(profile.email, "new-alice@example.test");
        assert_eq!(profile.avatar, "data:text/plain;base64,YQ==");
        assert_eq!(profile.language, "en");
        assert_eq!(profile.default_currency, "USD");
        assert_eq!(profile.first_day_of_week, 2);
        assert_eq!(profile.default_account_id, Some(100));
        assert_eq!(profile.transaction_edit_scope, 3);
        assert_eq!(profile.fiscal_year_start, 4);
        assert_eq!(profile.calendar_display_type, 5);
        assert_eq!(profile.date_display_type, 6);
        assert_eq!(profile.long_date_format, 7);
        assert_eq!(profile.short_date_format, 8);
        assert_eq!(profile.long_time_format, 9);
        assert_eq!(profile.short_time_format, 10);
        assert_eq!(profile.fiscal_year_format, 11);
        assert_eq!(profile.currency_display_type, 12);
        assert_eq!(profile.numeral_system, 13);
        assert_eq!(profile.decimal_separator, 14);
        assert_eq!(profile.digit_grouping_symbol, 15);
        assert_eq!(profile.digit_grouping, 16);
        assert_eq!(profile.coordinate_display_type, 17);
        assert_eq!(profile.expense_amount_color, 18);
        assert_eq!(profile.income_amount_color, 19);
        assert_eq!(profile.cash_account_id, None);
        assert_eq!(profile.cash_transfer_category_id, Some(200));
        assert!(!profile.import_learning_enabled);
        assert!(!profile.email_verified);
        assert_eq!(
            profile.investment_platform_keywords.as_deref(),
            Some(r#"["蚂蚁财富","雪球"]"#)
        );
        assert_eq!(
            profile.investment_product_keywords.as_deref(),
            Some(r#"["基金"]"#)
        );
        assert_eq!(
            profile.investment_exclude_keywords.as_deref(),
            Some(r#"["还款"]"#)
        );
        assert_eq!(
            get_auth_user_profile(&connection, user_id(7))?
                .expect("other profile")
                .nickname,
            "Bob"
        );
        assert!(auth_email_exists_for_other_user(
            &connection,
            user_id(42),
            "bob@example.test"
        )?);
        assert!(!auth_email_exists_for_other_user(
            &connection,
            user_id(42),
            "new-alice@example.test"
        )?);
        assert!(auth_account_belongs_to_user(&connection, user_id(42), 100)?);
        assert!(!auth_account_belongs_to_user(
            &connection,
            user_id(42),
            101
        )?);
        assert!(auth_category_belongs_to_user(
            &connection,
            user_id(42),
            200
        )?);
        assert!(!auth_category_belongs_to_user(
            &connection,
            user_id(42),
            201
        )?);
        assert_eq!(
            count_auth_events_since(
                &connection,
                user_id(42),
                "verification_email_resend_requested",
                "2026-01-03T00:01:00"
            )?,
            1
        );
        let resend_draft = AuthLogDraft {
            user_id: Some(user_id(42)),
            username: "alice".to_string(),
            event_type: "verification_email_resend_requested".to_string(),
            ip_address: "198.51.100.13".to_string(),
            user_agent: "Mozilla".to_string(),
            success: true,
            error_message: None,
            metadata: Some(r#"{"email_present":true}"#.to_string()),
            created_at: "2026-01-03T00:05:00".to_string(),
        };
        assert!(create_auth_log_under_event_limit(
            &connection,
            user_id(42),
            "verification_email_resend_requested",
            "2026-01-03T00:01:00",
            2,
            &resend_draft,
        )?);
        assert!(!create_auth_log_under_event_limit(
            &connection,
            user_id(42),
            "verification_email_resend_requested",
            "2026-01-03T00:01:00",
            2,
            &resend_draft,
        )?);
        assert_eq!(
            count_auth_events_since(
                &connection,
                user_id(42),
                "verification_email_resend_requested",
                "2026-01-03T00:01:00"
            )?,
            2
        );
        assert!(update_auth_user_profile_with_auth_log(
            &connection,
            user_id(42),
            &[AuthUserProfileUpdate::Nickname("Alice Logged".to_string())],
            "2026-01-02T00:01:00",
            &AuthLogDraft {
                user_id: Some(user_id(42)),
                username: "alice".to_string(),
                event_type: "profile_email_changed".to_string(),
                ip_address: "127.0.0.1".to_string(),
                user_agent: "Mozilla".to_string(),
                success: true,
                error_message: None,
                metadata: Some(r#"{"email_changed":true}"#.to_string()),
                created_at: "2026-01-02T00:01:00".to_string(),
            },
        )?);
        assert_eq!(
            get_auth_user_profile(&connection, user_id(42))?
                .expect("logged profile")
                .nickname,
            "Alice Logged"
        );
        assert_eq!(
            count_auth_events_since(
                &connection,
                user_id(42),
                "profile_email_changed",
                "2026-01-02T00:00:00"
            )?,
            1
        );

        let external_auths = list_user_external_auths(&connection, user_id(42))?;
        assert_eq!(
            external_auths
                .iter()
                .map(|item| item.external_auth_type.as_str())
                .collect::<Vec<_>>(),
            vec!["google", "github"]
        );
        assert_eq!(external_auths[0].external_username, "");
        assert_eq!(
            get_user_external_auth(&connection, user_id(42), "github")?
                .expect("github auth")
                .external_username,
            "alice-gh"
        );
        assert!(get_user_external_auth(&connection, user_id(42), "missing")?.is_none());
        assert!(delete_user_external_auth(
            &connection,
            user_id(42),
            "github"
        )?);
        assert!(get_user_external_auth(&connection, user_id(42), "github")?.is_none());
        assert!(!delete_user_external_auth(
            &connection,
            user_id(42),
            "github"
        )?);
        assert_eq!(list_user_external_auths(&connection, user_id(7))?.len(), 1);

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[
                ApplicationCloudSettingDraft {
                    setting_key: "showAmountInHomePage".to_string(),
                    setting_value: "false".to_string(),
                },
                ApplicationCloudSettingDraft {
                    setting_key: "itemsCountInTransactionListPage".to_string(),
                    setting_value: "25".to_string(),
                },
            ],
            true,
            "2026-01-03T00:00:00",
        )?);
        let settings = list_application_cloud_settings(&connection, user_id(42))?;
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].setting_key, "showAmountInHomePage");
        assert_eq!(settings[0].setting_value, "false");
        assert!(settings
            .iter()
            .all(|setting| setting.setting_key != "autoSaveTransactionDraft"));
        assert_eq!(
            list_application_cloud_settings(&connection, user_id(7))?.len(),
            1
        );

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[
                ApplicationCloudSettingDraft {
                    setting_key: "   ".to_string(),
                    setting_value: "ignored".to_string(),
                },
                ApplicationCloudSettingDraft {
                    setting_key: "autoSaveTransactionDraft".to_string(),
                    setting_value: "draft".to_string(),
                },
            ],
            false,
            "2026-01-03T00:01:00",
        )?);
        let settings = list_application_cloud_settings(&connection, user_id(42))?;
        assert_eq!(settings.len(), 3);
        assert!(settings
            .iter()
            .any(|setting| setting.setting_key == "autoSaveTransactionDraft"));

        assert!(update_application_cloud_settings(
            &connection,
            user_id(42),
            &[],
            true,
            "2026-01-03T00:02:00",
        )?);
        assert!(list_application_cloud_settings(&connection, user_id(42))?.is_empty());

        assert!(delete_application_cloud_settings(&connection, user_id(42))?);
        assert!(list_application_cloud_settings(&connection, user_id(42))?.is_empty());
        assert_eq!(
            list_application_cloud_settings(&connection, user_id(7))?.len(),
            1
        );
        Ok(())
    }

    #[test]
    fn login_failure_primitives_cover_lockout_and_reset_edges() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                failed_login_attempts INTEGER,
                locked_until TEXT,
                last_login_at TEXT,
                last_login_ip TEXT
            );
            INSERT INTO users(id, failed_login_attempts, locked_until) VALUES
                (1, 0, NULL),
                (2, 2, NULL),
                (3, 8, '2020-01-01T00:00:00'),
                (4, 4, '2026-01-01T00:00:00');
            "#,
        )?;

        assert_eq!(
            increment_failed_login(&connection, user_id(999), 3, "2026-01-02T00:00:00", None,)?,
            None
        );
        assert_eq!(
            increment_failed_login(&connection, user_id(1), 3, "2026-01-02T00:00:00", None)?,
            Some(LoginFailureUpdate {
                failed_attempts: 1,
                locked: false
            })
        );
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (1, None)
        );

        assert_eq!(
            increment_failed_login(&connection, user_id(2), 3, "2026-01-02T00:00:00", None)?,
            Some(LoginFailureUpdate {
                failed_attempts: 3,
                locked: true
            })
        );
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 2",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (3, Some("2026-01-02T00:00:00".to_string()))
        );

        assert_eq!(
            increment_failed_login(
                &connection,
                user_id(3),
                3,
                "2026-01-02T00:00:00",
                Some("2020-01-01T00:00:00"),
            )?,
            Some(LoginFailureUpdate {
                failed_attempts: 1,
                locked: false
            })
        );
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 3",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (1, None)
        );

        assert!(clear_expired_login_lock(&connection, user_id(4))?);
        assert_eq!(
            connection.query_row(
                "SELECT failed_login_attempts, locked_until FROM users WHERE id = 4",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            )?,
            (0, None)
        );

        assert!(update_user_last_login(
            &connection,
            user_id(1),
            "2026-01-03T00:00:00",
            "127.0.0.1",
        )?);
        assert_eq!(
            connection.query_row(
                "SELECT last_login_at, last_login_ip, failed_login_attempts, locked_until FROM users WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                }
            )?,
            (
                Some("2026-01-03T00:00:00".to_string()),
                Some("127.0.0.1".to_string()),
                0,
                None
            )
        );
        Ok(())
    }
}
