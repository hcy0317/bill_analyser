// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{TimeZone, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, Connection, Error as RusqliteError, ErrorCode, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{Map, Number, Value};

use crate::{run_transaction, DbError, DbResult};

pub type CategoryRecord = Map<String, Value>;

const CATEGORY_INSERT_SQL: &str = "INSERT INTO categories (
                type, main_category, sub_category, description, priority,
                keywords, hidden, icon, color, created_at, user_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CategoryEnsureSummary {
    pub created: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryStatistic {
    pub main_category: String,
    pub sub_category: String,
    pub count: i64,
    pub total_amount: f64,
}

pub struct CategoriesRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> CategoriesRepository<'conn> {
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    pub fn list_categories(&mut self, user_id: i64) -> DbResult<Vec<CategoryRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, user_id, type, main_category, sub_category, description,
                    priority, keywords, hidden, icon, color, created_at
             FROM categories
             WHERE user_id = ?
             ORDER BY priority ASC, main_category, sub_category",
        )?;
        let rows = statement.query_map(params![user_id], category_from_row)?;
        let categories = rows.collect::<Result<Vec<_>, _>>()?;
        if !categories.is_empty() {
            return Ok(categories);
        }

        let mut fallback_statement = self.connection.prepare(
            "SELECT DISTINCT main_category, sub_category
             FROM bills
             WHERE main_category IS NOT NULL AND user_id = ?
             ORDER BY main_category, sub_category",
        )?;
        let rows = fallback_statement.query_map(params![user_id], bill_category_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_category_by_id(
        &mut self,
        category_id: i64,
        user_id: i64,
    ) -> DbResult<Option<CategoryRecord>> {
        self.connection
            .query_row(
                "SELECT id, user_id, type, main_category, sub_category, description,
                        priority, keywords, hidden, icon, color, created_at
                 FROM categories
                 WHERE id = ? AND user_id = ?",
                params![category_id, user_id],
                category_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn get_category_by_name(
        &mut self,
        main_category: &str,
        sub_category: &str,
        user_id: i64,
    ) -> DbResult<Option<CategoryRecord>> {
        self.connection
            .query_row(
                "SELECT id, user_id, type, main_category, sub_category, description,
                        priority, keywords, hidden, icon, color, created_at
                 FROM categories
                 WHERE main_category = ? AND sub_category = ? AND user_id = ?",
                params![main_category, sub_category, user_id],
                category_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn create_category(&mut self, payload: &Value, user_id: i64) -> DbResult<Option<i64>> {
        let now = utc_now_iso();
        let values = category_insert_values(payload, user_id, now)?;

        let result = self.connection.execute(CATEGORY_INSERT_SQL, values);
        match result {
            Ok(_) => Ok(Some(self.connection.last_insert_rowid())),
            Err(error) if is_constraint_error(&error) => Ok(None),
            Err(error) => Err(DbError::from(error)),
        }
    }

    pub fn ensure_categories(
        &mut self,
        payload: &Value,
        user_id: i64,
    ) -> DbResult<CategoryEnsureSummary> {
        let categories = payload.as_array().ok_or_else(|| {
            DbError::InvalidOperation("category ensure payload must be an array".to_string())
        })?;

        run_transaction(self.connection, |transaction| {
            let mut summary = CategoryEnsureSummary {
                created: 0,
                skipped: 0,
            };

            for category in categories {
                let values = category_insert_values(category, user_id, utc_now_iso())?;
                match transaction.execute(CATEGORY_INSERT_SQL, values) {
                    Ok(_) => summary.created += 1,
                    Err(error) if is_constraint_error(&error) => summary.skipped += 1,
                    Err(error) => return Err(DbError::from(error)),
                }
            }

            Ok(summary)
        })
    }

    pub fn update_category(
        &mut self,
        category_id: i64,
        payload: &Value,
        user_id: i64,
    ) -> DbResult<bool> {
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("category update payload must be an object".to_string())
        })?;
        if object.is_empty() {
            return Ok(false);
        }

        let mut assignments: Vec<&str> = Vec::new();
        let mut values: Vec<SqlValue> = Vec::new();

        for (key, value) in object {
            match key.as_str() {
                "id" | "created_at" | "updated_at" => {}
                "type" => push_assignment(&mut assignments, &mut values, "type", value)?,
                "main_category" => {
                    assignments.push("main_category = ?");
                    values.push(sql_required_text(Some(value), "main_category")?);
                }
                "sub_category" => {
                    push_assignment(&mut assignments, &mut values, "sub_category", value)?;
                }
                "description" => {
                    push_assignment(&mut assignments, &mut values, "description", value)?;
                }
                "priority" | "displayOrder" => {
                    assignments.push("priority = ?");
                    values.push(SqlValue::Integer(int_value(value, key)?));
                }
                "keywords" => push_assignment(&mut assignments, &mut values, "keywords", value)?,
                "hidden" => {
                    assignments.push("hidden = ?");
                    values.push(SqlValue::Integer(bool_int_value(value, key)?));
                }
                "icon" => push_assignment(&mut assignments, &mut values, "icon", value)?,
                "color" => push_assignment(&mut assignments, &mut values, "color", value)?,
                _ => {}
            }
        }
        if assignments.is_empty() {
            return Ok(false);
        }

        values.push(SqlValue::Integer(category_id));
        values.push(SqlValue::Integer(user_id));
        let sql = format!(
            "UPDATE categories SET {} WHERE id = ? AND user_id = ?",
            assignments.join(", ")
        );
        let changed = self
            .connection
            .execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(changed > 0)
    }

    pub fn delete_category(&mut self, category_id: i64, user_id: i64) -> DbResult<bool> {
        let category = self.get_category_by_id(category_id, user_id)?;
        let Some(category) = category else {
            return Ok(false);
        };

        let main_category = text_field(&category, "main_category");
        let sub_category = text_field(&category, "sub_category");
        if sub_category.is_empty() {
            let changed = self.connection.execute(
                "DELETE FROM categories WHERE main_category = ? AND user_id = ?",
                params![main_category, user_id],
            )?;
            return Ok(changed > 0);
        }

        let changed = self.connection.execute(
            "DELETE FROM categories WHERE id = ? AND user_id = ?",
            params![category_id, user_id],
        )?;
        Ok(changed > 0)
    }

    pub fn delete_categories_by_main_category(
        &mut self,
        main_category: &str,
        user_id: i64,
    ) -> DbResult<bool> {
        let changed = self.connection.execute(
            "DELETE FROM categories WHERE main_category = ? AND user_id = ?",
            params![main_category, user_id],
        )?;
        Ok(changed > 0)
    }

    pub fn update_main_category_name(
        &mut self,
        old_name: &str,
        new_name: &str,
        user_id: i64,
    ) -> DbResult<bool> {
        let result = run_transaction(self.connection, |transaction| {
            let changed = transaction.execute(
                "UPDATE categories SET main_category = ? WHERE main_category = ? AND user_id = ?",
                params![new_name, old_name, user_id],
            )?;
            Ok(changed > 0)
        });
        match result {
            Ok(changed) => Ok(changed),
            Err(DbError::Sqlite(error)) if is_constraint_error(&error) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn category_statistics(
        &mut self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        user_id: i64,
    ) -> DbResult<Vec<CategoryStatistic>> {
        let mut sql = String::from(
            "SELECT
                main_category,
                COALESCE(sub_category, '') AS sub_category,
                COUNT(*) AS count,
                COALESCE(SUM(amount), 0) AS total_amount
             FROM bills
             WHERE main_category IS NOT NULL AND user_id = ?",
        );
        let mut values = vec![SqlValue::Integer(user_id)];

        if let Some(value) = non_empty_text(start_date) {
            sql.push_str(" AND date >= ?");
            values.push(SqlValue::Text(value.to_string()));
        }
        if let Some(value) = non_empty_text(end_date) {
            sql.push_str(" AND date <= ?");
            values.push(SqlValue::Text(value.to_string()));
        }

        sql.push_str(" GROUP BY main_category, sub_category, type ORDER BY total_amount DESC");

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(
            rusqlite::params_from_iter(values),
            category_statistic_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }
}

pub fn open_categories_connection(db_path: &str) -> DbResult<Connection> {
    if db_path == ":memory:" || db_path == "file::memory:?cache=shared" {
        return Err(DbError::InvalidOperation(
            "in-memory sqlite paths must use the Python category implementation".to_string(),
        ));
    }

    let connection = Connection::open(db_path)?;
    connection.busy_timeout(Duration::from_secs(10))?;
    // Match the long-lived Python aiosqlite connection: this slice must not
    // make legacy userless category fixtures stricter than the existing path.
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    connection.execute_batch("PRAGMA busy_timeout=10000;")?;
    Ok(connection)
}

fn category_from_row(row: &Row<'_>) -> rusqlite::Result<CategoryRecord> {
    let mut category = Map::new();
    for column in [
        "id",
        "user_id",
        "type",
        "main_category",
        "sub_category",
        "description",
        "priority",
        "keywords",
        "hidden",
        "icon",
        "color",
        "created_at",
    ] {
        category.insert(
            column.to_string(),
            sqlite_value_to_json(row.get_ref(column)?),
        );
    }
    Ok(category)
}

fn bill_category_from_row(row: &Row<'_>) -> rusqlite::Result<CategoryRecord> {
    let mut category = Map::new();
    category.insert("id".to_string(), Value::Number(Number::from(0)));
    category.insert(
        "main_category".to_string(),
        sqlite_value_to_json(row.get_ref("main_category")?),
    );
    category.insert(
        "sub_category".to_string(),
        sqlite_value_to_json(row.get_ref("sub_category")?),
    );
    category.insert("description".to_string(), Value::String(String::new()));
    category.insert("priority".to_string(), Value::Number(Number::from(0)));
    category.insert("keywords".to_string(), Value::String(String::new()));
    Ok(category)
}

fn category_statistic_from_row(row: &Row<'_>) -> rusqlite::Result<CategoryStatistic> {
    Ok(CategoryStatistic {
        main_category: row.get("main_category")?,
        sub_category: row.get("sub_category")?,
        count: row.get("count")?,
        total_amount: row.get("total_amount")?,
    })
}

fn category_insert_values(
    payload: &Value,
    user_id: i64,
    created_at: String,
) -> DbResult<[SqlValue; 11]> {
    Ok([
        sql_value_or_default(payload.get("type"), SqlValue::Integer(1))?,
        sql_required_text(payload.get("main_category"), "main_category")?,
        sql_value_or_default(payload.get("sub_category"), SqlValue::Text(String::new()))?,
        sql_value_or_default(payload.get("description"), SqlValue::Text(String::new()))?,
        sql_value_or_default(payload.get("priority"), SqlValue::Integer(0))?,
        sql_value_or_default(payload.get("keywords"), SqlValue::Text(String::new()))?,
        SqlValue::Integer(optional_bool_int(payload, "hidden")?.unwrap_or(0)),
        sql_value_or_default(payload.get("icon"), SqlValue::Text(String::new()))?,
        sql_value_or_default(payload.get("color"), SqlValue::Text(String::new()))?,
        SqlValue::Text(created_at),
        SqlValue::Integer(user_id),
    ])
}

fn push_assignment(
    assignments: &mut Vec<&str>,
    values: &mut Vec<SqlValue>,
    column: &'static str,
    value: &Value,
) -> DbResult<()> {
    assignments.push(match column {
        "main_category" => "main_category = ?",
        "sub_category" => "sub_category = ?",
        "description" => "description = ?",
        "keywords" => "keywords = ?",
        "icon" => "icon = ?",
        "color" => "color = ?",
        "type" => "type = ?",
        _ => {
            return Err(DbError::InvalidOperation(format!(
                "unsupported category update field: {column}"
            )));
        }
    });
    values.push(json_value_to_sql(value)?);
    Ok(())
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

fn sql_value_or_default(value: Option<&Value>, default: SqlValue) -> DbResult<SqlValue> {
    match value {
        Some(value) => json_value_to_sql(value),
        None => Ok(default),
    }
}

fn sql_required_text(value: Option<&Value>, key: &str) -> DbResult<SqlValue> {
    let Some(value) = value else {
        return Err(DbError::InvalidOperation(format!("{key} is required")));
    };
    match value {
        Value::String(text) if text.trim().is_empty() => Err(DbError::InvalidOperation(format!(
            "{key} must not be empty"
        ))),
        Value::String(text) => Ok(SqlValue::Text(text.clone())),
        _ => Err(DbError::InvalidOperation(format!("{key} must be text"))),
    }
}

fn json_value_to_sql(value: &Value) -> DbResult<SqlValue> {
    Ok(match value {
        Value::Null => SqlValue::Null,
        Value::Bool(flag) => SqlValue::Integer(i64::from(*flag)),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                SqlValue::Integer(integer)
            } else if let Some(unsigned) = number.as_u64() {
                SqlValue::Integer(i64::try_from(unsigned).map_err(|_| {
                    DbError::InvalidOperation("integer value is too large".to_string())
                })?)
            } else if let Some(float) = number.as_f64() {
                SqlValue::Real(float)
            } else {
                return Err(DbError::InvalidOperation(
                    "invalid numeric value".to_string(),
                ));
            }
        }
        Value::String(text) => SqlValue::Text(text.clone()),
        Value::Array(_) | Value::Object(_) => SqlValue::Text(value.to_string()),
    })
}

fn optional_bool_int(payload: &Value, key: &str) -> DbResult<Option<i64>> {
    payload
        .get(key)
        .map(|value| bool_int_value(value, key))
        .transpose()
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
    if let Some(number) = value.as_i64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        return text
            .trim()
            .parse::<i64>()
            .map_err(|_| DbError::InvalidOperation(format!("{key} must be integer")));
    }
    Err(DbError::InvalidOperation(format!("{key} must be integer")))
}

fn text_field(record: &CategoryRecord, key: &str) -> String {
    record
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn non_empty_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn is_constraint_error(error: &RusqliteError) -> bool {
    matches!(
        error,
        RusqliteError::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE categories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    type INTEGER DEFAULT 1,
                    main_category TEXT NOT NULL,
                    sub_category TEXT NOT NULL,
                    description TEXT,
                    priority INTEGER DEFAULT 0,
                    keywords TEXT,
                    hidden BOOLEAN DEFAULT 0,
                    icon TEXT,
                    color TEXT,
                    created_at TEXT NOT NULL,
                    UNIQUE(user_id, main_category, sub_category)
                );
                CREATE TABLE bills (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    main_category TEXT,
                    sub_category TEXT
                );",
            )
            .unwrap();
        connection
    }

    #[test]
    fn taxonomy_categories_crud_tree_fields_and_user_scope_round_trip() {
        let mut connection = create_connection();
        let mut repository = CategoriesRepository::new(&mut connection);

        let parent_id = repository
            .create_category(
                &serde_json::json!({
                    "type": 3,
                    "main_category": "餐饮",
                    "sub_category": "",
                    "description": "父级",
                    "priority": 20,
                    "keywords": "",
                    "hidden": false,
                    "icon": "mdi-food",
                    "color": "#FFAA00"
                }),
                7,
            )
            .unwrap()
            .unwrap();
        let child_id = repository
            .create_category(
                &serde_json::json!({
                    "type": 3,
                    "main_category": "餐饮",
                    "sub_category": "午餐",
                    "description": "子级",
                    "priority": 10,
                    "keywords": "快餐",
                    "hidden": true
                }),
                7,
            )
            .unwrap()
            .unwrap();
        let other_user_id = repository
            .create_category(
                &serde_json::json!({
                    "type": 3,
                    "main_category": "餐饮",
                    "sub_category": "",
                    "priority": 1
                }),
                8,
            )
            .unwrap()
            .unwrap();

        assert!(repository
            .update_category(
                child_id,
                &serde_json::json!({"description": "已更新", "displayOrder": 5}),
                7,
            )
            .unwrap());
        assert!(!repository
            .update_category(
                other_user_id,
                &serde_json::json!({"description": "越权"}),
                7
            )
            .unwrap());

        let categories = repository.list_categories(7).unwrap();
        assert_eq!(
            categories
                .iter()
                .map(|category| text_field(category, "sub_category"))
                .collect::<Vec<_>>(),
            vec!["午餐".to_string(), String::new()]
        );
        assert_eq!(
            repository
                .get_category_by_name("餐饮", "午餐", 7)
                .unwrap()
                .unwrap()
                .get("description"),
            Some(&Value::String("已更新".to_string()))
        );
        assert!(repository
            .get_category_by_id(parent_id, 8)
            .unwrap()
            .is_none());
    }

    #[test]
    fn taxonomy_categories_update_rejects_empty_main_category() {
        let mut connection = create_connection();
        let mut repository = CategoriesRepository::new(&mut connection);

        let category_id = repository
            .create_category(
                &serde_json::json!({"main_category": "有效主类", "sub_category": ""}),
                7,
            )
            .unwrap()
            .unwrap();
        let err = repository
            .update_category(category_id, &serde_json::json!({"main_category": ""}), 7)
            .unwrap_err();
        assert!(format!("{err}").contains("main_category must not be empty"));
        let err = repository
            .update_category(category_id, &serde_json::json!({"main_category": "   "}), 7)
            .unwrap_err();
        assert!(format!("{err}").contains("main_category must not be empty"));

        assert_eq!(
            text_field(
                &repository
                    .get_category_by_id(category_id, 7)
                    .unwrap()
                    .unwrap(),
                "main_category",
            ),
            "有效主类"
        );
    }

    #[test]
    fn taxonomy_categories_delete_parent_cascades_by_main_category() {
        let mut connection = create_connection();
        let mut repository = CategoriesRepository::new(&mut connection);

        let parent_id = repository
            .create_category(
                &serde_json::json!({"main_category": "出行", "sub_category": ""}),
                7,
            )
            .unwrap()
            .unwrap();
        let child_id = repository
            .create_category(
                &serde_json::json!({"main_category": "出行", "sub_category": "地铁"}),
                7,
            )
            .unwrap()
            .unwrap();
        let other_child_id = repository
            .create_category(
                &serde_json::json!({"main_category": "出行", "sub_category": "公交"}),
                8,
            )
            .unwrap()
            .unwrap();

        assert!(repository.delete_category(parent_id, 7).unwrap());
        assert!(repository
            .get_category_by_id(parent_id, 7)
            .unwrap()
            .is_none());
        assert!(repository
            .get_category_by_id(child_id, 7)
            .unwrap()
            .is_none());
        assert!(repository
            .get_category_by_id(other_child_id, 8)
            .unwrap()
            .is_some());
        assert!(!repository
            .delete_categories_by_main_category("出行", 7)
            .unwrap());
    }

    #[test]
    fn taxonomy_categories_fallback_reads_bills_when_category_table_is_empty() {
        let mut connection = create_connection();
        connection
            .execute(
                "INSERT INTO bills (user_id, main_category, sub_category) VALUES (?, ?, ?)",
                params![7, "账单分类", "早餐"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO bills (user_id, main_category, sub_category) VALUES (?, ?, ?)",
                params![8, "其他用户", "通勤"],
            )
            .unwrap();

        let mut repository = CategoriesRepository::new(&mut connection);
        let categories = repository.list_categories(7).unwrap();
        assert_eq!(categories.len(), 1);
        assert_eq!(
            categories[0].get("main_category"),
            Some(&Value::String("账单分类".to_string()))
        );
        assert_eq!(categories[0].len(), 6);
    }

    #[test]
    fn taxonomy_categories_duplicate_create_and_rename_conflict_match_python_contract() {
        let mut connection = create_connection();
        let mut repository = CategoriesRepository::new(&mut connection);

        assert!(repository
            .create_category(
                &serde_json::json!({"main_category": "旧主类", "sub_category": ""}),
                7,
            )
            .unwrap()
            .is_some());
        assert!(repository
            .create_category(
                &serde_json::json!({"main_category": "旧主类", "sub_category": "重复子类"}),
                7,
            )
            .unwrap()
            .is_some());
        assert!(repository
            .create_category(
                &serde_json::json!({"main_category": "新主类", "sub_category": ""}),
                7,
            )
            .unwrap()
            .is_some());
        assert!(repository
            .create_category(
                &serde_json::json!({"main_category": "新主类", "sub_category": "重复子类"}),
                7,
            )
            .unwrap()
            .is_some());
        assert!(repository
            .create_category(
                &serde_json::json!({"main_category": "旧主类", "sub_category": ""}),
                7,
            )
            .unwrap()
            .is_none());

        assert!(!repository
            .update_main_category_name("旧主类", "新主类", 7)
            .unwrap());
        assert_eq!(
            text_field(
                &repository
                    .get_category_by_name("旧主类", "重复子类", 7)
                    .unwrap()
                    .unwrap(),
                "main_category",
            ),
            "旧主类"
        );
    }

    #[test]
    fn taxonomy_categories_bulk_ensure_creates_missing_and_skips_existing() {
        let mut connection = create_connection();
        let mut repository = CategoriesRepository::new(&mut connection);

        let payload = serde_json::json!([
            {"main_category": "默认分类", "sub_category": "", "priority": 1},
            {"main_category": "默认分类", "sub_category": "早餐", "priority": 2}
        ]);
        assert_eq!(
            repository.ensure_categories(&payload, 7).unwrap(),
            CategoryEnsureSummary {
                created: 2,
                skipped: 0,
            }
        );
        assert_eq!(
            repository.ensure_categories(&payload, 7).unwrap(),
            CategoryEnsureSummary {
                created: 0,
                skipped: 2,
            }
        );

        let categories = repository.list_categories(7).unwrap();
        assert_eq!(categories.len(), 2);
        assert_eq!(
            categories
                .iter()
                .map(|category| text_field(category, "sub_category"))
                .collect::<Vec<_>>(),
            vec![String::new(), "早餐".to_string()]
        );
    }
}
