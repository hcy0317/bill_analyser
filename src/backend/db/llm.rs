// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use bill_analyser_core::{
    build_runtime_llm_config_from_saved_config, normalize_llm_advanced_settings,
    normalize_provider_auth_config,
};
use chrono::Utc;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{json, Map, Value};

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq)]
pub struct LlmConfigDraft {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub credential_config: Value,
    pub advanced_settings: Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LlmConfigUpdate {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub credential_config: Option<Value>,
    pub advanced_settings: Option<Value>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmCandidateDraft {
    pub user_id: i64,
    pub candidate_type: String,
    pub source_bill_ids: Vec<i64>,
    pub suggested_main_category: String,
    pub suggested_sub_category: String,
    pub suggested_rule_expression: String,
    pub confidence: f64,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_response_raw: String,
}

pub fn init_llm_runtime_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS llm_candidates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            type TEXT NOT NULL DEFAULT 'classification',
            source_bill_ids TEXT,
            suggested_main_category TEXT,
            suggested_sub_category TEXT,
            suggested_rule_expression TEXT,
            confidence REAL DEFAULT 0.0,
            llm_provider TEXT,
            llm_model TEXT,
            llm_response_raw TEXT,
            status TEXT DEFAULT 'pending',
            created_at TEXT DEFAULT (datetime('now','localtime')),
            reviewed_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_llm_candidates_user_status
            ON llm_candidates(user_id, status);

        CREATE TABLE IF NOT EXISTS llm_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT 'openai',
            model TEXT NOT NULL DEFAULT '',
            api_key TEXT DEFAULT '',
            base_url TEXT DEFAULT '',
            credential_config TEXT NOT NULL DEFAULT '{}',
            advanced_settings TEXT NOT NULL DEFAULT '{}',
            is_active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            UNIQUE(user_id, name)
        );
        CREATE INDEX IF NOT EXISTS idx_llm_configs_user_active
            ON llm_configs(user_id, is_active);
        ",
    )?;
    if !table_has_column(connection, "llm_configs", "advanced_settings")? {
        connection.execute(
            "ALTER TABLE llm_configs ADD COLUMN advanced_settings TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }
    if !table_has_column(connection, "llm_configs", "credential_config")? {
        connection.execute(
            "ALTER TABLE llm_configs ADD COLUMN credential_config TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }
    Ok(())
}

pub fn list_llm_configs(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    let mut statement = connection.prepare(
        "SELECT * FROM llm_configs WHERE user_id = ?1 ORDER BY is_active DESC, updated_at DESC",
    )?;
    let rows = statement.query_map(params![user_id], llm_config_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_active_llm_config(connection: &Connection, user_id: i64) -> DbResult<Option<Value>> {
    connection
        .query_row(
            "SELECT * FROM llm_configs WHERE user_id = ?1 AND is_active = 1 LIMIT 1",
            params![user_id],
            llm_config_from_row,
        )
        .optional()
        .map_err(Into::into)
}

pub fn create_llm_config(
    connection: &Connection,
    user_id: i64,
    draft: &LlmConfigDraft,
) -> DbResult<Value> {
    let now = now_text();
    if draft.is_active {
        connection.execute(
            "UPDATE llm_configs SET is_active = 0, updated_at = ?1 WHERE user_id = ?2 AND is_active = 1",
            params![now, user_id],
        )?;
    }
    connection.execute(
        "INSERT INTO llm_configs (
            user_id, name, provider, model, api_key, base_url,
            credential_config, advanced_settings, is_active, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            user_id,
            draft.name,
            draft.provider,
            draft.model,
            draft.api_key,
            draft.base_url,
            serialize_credential_config(&draft.credential_config, &draft.api_key),
            serialize_advanced_settings(&draft.advanced_settings),
            i64::from(draft.is_active),
            now,
        ],
    )?;
    let config_id = connection.last_insert_rowid();
    get_llm_config_by_id(connection, config_id, user_id)?.ok_or_else(|| {
        DbError::InvalidOperation("created llm config could not be reloaded".to_string())
    })
}

pub fn update_llm_config(
    connection: &Connection,
    config_id: i64,
    user_id: i64,
    update: &LlmConfigUpdate,
) -> DbResult<Option<Value>> {
    if get_llm_config_by_id(connection, config_id, user_id)?.is_none() {
        return Ok(None);
    }

    let now = now_text();
    if update.is_active == Some(true) {
        connection.execute(
            "UPDATE llm_configs SET is_active = 0, updated_at = ?1 WHERE user_id = ?2 AND id != ?3",
            params![now, user_id, config_id],
        )?;
    }

    let mut assignments = vec!["updated_at = ?".to_string()];
    let mut values = vec![SqlValue::Text(now)];
    if let Some(name) = update.name.as_ref() {
        assignments.push("name = ?".to_string());
        values.push(SqlValue::Text(name.clone()));
    }
    if let Some(provider) = update.provider.as_ref() {
        assignments.push("provider = ?".to_string());
        values.push(SqlValue::Text(provider.clone()));
    }
    if let Some(model) = update.model.as_ref() {
        assignments.push("model = ?".to_string());
        values.push(SqlValue::Text(model.clone()));
    }
    if let Some(api_key) = update.api_key.as_ref() {
        assignments.push("api_key = ?".to_string());
        values.push(SqlValue::Text(api_key.clone()));
    }
    if let Some(base_url) = update.base_url.as_ref() {
        assignments.push("base_url = ?".to_string());
        values.push(SqlValue::Text(base_url.clone()));
    }
    if let Some(credential_config) = update.credential_config.as_ref() {
        assignments.push("credential_config = ?".to_string());
        values.push(SqlValue::Text(serialize_credential_config(
            credential_config,
            update.api_key.as_deref().unwrap_or_default(),
        )));
    }
    if let Some(advanced_settings) = update.advanced_settings.as_ref() {
        assignments.push("advanced_settings = ?".to_string());
        values.push(SqlValue::Text(serialize_advanced_settings(
            advanced_settings,
        )));
    }
    if let Some(is_active) = update.is_active {
        assignments.push("is_active = ?".to_string());
        values.push(SqlValue::Integer(i64::from(is_active)));
    }
    values.push(SqlValue::Integer(config_id));
    values.push(SqlValue::Integer(user_id));
    connection.execute(
        &format!(
            "UPDATE llm_configs SET {} WHERE id = ? AND user_id = ?",
            assignments.join(", ")
        ),
        params_from_iter(values),
    )?;
    get_llm_config_by_id(connection, config_id, user_id)
}

pub fn delete_llm_config(connection: &Connection, config_id: i64, user_id: i64) -> DbResult<bool> {
    let deleted = connection.execute(
        "DELETE FROM llm_configs WHERE id = ?1 AND user_id = ?2",
        params![config_id, user_id],
    )?;
    Ok(deleted > 0)
}

pub fn activate_llm_config(
    connection: &Connection,
    config_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    if get_llm_config_by_id(connection, config_id, user_id)?.is_none() {
        return Ok(false);
    }
    let now = now_text();
    connection.execute(
        "UPDATE llm_configs SET is_active = 0, updated_at = ?1 WHERE user_id = ?2",
        params![now, user_id],
    )?;
    connection.execute(
        "UPDATE llm_configs SET is_active = 1, updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
        params![now, config_id, user_id],
    )?;
    Ok(true)
}

pub fn effective_llm_config_from_saved(connection: &Connection, user_id: i64) -> DbResult<Value> {
    get_active_llm_config(connection, user_id).map(|active| {
        active
            .as_ref()
            .map(build_runtime_llm_config_from_saved_config)
            .unwrap_or_else(default_llm_runtime_config)
    })
}

pub fn list_llm_candidates(
    connection: &Connection,
    user_id: i64,
    status: Option<&str>,
    candidate_type: Option<&str>,
    limit: i64,
    offset: i64,
) -> DbResult<Vec<Value>> {
    let mut sql = String::from("SELECT * FROM llm_candidates WHERE user_id = ?");
    let mut values = vec![SqlValue::Integer(user_id)];
    if let Some(status) = status {
        sql.push_str(" AND status = ?");
        values.push(SqlValue::Text(status.to_string()));
    }
    if let Some(candidate_type) = candidate_type {
        sql.push_str(" AND type = ?");
        values.push(SqlValue::Text(candidate_type.to_string()));
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
    values.push(SqlValue::Integer(limit.max(0)));
    values.push(SqlValue::Integer(offset.max(0)));

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), llm_candidate_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn count_llm_candidates(
    connection: &Connection,
    user_id: i64,
    status: Option<&str>,
    candidate_type: Option<&str>,
) -> DbResult<i64> {
    let mut sql = String::from("SELECT COUNT(*) FROM llm_candidates WHERE user_id = ?");
    let mut values = vec![SqlValue::Integer(user_id)];
    if let Some(status) = status {
        sql.push_str(" AND status = ?");
        values.push(SqlValue::Text(status.to_string()));
    }
    if let Some(candidate_type) = candidate_type {
        sql.push_str(" AND type = ?");
        values.push(SqlValue::Text(candidate_type.to_string()));
    }
    connection
        .query_row(&sql, params_from_iter(values), |row| row.get::<_, i64>(0))
        .map_err(Into::into)
}

pub fn get_llm_candidate_by_id(
    connection: &Connection,
    candidate_id: i64,
    user_id: i64,
) -> DbResult<Option<Value>> {
    connection
        .query_row(
            "SELECT * FROM llm_candidates WHERE id = ?1 AND user_id = ?2",
            params![candidate_id, user_id],
            llm_candidate_from_row,
        )
        .optional()
        .map_err(Into::into)
}

pub fn create_llm_candidate(connection: &Connection, draft: &LlmCandidateDraft) -> DbResult<Value> {
    let source_bill_ids = Value::Array(
        draft
            .source_bill_ids
            .iter()
            .copied()
            .map(Value::from)
            .collect(),
    )
    .to_string();
    connection.execute(
        "INSERT INTO llm_candidates (
            user_id, type, source_bill_ids, suggested_main_category,
            suggested_sub_category, suggested_rule_expression, confidence,
            llm_provider, llm_model, llm_response_raw, status, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)",
        params![
            draft.user_id,
            draft.candidate_type,
            source_bill_ids,
            draft.suggested_main_category,
            draft.suggested_sub_category,
            draft.suggested_rule_expression,
            draft.confidence,
            draft.llm_provider,
            draft.llm_model,
            draft.llm_response_raw,
            now_text(),
        ],
    )?;
    let candidate_id = connection.last_insert_rowid();
    get_llm_candidate_by_id(connection, candidate_id, draft.user_id)?.ok_or_else(|| {
        DbError::InvalidOperation("created llm candidate could not be reloaded".to_string())
    })
}

pub fn update_llm_candidate_status(
    connection: &Connection,
    candidate_id: i64,
    status: &str,
    user_id: i64,
) -> DbResult<bool> {
    let updated = connection.execute(
        "UPDATE llm_candidates SET status = ?1, reviewed_at = ?2 WHERE id = ?3 AND user_id = ?4",
        params![status, now_text(), candidate_id, user_id],
    )?;
    Ok(updated > 0)
}

pub fn accept_llm_candidate(
    connection: &Connection,
    candidate_id: i64,
    user_id: i64,
) -> DbResult<Option<Value>> {
    let Some(candidate) = get_llm_candidate_by_id(connection, candidate_id, user_id)? else {
        return Ok(None);
    };
    update_llm_candidate_status(connection, candidate_id, "accepted", user_id)?;

    let mut result = Map::new();
    result.insert("candidate_id".to_string(), json!(candidate_id));
    result.insert("status".to_string(), json!("accepted"));
    if should_materialize_rule_candidate(&candidate) {
        if let Some(rule_id) = create_rule_for_llm_candidate(connection, &candidate, user_id)? {
            result.insert("created_rule_id".to_string(), json!(rule_id));
        }
    }
    Ok(Some(Value::Object(result)))
}

pub fn reject_llm_candidate(
    connection: &Connection,
    candidate_id: i64,
    user_id: i64,
) -> DbResult<Option<bool>> {
    if get_llm_candidate_by_id(connection, candidate_id, user_id)?.is_none() {
        return Ok(None);
    }
    update_llm_candidate_status(connection, candidate_id, "rejected", user_id).map(Some)
}

pub fn default_llm_runtime_config() -> Value {
    json!({
        "enabled": false,
        "provider": "openai",
        "provider_config": {},
        "advanced_settings": {},
    })
}

fn get_llm_config_by_id(
    connection: &Connection,
    config_id: i64,
    user_id: i64,
) -> DbResult<Option<Value>> {
    connection
        .query_row(
            "SELECT * FROM llm_configs WHERE id = ?1 AND user_id = ?2",
            params![config_id, user_id],
            llm_config_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn llm_config_from_row(row: &Row<'_>) -> rusqlite::Result<Value> {
    let advanced_settings = row_text(row, "advanced_settings")?;
    let credential_config = row_text(row, "credential_config")?;
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "user_id": row.get::<_, i64>("user_id")?,
        "name": row_text(row, "name")?,
        "provider": row_text(row, "provider")?,
        "model": row_text(row, "model")?,
        "api_key": row_text(row, "api_key")?,
        "base_url": row_text(row, "base_url")?,
        "credential_config": normalized_credential_value(&Value::String(credential_config)),
        "advanced_settings": normalized_settings_value(&Value::String(advanced_settings)),
        "is_active": row.get::<_, i64>("is_active")?,
        "created_at": row_text(row, "created_at")?,
        "updated_at": row_text(row, "updated_at")?,
    }))
}

fn llm_candidate_from_row(row: &Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "user_id": row.get::<_, i64>("user_id")?,
        "type": row_text(row, "type")?,
        "source_bill_ids": row_text(row, "source_bill_ids")?,
        "suggested_main_category": row_text(row, "suggested_main_category")?,
        "suggested_sub_category": row_text(row, "suggested_sub_category")?,
        "suggested_rule_expression": row_text(row, "suggested_rule_expression")?,
        "confidence": row.get::<_, Option<f64>>("confidence")?.unwrap_or(0.0),
        "llm_provider": row_text(row, "llm_provider")?,
        "llm_model": row_text(row, "llm_model")?,
        "llm_response_raw": row_text(row, "llm_response_raw")?,
        "status": row_text(row, "status")?,
        "created_at": row_text(row, "created_at")?,
        "reviewed_at": row_text(row, "reviewed_at")?,
    }))
}

fn row_text(row: &Row<'_>, column: &str) -> rusqlite::Result<String> {
    row.get::<_, Option<String>>(column)
        .map(|value| value.unwrap_or_default())
}

fn serialize_advanced_settings(value: &Value) -> String {
    normalized_settings_value(value).to_string()
}

fn serialize_credential_config(value: &Value, legacy_api_key: &str) -> String {
    let mut normalized = normalized_credential_value(value);
    if normalized
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .is_empty()
        && !legacy_api_key.trim().is_empty()
    {
        if let Some(object) = normalized.as_object_mut() {
            object.insert("access_token".to_string(), json!(legacy_api_key.trim()));
            object.insert("credential_mode".to_string(), json!("api_key"));
        }
    }
    normalized.to_string()
}

fn normalized_credential_value(value: &Value) -> Value {
    match value {
        Value::String(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .ok()
            .map(|parsed| normalize_provider_auth_config(Some(&parsed)))
            .unwrap_or_else(|| normalize_provider_auth_config(Some(value))),
        Value::String(_) | Value::Null => json!({}),
        _ => normalize_provider_auth_config(Some(value)),
    }
}

fn normalized_settings_value(value: &Value) -> Value {
    Value::Object(normalize_llm_advanced_settings(Some(value)))
}

fn should_materialize_rule_candidate(candidate: &Value) -> bool {
    candidate
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|candidate_type| candidate_type.starts_with("rule_"))
        && candidate
            .get("suggested_rule_expression")
            .and_then(Value::as_str)
            .is_some_and(|expression| !expression.trim().is_empty())
}

fn create_rule_for_llm_candidate(
    connection: &Connection,
    candidate: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let main_category = text_field(candidate, "suggested_main_category");
    let sub_category = text_field(candidate, "suggested_sub_category");
    let rule_expression = text_field(candidate, "suggested_rule_expression");
    if bill_analyser_core::category_rules::compile_rule_expression(rule_expression.trim(), false)
        .is_empty
    {
        return Ok(None);
    }
    let category_id = connection
        .query_row(
            "SELECT id FROM categories WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3",
            params![user_id, main_category, sub_category],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(category_id) = category_id else {
        return Ok(None);
    };
    let rule_duplicate = connection
        .query_row(
            "SELECT 1 FROM category_rules WHERE user_id = ?1 AND category_id = ?2 AND rule_expression = ?3 LIMIT 1",
            params![user_id, category_id, rule_expression.trim()],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if rule_duplicate {
        return Ok(None);
    }
    let candidate_type = text_field(candidate, "type");
    let name_prefix = if candidate_type == "rule_synthesis" {
        "LLM synthesized"
    } else {
        "LLM induced"
    };
    connection.execute(
        "INSERT INTO category_rules (
            user_id, category_id, name, priority, rule_expression,
            regex_enabled, enabled, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, 50, ?4, 0, 1, ?5, ?5)",
        params![
            user_id,
            category_id,
            format!("{name_prefix}: {main_category}/{sub_category}"),
            rule_expression.trim(),
            now_text(),
        ],
    )?;
    Ok(Some(connection.last_insert_rowid()))
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> DbResult<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name = match row.get_ref(1)? {
            ValueRef::Text(value) => String::from_utf8_lossy(value).to_string(),
            _ => String::new(),
        };
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}
