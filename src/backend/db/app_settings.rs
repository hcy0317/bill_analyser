// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use bill_analyser_core::{
    normalize_ocr_config, OcrConfigContract, OCR_AVAILABLE_PROVIDERS, OCR_DISABLED_PROVIDER_NAME,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{DbError, DbResult};

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
pub fn init_app_settings_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL UNIQUE,
            value TEXT,
            value_type TEXT DEFAULT 'string',
            description TEXT,
            is_encrypted BOOLEAN DEFAULT 0,
            updated_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_app_settings_key ON app_settings(key);
        ",
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_app_setting(connection: &Connection, key: &str) -> DbResult<Option<String>> {
    let normalized_key = normalize_setting_key(key)?;
    let value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![normalized_key],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(value)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_app_setting_row(connection: &Connection, key: &str) -> DbResult<Option<AppSettingRow>> {
    let normalized_key = normalize_setting_key(key)?;
    connection
        .query_row(
            "SELECT id, key, value, value_type, description, is_encrypted, created_at, updated_at
             FROM app_settings WHERE key = ?1",
            params![normalized_key],
            app_setting_row_from_row,
        )
        .optional()
        .map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn set_app_setting(connection: &Connection, draft: &AppSettingDraft) -> DbResult<bool> {
    let key = normalize_setting_key(&draft.key)?;
    let value_type = normalize_setting_value_type(&draft.value_type);
    let now = now_text();
    connection.execute(
        "
        INSERT INTO app_settings (
            key, value, value_type, description, is_encrypted, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            value_type = excluded.value_type,
            description = excluded.description,
            is_encrypted = excluded.is_encrypted,
            updated_at = excluded.updated_at
        ",
        params![
            key,
            draft.value,
            value_type,
            draft.description,
            draft.is_encrypted,
            now,
        ],
    )?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn load_ocr_config_setting(connection: &Connection) -> DbResult<OcrConfigContract> {
    let stored = get_app_setting(connection, OCR_CONFIG_SETTING_KEY)?;
    let parsed = stored
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok());
    Ok(normalize_ocr_config(parsed.as_ref()))
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
pub fn store_ocr_config_setting(
    connection: &Connection,
    value: Option<&Value>,
) -> DbResult<OcrConfigContract> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "runtime",
        operation = "store_ocr_config_setting",
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
    set_app_setting(
        connection,
        &AppSettingDraft {
            key: OCR_CONFIG_SETTING_KEY.to_string(),
            value: stored_value,
            value_type: "json".to_string(),
            description: Some("Receipt OCR runtime configuration".to_string()),
            is_encrypted: false,
        },
    )?;
    Ok(config)
}

fn app_setting_row_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppSettingRow> {
    Ok(AppSettingRow {
        id: row.get("id")?,
        key: row.get("key")?,
        value: row.get("value")?,
        value_type: row.get("value_type")?,
        description: row.get("description")?,
        is_encrypted: row.get("is_encrypted")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
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

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_setting_value_type(value_type: &str) -> String {
    let trimmed = value_type.trim();
    if trimmed.is_empty() {
        "string".to_string()
    } else {
        trimmed.to_string()
    }
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

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}
