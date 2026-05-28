// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bill_analyser_core::{
    build_user_investment_keyword_settings, category_rules::escape_rule_expression_term,
};
use chrono::{TimeZone, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{
    params, params_from_iter, Connection, Error as RusqliteError, ErrorCode, OptionalExtension, Row,
};
use serde_json::{Map, Number, Value};

use crate::{
    auth_registration::{ensure_default_category_seed, RegisterDefaultSeedSummary},
    run_transaction, DbError, DbResult,
};

pub type CategoryRuleRecord = Map<String, Value>;

const LEGACY_RULE_OPERATORS: &[(&str, &str, Option<char>)] = &[
    ("OR:", "OR", Some('|')),
    ("AND:", "AND", Some('|')),
    ("NOT:", "NOT", Some('|')),
    ("REGEX:", "REGEX", None),
];
const LEGACY_RULE_PREFIXES: &[&str] = &["OR:", "AND:", "NOT:", "REGEX:"];
const MIGRATED_INVESTMENT_RULE_NAME: &str = "migrated:investment-recognition";
const INVESTMENT_CATEGORY_TYPE: i64 = 5;
const INVESTMENT_CATEGORY_PREFERENCE_TOKENS: &[&str] = &[
    "基金",
    "fund",
    "投资理财",
    "投资本金",
    "investment principal",
    "investment",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRuleMigrationSummary {
    pub migrated: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyKeywordCategory {
    id: i64,
    main_category: String,
    sub_category: String,
    priority: i64,
    keywords: String,
}

pub struct CategoryRulesRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> CategoryRulesRepository<'conn> {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn list_rules(
        &mut self,
        user_id: i64,
        category_id: Option<i64>,
        enabled_only: bool,
    ) -> DbResult<Vec<CategoryRuleRecord>> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "list_rules",
            "business operation entered"
        );
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

    #[tracing::instrument(level = "debug", skip_all)]
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

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn create_rule(&mut self, payload: &Value, user_id: i64) -> DbResult<Option<i64>> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "create_rule",
            "business operation entered"
        );
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

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn update_rule(&mut self, rule_id: i64, payload: &Value, user_id: i64) -> DbResult<bool> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "update_rule",
            "business operation entered"
        );
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

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn delete_rule(&mut self, rule_id: i64, user_id: i64) -> DbResult<bool> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "delete_rule",
            "business operation entered"
        );
        let changed = self.connection.execute(
            "DELETE FROM category_rules WHERE id = ? AND user_id = ?",
            params![rule_id, user_id],
        )?;
        Ok(changed > 0)
    }

    #[tracing::instrument(level = "debug", skip_all)]
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

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn ensure_default_seed(&mut self, user_id: i64) -> DbResult<RegisterDefaultSeedSummary> {
        #[cfg(not(coverage))]
        tracing::info!(
            domain = "taxonomy",
            operation = "ensure_default_seed",
            "business operation entered"
        );
        let now = utc_now_iso();
        ensure_default_category_seed(self.connection, user_id, &now)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn migrate_keywords_to_rules(
        &mut self,
        user_id: i64,
    ) -> DbResult<CategoryRuleMigrationSummary> {
        let categories = self.legacy_keyword_categories(user_id)?;
        let now = utc_now_iso();
        let mut summary = CategoryRuleMigrationSummary {
            migrated: 0,
            skipped: 0,
        };

        for category in categories {
            let rule_expression = convert_old_keyword_syntax(&category.keywords);
            if self.category_rule_expression_exists(user_id, category.id, &rule_expression)? {
                summary.skipped += 1;
                continue;
            }

            let result = self.connection.execute(
                "INSERT INTO category_rules (
                    user_id, category_id, name, priority, rule_expression,
                    regex_enabled, enabled, created_at, updated_at
                 )
                 VALUES (?, ?, ?, ?, ?, 0, 1, ?, ?)",
                params![
                    user_id,
                    category.id,
                    format!(
                        "migrated:{}/{}",
                        category.main_category, category.sub_category
                    ),
                    category.priority,
                    rule_expression,
                    now,
                    now,
                ],
            );
            match result {
                Ok(_) => summary.migrated += 1,
                Err(error) if is_constraint_error(&error) => summary.skipped += 1,
                Err(error) => return Err(DbError::from(error)),
            }
        }

        let investment_summary = self.migrate_investment_settings_to_rules(user_id, &now)?;
        summary.migrated += investment_summary.migrated;
        summary.skipped += investment_summary.skipped;
        Ok(summary)
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

    fn legacy_keyword_categories(&self, user_id: i64) -> DbResult<Vec<LegacyKeywordCategory>> {
        let mut statement = self.connection.prepare(
            "SELECT id, main_category, sub_category, priority, keywords
             FROM categories
             WHERE user_id = ? AND keywords IS NOT NULL AND keywords != ''
             ORDER BY priority ASC, id ASC",
        )?;
        let rows = statement.query_map(params![user_id], |row| {
            Ok(LegacyKeywordCategory {
                id: row.get("id")?,
                main_category: row
                    .get::<_, Option<String>>("main_category")?
                    .unwrap_or_default(),
                sub_category: row
                    .get::<_, Option<String>>("sub_category")?
                    .unwrap_or_default(),
                priority: row.get::<_, Option<i64>>("priority")?.unwrap_or(0),
                keywords: row
                    .get::<_, Option<String>>("keywords")?
                    .unwrap_or_default(),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    fn category_rule_expression_exists(
        &self,
        user_id: i64,
        category_id: i64,
        rule_expression: &str,
    ) -> DbResult<bool> {
        self.connection
            .query_row(
                "SELECT 1 FROM category_rules
                 WHERE user_id = ? AND category_id = ?
                   AND rule_expression = ? AND regex_enabled = 0
                 LIMIT 1",
                params![user_id, category_id, rule_expression],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(DbError::from)
    }

    fn migrate_investment_settings_to_rules(
        &mut self,
        user_id: i64,
        now: &str,
    ) -> DbResult<CategoryRuleMigrationSummary> {
        let Some(user_keywords) = self.user_investment_keyword_source(user_id)? else {
            return Ok(CategoryRuleMigrationSummary {
                migrated: 0,
                skipped: 0,
            });
        };
        let Some(target_category) = self.select_investment_rule_category(user_id)? else {
            return Ok(CategoryRuleMigrationSummary {
                migrated: 0,
                skipped: 0,
            });
        };
        let rule_expression = investment_settings_to_rule_expression(
            &build_user_investment_keyword_settings(Some(&user_keywords)),
        );
        if rule_expression.is_empty() {
            return Ok(CategoryRuleMigrationSummary {
                migrated: 0,
                skipped: 1,
            });
        }
        if self.category_rule_expression_exists(user_id, target_category.id, &rule_expression)? {
            return Ok(CategoryRuleMigrationSummary {
                migrated: 0,
                skipped: 1,
            });
        }

        let result = self.connection.execute(
            "INSERT INTO category_rules (
                user_id, category_id, name, priority, rule_expression,
                regex_enabled, enabled, created_at, updated_at
             )
             VALUES (?, ?, ?, ?, ?, 0, 1, ?, ?)",
            params![
                user_id,
                target_category.id,
                MIGRATED_INVESTMENT_RULE_NAME,
                target_category.priority,
                rule_expression,
                now,
                now,
            ],
        );
        match result {
            Ok(_) => Ok(CategoryRuleMigrationSummary {
                migrated: 1,
                skipped: 0,
            }),
            Err(error) if is_constraint_error(&error) => Ok(CategoryRuleMigrationSummary {
                migrated: 0,
                skipped: 1,
            }),
            Err(error) => Err(DbError::from(error)),
        }
    }

    fn user_investment_keyword_source(&self, user_id: i64) -> DbResult<Option<Map<String, Value>>> {
        self.connection
            .query_row(
                "SELECT investment_platform_keywords, investment_product_keywords,
                        investment_exclude_keywords
                 FROM users
                 WHERE id = ? LIMIT 1",
                params![user_id],
                |row| {
                    let mut source = Map::new();
                    for column in [
                        "investment_platform_keywords",
                        "investment_product_keywords",
                        "investment_exclude_keywords",
                    ] {
                        let value = row.get::<_, Option<String>>(column)?;
                        source.insert(column.to_string(), value.map_or(Value::Null, Value::String));
                    }
                    Ok(source)
                },
            )
            .optional()
            .map_err(DbError::from)
    }

    fn select_investment_rule_category(
        &self,
        user_id: i64,
    ) -> DbResult<Option<LegacyKeywordCategory>> {
        let mut statement = self.connection.prepare(
            "SELECT id, main_category, sub_category, priority, COALESCE(keywords, '') AS keywords
             FROM categories
             WHERE user_id = ? AND type = ?
             ORDER BY priority ASC, id ASC",
        )?;
        let rows = statement.query_map(params![user_id, INVESTMENT_CATEGORY_TYPE], |row| {
            Ok(LegacyKeywordCategory {
                id: row.get("id")?,
                main_category: row
                    .get::<_, Option<String>>("main_category")?
                    .unwrap_or_default(),
                sub_category: row
                    .get::<_, Option<String>>("sub_category")?
                    .unwrap_or_default(),
                priority: row.get::<_, Option<i64>>("priority")?.unwrap_or(0),
                keywords: row
                    .get::<_, Option<String>>("keywords")?
                    .unwrap_or_default(),
            })
        })?;
        let categories = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(categories.into_iter().min_by_key(investment_category_score))
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

fn split_legacy_delimited_text(text: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escape_pending = false;
    for value in text.chars() {
        if escape_pending {
            if value == separator {
                current.push(separator);
            } else {
                current.push('\\');
                current.push(value);
            }
            escape_pending = false;
            continue;
        }
        if value == '\\' {
            escape_pending = true;
            continue;
        }
        if value == separator {
            parts.push(current);
            current = String::new();
            continue;
        }
        current.push(value);
    }
    if escape_pending {
        current.push('\\');
    }
    parts.push(current);
    parts
}

fn has_legacy_rule_prefix(keyword_text: &str) -> bool {
    split_legacy_delimited_text(keyword_text, '&')
        .into_iter()
        .any(|part| {
            let normalized = part.trim().to_ascii_uppercase();
            LEGACY_RULE_PREFIXES
                .iter()
                .any(|prefix| normalized.starts_with(prefix))
        })
}

fn format_rule_clause(operator: &str, keywords: impl IntoIterator<Item = String>) -> String {
    let escaped_keywords = keywords
        .into_iter()
        .map(|keyword| keyword.trim().to_string())
        .filter(|keyword| !keyword.is_empty())
        .map(|keyword| escape_rule_expression_term(&keyword))
        .collect::<Vec<_>>();
    if escaped_keywords.is_empty() {
        String::new()
    } else {
        format!("{operator}={{{}}}", escaped_keywords.join(","))
    }
}

pub(crate) fn convert_old_keyword_syntax(old_keywords: &str) -> String {
    let old_keywords = old_keywords.trim();
    if old_keywords.is_empty() {
        return String::new();
    }
    if !has_legacy_rule_prefix(old_keywords) {
        return format_rule_clause("OR", [old_keywords.to_string()]);
    }

    let mut blocks = Vec::new();
    for part in split_legacy_delimited_text(old_keywords, '&') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let normalized = part.to_ascii_uppercase();
        let mut matched_operator = false;
        for (prefix, operator, separator) in LEGACY_RULE_OPERATORS {
            if !normalized.starts_with(prefix) {
                continue;
            }
            let payload = &part[prefix.len()..];
            let keywords = separator.map_or_else(
                || vec![payload.trim().to_string()],
                |separator| split_legacy_delimited_text(payload, separator),
            );
            let clause = format_rule_clause(operator, keywords);
            if !clause.is_empty() {
                blocks.push(clause);
            }
            matched_operator = true;
            break;
        }
        if !matched_operator {
            let clause = format_rule_clause("OR", [part.to_string()]);
            if !clause.is_empty() {
                blocks.push(clause);
            }
        }
    }

    if blocks.is_empty() {
        old_keywords.to_string()
    } else {
        blocks.join("+")
    }
}

fn investment_settings_to_rule_expression(settings: &Value) -> String {
    let platform_clause =
        format_rule_clause("OR", value_array_strings(settings.get("platform_keywords")));
    let product_clause =
        format_rule_clause("OR", value_array_strings(settings.get("product_keywords")));
    let exclude_clause =
        format_rule_clause("NOT", value_array_strings(settings.get("exclude_keywords")));
    let positive_clauses = [platform_clause, product_clause]
        .into_iter()
        .filter(|clause| !clause.is_empty())
        .collect::<Vec<_>>();
    if positive_clauses.is_empty() {
        return String::new();
    }
    let mut blocks = vec![positive_clauses.join("+")];
    if !exclude_clause.is_empty() {
        blocks.push(exclude_clause);
    }
    blocks.join("+")
}

fn value_array_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(json_value_to_string).collect(),
        _ => Vec::new(),
    }
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn investment_category_score(category: &LegacyKeywordCategory) -> (usize, i64) {
    let text = format!("{} {}", category.main_category, category.sub_category).to_lowercase();
    let token_score = INVESTMENT_CATEGORY_PREFERENCE_TOKENS
        .iter()
        .position(|token| text.contains(token))
        .unwrap_or(INVESTMENT_CATEGORY_PREFERENCE_TOKENS.len());
    (token_score, category.priority)
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

    #[test]
    fn default_seed_and_keyword_migration_are_user_scoped_and_idempotent() {
        let mut connection = full_seed_connection();

        {
            let mut repository = CategoryRulesRepository::new(&mut connection);
            let seeded = repository.ensure_default_seed(42).expect("seed defaults");
            assert!(seeded.categories_created > 80);
            assert!(seeded.rules_created > 30);
            assert_eq!(seeded.rules_missing_categories, 0);

            let repeated_seed = repository.ensure_default_seed(42).expect("repeat seed");
            assert_eq!(repeated_seed.categories_created, 0);
            assert_eq!(repeated_seed.rules_created, 0);
            assert_eq!(repeated_seed.rules_missing_categories, 0);
            assert!(repeated_seed.categories_skipped > 80);
            assert!(repeated_seed.rules_skipped > 30);
        }

        let delivery_id = category_id(&connection, 42, "餐饮", "外卖");
        {
            let mut repository = CategoryRulesRepository::new(&mut connection);
            let delivery_rules = repository
                .list_rules(42, Some(delivery_id), false)
                .expect("delivery rules");
            assert_eq!(delivery_rules[0]["name"], "default:餐饮/外卖");
        }

        connection
            .execute_batch(
                "
                INSERT INTO categories(
                    id, user_id, type, main_category, sub_category, description,
                    priority, keywords, hidden, icon, color, created_at
                )
                VALUES
                    (500, 42, 3, '迁移测试', '咖啡', '', 11, 'OR:星巴克|咖啡&AND:早餐&NOT:退款', 0, '', '', 'now'),
                    (501, 42, 3, '特殊字符迁移测试', '完整字面量', '', 10, '商户A,咖啡+拿铁{热}|杯', 0, '', '', 'now'),
                    (502, 42, 3, '已有迁移', '已有规则', '', 12, 'OR:不应重复迁移', 0, '', '', 'now'),
                    (503, 77, 3, '跨用户迁移测试', '隔离', '', 13, 'OR:跨用户关键词', 0, '', '', 'now'),
                    (504, 42, 5, '投资理财', '基金', '', 8, '', 0, '', '', 'now');
                INSERT INTO category_rules(
                    id, user_id, category_id, name, priority, rule_expression,
                    regex_enabled, enabled, applied_count, last_applied_at, created_at, updated_at
                )
                VALUES
                    (700, 42, 502, 'manual existing rule', 3, 'OR={手工规则}', 0, 1, 0, NULL, 'now', 'now');
                ",
            )
            .expect("legacy rows");

        let mut repository = CategoryRulesRepository::new(&mut connection);
        let migrated = repository
            .migrate_keywords_to_rules(42)
            .expect("migrate keywords");
        assert_eq!(migrated.migrated, 4);
        assert_eq!(migrated.skipped, 0);

        let coffee_rules = repository
            .list_rules(42, Some(500), false)
            .expect("coffee rules");
        assert_eq!(
            coffee_rules[0]["rule_expression"],
            "OR={星巴克,咖啡}+AND={早餐}+NOT={退款}"
        );
        let literal_rules = repository
            .list_rules(42, Some(501), false)
            .expect("literal rules");
        assert_eq!(
            literal_rules[0]["rule_expression"],
            r"OR={商户A\,咖啡\+拿铁\{热\}\|杯}"
        );
        let existing_rules = repository
            .list_rules(42, Some(502), false)
            .expect("existing rules");
        assert_eq!(existing_rules.len(), 2);
        assert!(existing_rules
            .iter()
            .any(|rule| rule["rule_expression"] == "OR={手工规则}"));
        assert!(existing_rules
            .iter()
            .any(|rule| rule["rule_expression"] == "OR={不应重复迁移}"));

        let investment_rules = repository
            .list_rules(42, Some(504), false)
            .expect("investment rules");
        assert_eq!(
            investment_rules[0]["name"],
            "migrated:investment-recognition"
        );
        assert_eq!(
            investment_rules[0]["rule_expression"],
            "OR={蚂蚁财富,天天基金}+OR={基金,ETF}+NOT={还款,账单}"
        );
        assert!(repository
            .list_rules(77, Some(503), false)
            .expect("other user rules")
            .is_empty());

        let repeated = repository
            .migrate_keywords_to_rules(42)
            .expect("repeat migrate");
        assert_eq!(repeated.migrated, 0);
        assert_eq!(repeated.skipped, 4);
    }

    #[test]
    fn legacy_keyword_conversion_covers_escape_and_scalar_edges() {
        assert_eq!(
            super::split_legacy_delimited_text("a\\|b|c\\&d|tail\\", '|'),
            vec!["a|b", "c\\&d", "tail\\"]
        );
        assert_eq!(
            super::split_legacy_delimited_text("a\\&b&c", '&'),
            vec!["a&b", "c"]
        );
        assert_eq!(super::convert_old_keyword_syntax("  "), "");
        assert_eq!(super::convert_old_keyword_syntax("OR:"), "OR:");
        assert_eq!(
            super::convert_old_keyword_syntax("OR:咖啡&&AND:早餐&NOTE"),
            "OR={咖啡}+AND={早餐}+OR={NOTE}"
        );
        assert_eq!(
            super::convert_old_keyword_syntax("OR:星巴克\\|臻选|咖啡"),
            r"OR={星巴克\|臻选,咖啡}"
        );

        assert!(super::value_array_strings(None).is_empty());
        assert_eq!(super::json_value_to_string(&serde_json::json!(123)), "123");
        assert_eq!(
            super::json_value_to_string(&serde_json::json!(true)),
            "true"
        );
        assert_eq!(super::json_value_to_string(&serde_json::Value::Null), "");
        assert_eq!(
            super::json_value_to_string(&serde_json::json!(["nested"])),
            "[\"nested\"]"
        );
        assert_eq!(
            super::json_value_to_string(&serde_json::json!({"kind": "fund"})),
            "{\"kind\":\"fund\"}"
        );

        let expression = super::investment_settings_to_rule_expression(&serde_json::json!({
            "platform_keywords": [123, true, null, ["nested"], {"kind": "fund"}],
            "product_keywords": [],
            "exclude_keywords": ["账单"]
        }));
        assert_eq!(
            expression,
            r#"OR={123,true,["nested"],\{"kind":"fund"\}}+NOT={账单}"#
        );
        assert_eq!(
            super::investment_settings_to_rule_expression(&serde_json::json!({
                "platform_keywords": [],
                "product_keywords": [],
                "exclude_keywords": ["账单"]
            })),
            ""
        );
    }

    #[test]
    fn investment_keyword_migration_skips_missing_user_target_and_empty_rules() {
        let mut connection = full_seed_connection();
        connection
            .execute_batch(
                "
                INSERT INTO users(
                    id, investment_platform_keywords,
                    investment_product_keywords, investment_exclude_keywords
                )
                VALUES
                    (88, '[\"平台\"]', '[]', '[]'),
                    (89, '[]', '[]', '[\"账单\"]');
                INSERT INTO categories(
                    id, user_id, type, main_category, sub_category, description,
                    priority, keywords, hidden, icon, color, created_at
                )
                VALUES
                    (589, 89, 5, '投资', '空规则', '', 1, '', 0, '', '', 'now');
                ",
            )
            .expect("edge fixture");

        let mut repository = CategoryRulesRepository::new(&mut connection);
        let missing_user = repository
            .migrate_keywords_to_rules(404)
            .expect("missing user");
        assert_eq!(missing_user.migrated, 0);
        assert_eq!(missing_user.skipped, 0);

        let missing_target = repository
            .migrate_keywords_to_rules(88)
            .expect("missing investment category");
        assert_eq!(missing_target.migrated, 0);
        assert_eq!(missing_target.skipped, 0);

        let empty_rule = repository
            .migrate_keywords_to_rules(89)
            .expect("empty investment rule");
        assert_eq!(empty_rule.migrated, 0);
        assert_eq!(empty_rule.skipped, 1);
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

    fn full_seed_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open");
        connection
            .execute_batch(
                "
                CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    investment_platform_keywords TEXT,
                    investment_product_keywords TEXT,
                    investment_exclude_keywords TEXT
                );
                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    type INTEGER DEFAULT 1,
                    main_category TEXT NOT NULL,
                    sub_category TEXT NOT NULL,
                    description TEXT,
                    priority INTEGER DEFAULT 0,
                    keywords TEXT,
                    hidden INTEGER DEFAULT 0,
                    icon TEXT,
                    color TEXT,
                    created_at TEXT NOT NULL,
                    UNIQUE(user_id, main_category, sub_category)
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
                INSERT INTO users(
                    id, investment_platform_keywords,
                    investment_product_keywords, investment_exclude_keywords
                )
                VALUES
                    (42, '[\"蚂蚁财富\", \"天天基金\", \"蚂蚁财富\"]', '[\"基金\", \"ETF\"]', '[\"还款\", \"账单\"]'),
                    (77, NULL, NULL, NULL);
                ",
            )
            .expect("schema");
        connection
    }

    fn category_id(connection: &Connection, user_id: i64, main: &str, sub: &str) -> i64 {
        connection
            .query_row(
                "SELECT id FROM categories WHERE user_id = ? AND main_category = ? AND sub_category = ?",
                rusqlite::params![user_id, main, sub],
                |row| row.get(0),
            )
            .expect("category exists")
    }
}
