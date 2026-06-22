// 中文导读：PostgreSQL repository 层，负责 LLM 配置、候选项与规则落库。
// 维护重点：运行态只读写 Postgres llm_* 表；category rule 物化使用当前 JSONB 规则合同。
// 不变式：所有查询必须按 user_id 过滤，不能回退 non-Postgres。

use bill_analyser_core::{
    build_runtime_llm_config_from_saved_config, normalize_llm_advanced_settings,
    normalize_provider_auth_config,
};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, QueryBuilder, Row};

use crate::{DbError, DbResult, PostgresPool};

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

mod candidates;
mod configs;

pub use candidates::*;
pub use configs::*;

pub fn default_llm_runtime_config() -> Value {
    json!({
        "enabled": false,
        "provider": "openai",
        "provider_config": {},
        "advanced_settings": {},
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_postgres_llm_config_by_id(
    pool: &PostgresPool,
    config_id: i64,
    user_id: i64,
) -> DbResult<Option<Value>> {
    let row = sqlx::query(
        "
        SELECT id, user_id, name, provider, model, api_key, base_url,
               credential_config, advanced_settings, is_active, created_at, updated_at
        FROM llm_configs
        WHERE id = $1 AND user_id = $2
        ",
    )
    .bind(config_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(llm_config_from_row).transpose()
}

async fn deactivate_postgres_llm_configs(
    pool: &PostgresPool,
    user_id: i64,
    excluded_config_id: Option<i64>,
) -> DbResult<()> {
    match excluded_config_id {
        Some(config_id) => {
            sqlx::query(
                "
                UPDATE llm_configs
                SET is_active = false, updated_at = now(), version = version + 1
                WHERE user_id = $1 AND id != $2 AND is_active = true
                ",
            )
            .bind(user_id)
            .bind(config_id)
            .execute(pool)
            .await?;
        }
        None => {
            sqlx::query(
                "
                UPDATE llm_configs
                SET is_active = false, updated_at = now(), version = version + 1
                WHERE user_id = $1 AND is_active = true
                ",
            )
            .bind(user_id)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

fn llm_config_from_row(row: sqlx::postgres::PgRow) -> DbResult<Value> {
    let advanced_settings: Value = row.try_get("advanced_settings")?;
    let credential_config: Value = row.try_get("credential_config")?;
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "user_id": row.try_get::<i64, _>("user_id")?,
        "name": row_text(&row, "name")?,
        "provider": row_text(&row, "provider")?,
        "model": row_text(&row, "model")?,
        "api_key": row_text(&row, "api_key")?,
        "base_url": row_text(&row, "base_url")?,
        "credential_config": normalized_credential_value(&credential_config),
        "advanced_settings": normalized_settings_value(&advanced_settings),
        "is_active": row.try_get::<bool, _>("is_active")?,
        "created_at": timestamp_text(&row, "created_at")?,
        "updated_at": timestamp_text(&row, "updated_at")?,
    }))
}

fn llm_candidate_from_row(row: sqlx::postgres::PgRow) -> DbResult<Value> {
    let source_bill_ids: Value = row.try_get("source_bill_ids")?;
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "user_id": row.try_get::<i64, _>("user_id")?,
        "type": row_text(&row, "type")?,
        "source_bill_ids": source_bill_ids,
        "suggested_main_category": row_text(&row, "suggested_main_category")?,
        "suggested_sub_category": row_text(&row, "suggested_sub_category")?,
        "suggested_rule_expression": row_text(&row, "suggested_rule_expression")?,
        "confidence": row.try_get::<Option<f64>, _>("confidence")?.unwrap_or(0.0),
        "llm_provider": row_text(&row, "llm_provider")?,
        "llm_model": row_text(&row, "llm_model")?,
        "llm_response_raw": row_text(&row, "llm_response_raw")?,
        "status": row_text(&row, "status")?,
        "created_at": timestamp_text(&row, "created_at")?,
        "reviewed_at": row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("reviewed_at")?
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
    }))
}

fn row_text(row: &sqlx::postgres::PgRow, column: &str) -> sqlx::Result<String> {
    row.try_get::<Option<String>, _>(column)
        .map(|value| value.unwrap_or_default())
}

fn serialize_advanced_settings(value: &Value) -> Value {
    normalized_settings_value(value)
}

fn serialize_credential_config(value: &Value, api_key: &str) -> Value {
    let mut normalized = normalized_credential_value(value);
    if normalized
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .is_empty()
        && !api_key.trim().is_empty()
    {
        if let Some(object) = normalized.as_object_mut() {
            object.insert("access_token".to_string(), json!(api_key.trim()));
            object.insert("credential_mode".to_string(), json!("api_key"));
        }
    }
    normalized
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

#[tracing::instrument(level = "debug", skip_all)]
async fn create_postgres_rule_for_llm_candidate(
    pool: &PostgresPool,
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
    let category_id = sqlx::query(
        "
        SELECT id
        FROM categories
        WHERE user_id = $1
          AND (
              path = $2
              OR (path IS NULL AND name = $3)
              OR (split_part(path, '/', 1) = $3 AND COALESCE(NULLIF(substring(path from position('/' in path) + 1), ''), '') = $4)
          )
        ORDER BY display_order ASC, id ASC
        LIMIT 1
        ",
    )
    .bind(user_id)
    .bind(category_path(&main_category, &sub_category))
    .bind(&main_category)
    .bind(&sub_category)
    .fetch_optional(pool)
    .await?
    .map(|row| row.try_get::<i64, _>("id"))
    .transpose()?;
    let Some(category_id) = category_id else {
        return Ok(None);
    };

    let rule_expression_json = rule_expression_json(rule_expression.trim(), false);
    let duplicate = sqlx::query(
        "
        SELECT 1
        FROM category_rules
        WHERE user_id = $1
          AND category_id = $2
          AND rule_expression = $3
        LIMIT 1
        ",
    )
    .bind(user_id)
    .bind(category_id)
    .bind(&rule_expression_json)
    .fetch_optional(pool)
    .await?
    .is_some();
    if duplicate {
        return Ok(None);
    }

    let candidate_type = text_field(candidate, "type");
    let name_prefix = if candidate_type == "rule_synthesis" {
        "LLM synthesized"
    } else {
        "LLM induced"
    };
    let row = sqlx::query(
        "
        INSERT INTO category_rules (
            user_id, category_id, name, priority, rule_expression,
            enabled, created_at, updated_at
        )
        VALUES ($1, $2, $3, 50, $4, true, now(), now())
        RETURNING id
        ",
    )
    .bind(user_id)
    .bind(category_id)
    .bind(format!(
        "{name_prefix}: {}",
        category_path(&main_category, &sub_category)
    ))
    .bind(rule_expression_json)
    .fetch_one(pool)
    .await?;
    row.try_get("id").map(Some).map_err(Into::into)
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn category_path(main: &str, sub: &str) -> String {
    match (main.trim(), sub.trim()) {
        ("", "") => String::new(),
        (main, "") => main.to_string(),
        ("", sub) => sub.to_string(),
        (main, sub) => format!("{main}/{sub}"),
    }
}

fn rule_expression_json(expression: &str, regex_enabled: bool) -> Value {
    json!({
        "expression": expression,
        "regex_enabled": regex_enabled,
    })
}

fn timestamp_text(row: &sqlx::postgres::PgRow, column: &str) -> sqlx::Result<String> {
    row.try_get::<chrono::DateTime<chrono::Utc>, _>(column)
        .map(|value| value.to_rfc3339())
}
