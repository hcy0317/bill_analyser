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

// 中文说明：把前端资料更新枚举应用到 users 表字段与 JSONB metadata，邮箱变更时同步撤销验证状态。
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

// 中文说明：在认证事务内写入审计事件，保证登录、2FA、重置等事件与业务提交同生命周期。
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

// 中文说明：在认证事务内创建 token session，统一访问 token、refresh token 和过期时间的 Postgres 写入。
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

// 中文说明：组装认证审计 metadata，保留客户端、错误和额外 JSON 字段但不暴露为顶层表字段。
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

// 中文说明：读取用户 JSONB metadata，作为 Postgres auth/profile 设置字段的 authoritative 来源。
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

// 中文说明：原子更新用户 metadata 和 updated_at，供 profile、锁定状态和设置同步复用。
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

// 中文说明：把 users 查询行投影为登录校验模型，合并 profile 字段与登录锁定/2FA metadata。
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

// 中文说明：把 users 行投影为前端 profile 响应，统一 JSONB 设置字段的默认值和 nickname 兜底。
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

// 中文说明：把 external_auth 行转换为外部认证绑定模型，隔离 SQLx 行字段读取细节。
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

// 中文说明：从 metadata 多候选键读取字符串设置，兼容数字和布尔旧值并应用默认值。
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

// 中文说明：从 metadata 读取可空字符串设置，空字符串按缺失处理以保持前端默认值逻辑。
fn metadata_optional_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    let value = metadata_string(metadata, keys, "");
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

// 中文说明：从 metadata 多候选键读取整数设置，兼容字符串和布尔旧值并应用默认值。
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

// 中文说明：从 metadata 读取可空整数设置，空字符串和缺失字段保持 None。
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

// 中文说明：从 metadata 读取布尔设置，兼容 1/0、true/false、yes/no 等历史表示。
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

// 中文说明：写入字符串 metadata 字段，确保非对象 metadata 先被重置为对象。
fn metadata_set_string(metadata: &mut Value, key: &str, value: &str) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::String(value.to_string()));
}

// 中文说明：写入整数 metadata 字段，供 profile 数值设置统一落到 JSONB。
fn metadata_set_i64(metadata: &mut Value, key: &str, value: i64) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::from(value));
}

// 中文说明：写入布尔 metadata 字段，供登录锁定、邮箱验证和用户偏好复用。
fn metadata_set_bool(metadata: &mut Value, key: &str, value: bool) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::from(value));
}

// 中文说明：写入可空整数 metadata，None 表示删除字段而不是写入 JSON null。
fn metadata_set_optional_i64(metadata: &mut Value, key: &str, value: Option<i64>) {
    let object = ensure_metadata_object(metadata);
    if let Some(value) = value {
        object.insert(key.to_string(), Value::from(value));
    } else {
        object.remove(key);
    }
}

// 中文说明：确保 metadata 可写为对象，遇到旧的 null/非对象值时重置为空对象。
fn ensure_metadata_object(metadata: &mut Value) -> &mut Map<String, Value> {
    if !metadata.is_object() {
        *metadata = Value::Object(Map::new());
    }
    metadata.as_object_mut().expect("metadata object")
}

// 中文说明：解析审计扩展 metadata，非法 JSON 保留为 Null 以避免中断认证事务。
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
        Value::Object(object) => object
            .get("value")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

// 中文说明：把核心 UserId 转为 PostgreSQL BIGINT，防止超出数据库范围的 id 进入 SQL bind。
fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get()).map_err(|_| {
        DbError::InvalidOperation("user id is outside PostgreSQL BIGINT range".to_string())
    })
}

// 中文说明：把 PostgreSQL BIGINT id 转回核心 UserId，拒绝负数和零值。
fn user_id_from_i64(raw_id: i64) -> DbResult<UserId> {
    let raw_id = u64::try_from(raw_id)
        .map_err(|_| DbError::InvalidOperation("Postgres user id must be positive".to_string()))?;
    UserId::new(raw_id)
        .map_err(|_| DbError::InvalidOperation("Postgres user id must be non-zero".to_string()))
}

// 中文说明：把 SQLx 错误收敛为 auth repository 的 DbError，避免调用方依赖 Postgres 具体错误类型。
fn postgres_auth_error(error: sqlx::Error) -> DbError {
    DbError::InvalidOperation(format!("postgres auth error: {error}"))
}

include!("auth_postgres/tests.rs");
