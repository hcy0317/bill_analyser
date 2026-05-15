use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{TimeZone, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row, Transaction};
use serde_json::{Map, Number, Value};

use crate::{run_transaction, DbError, DbResult};

pub type AccountRecord = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDisplayOrder {
    pub account_id: i64,
    pub display_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTransactionsMoveResult {
    pub success: bool,
    pub message: String,
    pub moved_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTransactionsClearResult {
    pub success: bool,
    pub message: String,
    pub deleted_count: i64,
}

pub struct AccountsRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> AccountsRepository<'conn> {
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    pub fn list_accounts(&mut self, user_id: i64) -> DbResult<Vec<AccountRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, user_id, name, type, category, currency, icon, color,
                    balance, initial_balance, hidden, display_order, comment, aliases,
                    parent_id, created_at, updated_at
             FROM accounts
             WHERE user_id = ?
             ORDER BY display_order, name",
        )?;
        let rows = statement.query_map(params![user_id], account_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_account(
        &mut self,
        account_id: i64,
        user_id: i64,
    ) -> DbResult<Option<AccountRecord>> {
        self.connection
            .query_row(
                "SELECT id, user_id, name, type, category, currency, icon, color,
                        balance, initial_balance, hidden, display_order, comment, aliases,
                        parent_id, created_at, updated_at
                 FROM accounts
                 WHERE id = ? AND user_id = ?",
                params![account_id, user_id],
                account_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn get_sub_accounts(
        &mut self,
        parent_id: i64,
        user_id: i64,
    ) -> DbResult<Vec<AccountRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, user_id, name, type, category, currency, icon, color,
                    balance, initial_balance, hidden, display_order, comment, aliases,
                    parent_id, created_at, updated_at
             FROM accounts
             WHERE parent_id = ? AND user_id = ?",
        )?;
        let rows = statement.query_map(params![parent_id, user_id], account_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn create_account(&mut self, payload: &Value, user_id: i64) -> DbResult<i64> {
        let now = utc_now_iso();
        let parent_id = parent_id_value(payload);
        let aliases_value = aliases_sql_value(payload.get("aliases"))?;

        self.connection.execute(
            "INSERT INTO accounts (
                name, type, category, currency, icon, color,
                balance, initial_balance, hidden, display_order,
                comment, aliases, parent_id, created_at, updated_at, user_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            [
                sql_value_or_null(payload.get("name"))?,
                sql_value_or_default(payload.get("type"), SqlValue::Integer(1))?,
                sql_value_or_null(payload.get("category"))?,
                sql_value_or_default(payload.get("currency"), SqlValue::Text("CNY".to_string()))?,
                sql_value_or_null(payload.get("icon"))?,
                sql_value_or_null(payload.get("color"))?,
                sql_value_or_default(payload.get("balance"), SqlValue::Real(0.0))?,
                sql_value_or_default(payload.get("initial_balance"), SqlValue::Real(0.0))?,
                SqlValue::Integer(optional_bool_int(payload, "hidden")?.unwrap_or(0)),
                sql_value_or_default(payload.get("display_order"), SqlValue::Integer(0))?,
                sql_value_or_null(payload.get("comment"))?,
                aliases_value,
                SqlValue::Integer(parent_id),
                SqlValue::Text(now.clone()),
                SqlValue::Text(now),
                SqlValue::Integer(user_id),
            ],
        )?;

        let account_id = self.connection.last_insert_rowid();
        if let Some(sub_accounts) = payload.get("subAccounts").and_then(Value::as_array) {
            for sub_account in sub_accounts {
                let mut sub_payload = sub_account.as_object().cloned().ok_or_else(|| {
                    DbError::InvalidOperation("subAccounts items must be objects".to_string())
                })?;
                sub_payload.insert(
                    "parentId".to_string(),
                    Value::Number(Number::from(account_id)),
                );
                self.create_account(&Value::Object(sub_payload), user_id)?;
            }
        }
        Ok(account_id)
    }

    pub fn update_account(
        &mut self,
        account_id: i64,
        payload: &Value,
        user_id: i64,
    ) -> DbResult<bool> {
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("account update payload must be an object".to_string())
        })?;
        if object.is_empty() {
            return Ok(false);
        }

        let mut assignments: Vec<&str> = Vec::new();
        let mut values: Vec<SqlValue> = Vec::new();

        for (key, value) in object {
            match key.as_str() {
                "parentId" | "parent_id" => {
                    assignments.push("parent_id = ?");
                    values.push(SqlValue::Integer(int_value(value, key)?));
                }
                "subAccounts" => {}
                "name" => push_sql_assignment(&mut assignments, &mut values, "name", value)?,
                "type" => push_sql_assignment(&mut assignments, &mut values, "type", value)?,
                "category" => {
                    push_sql_assignment(&mut assignments, &mut values, "category", value)?
                }
                "currency" => {
                    push_sql_assignment(&mut assignments, &mut values, "currency", value)?
                }
                "icon" => push_sql_assignment(&mut assignments, &mut values, "icon", value)?,
                "color" => push_sql_assignment(&mut assignments, &mut values, "color", value)?,
                "balance" => push_sql_assignment(&mut assignments, &mut values, "balance", value)?,
                "initial_balance" => {
                    push_sql_assignment(&mut assignments, &mut values, "initial_balance", value)?;
                }
                "hidden" => {
                    assignments.push("hidden = ?");
                    values.push(SqlValue::Integer(bool_int_value(value, key)?));
                }
                "display_order" | "displayOrder" => {
                    assignments.push("display_order = ?");
                    values.push(SqlValue::Integer(int_value(value, key)?));
                }
                "comment" => push_sql_assignment(&mut assignments, &mut values, "comment", value)?,
                "aliases" => {
                    assignments.push("aliases = ?");
                    values.push(aliases_sql_value(Some(value))?);
                }
                _ => {}
            }
        }

        assignments.push("updated_at = ?");
        values.push(SqlValue::Text(utc_now_iso()));
        values.push(SqlValue::Integer(account_id));
        values.push(SqlValue::Integer(user_id));

        let sql = format!(
            "UPDATE accounts SET {} WHERE id = ? AND user_id = ?",
            assignments.join(", ")
        );
        let changed = self
            .connection
            .execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(changed > 0)
    }

    pub fn delete_account(&mut self, account_id: i64, user_id: i64) -> DbResult<bool> {
        let changed = self.connection.execute(
            "DELETE FROM accounts WHERE id = ? AND user_id = ?",
            params![account_id, user_id],
        )?;
        Ok(changed > 0)
    }

    pub fn update_display_orders(
        &mut self,
        orders: &[AccountDisplayOrder],
        user_id: i64,
    ) -> DbResult<bool> {
        if orders.is_empty() {
            return Ok(true);
        }

        let now = utc_now_iso();
        run_transaction(self.connection, |transaction| {
            for order in orders {
                transaction.execute(
                    "UPDATE accounts SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?",
                    params![order.display_order, now, order.account_id, user_id],
                )?;
            }
            Ok(())
        })?;
        Ok(true)
    }

    pub fn move_all_transactions(
        &mut self,
        from_account_id: i64,
        to_account_id: i64,
        user_id: i64,
    ) -> DbResult<AccountTransactionsMoveResult> {
        if from_account_id == to_account_id {
            return Ok(account_move_failure(
                "Source and target accounts must be different",
            ));
        }

        run_transaction(self.connection, |transaction| {
            if !account_exists_in_transaction(transaction, from_account_id, user_id)? {
                return Ok(account_move_failure("Source account not found"));
            }
            if !account_exists_in_transaction(transaction, to_account_id, user_id)? {
                return Ok(account_move_failure("Target account not found"));
            }

            let now = utc_now_iso();
            let mut moved_count = 0_i64;
            if table_exists(transaction, "bills")? {
                moved_count += row_count_to_i64(transaction.execute(
                    "UPDATE bills
                     SET source_account_id = ?1, updated_at = ?2
                     WHERE user_id = ?3 AND source_account_id = ?4",
                    params![to_account_id, now, user_id, from_account_id],
                )?);
                moved_count += row_count_to_i64(transaction.execute(
                    "UPDATE bills
                     SET destination_account_id = ?1, updated_at = ?2
                     WHERE user_id = ?3 AND destination_account_id = ?4",
                    params![to_account_id, now, user_id, from_account_id],
                )?);
            }
            if table_exists(transaction, "account_transfers")? {
                moved_count += row_count_to_i64(transaction.execute(
                    "UPDATE account_transfers
                     SET from_account_id = ?1
                     WHERE user_id = ?2 AND from_account_id = ?3",
                    params![to_account_id, user_id, from_account_id],
                )?);
                moved_count += row_count_to_i64(transaction.execute(
                    "UPDATE account_transfers
                     SET to_account_id = ?1
                     WHERE user_id = ?2 AND to_account_id = ?3",
                    params![to_account_id, user_id, from_account_id],
                )?);
            }

            Ok(AccountTransactionsMoveResult {
                success: true,
                message: "Transactions moved successfully".to_string(),
                moved_count,
            })
        })
    }

    pub fn delete_all_transactions_by_account(
        &mut self,
        account_id: i64,
        user_id: i64,
    ) -> DbResult<AccountTransactionsClearResult> {
        run_transaction(self.connection, |transaction| {
            if !account_exists_in_transaction(transaction, account_id, user_id)? {
                return Ok(account_clear_failure("Account not found"));
            }

            let bill_ids = list_account_bill_ids(transaction, user_id, account_id)?;
            let mut deleted_bill_count = 0_i64;
            if !bill_ids.is_empty() {
                delete_bill_side_effects(transaction, user_id, &bill_ids)?;
                delete_bill_tags(transaction, &bill_ids)?;
                let placeholders = placeholders(bill_ids.len());
                let mut sql_params = bill_ids
                    .iter()
                    .copied()
                    .map(SqlValue::Integer)
                    .collect::<Vec<_>>();
                sql_params.push(SqlValue::Integer(user_id));
                deleted_bill_count = row_count_to_i64(transaction.execute(
                    &format!("DELETE FROM bills WHERE id IN ({placeholders}) AND user_id = ?"),
                    params_from_iter(sql_params),
                )?);
            }

            let deleted_transfer_count = if table_exists(transaction, "account_transfers")? {
                row_count_to_i64(transaction.execute(
                    "DELETE FROM account_transfers
                     WHERE user_id = ?1 AND (from_account_id = ?2 OR to_account_id = ?2)",
                    params![user_id, account_id],
                )?)
            } else {
                0
            };

            Ok(AccountTransactionsClearResult {
                success: true,
                message: "Transactions deleted successfully".to_string(),
                deleted_count: deleted_bill_count + deleted_transfer_count,
            })
        })
    }
}

fn account_move_failure(message: &str) -> AccountTransactionsMoveResult {
    AccountTransactionsMoveResult {
        success: false,
        message: message.to_string(),
        moved_count: 0,
    }
}

fn account_clear_failure(message: &str) -> AccountTransactionsClearResult {
    AccountTransactionsClearResult {
        success: false,
        message: message.to_string(),
        deleted_count: 0,
    }
}

fn account_exists_in_transaction(
    transaction: &Transaction<'_>,
    account_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    Ok(transaction.query_row(
        "SELECT COUNT(*) > 0 FROM accounts WHERE id = ?1 AND user_id = ?2",
        params![account_id, user_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn list_account_bill_ids(
    transaction: &Transaction<'_>,
    user_id: i64,
    account_id: i64,
) -> DbResult<Vec<i64>> {
    if !table_exists(transaction, "bills")? {
        return Ok(Vec::new());
    }
    let mut statement = transaction.prepare(
        "SELECT id
         FROM bills
         WHERE user_id = ?1 AND (source_account_id = ?2 OR destination_account_id = ?2)",
    )?;
    let rows = statement.query_map(params![user_id, account_id], |row| row.get::<_, i64>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn delete_bill_side_effects(
    transaction: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<()> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(());
    }
    delete_pair_table_by_pair_columns(transaction, user_id, &bill_ids, "bill_pair_links")?;
    delete_pair_table_by_pair_columns(
        transaction,
        user_id,
        &bill_ids,
        "bill_transfer_pair_suppressions",
    )?;
    delete_pair_table_by_pair_columns(
        transaction,
        user_id,
        &bill_ids,
        "bill_investment_pair_suppressions",
    )?;
    if table_exists(transaction, "bill_learning_rule_suppressions")? {
        let placeholders = placeholders(bill_ids.len());
        let mut sql_params = vec![SqlValue::Integer(user_id)];
        sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
        transaction.execute(
            &format!(
                "DELETE FROM bill_learning_rule_suppressions
                 WHERE user_id = ? AND bill_id IN ({placeholders})"
            ),
            params_from_iter(sql_params),
        )?;
    }
    Ok(())
}

fn delete_pair_table_by_pair_columns(
    transaction: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
    table_name: &str,
) -> DbResult<()> {
    if !table_exists(transaction, table_name)? {
        return Ok(());
    }
    let placeholders = placeholders(bill_ids.len());
    let mut sql_params = vec![SqlValue::Integer(user_id)];
    sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    transaction.execute(
        &format!(
            "DELETE FROM {table_name}
             WHERE user_id = ?
               AND (left_bill_id IN ({placeholders}) OR right_bill_id IN ({placeholders}))"
        ),
        params_from_iter(sql_params),
    )?;
    Ok(())
}

fn delete_bill_tags(transaction: &Transaction<'_>, bill_ids: &[i64]) -> DbResult<()> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() || !table_exists(transaction, "bill_tags")? {
        return Ok(());
    }
    let placeholders = placeholders(bill_ids.len());
    let sql_params = bill_ids
        .iter()
        .copied()
        .map(SqlValue::Integer)
        .collect::<Vec<_>>();
    transaction.execute(
        &format!("DELETE FROM bill_tags WHERE bill_id IN ({placeholders})"),
        params_from_iter(sql_params),
    )?;
    Ok(())
}

fn table_exists(transaction: &Transaction<'_>, table_name: &str) -> DbResult<bool> {
    Ok(transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}

fn normalize_ids(values: &[i64]) -> Vec<i64> {
    values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn row_count_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub fn open_accounts_connection(db_path: &str) -> DbResult<Connection> {
    if db_path == ":memory:" || db_path == "file::memory:?cache=shared" {
        return Err(DbError::InvalidOperation(
            "in-memory sqlite paths must use the Python account implementation".to_string(),
        ));
    }

    let connection = Connection::open(db_path)?;
    connection.busy_timeout(Duration::from_secs(10))?;
    // The Python account facade never enabled SQLite foreign-key checks on its
    // long-lived aiosqlite connection. Keep that contract for this bridge so
    // legacy userless fixture/runtime databases are not made stricter in S5b.
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    connection.execute_batch("PRAGMA busy_timeout=10000;")?;
    Ok(connection)
}

pub fn parse_account_display_orders(raw_value: &Value) -> DbResult<Vec<AccountDisplayOrder>> {
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
                    "order item must contain account id and display order".to_string(),
                ));
            }
            Ok(AccountDisplayOrder {
                account_id: int_value(&pair[0], "account_id")?,
                display_order: int_value(&pair[1], "display_order")?,
            })
        })
        .collect()
}

fn account_from_row(row: &Row<'_>) -> rusqlite::Result<AccountRecord> {
    let mut account = Map::new();
    for column in [
        "id",
        "user_id",
        "name",
        "type",
        "category",
        "currency",
        "icon",
        "color",
        "balance",
        "initial_balance",
        "hidden",
        "display_order",
        "comment",
        "aliases",
        "parent_id",
        "created_at",
        "updated_at",
    ] {
        account.insert(
            column.to_string(),
            sqlite_value_to_json(row.get_ref(column)?),
        );
    }
    Ok(account)
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

fn parent_id_value(payload: &Value) -> i64 {
    payload
        .get("parentId")
        .and_then(|value| int_value(value, "parentId").ok())
        .filter(|value| *value != 0)
        .or_else(|| {
            payload
                .get("parent_id")
                .and_then(|value| int_value(value, "parent_id").ok())
        })
        .unwrap_or(0)
}

fn push_sql_assignment(
    assignments: &mut Vec<&str>,
    values: &mut Vec<SqlValue>,
    column: &'static str,
    value: &Value,
) -> DbResult<()> {
    assignments.push(match column {
        "initial_balance" => "initial_balance = ?",
        "display_order" => "display_order = ?",
        "parent_id" => "parent_id = ?",
        "name" => "name = ?",
        "type" => "type = ?",
        "category" => "category = ?",
        "currency" => "currency = ?",
        "icon" => "icon = ?",
        "color" => "color = ?",
        "balance" => "balance = ?",
        "comment" => "comment = ?",
        _ => {
            return Err(DbError::InvalidOperation(format!(
                "unsupported account update field: {column}"
            )))
        }
    });
    values.push(sql_value_or_null(Some(value))?);
    Ok(())
}

fn aliases_sql_value(value: Option<&Value>) -> DbResult<SqlValue> {
    match value {
        None | Some(Value::Null) => Ok(SqlValue::Null),
        Some(Value::Array(values)) => {
            let aliases: Vec<String> = values
                .iter()
                .map(python_alias_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            serde_json::to_string(&aliases)
                .map(SqlValue::Text)
                .map_err(|error| DbError::InvalidOperation(error.to_string()))
        }
        Some(Value::String(text)) => Ok(SqlValue::Text(text.clone())),
        Some(value) => Ok(SqlValue::Text(value.to_string())),
    }
}

fn python_alias_text(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => {
            if *flag {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::String(text) => text.clone(),
        Value::Number(_) | Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn sql_value_or_default(value: Option<&Value>, default: SqlValue) -> DbResult<SqlValue> {
    match value {
        Some(value) => json_value_to_sql(value),
        None => Ok(default),
    }
}

fn sql_value_or_null(value: Option<&Value>) -> DbResult<SqlValue> {
    match value {
        Some(value) => json_value_to_sql(value),
        None => Ok(SqlValue::Null),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    name TEXT NOT NULL,
                    type INTEGER NOT NULL,
                    category INTEGER,
                    currency TEXT DEFAULT 'CNY',
                    icon TEXT,
                    color TEXT,
                    balance REAL DEFAULT 0,
                    initial_balance REAL DEFAULT 0,
                    hidden BOOLEAN DEFAULT 0,
                    display_order INTEGER DEFAULT 0,
                    comment TEXT,
                    aliases TEXT,
                    parent_id INTEGER DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );",
            )
            .unwrap();
        connection
    }

    #[test]
    fn taxonomy_accounts_crud_keeps_user_scope_subaccounts_and_sort_order() {
        let mut connection = create_connection();
        let mut repository = AccountsRepository::new(&mut connection);

        let first_id = repository
            .create_account(
                &serde_json::json!({
                    "name": "父账户",
                    "type": 2,
                    "category": 1,
                    "balance": 12.5,
                    "aliases": ["主卡", " "],
                    "subAccounts": [{"name": "子账户", "type": 1, "balance": 3.5}]
                }),
                7,
            )
            .unwrap();
        let second_id = repository
            .create_account(
                &serde_json::json!({"name": "排序账户", "display_order": 1}),
                7,
            )
            .unwrap();
        let other_user_id = repository
            .create_account(&serde_json::json!({"name": "其他用户账户"}), 8)
            .unwrap();

        assert!(repository
            .update_account(
                first_id,
                &serde_json::json!({
                    "id": first_id.to_string(),
                    "displayOrder": 2,
                    "hidden": true,
                    "parent_id": second_id
                }),
                7
            )
            .unwrap());
        assert!(!repository
            .update_account(other_user_id, &serde_json::json!({"name": "越权"}), 7)
            .unwrap());

        let accounts = repository.list_accounts(7).unwrap();
        assert_eq!(
            accounts
                .iter()
                .map(|account| account["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["子账户", "排序账户", "父账户"]
        );
        assert_eq!(accounts[2]["hidden"], Value::Number(Number::from(1)));
        assert_eq!(
            accounts[2]["aliases"],
            Value::String("[\"主卡\"]".to_string())
        );

        assert_eq!(
            repository.get_account(first_id, 7).unwrap().unwrap()["parent_id"],
            Value::Number(Number::from(second_id))
        );

        let sub_accounts = repository.get_sub_accounts(first_id, 7).unwrap();
        assert_eq!(sub_accounts.len(), 1);
        assert_eq!(
            sub_accounts[0]["parent_id"],
            Value::Number(Number::from(first_id))
        );
        assert_eq!(
            repository.list_accounts(8).unwrap()[0]["name"],
            "其他用户账户"
        );

        assert!(repository.get_account(second_id, 7).unwrap().is_some());
    }

    #[test]
    fn taxonomy_accounts_file_connection_preserves_legacy_userless_account_writes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("legacy_accounts.db");
        {
            let connection = Connection::open(&db_path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE users (id INTEGER PRIMARY KEY);
                    CREATE TABLE accounts (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        user_id INTEGER NOT NULL DEFAULT 1,
                        name TEXT NOT NULL,
                        type INTEGER NOT NULL,
                        category INTEGER,
                        currency TEXT DEFAULT 'CNY',
                        icon TEXT,
                        color TEXT,
                        balance REAL DEFAULT 0,
                        initial_balance REAL DEFAULT 0,
                        hidden BOOLEAN DEFAULT 0,
                        display_order INTEGER DEFAULT 0,
                        comment TEXT,
                        aliases TEXT,
                        parent_id INTEGER DEFAULT 0,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL,
                        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                    );",
                )
                .unwrap();
        }

        let db_path_text = db_path.to_string_lossy().to_string();
        let mut connection = open_accounts_connection(&db_path_text).unwrap();
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 0);

        let mut repository = AccountsRepository::new(&mut connection);
        let account_id = repository
            .create_account(&serde_json::json!({"name": "无用户账户"}), 1)
            .unwrap();
        assert_eq!(
            repository.get_account(account_id, 1).unwrap().unwrap()["name"],
            "无用户账户"
        );
    }

    #[test]
    fn taxonomy_accounts_delete_and_display_orders_are_user_scoped() {
        let mut connection = create_connection();
        let mut repository = AccountsRepository::new(&mut connection);

        let first_id = repository
            .create_account(&serde_json::json!({"name": "账户A"}), 7)
            .unwrap();
        let second_id = repository
            .create_account(&serde_json::json!({"name": "账户B"}), 7)
            .unwrap();
        let other_user_id = repository
            .create_account(&serde_json::json!({"name": "其他用户账户"}), 8)
            .unwrap();

        assert!(repository
            .update_display_orders(
                &[
                    AccountDisplayOrder {
                        account_id: first_id,
                        display_order: 20,
                    },
                    AccountDisplayOrder {
                        account_id: second_id,
                        display_order: 10,
                    },
                    AccountDisplayOrder {
                        account_id: other_user_id,
                        display_order: 1,
                    },
                ],
                7,
            )
            .unwrap());

        let accounts = repository.list_accounts(7).unwrap();
        assert_eq!(
            accounts
                .iter()
                .map(|account| account["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["账户B", "账户A"]
        );
        assert_eq!(
            repository.get_account(other_user_id, 8).unwrap().unwrap()["display_order"],
            Value::Number(Number::from(0))
        );

        assert!(!repository.delete_account(other_user_id, 7).unwrap());
        assert!(repository.delete_account(first_id, 7).unwrap());
        assert!(repository.get_account(first_id, 7).unwrap().is_none());
    }

    #[test]
    fn taxonomy_accounts_parse_display_orders_validates_shape() {
        let orders = parse_account_display_orders(&serde_json::json!([[3, 2], [4, 1]])).unwrap();
        assert_eq!(orders[0].account_id, 3);
        assert_eq!(orders[1].display_order, 1);

        assert!(parse_account_display_orders(&serde_json::json!({"bad": true})).is_err());
        assert!(parse_account_display_orders(&serde_json::json!([[1]])).is_err());
        assert!(parse_account_display_orders(&serde_json::json!([{"id": 1}])).is_err());
    }

    #[test]
    fn taxonomy_accounts_alias_arrays_match_python_stringification_edges() {
        let aliases = aliases_sql_value(Some(&serde_json::json!([
            " 主卡 ", 12, true, false, null, ""
        ])))
        .unwrap();

        assert_eq!(
            aliases,
            SqlValue::Text("[\"主卡\",\"12\",\"True\",\"False\",\"None\"]".to_string())
        );
    }
}
