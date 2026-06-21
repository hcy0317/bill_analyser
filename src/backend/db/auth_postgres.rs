// 中文导读：PostgreSQL auth repository helpers for authoritative HTTP paths.
// 维护重点：Postgres 模式只读取 authoritative 表与 JSONB metadata，不回落 non-Postgres。
// 不变式：用户身份、登录锁定和 profile 投影必须保持 user-scope 与前端字段。

use std::collections::HashSet;

use bill_analyser_core::{
    auth::recovery_code_hash_input, category_rules::compile_rule_expression, UserId,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgRow, types::Json, Postgres, Row};

use crate::auth::{
    ApplicationCloudSettingDraft, ApplicationCloudSettingRow, AuthLogDraft, AuthLoginUserRow,
    AuthRefreshSessionRow, AuthTokenUserRow, AuthUserProfileRow, AuthUserProfileUpdate,
    CreateTokenSessionDraft, ExternalAuthRow, LoginFailureUpdate, TokenSessionRow,
};
use crate::auth_registration::{
    DefaultAccount, DefaultAccountRule, DefaultCategory, DefaultCategoryRule,
    RegisterDefaultSeedPackage, RegisterDefaultSeedSummary, RegisterPresetCategory,
    RegisterUserDraft, RegisterUserResult,
};
use crate::auth_registration_defaults::{
    STANDARD_DAILY_V1_ACCOUNTS, STANDARD_DAILY_V1_ACCOUNT_RULES, STANDARD_DAILY_V1_CATEGORIES,
    STANDARD_DAILY_V1_CATEGORY_RULES,
};
use crate::taxonomy::postgres_reads::ensure_postgres_category_rule_defaults;
use crate::{DbError, DbResult, PostgresPool};

include!("auth_postgres/identity_checks.rs");
include!("auth_postgres/read_models.rs");
include!("auth_postgres/audit_events.rs");
include!("auth_postgres/sessions.rs");
include!("auth_postgres/two_factor.rs");
include!("auth_postgres/profile_updates.rs");
include!("auth_postgres/cloud_settings.rs");
include!("auth_postgres/registration.rs");
include!("auth_postgres/registration_defaults_seed.rs");

fn apply_postgres_profile_update(
    update: &AuthUserProfileUpdate,
    email: &mut String,
    current_email: &str,
    display_name: &mut String,
    metadata: &mut Value,
) {
    match update {
        AuthUserProfileUpdate::Nickname(value) => {
            *display_name = value.clone();
            metadata_set_string(metadata, "nickname", value);
        }
        AuthUserProfileUpdate::Email(value) => {
            if value != current_email {
                metadata_set_bool(metadata, "email_verified", false);
            }
            *email = value.clone();
        }
        AuthUserProfileUpdate::Avatar(value) => metadata_set_string(metadata, "avatar", value),
        AuthUserProfileUpdate::Language(value) => metadata_set_string(metadata, "language", value),
        AuthUserProfileUpdate::DefaultCurrency(value) => {
            metadata_set_string(metadata, "default_currency", value);
        }
        AuthUserProfileUpdate::FirstDayOfWeek(value) => {
            metadata_set_i64(metadata, "first_day_of_week", *value);
        }
        AuthUserProfileUpdate::DefaultAccountId(value) => {
            metadata_set_optional_i64(metadata, "default_account_id", *value);
        }
        AuthUserProfileUpdate::TransactionEditScope(value) => {
            metadata_set_i64(metadata, "transaction_edit_scope", *value);
        }
        AuthUserProfileUpdate::FiscalYearStart(value) => {
            metadata_set_i64(metadata, "fiscal_year_start", *value);
        }
        AuthUserProfileUpdate::CalendarDisplayType(value) => {
            metadata_set_i64(metadata, "calendar_display_type", *value);
        }
        AuthUserProfileUpdate::DateDisplayType(value) => {
            metadata_set_i64(metadata, "date_display_type", *value);
        }
        AuthUserProfileUpdate::LongDateFormat(value) => {
            metadata_set_i64(metadata, "long_date_format", *value);
        }
        AuthUserProfileUpdate::ShortDateFormat(value) => {
            metadata_set_i64(metadata, "short_date_format", *value);
        }
        AuthUserProfileUpdate::LongTimeFormat(value) => {
            metadata_set_i64(metadata, "long_time_format", *value);
        }
        AuthUserProfileUpdate::ShortTimeFormat(value) => {
            metadata_set_i64(metadata, "short_time_format", *value);
        }
        AuthUserProfileUpdate::FiscalYearFormat(value) => {
            metadata_set_i64(metadata, "fiscal_year_format", *value);
        }
        AuthUserProfileUpdate::CurrencyDisplayType(value) => {
            metadata_set_i64(metadata, "currency_display_type", *value);
        }
        AuthUserProfileUpdate::NumeralSystem(value) => {
            metadata_set_i64(metadata, "numeral_system", *value);
        }
        AuthUserProfileUpdate::DecimalSeparator(value) => {
            metadata_set_i64(metadata, "decimal_separator", *value);
        }
        AuthUserProfileUpdate::DigitGroupingSymbol(value) => {
            metadata_set_i64(metadata, "digit_grouping_symbol", *value);
        }
        AuthUserProfileUpdate::DigitGrouping(value) => {
            metadata_set_i64(metadata, "digit_grouping", *value);
        }
        AuthUserProfileUpdate::CoordinateDisplayType(value) => {
            metadata_set_i64(metadata, "coordinate_display_type", *value);
        }
        AuthUserProfileUpdate::ExpenseAmountColor(value) => {
            metadata_set_i64(metadata, "expense_amount_color", *value);
        }
        AuthUserProfileUpdate::IncomeAmountColor(value) => {
            metadata_set_i64(metadata, "income_amount_color", *value);
        }
        AuthUserProfileUpdate::CashAccountId(value) => {
            metadata_set_optional_i64(metadata, "cash_account_id", *value);
        }
        AuthUserProfileUpdate::CashTransferCategoryId(value) => {
            metadata_set_optional_i64(metadata, "cash_transfer_category_id", *value);
        }
        AuthUserProfileUpdate::ImportLearningEnabled(value) => {
            metadata_set_bool(metadata, "import_learning_enabled", *value);
        }
        AuthUserProfileUpdate::InvestmentPlatformKeywords(value) => {
            metadata_set_string(metadata, "investment_platform_keywords", value);
        }
        AuthUserProfileUpdate::InvestmentProductKeywords(value) => {
            metadata_set_string(metadata, "investment_product_keywords", value);
        }
        AuthUserProfileUpdate::InvestmentExcludeKeywords(value) => {
            metadata_set_string(metadata, "investment_exclude_keywords", value);
        }
    }
}

async fn insert_postgres_auth_log_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    draft: &AuthLogDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor, metadata, created_at
        ) VALUES ($1, 'auth', $2, $3, $4, $5, $6::timestamptz)
        RETURNING id
        "#,
    )
    .bind(draft.user_id.map(user_id_i64).transpose()?)
    .bind(
        draft
            .user_id
            .map(|value| value.get().to_string())
            .unwrap_or_else(|| draft.username.clone()),
    )
    .bind(&draft.event_type)
    .bind(&draft.username)
    .bind(Json(auth_log_metadata(draft)))
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

async fn create_postgres_token_session_in_tx(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO token_sessions (
            user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4::timestamptz, NULLIF($5, '')::timestamptz,
            $6, $7, TRUE, NULL, $8::timestamptz, $8::timestamptz
        )
        RETURNING id
        "#,
    )
    .bind(user_id_i64(draft.user_id)?)
    .bind(&draft.token_hash)
    .bind(&draft.refresh_token_hash)
    .bind(&draft.expires_at)
    .bind(draft.refresh_expires_at.as_deref().unwrap_or(""))
    .bind(&draft.user_agent)
    .bind(&draft.ip_address)
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

fn auth_log_metadata(draft: &AuthLogDraft) -> Value {
    json!({
        "username": draft.username,
        "ip_address": draft.ip_address,
        "user_agent": draft.user_agent,
        "success": draft.success,
        "error_message": draft.error_message,
        "metadata": parse_auth_metadata(draft.metadata.as_deref()),
    })
}

async fn load_user_metadata(pool: &PostgresPool, user_id: UserId) -> DbResult<Value> {
    let row = sqlx::query("SELECT metadata FROM users WHERE id = $1")
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map_err(postgres_auth_error)?
        .ok_or_else(|| DbError::InvalidOperation("Postgres auth user not found".to_string()))?;
    let Json(metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    Ok(metadata)
}

async fn update_user_metadata(
    pool: &PostgresPool,
    user_id: UserId,
    metadata: Value,
    updated_at: &str,
) -> DbResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET metadata = $1,
            updated_at = COALESCE(NULLIF($2, '')::timestamptz, now())
        WHERE id = $3
        "#,
    )
    .bind(Json(metadata))
    .bind(updated_at)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map_err(postgres_auth_error)?;
    Ok(result.rows_affected() > 0)
}

fn postgres_login_user_from_row(row: &PgRow) -> DbResult<AuthLoginUserRow> {
    let profile = postgres_profile_from_row(row)?;
    let Json(metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    Ok(AuthLoginUserRow {
        profile,
        password_hash: row.try_get("password_hash").map_err(postgres_auth_error)?,
        is_active: metadata_bool(&metadata, &["is_active"], true),
        two_factor_enabled: metadata_bool(&metadata, &["two_factor_enabled"], false),
        two_factor_secret: metadata_string(&metadata, &["two_factor_secret"], ""),
        failed_login_attempts: metadata_i64(&metadata, &["failed_login_attempts"], 0),
        locked_until: metadata_string(&metadata, &["locked_until"], ""),
    })
}

fn postgres_profile_from_row(row: &PgRow) -> DbResult<AuthUserProfileRow> {
    let raw_id: i64 = row.try_get("id").map_err(postgres_auth_error)?;
    let user_id = user_id_from_i64(raw_id)?;
    let username: String = row.try_get("username").map_err(postgres_auth_error)?;
    let email: String = row.try_get("email").map_err(postgres_auth_error)?;
    let display_name: String = row.try_get("display_name").map_err(postgres_auth_error)?;
    let Json(metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    let nickname = if display_name.trim().is_empty() {
        metadata_string(&metadata, &["nickname"], &username)
    } else {
        display_name
    };
    Ok(AuthUserProfileRow {
        id: user_id,
        username,
        email,
        nickname,
        avatar: metadata_string(&metadata, &["avatar"], ""),
        default_account_id: metadata_optional_i64(&metadata, &["default_account_id"]),
        transaction_edit_scope: metadata_i64(&metadata, &["transaction_edit_scope"], 0),
        language: metadata_string(&metadata, &["language"], "zh_Hans"),
        default_currency: metadata_string(&metadata, &["default_currency"], "CNY"),
        first_day_of_week: metadata_i64(&metadata, &["first_day_of_week"], 1),
        fiscal_year_start: metadata_i64(&metadata, &["fiscal_year_start"], 1),
        calendar_display_type: metadata_i64(&metadata, &["calendar_display_type"], 0),
        date_display_type: metadata_i64(&metadata, &["date_display_type"], 0),
        long_date_format: metadata_i64(&metadata, &["long_date_format"], 0),
        short_date_format: metadata_i64(&metadata, &["short_date_format"], 0),
        long_time_format: metadata_i64(&metadata, &["long_time_format"], 0),
        short_time_format: metadata_i64(&metadata, &["short_time_format"], 0),
        fiscal_year_format: metadata_i64(&metadata, &["fiscal_year_format"], 0),
        currency_display_type: metadata_i64(&metadata, &["currency_display_type"], 0),
        numeral_system: metadata_i64(&metadata, &["numeral_system"], 0),
        decimal_separator: metadata_i64(&metadata, &["decimal_separator"], 0),
        digit_grouping_symbol: metadata_i64(&metadata, &["digit_grouping_symbol"], 0),
        digit_grouping: metadata_i64(&metadata, &["digit_grouping"], 0),
        coordinate_display_type: metadata_i64(&metadata, &["coordinate_display_type"], 0),
        expense_amount_color: metadata_i64(&metadata, &["expense_amount_color"], 0),
        income_amount_color: metadata_i64(&metadata, &["income_amount_color"], 0),
        cash_account_id: metadata_optional_i64(&metadata, &["cash_account_id"]),
        cash_transfer_category_id: metadata_optional_i64(&metadata, &["cash_transfer_category_id"]),
        import_learning_enabled: metadata_bool(&metadata, &["import_learning_enabled"], true),
        investment_platform_keywords: metadata_optional_string(
            &metadata,
            &["investment_platform_keywords"],
        ),
        investment_product_keywords: metadata_optional_string(
            &metadata,
            &["investment_product_keywords"],
        ),
        investment_exclude_keywords: metadata_optional_string(
            &metadata,
            &["investment_exclude_keywords"],
        ),
        email_verified: metadata_bool(&metadata, &["email_verified"], false),
    })
}

fn postgres_external_auth_from_row(row: &PgRow) -> DbResult<ExternalAuthRow> {
    Ok(ExternalAuthRow {
        external_auth_category: row
            .try_get("external_auth_category")
            .map_err(postgres_auth_error)?,
        external_auth_type: row
            .try_get("external_auth_type")
            .map_err(postgres_auth_error)?,
        external_username: row
            .try_get("external_username")
            .map_err(postgres_auth_error)?,
        created_at: row.try_get("created_at").map_err(postgres_auth_error)?,
    })
}

fn metadata_string(metadata: &Value, keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn metadata_optional_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    let value = metadata_string(metadata, keys, "");
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn metadata_i64(metadata: &Value, keys: &[&str], default: i64) -> i64 {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.trim().parse().ok(),
            Value::Bool(value) => Some(i64::from(*value)),
            _ => None,
        })
        .unwrap_or(default)
}

fn metadata_optional_i64(metadata: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => {
                let text = text.trim();
                if text.is_empty() {
                    None
                } else {
                    text.parse().ok()
                }
            }
            _ => None,
        })
}

fn metadata_bool(metadata: &Value, keys: &[&str], default: bool) -> bool {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::Bool(value) => Some(*value),
            Value::Number(number) => number.as_i64().map(|value| value != 0),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => Some(true),
                "0" | "false" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or(default)
}

fn metadata_set_string(metadata: &mut Value, key: &str, value: &str) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::String(value.to_string()));
}

fn metadata_set_i64(metadata: &mut Value, key: &str, value: i64) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::from(value));
}

fn metadata_set_bool(metadata: &mut Value, key: &str, value: bool) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::from(value));
}

fn metadata_set_optional_i64(metadata: &mut Value, key: &str, value: Option<i64>) {
    let object = ensure_metadata_object(metadata);
    if let Some(value) = value {
        object.insert(key.to_string(), Value::from(value));
    } else {
        object.remove(key);
    }
}

fn ensure_metadata_object(metadata: &mut Value) -> &mut Map<String, Value> {
    if !metadata.is_object() {
        *metadata = Value::Object(Map::new());
    }
    metadata.as_object_mut().expect("metadata object")
}

fn parse_auth_metadata(raw: Option<&str>) -> Value {
    raw.and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null)
}

fn json_value_to_setting_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn json_setting_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get()).map_err(|_| {
        DbError::InvalidOperation("user id is outside PostgreSQL BIGINT range".to_string())
    })
}

fn user_id_from_i64(raw_id: i64) -> DbResult<UserId> {
    let raw_id = u64::try_from(raw_id)
        .map_err(|_| DbError::InvalidOperation("Postgres user id must be positive".to_string()))?;
    UserId::new(raw_id)
        .map_err(|_| DbError::InvalidOperation("Postgres user id must be non-zero".to_string()))
}

fn postgres_auth_error(error: sqlx::Error) -> DbError {
    DbError::InvalidOperation(format!("postgres auth error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_helpers_preserve_postgres_json_edges() {
        let metadata = json!({
            "name_string": " Alice ",
            "name_number": 42,
            "name_bool": true,
            "empty": " ",
            "int_number": 7,
            "int_string": "8",
            "int_bool": true,
            "optional_empty": "",
            "bool_true": "yes",
            "bool_false": 0,
            "bool_invalid": "maybe"
        });

        assert_eq!(
            metadata_string(&metadata, &["missing", "name_string"], "fallback"),
            "Alice"
        );
        assert_eq!(
            metadata_string(&metadata, &["name_number"], "fallback"),
            "42"
        );
        assert_eq!(
            metadata_string(&metadata, &["name_bool"], "fallback"),
            "true"
        );
        assert_eq!(
            metadata_string(&metadata, &["empty"], "fallback"),
            "fallback"
        );
        assert_eq!(
            metadata_optional_string(&metadata, &["name_string"]),
            Some("Alice".to_string())
        );
        assert_eq!(metadata_optional_string(&metadata, &["missing"]), None);
        assert_eq!(metadata_i64(&metadata, &["int_number"], 1), 7);
        assert_eq!(metadata_i64(&metadata, &["int_string"], 1), 8);
        assert_eq!(metadata_i64(&metadata, &["int_bool"], 1), 1);
        assert_eq!(metadata_i64(&metadata, &["missing"], 9), 9);
        assert_eq!(metadata_optional_i64(&metadata, &["int_string"]), Some(8));
        assert_eq!(metadata_optional_i64(&metadata, &["optional_empty"]), None);
        assert!(metadata_bool(&metadata, &["bool_true"], false));
        assert!(!metadata_bool(&metadata, &["bool_false"], true));
        assert!(metadata_bool(&metadata, &["bool_invalid"], true));
    }

    #[test]
    fn metadata_mutation_and_json_helpers_cover_non_object_edges() {
        let mut metadata = Value::Null;
        metadata_set_string(&mut metadata, "last_login_ip", "127.0.0.1");
        metadata_set_i64(&mut metadata, "failed_login_attempts", 3);
        metadata_set_bool(&mut metadata, "email_verified", false);
        metadata_set_optional_i64(&mut metadata, "default_account_id", Some(42));
        metadata_set_optional_i64(&mut metadata, "cash_account_id", None);

        assert_eq!(metadata["last_login_ip"], "127.0.0.1");
        assert_eq!(metadata["failed_login_attempts"], 3);
        assert_eq!(metadata["email_verified"], false);
        assert_eq!(metadata["default_account_id"], 42);
        assert!(metadata.get("cash_account_id").is_none());
        assert_eq!(
            parse_auth_metadata(Some(r#"{"reason":"failed"}"#))["reason"],
            "failed"
        );
        assert_eq!(parse_auth_metadata(Some("not-json")), Value::Null);
        assert_eq!(parse_auth_metadata(None), Value::Null);
        assert_eq!(
            json_value_to_setting_string(&Value::String("enabled".to_string())),
            "enabled"
        );
        assert_eq!(json_value_to_setting_string(&Value::Null), "");
        assert_eq!(
            json_value_to_setting_string(&json!({"enabled": true})),
            "{\"enabled\":true}"
        );
    }

    #[test]
    fn profile_update_application_covers_all_postgres_metadata_variants() {
        let mut email = "old@example.test".to_string();
        let current_email = email.clone();
        let mut display_name = "Old".to_string();
        let mut metadata = json!({ "email_verified": true });
        let updates = [
            AuthUserProfileUpdate::Nickname("New".to_string()),
            AuthUserProfileUpdate::Email("new@example.test".to_string()),
            AuthUserProfileUpdate::Avatar("data:image/png;base64,avatar".to_string()),
            AuthUserProfileUpdate::Language("en".to_string()),
            AuthUserProfileUpdate::DefaultCurrency("USD".to_string()),
            AuthUserProfileUpdate::FirstDayOfWeek(0),
            AuthUserProfileUpdate::DefaultAccountId(Some(10)),
            AuthUserProfileUpdate::TransactionEditScope(2),
            AuthUserProfileUpdate::FiscalYearStart(4),
            AuthUserProfileUpdate::CalendarDisplayType(1),
            AuthUserProfileUpdate::DateDisplayType(2),
            AuthUserProfileUpdate::LongDateFormat(3),
            AuthUserProfileUpdate::ShortDateFormat(4),
            AuthUserProfileUpdate::LongTimeFormat(5),
            AuthUserProfileUpdate::ShortTimeFormat(6),
            AuthUserProfileUpdate::FiscalYearFormat(7),
            AuthUserProfileUpdate::CurrencyDisplayType(8),
            AuthUserProfileUpdate::NumeralSystem(9),
            AuthUserProfileUpdate::DecimalSeparator(10),
            AuthUserProfileUpdate::DigitGroupingSymbol(11),
            AuthUserProfileUpdate::DigitGrouping(12),
            AuthUserProfileUpdate::CoordinateDisplayType(13),
            AuthUserProfileUpdate::ExpenseAmountColor(14),
            AuthUserProfileUpdate::IncomeAmountColor(15),
            AuthUserProfileUpdate::CashAccountId(Some(16)),
            AuthUserProfileUpdate::CashTransferCategoryId(Some(17)),
            AuthUserProfileUpdate::ImportLearningEnabled(false),
            AuthUserProfileUpdate::InvestmentPlatformKeywords("[\"ETF\"]".to_string()),
            AuthUserProfileUpdate::InvestmentProductKeywords("[\"Fund\"]".to_string()),
            AuthUserProfileUpdate::InvestmentExcludeKeywords("[\"Exclude\"]".to_string()),
        ];

        for update in &updates {
            apply_postgres_profile_update(
                update,
                &mut email,
                &current_email,
                &mut display_name,
                &mut metadata,
            );
        }

        assert_eq!(email, "new@example.test");
        assert_eq!(display_name, "New");
        assert_eq!(metadata["nickname"], "New");
        assert_eq!(metadata["email_verified"], false);
        assert_eq!(metadata["avatar"], "data:image/png;base64,avatar");
        assert_eq!(metadata["language"], "en");
        assert_eq!(metadata["default_currency"], "USD");
        assert_eq!(metadata["first_day_of_week"], 0);
        assert_eq!(metadata["default_account_id"], 10);
        assert_eq!(metadata["transaction_edit_scope"], 2);
        assert_eq!(metadata["fiscal_year_start"], 4);
        assert_eq!(metadata["calendar_display_type"], 1);
        assert_eq!(metadata["date_display_type"], 2);
        assert_eq!(metadata["long_date_format"], 3);
        assert_eq!(metadata["short_date_format"], 4);
        assert_eq!(metadata["long_time_format"], 5);
        assert_eq!(metadata["short_time_format"], 6);
        assert_eq!(metadata["fiscal_year_format"], 7);
        assert_eq!(metadata["currency_display_type"], 8);
        assert_eq!(metadata["numeral_system"], 9);
        assert_eq!(metadata["decimal_separator"], 10);
        assert_eq!(metadata["digit_grouping_symbol"], 11);
        assert_eq!(metadata["digit_grouping"], 12);
        assert_eq!(metadata["coordinate_display_type"], 13);
        assert_eq!(metadata["expense_amount_color"], 14);
        assert_eq!(metadata["income_amount_color"], 15);
        assert_eq!(metadata["cash_account_id"], 16);
        assert_eq!(metadata["cash_transfer_category_id"], 17);
        assert_eq!(metadata["import_learning_enabled"], false);
        assert_eq!(metadata["investment_platform_keywords"], "[\"ETF\"]");
        assert_eq!(metadata["investment_product_keywords"], "[\"Fund\"]");
        assert_eq!(metadata["investment_exclude_keywords"], "[\"Exclude\"]");

        for update in [
            AuthUserProfileUpdate::DefaultAccountId(None),
            AuthUserProfileUpdate::CashAccountId(None),
            AuthUserProfileUpdate::CashTransferCategoryId(None),
        ] {
            apply_postgres_profile_update(
                &update,
                &mut email,
                &current_email,
                &mut display_name,
                &mut metadata,
            );
        }
        assert!(metadata.get("default_account_id").is_none());
        assert!(metadata.get("cash_account_id").is_none());
        assert!(metadata.get("cash_transfer_category_id").is_none());

        assert!(matches!(
            postgres_auth_error(sqlx::Error::RowNotFound),
            DbError::InvalidOperation(message) if message.contains("postgres auth error")
        ));
    }

    #[tokio::test]
    async fn empty_profile_update_short_circuits_before_postgres_io() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .expect("lazy postgres pool");

        let changed = update_postgres_auth_user_profile(
            &pool,
            UserId::new(1).expect("user id"),
            &[],
            "2026-01-01T00:00:00Z",
        )
        .await
        .expect("empty updates do not touch postgres");

        assert!(!changed);
    }

    #[test]
    fn postgres_user_id_conversion_rejects_invalid_bigint_boundaries() {
        assert!(user_id_from_i64(-1).is_err());
        assert!(user_id_from_i64(0).is_err());
        assert_eq!(user_id_from_i64(42).unwrap().get(), 42);
        assert!(user_id_i64(UserId::new(u64::MAX).unwrap()).is_err());
    }
}
