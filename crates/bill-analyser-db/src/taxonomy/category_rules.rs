use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{Map, Number, Value};

use crate::{DbError, DbResult};

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
             WHERE cr.user_id = ?",
        );
        let mut params = vec![SqlValue::Integer(user_id)];

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
                 WHERE cr.id = ? AND cr.user_id = ?",
                params![rule_id, user_id],
                category_rule_from_row,
            )
            .optional()
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
