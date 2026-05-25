// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{run_transaction, DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub display_order: i64,
    pub hidden: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDisplayOrder {
    pub tag_id: i64,
    pub display_order: i64,
}

pub struct TagsRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> TagsRepository<'conn> {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn list_tags(&mut self, user_id: i64) -> DbResult<Vec<TagRecord>> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "list_tags",
            "business operation entered"
        );
        let mut statement = self.connection.prepare(
            "SELECT id, user_id, name, color, icon, display_order, hidden, created_at, updated_at
             FROM tags
             WHERE user_id = ?
             ORDER BY display_order, created_at DESC",
        )?;
        let rows = statement.query_map(params![user_id], tag_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn get_tag(&mut self, tag_id: i64, user_id: i64) -> DbResult<Option<TagRecord>> {
        self.connection
            .query_row(
                "SELECT id, user_id, name, color, icon, display_order, hidden, created_at, updated_at
                 FROM tags
                 WHERE id = ? AND user_id = ?",
                params![tag_id, user_id],
                tag_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn create_tag(&mut self, payload: &Value, user_id: i64) -> DbResult<i64> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "create_tag",
            "business operation entered"
        );
        let name = optional_text(payload, "name")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DbError::InvalidOperation("name is required".to_string()))?;
        let color = optional_text(payload, "color").unwrap_or_else(|| "#000000".to_string());
        let icon = optional_text(payload, "icon").unwrap_or_default();
        let hidden = optional_bool_int(payload, "hidden").unwrap_or(0);
        let now = utc_now_iso();

        self.connection.execute(
            "INSERT INTO tags (name, color, icon, hidden, created_at, updated_at, user_id)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![name, color, icon, hidden, now, now, user_id],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn update_tag(&mut self, tag_id: i64, payload: &Value, user_id: i64) -> DbResult<bool> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "update_tag",
            "business operation entered"
        );
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("tag update payload must be an object".to_string())
        })?;
        if object.is_empty() {
            return Ok(false);
        }

        let mut assignments: Vec<&str> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();

        for (key, value) in object {
            match key.as_str() {
                "id" => {}
                "name" => {
                    assignments.push("name = ?");
                    values.push(sql_text_value(value, key)?);
                }
                "color" => {
                    assignments.push("color = ?");
                    values.push(sql_optional_text_value(value));
                }
                "icon" => {
                    assignments.push("icon = ?");
                    values.push(sql_optional_text_value(value));
                }
                "hidden" => {
                    assignments.push("hidden = ?");
                    values.push(rusqlite::types::Value::Integer(bool_int_value(value, key)?));
                }
                "display_order" | "displayOrder" => {
                    assignments.push("display_order = ?");
                    values.push(rusqlite::types::Value::Integer(int_value(value, key)?));
                }
                unknown => {
                    return Err(DbError::InvalidOperation(format!(
                        "unsupported tag update field: {unknown}"
                    )));
                }
            }
        }

        assignments.push("updated_at = ?");
        values.push(rusqlite::types::Value::Text(utc_now_iso()));
        values.push(rusqlite::types::Value::Integer(tag_id));
        values.push(rusqlite::types::Value::Integer(user_id));

        let sql = format!(
            "UPDATE tags SET {} WHERE id = ? AND user_id = ?",
            assignments.join(", ")
        );
        let changed = self
            .connection
            .execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(changed > 0)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn delete_tag(&mut self, tag_id: i64, user_id: i64) -> DbResult<bool> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "delete_tag",
            "business operation entered"
        );
        let changed = self.connection.execute(
            "DELETE FROM tags WHERE id = ? AND user_id = ?",
            params![tag_id, user_id],
        )?;
        Ok(changed > 0)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn update_display_orders(
        &mut self,
        orders: &[TagDisplayOrder],
        user_id: i64,
    ) -> DbResult<bool> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "update_display_orders",
            "business operation entered"
        );
        if orders.is_empty() {
            return Ok(true);
        }

        let now = utc_now_iso();
        run_transaction(self.connection, |transaction| {
            for order in orders {
                transaction.execute(
                    "UPDATE tags SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                    params![order.display_order, now, order.tag_id, user_id],
                )?;
            }
            Ok(())
        })?;
        Ok(true)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn open_tags_connection(db_path: &str) -> DbResult<Connection> {
    if db_path == ":memory:" || db_path == "file::memory:?cache=shared" {
        return Err(DbError::InvalidOperation(
            "in-memory sqlite paths must use the Python tag implementation".to_string(),
        ));
    }

    let connection = Connection::open(db_path)?;
    connection.busy_timeout(Duration::from_secs(10))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch("PRAGMA busy_timeout=10000;")?;
    Ok(connection)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_display_orders(raw_value: &Value) -> DbResult<Vec<TagDisplayOrder>> {
    let values = raw_value
        .as_array()
        .ok_or_else(|| DbError::InvalidOperation("orders must be an array".to_string()))?;
    values
        .iter()
        .map(|value| {
            let pair = value.as_array().ok_or_else(|| {
                DbError::InvalidOperation("order item must be an array".to_string())
            })?;
            if pair.len() != 2 {
                return Err(DbError::InvalidOperation(
                    "order item must contain tag id and display order".to_string(),
                ));
            }
            Ok(TagDisplayOrder {
                tag_id: int_value(&pair[0], "tag_id")?,
                display_order: int_value(&pair[1], "display_order")?,
            })
        })
        .collect()
}

fn tag_from_row(row: &Row<'_>) -> rusqlite::Result<TagRecord> {
    Ok(TagRecord {
        id: row.get("id")?,
        user_id: row.get("user_id")?,
        name: row.get("name")?,
        color: row.get("color")?,
        icon: row.get("icon")?,
        display_order: row.get("display_order")?,
        hidden: row.get("hidden")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn utc_now_iso() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    Utc.timestamp_opt(seconds, duration.subsec_nanos())
        .single()
        .unwrap_or_else(|| {
            Utc.timestamp_opt(0, 0)
                .single()
                .expect("unix epoch must be representable")
        })
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string()
}

fn optional_text(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn optional_bool_int(payload: &Value, key: &str) -> Option<i64> {
    payload
        .get(key)
        .and_then(|value| bool_int_value(value, key).ok())
}

fn sql_text_value(value: &Value, key: &str) -> DbResult<rusqlite::types::Value> {
    value
        .as_str()
        .map(|text| rusqlite::types::Value::Text(text.to_string()))
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be text")))
}

fn sql_optional_text_value(value: &Value) -> rusqlite::types::Value {
    value.as_str().map_or(rusqlite::types::Value::Null, |text| {
        rusqlite::types::Value::Text(text.to_string())
    })
}

fn bool_int_value(value: &Value, key: &str) -> DbResult<i64> {
    if let Some(flag) = value.as_bool() {
        return Ok(i64::from(flag));
    }
    let number = int_value(value, key)?;
    match number {
        0 | 1 => Ok(number),
        _ => Err(DbError::InvalidOperation(format!("{key} must be boolean"))),
    }
}

fn int_value(value: &Value, key: &str) -> DbResult<i64> {
    value
        .as_i64()
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be integer")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    name TEXT NOT NULL,
                    color TEXT,
                    icon TEXT,
                    display_order INTEGER DEFAULT 0,
                    hidden BOOLEAN DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(user_id, name)
                );",
            )
            .unwrap();
        connection
    }

    #[test]
    fn taxonomy_tags_crud_keeps_user_scope_and_sort_order() {
        let mut connection = create_connection();
        let mut repository = TagsRepository::new(&mut connection);

        let first_id = repository
            .create_tag(
                &serde_json::json!({"name": "早餐", "color": "#FF0000", "icon": "1"}),
                7,
            )
            .unwrap();
        let second_id = repository
            .create_tag(&serde_json::json!({"name": "通勤", "hidden": true}), 7)
            .unwrap();
        let other_user_id = repository
            .create_tag(&serde_json::json!({"name": "其他用户"}), 8)
            .unwrap();

        assert!(repository
            .update_tag(first_id, &serde_json::json!({"displayOrder": 2}), 7)
            .unwrap());
        assert!(repository
            .update_tag(
                second_id,
                &serde_json::json!({"id": second_id.to_string(), "display_order": 1, "icon": "2"}),
                7
            )
            .unwrap());
        assert!(!repository
            .update_tag(other_user_id, &serde_json::json!({"name": "越权"}), 7)
            .unwrap());

        let tags = repository.list_tags(7).unwrap();
        assert_eq!(
            tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
            vec!["通勤", "早餐"]
        );
        assert_eq!(tags[0].hidden, 1);
        assert_eq!(tags[0].icon.as_deref(), Some("2"));
        assert_eq!(repository.list_tags(8).unwrap()[0].name, "其他用户");
    }

    #[test]
    fn taxonomy_tags_delete_and_display_orders_are_user_scoped() {
        let mut connection = create_connection();
        let mut repository = TagsRepository::new(&mut connection);

        let first_id = repository
            .create_tag(&serde_json::json!({"name": "早餐"}), 7)
            .unwrap();
        let second_id = repository
            .create_tag(&serde_json::json!({"name": "通勤"}), 7)
            .unwrap();
        let other_user_id = repository
            .create_tag(&serde_json::json!({"name": "其他用户"}), 8)
            .unwrap();

        assert!(repository
            .update_display_orders(
                &[
                    TagDisplayOrder {
                        tag_id: first_id,
                        display_order: 20,
                    },
                    TagDisplayOrder {
                        tag_id: second_id,
                        display_order: 10,
                    },
                    TagDisplayOrder {
                        tag_id: other_user_id,
                        display_order: 1,
                    },
                ],
                7,
            )
            .unwrap());

        let tags = repository.list_tags(7).unwrap();
        assert_eq!(
            tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
            vec!["通勤", "早餐"]
        );
        assert_eq!(
            repository
                .get_tag(other_user_id, 8)
                .unwrap()
                .unwrap()
                .display_order,
            0
        );

        assert!(!repository.delete_tag(other_user_id, 7).unwrap());
        assert!(repository.delete_tag(first_id, 7).unwrap());
        assert!(repository.get_tag(first_id, 7).unwrap().is_none());
    }

    #[test]
    fn taxonomy_tags_parse_display_orders_validates_shape() {
        let orders = parse_display_orders(&serde_json::json!([[3, 2], [4, 1]])).unwrap();
        assert_eq!(orders[0].tag_id, 3);
        assert_eq!(orders[1].display_order, 1);

        assert!(parse_display_orders(&serde_json::json!({"bad": true})).is_err());
        assert!(parse_display_orders(&serde_json::json!([[1]])).is_err());
        assert!(parse_display_orders(&serde_json::json!([{"id": 1}])).is_err());
    }
}
