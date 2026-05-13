use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{TimeZone, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{
    params, params_from_iter, Connection, Error as RusqliteError, ErrorCode, OptionalExtension, Row,
};
use serde_json::{Map, Number, Value};

use crate::{run_transaction, DbError, DbResult};

pub type CategoryRuleRecord = Map<String, Value>;

pub struct CategoryRulesRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> CategoryRulesRepository<'conn> {
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    pub fn list_rules(
        &mut self,
        user_id: i64,
        category_id: Option<i64>,
        enabled_only: bool,
    ) -> DbResult<Vec<CategoryRuleRecord>> {
        let mut sql = String::from(
            "SELECT
                cr.id, cr.user_id, cr.category_id, cr.name, cr.priority,
                cr.rule_expression, cr.regex_enabled, cr.enabled, cr.applied_count,
                cr.last_applied_at, cr.created_at, cr.updated_at,
                c.main_category, c.sub_category, c.type AS category_type
             FROM category_rules cr
             JOIN categories c ON cr.category_id = c.id
             WHERE cr.user_id = ? AND c.user_id = ?",
        );
        let mut params = vec![SqlValue::Integer(user_id), SqlValue::Integer(user_id)];

        if let Some(category_id) = category_id {
            sql.push_str(" AND cr.category_id = ?");
            params.push(SqlValue::Integer(category_id));
        }
        if enabled_only {
            sql.push_str(" AND cr.enabled = 1");
        }
        sql.push_str(" ORDER BY cr.priority ASC, cr.id ASC");

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params), category_rule_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_rule(&mut self, rule_id: i64, user_id: i64) -> DbResult<Option<CategoryRuleRecord>> {
        self.connection
            .query_row(
                "SELECT
                    cr.id, cr.user_id, cr.category_id, cr.name, cr.priority,
                    cr.rule_expression, cr.regex_enabled, cr.enabled, cr.applied_count,
                    cr.last_applied_at, cr.created_at, cr.updated_at,
                    c.main_category, c.sub_category, c.type AS category_type
                 FROM category_rules cr
                 JOIN categories c ON cr.category_id = c.id
                 WHERE cr.id = ? AND cr.user_id = ? AND c.user_id = ?",
                params![rule_id, user_id, user_id],
                category_rule_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn create_rule(&mut self, payload: &Value, user_id: i64) -> DbResult<Option<i64>> {
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("category rule payload must be an object".to_string())
        })?;
        let category_id = object
            .get("category_id")
            .and_then(value_as_i64)
            .ok_or_else(|| DbError::InvalidOperation("category_id is required".to_string()))?;
        let rule_expression = required_rule_expression(object.get("rule_expression"))?;
        let name = object
            .get("name")
            .map(sql_text_value)
            .transpose()?
            .unwrap_or_default();
        let priority = object.get("priority").and_then(value_as_i64).unwrap_or(100);
        let regex_enabled = object
            .get("regex_enabled")
            .map(bool_int_value)
            .transpose()?
            .unwrap_or(0);
        let enabled = object
            .get("enabled")
            .map(bool_int_value)
            .transpose()?
            .unwrap_or(1);
        if !self.category_belongs_to_user(category_id, user_id)? {
            return Ok(None);
        }
        let now = utc_now_iso();

        let result = self.connection.execute(
            "INSERT INTO category_rules (
                user_id, category_id, name, priority, rule_expression,
                regex_enabled, enabled, created_at, updated_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                user_id,
                category_id,
                name,
                priority,
                rule_expression,
                regex_enabled,
                enabled,
                now,
                now,
            ],
        );
        match result {
            Ok(_) => Ok(Some(self.connection.last_insert_rowid())),
            Err(error) if is_constraint_error(&error) => Ok(None),
            Err(error) => Err(DbError::from(error)),
        }
    }

    pub fn update_rule(&mut self, rule_id: i64, payload: &Value, user_id: i64) -> DbResult<bool> {
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("category rule update payload must be an object".to_string())
        })?;
        if object.is_empty() {
            return Ok(false);
        }

        let mut assignments: Vec<&str> = Vec::new();
        let mut values: Vec<SqlValue> = Vec::new();
        for (key, value) in object {
            match key.as_str() {
                "id" | "user_id" | "created_at" | "updated_at" | "applied_count"
                | "last_applied_at" | "main_category" | "sub_category" | "category_type" => {}
                "category_id" => {
                    let category_id = required_i64(value, key)?;
                    if !self.category_belongs_to_user(category_id, user_id)? {
                        return Ok(false);
                    }
                    assignments.push("category_id = ?");
                    values.push(SqlValue::Integer(category_id));
                }
                "name" => {
                    assignments.push("name = ?");
                    values.push(SqlValue::Text(sql_text_value(value)?));
                }
                "priority" => {
                    assignments.push("priority = ?");
                    values.push(SqlValue::Integer(required_i64(value, key)?));
                }
                "rule_expression" => {
                    assignments.push("rule_expression = ?");
                    values.push(SqlValue::Text(required_rule_expression(Some(value))?));
                }
                "regex_enabled" => {
                    assignments.push("regex_enabled = ?");
                    values.push(SqlValue::Integer(bool_int_value(value)?));
                }
                "enabled" => {
                    assignments.push("enabled = ?");
                    values.push(SqlValue::Integer(bool_int_value(value)?));
                }
                _ => {}
            }
        }
        if assignments.is_empty() {
            return Ok(false);
        }

        assignments.push("updated_at = ?");
        values.push(SqlValue::Text(utc_now_iso()));
        values.push(SqlValue::Integer(rule_id));
        values.push(SqlValue::Integer(user_id));

        let sql = format!(
            "UPDATE category_rules SET {} WHERE id = ? AND user_id = ?",
            assignments.join(", ")
        );
        let result = self
            .connection
            .execute(&sql, rusqlite::params_from_iter(values));
        match result {
            Ok(changed) => Ok(changed > 0),
            Err(error) if is_constraint_error(&error) => Ok(false),
            Err(error) => Err(DbError::from(error)),
        }
    }

    pub fn delete_rule(&mut self, rule_id: i64, user_id: i64) -> DbResult<bool> {
        let changed = self.connection.execute(
            "DELETE FROM category_rules WHERE id = ? AND user_id = ?",
            params![rule_id, user_id],
        )?;
        Ok(changed > 0)
    }

    pub fn reorder_rules(&mut self, rule_ids: &[i64], user_id: i64) -> DbResult<bool> {
        let now = utc_now_iso();
        run_transaction(self.connection, |transaction| {
            for (priority, rule_id) in rule_ids.iter().enumerate() {
                transaction.execute(
                    "UPDATE category_rules SET priority = ?, updated_at = ?
                     WHERE id = ? AND user_id = ?",
                    params![
                        i64::try_from(priority + 1).unwrap_or(i64::MAX),
                        now,
                        rule_id,
                        user_id
                    ],
                )?;
            }
            Ok(())
        })?;
        Ok(true)
    }

    fn category_belongs_to_user(&self, category_id: i64, user_id: i64) -> DbResult<bool> {
        self.connection
            .query_row(
                "SELECT 1 FROM categories WHERE id = ? AND user_id = ? LIMIT 1",
                params![category_id, user_id],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(DbError::from)
    }
}

fn category_rule_from_row(row: &Row<'_>) -> rusqlite::Result<CategoryRuleRecord> {
    let mut record = Map::new();
    for column in [
        "id",
        "user_id",
        "category_id",
        "name",
        "priority",
        "rule_expression",
        "regex_enabled",
        "enabled",
        "applied_count",
        "last_applied_at",
        "created_at",
        "updated_at",
        "main_category",
        "sub_category",
        "category_type",
    ] {
        record.insert(
            column.to_string(),
            sqlite_value_to_json(row.get_ref(column)?),
        );
    }
    Ok(record)
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

fn utc_now_iso() -> String {
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

fn is_constraint_error(error: &RusqliteError) -> bool {
    matches!(
        error,
        RusqliteError::SqliteFailure(failure, _) if failure.code == ErrorCode::ConstraintViolation
    )
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn required_i64(value: &Value, field: &str) -> DbResult<i64> {
    value_as_i64(value).ok_or_else(|| {
        DbError::InvalidOperation(format!("{field} must be an integer-compatible value"))
    })
}

fn required_rule_expression(value: Option<&Value>) -> DbResult<String> {
    match value {
        Some(Value::Null) | None => Err(DbError::InvalidOperation(
            "rule_expression is required".to_string(),
        )),
        Some(value) => sql_text_value(value),
    }
}

fn bool_int_value(value: &Value) -> DbResult<i64> {
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

fn sql_text_value(value: &Value) -> DbResult<String> {
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

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::CategoryRulesRepository;

    #[test]
    fn list_rules_filters_user_category_and_enabled_state() {
        let mut connection = fixture_connection();
        let mut repository = CategoryRulesRepository::new(&mut connection);

        let enabled = repository.list_rules(42, None, true).expect("enabled");
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0]["id"], 60);
        assert_eq!(enabled[0]["main_category"], "餐饮");
        assert_eq!(enabled[0]["sub_category"], "午餐");
        assert_eq!(enabled[0]["category_type"], 3);
        assert!(!enabled.iter().any(|rule| rule["user_id"] == 77));

        let all_for_category = repository
            .list_rules(42, Some(31), false)
            .expect("all for category");
        assert_eq!(all_for_category.len(), 2);
        assert_eq!(all_for_category[0]["priority"], 10);
        assert_eq!(all_for_category[1]["enabled"], 0);

        let missing = repository
            .list_rules(42, Some(999), false)
            .expect("missing category");
        assert!(missing.is_empty());

        let rule = repository
            .get_rule(60, 42)
            .expect("get rule")
            .expect("rule");
        assert_eq!(rule["name"], "午餐规则");
        assert_eq!(rule["rule_expression"], "OR={午餐,饭}");
        assert!(repository.get_rule(96, 42).expect("other user").is_none());

        let created_id = repository
            .create_rule(
                &serde_json::json!({
                    "category_id": 31,
                    "name": "晚餐规则",
                    "priority": 15,
                    "rule_expression": "OR={晚餐}",
                    "regex_enabled": false,
                    "enabled": true
                }),
                42,
            )
            .expect("create")
            .expect("created id");
        let created = repository
            .get_rule(created_id, 42)
            .expect("get created")
            .expect("created rule");
        assert_eq!(created["name"], "晚餐规则");
        assert_eq!(created["enabled"], 1);

        assert!(repository
            .update_rule(
                created_id,
                &serde_json::json!({
                    "name": "晚餐规则更新",
                    "priority": 5,
                    "enabled": false
                }),
                42,
            )
            .expect("update"));
        let updated = repository
            .get_rule(created_id, 42)
            .expect("get updated")
            .expect("updated rule");
        assert_eq!(updated["name"], "晚餐规则更新");
        assert_eq!(updated["priority"], 5);
        assert_eq!(updated["enabled"], 0);

        assert!(repository
            .create_rule(
                &serde_json::json!({
                    "category_id": 97,
                    "rule_expression": "OR={其他用户分类}"
                }),
                42,
            )
            .expect("cross category create")
            .is_none());
        assert!(!repository
            .update_rule(created_id, &serde_json::json!({"category_id": 97}), 42,)
            .expect("cross category update"));

        assert!(!repository
            .update_rule(96, &serde_json::json!({"name": "cross"}), 42)
            .expect("cross user update"));
        assert!(repository
            .reorder_rules(&[created_id, 60, 61, 96], 42)
            .expect("reorder"));
        assert_eq!(
            repository
                .get_rule(created_id, 42)
                .expect("get reordered")
                .expect("reordered")["priority"],
            1
        );
        assert_eq!(
            repository
                .get_rule(60, 42)
                .expect("get reordered existing")
                .expect("existing")["priority"],
            2
        );
        assert!(repository
            .delete_rule(created_id, 42)
            .expect("delete created"));
        assert!(repository
            .get_rule(created_id, 42)
            .expect("deleted")
            .is_none());
        assert!(!repository.delete_rule(96, 42).expect("cross user delete"));
        assert!(repository.get_rule(96, 77).expect("other user").is_some());
    }

    #[test]
    fn mutation_helpers_validate_payload_edges() {
        let mut connection = fixture_connection();
        let mut repository = CategoryRulesRepository::new(&mut connection);

        assert!(repository
            .create_rule(&serde_json::Value::Null, 42)
            .expect_err("create scalar")
            .to_string()
            .contains("payload must be an object"));
        assert!(repository
            .create_rule(&serde_json::json!({"rule_expression": "OR={午餐}"}), 42)
            .expect_err("missing category")
            .to_string()
            .contains("category_id is required"));
        assert!(repository
            .create_rule(&serde_json::json!({"category_id": 31}), 42)
            .expect_err("missing expression")
            .to_string()
            .contains("rule_expression is required"));
        assert!(repository
            .create_rule(
                &serde_json::json!({
                    "category_id": 31,
                    "rule_expression": null
                }),
                42,
            )
            .expect_err("null expression")
            .to_string()
            .contains("rule_expression is required"));
        assert!(repository
            .create_rule(
                &serde_json::json!({
                    "category_id": 31,
                    "rule_expression": "OR={午餐}",
                    "name": ["bad"]
                }),
                42,
            )
            .expect_err("non scalar text")
            .to_string()
            .contains("text field must be scalar"));
        assert!(repository
            .create_rule(
                &serde_json::json!({
                    "category_id": 31,
                    "rule_expression": "OR={午餐}",
                    "regex_enabled": 1.5
                }),
                42,
            )
            .expect_err("invalid numeric bool")
            .to_string()
            .contains("boolean field must be an integer"));
        assert!(repository
            .create_rule(
                &serde_json::json!({
                    "category_id": 31,
                    "rule_expression": "OR={午餐}",
                    "regex_enabled": "maybe"
                }),
                42,
            )
            .expect_err("invalid string bool")
            .to_string()
            .contains("boolean field must be truthy or falsy"));

        let created_id = repository
            .create_rule(
                &serde_json::json!({
                    "category_id": "31",
                    "name": 123,
                    "priority": "30",
                    "rule_expression": true,
                    "regex_enabled": "yes",
                    "enabled": 0
                }),
                42,
            )
            .expect("scalar coercion create")
            .expect("created");
        let created = repository
            .get_rule(created_id, 42)
            .expect("get scalar created")
            .expect("scalar created");
        assert_eq!(created["name"], "123");
        assert_eq!(created["priority"], 30);
        assert_eq!(created["rule_expression"], "true");
        assert_eq!(created["regex_enabled"], 1);
        assert_eq!(created["enabled"], 0);

        assert!(repository
            .update_rule(created_id, &serde_json::Value::Null, 42)
            .expect_err("update scalar")
            .to_string()
            .contains("update payload must be an object"));
        assert!(repository
            .update_rule(
                created_id,
                &serde_json::json!({"rule_expression": null}),
                42
            )
            .expect_err("null update expression")
            .to_string()
            .contains("rule_expression is required"));
        assert!(!repository
            .update_rule(created_id, &serde_json::json!({}), 42)
            .expect("empty update"));
        assert!(!repository
            .update_rule(created_id, &serde_json::json!({"ignored": true}), 42)
            .expect("ignored update"));
        assert!(repository
            .update_rule(
                created_id,
                &serde_json::json!({
                    "category_id": 31,
                    "regex_enabled": 0,
                    "enabled": "off"
                }),
                42,
            )
            .expect("category and bool update"));
        assert!(repository
            .update_rule(created_id, &serde_json::json!({"priority": null}), 42)
            .expect_err("invalid priority")
            .to_string()
            .contains("priority must be an integer-compatible value"));
    }

    fn fixture_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open");
        connection
            .execute_batch(
                "
                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    type INTEGER DEFAULT 1,
                    main_category TEXT NOT NULL,
                    sub_category TEXT NOT NULL
                );
                CREATE TABLE category_rules (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    category_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    priority INTEGER DEFAULT 0,
                    rule_expression TEXT NOT NULL,
                    regex_enabled INTEGER DEFAULT 0,
                    enabled INTEGER DEFAULT 1,
                    applied_count INTEGER DEFAULT 0,
                    last_applied_at TEXT,
                    created_at TEXT,
                    updated_at TEXT
                );
                INSERT INTO categories(id, user_id, type, main_category, sub_category)
                VALUES
                    (31, 42, 3, '餐饮', '午餐'),
                    (97, 77, 3, '其他用户分类', '');
                INSERT INTO category_rules(
                    id, user_id, category_id, name, priority, rule_expression,
                    regex_enabled, enabled, applied_count, last_applied_at, created_at, updated_at
                )
                VALUES
                    (60, 42, 31, '午餐规则', 10, 'OR={午餐,饭}', 0, 1, 2, '2026-01-02T00:00:00', 'now', 'now'),
                    (61, 42, 31, '禁用规则', 20, 'OR={禁用}', 0, 0, 0, NULL, 'now', 'now'),
                    (96, 77, 97, '其他用户规则', 1, 'OR={其他}', 0, 1, 1, NULL, 'now', 'now');
                ",
            )
            .expect("schema");
        connection
    }
}
