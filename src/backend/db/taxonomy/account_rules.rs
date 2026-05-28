// 中文导读：SQLite repository 层，负责账户识别规则 schema、CRUD、排序和 shadow 匹配读模型。
// 维护重点：SQL 和 user-scope 校验集中在这里，HTTP/import 层不得复制这些查询。
// 不变式：账户规则先作为可管理、可测试的规则资产存在；导入行为切换必须由后续切片显式完成。

use std::collections::BTreeSet;

use bill_analyser_core::account_rules::{
    normalize_account_role_scope, normalize_account_rule_field_scope,
    normalize_transaction_type_scope, AccountRuleCandidate, AccountRuleMatchContext,
};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::{Map, Value};

use crate::{run_transaction, DbError, DbResult};

mod helpers;
#[cfg(test)]
mod tests;

use helpers::{
    account_rule_candidate_from_record, account_rule_from_row, bool_int_value, field_scope_json,
    is_constraint_error, required_i64, required_rule_expression, sql_text_value, utc_now_iso,
    value_as_i64,
};

pub type AccountRuleRecord = Map<String, Value>;

pub struct AccountRulesRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> AccountRulesRepository<'conn> {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn list_rules(
        &mut self,
        user_id: i64,
        account_id: Option<i64>,
        enabled_only: bool,
        account_role_scope: Option<&str>,
        transaction_type_scope: Option<&str>,
    ) -> DbResult<Vec<AccountRuleRecord>> {
        let mut sql = String::from(
            "SELECT
                ar.id, ar.user_id, ar.account_id, ar.name, ar.priority,
                ar.rule_expression, ar.regex_enabled, ar.enabled, ar.applied_count,
                ar.last_applied_at, ar.match_count, ar.last_matched_at,
                ar.account_role_scope, ar.transaction_type_scope,
                ar.field_scope, ar.source, ar.source_key, ar.created_at, ar.updated_at,
                a.name AS account_name, a.type AS account_type, a.hidden AS account_hidden
             FROM account_rules ar
             JOIN accounts a ON ar.account_id = a.id
             WHERE ar.user_id = ? AND a.user_id = ?",
        );
        let mut params = vec![SqlValue::Integer(user_id), SqlValue::Integer(user_id)];

        if let Some(account_id) = account_id {
            sql.push_str(" AND ar.account_id = ?");
            params.push(SqlValue::Integer(account_id));
        }
        if enabled_only {
            sql.push_str(" AND ar.enabled = 1");
        }
        if let Some(scope) = account_role_scope {
            let scope =
                normalize_account_role_scope(Some(scope)).map_err(DbError::InvalidOperation)?;
            sql.push_str(" AND ar.account_role_scope = ?");
            params.push(SqlValue::Text(scope));
        }
        if let Some(scope) = transaction_type_scope {
            let scope =
                normalize_transaction_type_scope(Some(scope)).map_err(DbError::InvalidOperation)?;
            sql.push_str(" AND ar.transaction_type_scope = ?");
            params.push(SqlValue::Text(scope));
        }
        sql.push_str(" ORDER BY ar.priority ASC, ar.id ASC");

        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params), account_rule_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn get_rule(&mut self, rule_id: i64, user_id: i64) -> DbResult<Option<AccountRuleRecord>> {
        self.connection
            .query_row(
                "SELECT
                    ar.id, ar.user_id, ar.account_id, ar.name, ar.priority,
                    ar.rule_expression, ar.regex_enabled, ar.enabled, ar.applied_count,
                    ar.last_applied_at, ar.match_count, ar.last_matched_at,
                    ar.account_role_scope, ar.transaction_type_scope,
                    ar.field_scope, ar.source, ar.source_key, ar.created_at, ar.updated_at,
                    a.name AS account_name, a.type AS account_type, a.hidden AS account_hidden
                 FROM account_rules ar
                 JOIN accounts a ON ar.account_id = a.id
                 WHERE ar.id = ? AND ar.user_id = ? AND a.user_id = ?",
                params![rule_id, user_id, user_id],
                account_rule_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn create_rule(&mut self, payload: &Value, user_id: i64) -> DbResult<Option<i64>> {
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("account rule payload must be an object".to_string())
        })?;
        let account_id = object
            .get("account_id")
            .or_else(|| object.get("accountId"))
            .and_then(|value| value_as_i64(Some(value)))
            .ok_or_else(|| DbError::InvalidOperation("account_id is required".to_string()))?;
        if !self.account_belongs_to_user(account_id, user_id)? {
            return Ok(None);
        }
        let rule_expression = required_rule_expression(
            object
                .get("rule_expression")
                .or_else(|| object.get("ruleExpression")),
        )?;
        let name = object
            .get("name")
            .map(sql_text_value)
            .transpose()?
            .unwrap_or_default();
        let priority = value_as_i64(object.get("priority")).unwrap_or(100);
        let regex_enabled = object
            .get("regex_enabled")
            .or_else(|| object.get("regexEnabled"))
            .map(bool_int_value)
            .transpose()?
            .unwrap_or(0);
        let enabled = object
            .get("enabled")
            .map(bool_int_value)
            .transpose()?
            .unwrap_or(1);
        let account_role_scope = normalize_account_role_scope(
            object
                .get("account_role_scope")
                .or_else(|| object.get("accountRoleScope"))
                .and_then(Value::as_str),
        )
        .map_err(DbError::InvalidOperation)?;
        let transaction_type_scope = normalize_transaction_type_scope(
            object
                .get("transaction_type_scope")
                .or_else(|| object.get("transactionTypeScope"))
                .and_then(Value::as_str),
        )
        .map_err(DbError::InvalidOperation)?;
        let field_scope = normalize_account_rule_field_scope(
            object
                .get("field_scope")
                .or_else(|| object.get("fieldScope")),
        )
        .map_err(DbError::InvalidOperation)?;
        let source = object
            .get("source")
            .map(sql_text_value)
            .transpose()?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "manual".to_string());
        let now = utc_now_iso();

        let result = self.connection.execute(
            "INSERT INTO account_rules (
                user_id, account_id, name, priority, rule_expression,
                regex_enabled, enabled, account_role_scope, transaction_type_scope,
                field_scope, source, created_at, updated_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                user_id,
                account_id,
                name,
                priority,
                rule_expression,
                regex_enabled,
                enabled,
                account_role_scope,
                transaction_type_scope,
                field_scope_json(&field_scope)?,
                source,
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
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("account rule update payload must be an object".to_string())
        })?;
        if object.is_empty() {
            return Ok(false);
        }

        let mut assignments: Vec<&str> = Vec::new();
        let mut values: Vec<SqlValue> = Vec::new();
        for (key, value) in object {
            match key.as_str() {
                "id" | "user_id" | "userId" | "created_at" | "createdAt" | "updated_at"
                | "updatedAt" | "applied_count" | "appliedCount" | "last_applied_at"
                | "lastAppliedAt" | "match_count" | "matchCount" | "last_matched_at"
                | "lastMatchedAt" | "source_key" | "sourceKey" | "account_name" | "accountName"
                | "account_type" | "accountType" | "account_hidden" | "accountHidden" => {}
                "account_id" | "accountId" => {
                    let account_id = required_i64(value, "account_id")?;
                    if !self.account_belongs_to_user(account_id, user_id)? {
                        return Ok(false);
                    }
                    assignments.push("account_id = ?");
                    values.push(SqlValue::Integer(account_id));
                }
                "name" => {
                    assignments.push("name = ?");
                    values.push(SqlValue::Text(sql_text_value(value)?));
                }
                "priority" => {
                    assignments.push("priority = ?");
                    values.push(SqlValue::Integer(required_i64(value, "priority")?));
                }
                "rule_expression" | "ruleExpression" => {
                    assignments.push("rule_expression = ?");
                    values.push(SqlValue::Text(required_rule_expression(Some(value))?));
                }
                "regex_enabled" | "regexEnabled" => {
                    assignments.push("regex_enabled = ?");
                    values.push(SqlValue::Integer(bool_int_value(value)?));
                }
                "enabled" => {
                    assignments.push("enabled = ?");
                    values.push(SqlValue::Integer(bool_int_value(value)?));
                }
                "account_role_scope" | "accountRoleScope" => {
                    assignments.push("account_role_scope = ?");
                    values.push(SqlValue::Text(
                        normalize_account_role_scope(value.as_str())
                            .map_err(DbError::InvalidOperation)?,
                    ));
                }
                "transaction_type_scope" | "transactionTypeScope" => {
                    assignments.push("transaction_type_scope = ?");
                    values.push(SqlValue::Text(
                        normalize_transaction_type_scope(value.as_str())
                            .map_err(DbError::InvalidOperation)?,
                    ));
                }
                "field_scope" | "fieldScope" => {
                    assignments.push("field_scope = ?");
                    values.push(SqlValue::Text(field_scope_json(
                        &normalize_account_rule_field_scope(Some(value))
                            .map_err(DbError::InvalidOperation)?,
                    )?));
                }
                "source" => {
                    assignments.push("source = ?");
                    values.push(SqlValue::Text(sql_text_value(value)?));
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
            "UPDATE account_rules SET {} WHERE id = ? AND user_id = ?",
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
        let changed = self.connection.execute(
            "DELETE FROM account_rules WHERE id = ? AND user_id = ?",
            params![rule_id, user_id],
        )?;
        Ok(changed > 0)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn reorder_rules(&mut self, rule_ids: &[i64], user_id: i64) -> DbResult<bool> {
        let unique_ids = rule_ids.iter().copied().collect::<BTreeSet<_>>();
        if unique_ids.len() != rule_ids.len() {
            return Ok(false);
        }
        if !rule_ids.is_empty()
            && self.count_matching_user_rules(&unique_ids, user_id)?
                != i64::try_from(unique_ids.len()).unwrap_or(i64::MAX)
        {
            return Ok(false);
        }
        let now = utc_now_iso();
        run_transaction(self.connection, |transaction| {
            for (priority, rule_id) in rule_ids.iter().enumerate() {
                transaction.execute(
                    "UPDATE account_rules SET priority = ?, updated_at = ?
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
    pub fn list_match_candidates(&mut self, user_id: i64) -> DbResult<Vec<AccountRuleCandidate>> {
        self.list_rules(user_id, None, true, None, None)?
            .into_iter()
            .map(account_rule_candidate_from_record)
            .collect()
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn test_rule_match(
        &mut self,
        rule_id: i64,
        user_id: i64,
        context: &AccountRuleMatchContext,
        requested_role_scope: &str,
        transaction_type_scope: &str,
    ) -> DbResult<Option<bill_analyser_core::account_rules::AccountRuleMatch>> {
        let Some(rule) = self.get_rule(rule_id, user_id)? else {
            return Ok(None);
        };
        let candidate = account_rule_candidate_from_record(rule)?;
        Ok(bill_analyser_core::account_rules::match_account_rules(
            &[candidate],
            context,
            requested_role_scope,
            transaction_type_scope,
        ))
    }

    fn account_belongs_to_user(&self, account_id: i64, user_id: i64) -> DbResult<bool> {
        self.connection
            .query_row(
                "SELECT 1 FROM accounts WHERE id = ? AND user_id = ? LIMIT 1",
                params![account_id, user_id],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(DbError::from)
    }

    fn count_matching_user_rules(&self, rule_ids: &BTreeSet<i64>, user_id: i64) -> DbResult<i64> {
        let placeholders = std::iter::repeat_n("?", rule_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let mut values = vec![SqlValue::Integer(user_id)];
        values.extend(rule_ids.iter().copied().map(SqlValue::Integer));
        self.connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM account_rules \
                     WHERE user_id = ? AND id IN ({placeholders})"
                ),
                params_from_iter(values),
                |row| row.get(0),
            )
            .map_err(DbError::from)
    }
}
