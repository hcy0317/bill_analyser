use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bill_analyser_core::account_rules::{
    normalize_account_role_scope, normalize_account_rule_field_scope,
    normalize_transaction_type_scope, AccountRuleCandidate, DEFAULT_FIELD_SCOPES,
};
use chrono::{TimeZone, Utc};
use rusqlite::types::ValueRef;
use rusqlite::{Error as RusqliteError, ErrorCode, Row};
use serde_json::{Map, Number, Value};

use crate::{DbError, DbResult};

use super::AccountRuleRecord;
#[derive(Debug)]
pub(super) struct AccountAliasSource {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) aliases: Vec<String>,
    pub(super) hidden: bool,
}

pub(super) fn account_rule_from_row(row: &Row<'_>) -> rusqlite::Result<AccountRuleRecord> {
    let mut record = Map::new();
    for column in [
        "id",
        "user_id",
        "account_id",
        "name",
        "priority",
        "rule_expression",
        "regex_enabled",
        "enabled",
        "applied_count",
        "last_applied_at",
        "match_count",
        "last_matched_at",
        "account_role_scope",
        "transaction_type_scope",
        "source",
        "source_key",
        "created_at",
        "updated_at",
        "account_name",
        "account_type",
        "account_hidden",
    ] {
        record.insert(
            column.to_string(),
            sqlite_value_to_json(row.get_ref(column)?),
        );
    }
    let raw_field_scope = row
        .get::<_, Option<String>>("field_scope")?
        .unwrap_or_default();
    record.insert(
        "field_scope".to_string(),
        Value::Array(
            parse_field_scope_json(&raw_field_scope)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    Ok(record)
}

pub(super) fn account_rule_candidate_from_record(
    record: AccountRuleRecord,
) -> DbResult<AccountRuleCandidate> {
    Ok(AccountRuleCandidate {
        rule_id: value_as_i64(record.get("id")).unwrap_or(0),
        account_id: value_as_i64(record.get("account_id")).unwrap_or(0),
        account_role_scope: normalize_account_role_scope(
            record.get("account_role_scope").and_then(Value::as_str),
        )
        .map_err(DbError::InvalidOperation)?,
        transaction_type_scope: normalize_transaction_type_scope(
            record.get("transaction_type_scope").and_then(Value::as_str),
        )
        .map_err(DbError::InvalidOperation)?,
        field_scope: normalize_account_rule_field_scope(record.get("field_scope"))
            .map_err(DbError::InvalidOperation)?,
        rule_expression: record
            .get("rule_expression")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        regex_enabled: record.get("regex_enabled").is_some_and(value_truthy),
        enabled: record.get("enabled").map(value_truthy).unwrap_or(true),
        priority: value_as_i64(record.get("priority")).unwrap_or(100),
    })
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(number) => Value::Number(Number::from(number)),
        ValueRef::Real(number) => Number::from_f64(number).map_or(Value::Null, Value::Number),
        ValueRef::Text(text) => Value::String(String::from_utf8_lossy(text).to_string()),
        ValueRef::Blob(blob) => Value::String(String::from_utf8_lossy(blob).to_string()),
    }
}

pub(super) fn utc_now_iso() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    let nanos = duration.subsec_nanos();
    Utc.timestamp_opt(seconds, nanos)
        .single()
        .map(|value| value.naive_utc().format("%Y-%m-%dT%H:%M:%S%.f").to_string())
        .unwrap_or_else(|| "1970-01-01T00:00:00".to_string())
}

pub(super) fn is_constraint_error(error: &RusqliteError) -> bool {
    matches!(
        error,
        RusqliteError::SqliteFailure(failure, _) if failure.code == ErrorCode::ConstraintViolation
    )
}

pub(super) fn value_as_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(flag)) => Some(i64::from(*flag)),
        Some(Value::Null) | Some(Value::Array(_)) | Some(Value::Object(_)) | None => None,
    }
}

pub(super) fn required_i64(value: &Value, field: &str) -> DbResult<i64> {
    value_as_i64(Some(value)).ok_or_else(|| {
        DbError::InvalidOperation(format!("{field} must be an integer-compatible value"))
    })
}

pub(super) fn required_rule_expression(value: Option<&Value>) -> DbResult<String> {
    match value {
        Some(Value::Null) | None => Err(DbError::InvalidOperation(
            "rule_expression is required".to_string(),
        )),
        Some(value) => {
            let expression = sql_text_value(value)?;
            if expression.trim().is_empty() {
                Err(DbError::InvalidOperation(
                    "rule_expression is required".to_string(),
                ))
            } else {
                Ok(expression)
            }
        }
    }
}

pub(super) fn bool_int_value(value: &Value) -> DbResult<i64> {
    match value {
        Value::Bool(flag) => Ok(i64::from(*flag)),
        Value::Number(number) => number
            .as_i64()
            .map(|value| i64::from(value != 0))
            .ok_or_else(|| {
                DbError::InvalidOperation("boolean field must be an integer".to_string())
            }),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Ok(1),
                "false" | "0" | "no" | "off" | "" => Ok(0),
                _ => Err(DbError::InvalidOperation(
                    "boolean field must be truthy or falsy".to_string(),
                )),
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => Err(DbError::InvalidOperation(
            "boolean field must be truthy or falsy".to_string(),
        )),
    }
}

pub(super) fn sql_text_value(value: &Value) -> DbResult<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Null => Ok(String::new()),
        Value::Array(_) | Value::Object(_) => Err(DbError::InvalidOperation(
            "text field must be scalar".to_string(),
        )),
    }
}

pub(super) fn field_scope_json(scopes: &[String]) -> DbResult<String> {
    serde_json::to_string(scopes)
        .map_err(|error| DbError::InvalidOperation(format!("invalid field_scope: {error}")))
}

fn parse_field_scope_json(raw: &str) -> Vec<String> {
    normalize_account_rule_field_scope(Some(&Value::String(raw.to_string()))).unwrap_or_else(|_| {
        DEFAULT_FIELD_SCOPES
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    })
}

pub(super) fn normalize_alias(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(super) fn normalize_rule_expression_alias(expression: &str) -> String {
    expression
        .trim()
        .strip_prefix("OR={")
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(expression)
        .replace('\\', "")
        .trim()
        .to_ascii_lowercase()
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        ),
        Value::Null | Value::Array(_) | Value::Object(_) => false,
    }
}
