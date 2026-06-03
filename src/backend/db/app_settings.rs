// 中文导读：PostgreSQL repository 层，负责运行态设置与 OCR 配置。
// 维护重点：配置保存在 user-scoped settings 表，不再初始化或读取 non-Postgres app_settings。
// 不变式：设置 key 必须非空；OCR provider 必须通过 core 合同校验。

use bill_analyser_core::{
    normalize_ocr_config, OcrConfigContract, OCR_AVAILABLE_PROVIDERS, OCR_DISABLED_PROVIDER_NAME,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;

use crate::{DbError, DbResult, PostgresPool};

pub const OCR_CONFIG_SETTING_KEY: &str = "receipt_ocr_config";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettingDraft {
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub description: Option<String>,
    pub is_encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettingRow {
    pub id: i64,
    pub key: String,
    pub value: Option<String>,
    pub value_type: String,
    pub description: Option<String>,
    pub is_encrypted: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_app_setting(
    pool: &PostgresPool,
    user_id: i64,
    key: &str,
) -> DbResult<Option<String>> {
    let row = get_postgres_app_setting_value(pool, user_id, key).await?;
    Ok(row.map(|value| value.to_string()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_app_setting_row(
    pool: &PostgresPool,
    user_id: i64,
    key: &str,
) -> DbResult<Option<AppSettingRow>> {
    let normalized_key = normalize_setting_key(key)?;
    let row = sqlx::query(
        "
        SELECT id, key, value, sensitive, created_at, updated_at
        FROM settings
        WHERE user_id = $1 AND key = $2
        ",
    )
    .bind(user_id)
    .bind(normalized_key)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let value: Value = row.try_get("value")?;
        let sensitive: bool = row.try_get("sensitive")?;
        Ok(AppSettingRow {
            id: row.try_get("id")?,
            key: row.try_get("key")?,
            value: Some(value.to_string()),
            value_type: "json".to_string(),
            description: None,
            is_encrypted: sensitive,
            created_at: timestamp_text(&row, "created_at")?,
            updated_at: timestamp_text(&row, "updated_at")?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn set_postgres_app_setting(
    pool: &PostgresPool,
    user_id: i64,
    draft: &AppSettingDraft,
) -> DbResult<bool> {
    let key = normalize_setting_key(&draft.key)?;
    let value = parse_setting_json(&draft.value)?;
    sqlx::query(
        "
        INSERT INTO settings (user_id, key, value, sensitive, created_at, updated_at)
        VALUES ($1, $2, $3, $4, now(), now())
        ON CONFLICT (user_id, key) DO UPDATE SET
            value = EXCLUDED.value,
            sensitive = EXCLUDED.sensitive,
            updated_at = now(),
            version = settings.version + 1
        ",
    )
    .bind(user_id)
    .bind(key)
    .bind(value)
    .bind(draft.is_encrypted)
    .execute(pool)
    .await?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn load_postgres_ocr_config_setting(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<OcrConfigContract> {
    let stored = get_postgres_app_setting_value(pool, user_id, OCR_CONFIG_SETTING_KEY).await?;
    Ok(normalize_ocr_config(stored.as_ref()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_ocr_config_for_storage(value: Option<&Value>) -> DbResult<OcrConfigContract> {
    let object = value.and_then(Value::as_object);
    let provider = object
        .and_then(|item| item.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or(OCR_DISABLED_PROVIDER_NAME);
    validate_ocr_provider(provider)?;
    Ok(normalize_ocr_config(value))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn store_postgres_ocr_config_setting(
    pool: &PostgresPool,
    user_id: i64,
    value: Option<&Value>,
) -> DbResult<OcrConfigContract> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "runtime",
        operation = "store_postgres_ocr_config_setting",
        "business operation entered"
    );
    let config = normalize_ocr_config_for_storage(value)?;
    let stored_value = json!({
        "provider": config.provider,
        "lang": config.lang,
        "model": config.model,
        "base_url": config.base_url,
        "parameters": config.parameters,
        "credential_config": config.credential_config,
    })
    .to_string();
    set_postgres_app_setting(
        pool,
        user_id,
        &AppSettingDraft {
            key: OCR_CONFIG_SETTING_KEY.to_string(),
            value: stored_value,
            value_type: "json".to_string(),
            description: Some("Receipt OCR runtime configuration".to_string()),
            is_encrypted: false,
        },
    )
    .await?;
    Ok(config)
}

async fn get_postgres_app_setting_value(
    pool: &PostgresPool,
    user_id: i64,
    key: &str,
) -> DbResult<Option<Value>> {
    let normalized_key = normalize_setting_key(key)?;
    let row = sqlx::query("SELECT value FROM settings WHERE user_id = $1 AND key = $2")
        .bind(user_id)
        .bind(normalized_key)
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("value"))
        .transpose()
        .map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_setting_key(key: &str) -> DbResult<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(DbError::InvalidOperation(
            "app setting key cannot be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn parse_setting_json(value: &str) -> DbResult<Value> {
    serde_json::from_str(value).map_err(|error| {
        DbError::InvalidOperation(format!("app setting value must be valid JSON: {error}"))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn validate_ocr_provider(provider: &str) -> DbResult<()> {
    let normalized = provider.trim().to_lowercase();
    let provider_name = if normalized.is_empty() || matches!(normalized.as_str(), "none" | "off") {
        OCR_DISABLED_PROVIDER_NAME
    } else {
        normalized.as_str()
    };
    if provider_name == OCR_DISABLED_PROVIDER_NAME
        || OCR_AVAILABLE_PROVIDERS.contains(&provider_name)
    {
        Ok(())
    } else {
        Err(DbError::InvalidOperation(
            "unknown OCR provider".to_string(),
        ))
    }
}

fn timestamp_text(row: &sqlx::postgres::PgRow, column: &str) -> sqlx::Result<String> {
    row.try_get::<chrono::DateTime<chrono::Utc>, _>(column)
        .map(|value| value.to_rfc3339())
}
