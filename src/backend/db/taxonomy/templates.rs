// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{TimeZone, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{Map, Number, Value};

use crate::{run_transaction, DbError, DbResult};

pub type TemplateRecord = Map<String, Value>;

const BILL_TEMPLATE_COLUMNS: &[&str] = &[
    "id",
    "user_id",
    "name",
    "description",
    "type",
    "category",
    "amount",
    "account",
    "counterparty",
    "destination_amount",
    "hide_amount",
    "tag",
    "comment",
    "is_favorite",
    "use_count",
    "last_used_at",
    "display_order",
    "hidden",
    "utc_offset",
    "created_at",
    "updated_at",
];

const RECURRING_TEMPLATE_COLUMNS: &[&str] = &[
    "id",
    "user_id",
    "template_id",
    "name",
    "description",
    "type",
    "category",
    "amount",
    "account",
    "counterparty",
    "destination_amount",
    "hide_amount",
    "tag",
    "comment",
    "frequency",
    "scheduled_frequency_type",
    "start_date",
    "end_date",
    "next_date",
    "enabled",
    "auto_create",
    "display_order",
    "hidden",
    "utc_offset",
    "created_at",
    "updated_at",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDisplayOrder {
    pub template_id: i64,
    pub display_order: i64,
}

pub struct TemplatesRepository<'conn> {
    connection: &'conn mut Connection,
}

impl<'conn> TemplatesRepository<'conn> {
    pub fn new(connection: &'conn mut Connection) -> Self {
        Self { connection }
    }

    pub fn list_templates(
        &mut self,
        user_id: i64,
        template_type: Option<i64>,
    ) -> DbResult<Vec<TemplateRecord>> {
        let mut templates = Vec::new();
        if template_type.is_none() || template_type == Some(1) {
            let rows = self.list_bill_template_rows(user_id)?;
            templates.extend(
                rows.into_iter()
                    .map(|row| serialize_template_row(&row, 1))
                    .collect::<Vec<_>>(),
            );
        }
        if template_type.is_none() || template_type == Some(2) {
            let rows = self.list_recurring_template_rows(user_id, false)?;
            templates.extend(
                rows.into_iter()
                    .map(|row| serialize_template_row(&row, 2))
                    .collect::<Vec<_>>(),
            );
        }
        Ok(templates)
    }

    pub fn get_template_by_id(
        &mut self,
        template_id: i64,
        user_id: i64,
        template_type: Option<i64>,
    ) -> DbResult<Option<TemplateRecord>> {
        if template_type.is_none() || template_type == Some(1) {
            if let Some(row) = self.get_bill_template_row(template_id, user_id)? {
                return Ok(Some(serialize_template_row(&row, 1)));
            }
        }
        if template_type.is_none() || template_type == Some(2) {
            if let Some(row) = self.get_recurring_template_row(template_id, user_id)? {
                return Ok(Some(serialize_template_row(&row, 2)));
            }
        }
        Ok(None)
    }

    pub fn list_enabled_recurring_templates(
        &mut self,
        user_id: i64,
    ) -> DbResult<Vec<TemplateRecord>> {
        self.list_recurring_template_rows(user_id, true)
    }

    pub fn create_template(&mut self, payload: &Value, user_id: i64) -> DbResult<i64> {
        let template_type = int_or_default(payload.get("templateType"), 1)?;
        let display_order = self.next_display_order(template_type, user_id)?;
        let now = utc_now_iso();

        if template_type == 2 {
            self.connection.execute(
                "INSERT INTO recurring_bills (
                    user_id, template_id, name, description, type, category, amount,
                    account, counterparty, destination_amount, hide_amount, tag,
                    comment, frequency, scheduled_frequency_type, start_date, end_date,
                    next_date, hidden, display_order, utc_offset, enabled, auto_create,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    user_id,
                    SqlValue::Null,
                    sql_value_or_default(payload.get("name"), SqlValue::Text(String::new()))?,
                    sql_value_or_default(payload.get("description"), SqlValue::Text(String::new()))?,
                    sql_value_or_null(payload.get("type"))?,
                    sql_value_or_default(payload.get("categoryId"), SqlValue::Text(String::new()))?,
                    SqlValue::Real(float_or_zero(payload.get("sourceAmount"))?),
                    sql_value_or_default(payload.get("sourceAccountId"), SqlValue::Text("0".to_string()))?,
                    sql_value_or_default(payload.get("destinationAccountId"), SqlValue::Text("0".to_string()))?,
                    SqlValue::Real(float_or_zero(payload.get("destinationAmount"))?),
                    SqlValue::Integer(i64::from(python_truthy(payload.get("hideAmount")))),
                    SqlValue::Text(serialize_tag_ids(payload.get("tagIds"))),
                    sql_value_or_default(payload.get("comment"), SqlValue::Text(String::new()))?,
                    sql_value_or_default(payload.get("scheduledFrequency"), SqlValue::Text(String::new()))?,
                    SqlValue::Integer(int_or_default(payload.get("scheduledFrequencyType"), 0)?),
                    sql_value_or_null(payload.get("scheduledStartDate"))?,
                    sql_value_or_null(payload.get("scheduledEndDate"))?,
                    next_date_value(payload, &now)?,
                    SqlValue::Integer(i64::from(python_truthy(payload.get("hidden")))),
                    display_order,
                    int_or_default(payload.get("utcOffset"), 0)?,
                    1,
                    0,
                    now,
                    now,
                ],
            )?;
        } else {
            self.connection.execute(
                "INSERT INTO bill_templates (
                    user_id, name, description, type, category, amount, account,
                    counterparty, destination_amount, hide_amount, tag, comment,
                    is_favorite, display_order, hidden, utc_offset, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    user_id,
                    sql_value_or_default(payload.get("name"), SqlValue::Text(String::new()))?,
                    sql_value_or_default(
                        payload.get("description"),
                        SqlValue::Text(String::new())
                    )?,
                    sql_value_or_null(payload.get("type"))?,
                    sql_value_or_default(payload.get("categoryId"), SqlValue::Text(String::new()))?,
                    SqlValue::Real(float_or_zero(payload.get("sourceAmount"))?),
                    sql_value_or_default(
                        payload.get("sourceAccountId"),
                        SqlValue::Text("0".to_string())
                    )?,
                    sql_value_or_default(
                        payload.get("destinationAccountId"),
                        SqlValue::Text("0".to_string())
                    )?,
                    SqlValue::Real(float_or_zero(payload.get("destinationAmount"))?),
                    SqlValue::Integer(i64::from(python_truthy(payload.get("hideAmount")))),
                    SqlValue::Text(serialize_tag_ids(payload.get("tagIds"))),
                    sql_value_or_default(payload.get("comment"), SqlValue::Text(String::new()))?,
                    0,
                    display_order,
                    SqlValue::Integer(i64::from(python_truthy(payload.get("hidden")))),
                    int_or_default(payload.get("utcOffset"), 0)?,
                    now,
                    now,
                ],
            )?;
        }

        Ok(self.connection.last_insert_rowid())
    }

    pub fn update_template(
        &mut self,
        template_id: i64,
        payload: &Value,
        user_id: i64,
        template_type: Option<i64>,
    ) -> DbResult<bool> {
        let object = payload.as_object().ok_or_else(|| {
            DbError::InvalidOperation("template update payload must be an object".to_string())
        })?;
        if object.is_empty() {
            return Ok(false);
        }

        let resolved_type = template_type.unwrap_or(1);
        let mut assignments: Vec<&str> = Vec::new();
        let mut values: Vec<SqlValue> = Vec::new();

        for (key, value) in object {
            match key.as_str() {
                "name" => push_assignment(&mut assignments, &mut values, "name", value)?,
                "type" => push_assignment(&mut assignments, &mut values, "type", value)?,
                "categoryId" => push_assignment(&mut assignments, &mut values, "category", value)?,
                "sourceAccountId" => {
                    push_assignment(&mut assignments, &mut values, "account", value)?
                }
                "destinationAccountId" => {
                    push_assignment(&mut assignments, &mut values, "counterparty", value)?;
                }
                "sourceAmount" => {
                    assignments.push("amount = ?");
                    values.push(SqlValue::Real(float_or_zero(Some(value))?));
                }
                "destinationAmount" => {
                    assignments.push("destination_amount = ?");
                    values.push(SqlValue::Real(float_or_zero(Some(value))?));
                }
                "hideAmount" => {
                    assignments.push("hide_amount = ?");
                    values.push(SqlValue::Integer(i64::from(python_truthy(Some(value)))));
                }
                "comment" => push_assignment(&mut assignments, &mut values, "comment", value)?,
                "hidden" => {
                    assignments.push("hidden = ?");
                    values.push(SqlValue::Integer(i64::from(python_truthy(Some(value)))));
                }
                "displayOrder" => {
                    assignments.push("display_order = ?");
                    values.push(SqlValue::Integer(int_or_default(Some(value), 0)?));
                }
                "utcOffset" => {
                    assignments.push("utc_offset = ?");
                    values.push(SqlValue::Integer(int_or_default(Some(value), 0)?));
                }
                "tagIds" => {
                    assignments.push("tag = ?");
                    values.push(SqlValue::Text(serialize_tag_ids(Some(value))));
                }
                "scheduledFrequencyType" if resolved_type == 2 => {
                    assignments.push("scheduled_frequency_type = ?");
                    values.push(SqlValue::Integer(int_or_default(Some(value), 0)?));
                }
                "scheduledFrequency" if resolved_type == 2 => {
                    push_assignment(&mut assignments, &mut values, "frequency", value)?;
                }
                "scheduledStartDate" if resolved_type == 2 => {
                    push_assignment(&mut assignments, &mut values, "start_date", value)?;
                    assignments.push("next_date = ?");
                    values.push(json_value_to_sql(value)?);
                }
                "scheduledEndDate" if resolved_type == 2 => {
                    push_assignment(&mut assignments, &mut values, "end_date", value)?;
                }
                _ => {}
            }
        }

        assignments.push("updated_at = ?");
        values.push(SqlValue::Text(utc_now_iso()));
        values.push(SqlValue::Integer(template_id));
        values.push(SqlValue::Integer(user_id));

        let sql = format!(
            "UPDATE {} SET {} WHERE id = ? AND user_id = ?",
            template_table_name(resolved_type),
            assignments.join(", ")
        );
        let changed = self
            .connection
            .execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(changed > 0)
    }

    pub fn delete_template(
        &mut self,
        template_id: i64,
        user_id: i64,
        template_type: Option<i64>,
    ) -> DbResult<bool> {
        let changed = self.connection.execute(
            &format!(
                "DELETE FROM {} WHERE id = ? AND user_id = ?",
                template_table_name(template_type.unwrap_or(1))
            ),
            params![template_id, user_id],
        )?;
        Ok(changed > 0)
    }

    pub fn update_display_orders(
        &mut self,
        orders: &[TemplateDisplayOrder],
        template_type: i64,
        user_id: i64,
    ) -> DbResult<bool> {
        if orders.is_empty() {
            return Ok(true);
        }

        let table_name = template_table_name(template_type);
        let now = utc_now_iso();
        run_transaction(self.connection, |transaction| {
            for order in orders {
                transaction.execute(
                    &format!(
                        "UPDATE {table_name} SET display_order = ?, updated_at = ? WHERE id = ? AND user_id = ?"
                    ),
                    params![order.display_order, now, order.template_id, user_id],
                )?;
            }
            Ok(())
        })?;
        Ok(true)
    }

    fn list_bill_template_rows(&mut self, user_id: i64) -> DbResult<Vec<TemplateRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT id, user_id, name, description, type, category, amount,
                    account, counterparty, destination_amount, hide_amount, tag,
                    comment, is_favorite, use_count, last_used_at, display_order,
                    hidden, utc_offset, created_at, updated_at
             FROM bill_templates
             WHERE user_id = ?
             ORDER BY COALESCE(display_order, 0), is_favorite DESC, use_count DESC, name",
        )?;
        let rows = statement.query_map(params![user_id], bill_template_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    fn list_recurring_template_rows(
        &mut self,
        user_id: i64,
        enabled_only: bool,
    ) -> DbResult<Vec<TemplateRecord>> {
        let sql = if enabled_only {
            "SELECT id, user_id, template_id, name, description, type, category, amount,
                    account, counterparty, destination_amount, hide_amount, tag,
                    comment, frequency, scheduled_frequency_type, start_date, end_date,
                    next_date, enabled, auto_create, display_order, hidden, utc_offset,
                    created_at, updated_at
             FROM recurring_bills
             WHERE user_id = ? AND enabled = 1
             ORDER BY COALESCE(display_order, 0), name"
        } else {
            "SELECT id, user_id, template_id, name, description, type, category, amount,
                    account, counterparty, destination_amount, hide_amount, tag,
                    comment, frequency, scheduled_frequency_type, start_date, end_date,
                    next_date, enabled, auto_create, display_order, hidden, utc_offset,
                    created_at, updated_at
             FROM recurring_bills
             WHERE user_id = ?
             ORDER BY COALESCE(display_order, 0), name"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params![user_id], recurring_template_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    fn get_bill_template_row(
        &mut self,
        template_id: i64,
        user_id: i64,
    ) -> DbResult<Option<TemplateRecord>> {
        self.connection
            .query_row(
                "SELECT id, user_id, name, description, type, category, amount,
                        account, counterparty, destination_amount, hide_amount, tag,
                        comment, is_favorite, use_count, last_used_at, display_order,
                        hidden, utc_offset, created_at, updated_at
                 FROM bill_templates
                 WHERE id = ? AND user_id = ?",
                params![template_id, user_id],
                bill_template_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    fn get_recurring_template_row(
        &mut self,
        template_id: i64,
        user_id: i64,
    ) -> DbResult<Option<TemplateRecord>> {
        self.connection
            .query_row(
                "SELECT id, user_id, template_id, name, description, type, category, amount,
                        account, counterparty, destination_amount, hide_amount, tag,
                        comment, frequency, scheduled_frequency_type, start_date, end_date,
                        next_date, enabled, auto_create, display_order, hidden, utc_offset,
                        created_at, updated_at
                 FROM recurring_bills
                 WHERE id = ? AND user_id = ?",
                params![template_id, user_id],
                recurring_template_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    fn next_display_order(&mut self, template_type: i64, user_id: i64) -> DbResult<i64> {
        let table_name = template_table_name(template_type);
        let max_order = self.connection.query_row(
            &format!("SELECT COALESCE(MAX(display_order), 0) FROM {table_name} WHERE user_id = ?"),
            params![user_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(max_order + 1)
    }
}

pub fn open_templates_connection(db_path: &str) -> DbResult<Connection> {
    if db_path == ":memory:" || db_path == "file::memory:?cache=shared" {
        return Err(DbError::InvalidOperation(
            "in-memory sqlite paths must use the Python template implementation".to_string(),
        ));
    }

    let connection = Connection::open(db_path)?;
    connection.busy_timeout(Duration::from_secs(10))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch("PRAGMA busy_timeout=10000;")?;
    Ok(connection)
}

pub fn parse_template_display_orders(raw_value: &Value) -> DbResult<Vec<TemplateDisplayOrder>> {
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
                    "order item must contain template id and display order".to_string(),
                ));
            }
            Ok(TemplateDisplayOrder {
                template_id: int_or_default(Some(&pair[0]), 0)?,
                display_order: int_or_default(Some(&pair[1]), 0)?,
            })
        })
        .collect()
}

fn bill_template_from_row(row: &Row<'_>) -> rusqlite::Result<TemplateRecord> {
    row_to_map(row, BILL_TEMPLATE_COLUMNS)
}

fn recurring_template_from_row(row: &Row<'_>) -> rusqlite::Result<TemplateRecord> {
    row_to_map(row, RECURRING_TEMPLATE_COLUMNS)
}

fn row_to_map(row: &Row<'_>, columns: &[&str]) -> rusqlite::Result<TemplateRecord> {
    let mut result = Map::new();
    for column in columns {
        result.insert(
            (*column).to_string(),
            sqlite_value_to_json(row.get_ref(*column)?),
        );
    }
    Ok(result)
}

fn serialize_template_row(row: &TemplateRecord, template_type: i64) -> TemplateRecord {
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(string_field(row, "id")));
    result.insert("timeSequenceId".to_string(), Value::String(String::new()));
    result.insert(
        "templateType".to_string(),
        Value::Number(Number::from(template_type)),
    );
    result.insert("name".to_string(), value_or_empty_string(row, "name"));
    result.insert(
        "type".to_string(),
        Value::Number(Number::from(normalize_template_transaction_type(
            row.get("type"),
        ))),
    );
    result.insert(
        "categoryId".to_string(),
        Value::String(string_field(row, "category")),
    );
    result.insert(
        "time".to_string(),
        Value::Number(Number::from(int_field(row, "scheduled_at"))),
    );
    result.insert(
        "utcOffset".to_string(),
        Value::Number(Number::from(int_field(row, "utc_offset"))),
    );
    result.insert(
        "sourceAccountId".to_string(),
        Value::String(default_string_field(row, "account", "0")),
    );
    result.insert(
        "destinationAccountId".to_string(),
        Value::String(default_string_field(row, "counterparty", "0")),
    );
    result.insert(
        "sourceAmount".to_string(),
        number_value(float_field(row, "amount")),
    );
    result.insert(
        "destinationAmount".to_string(),
        number_value(float_field(row, "destination_amount")),
    );
    result.insert(
        "hideAmount".to_string(),
        Value::Bool(bool_field(row, "hide_amount")),
    );
    result.insert(
        "tagIds".to_string(),
        Value::Array(deserialize_tag_ids(row.get("tag"))),
    );
    result.insert("comment".to_string(), value_or_empty_string(row, "comment"));
    result.insert("editable".to_string(), Value::Bool(true));
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(int_field(row, "display_order"))),
    );
    result.insert("hidden".to_string(), Value::Bool(bool_field(row, "hidden")));
    result.insert(
        "scheduledFrequencyType".to_string(),
        recurring_number_or_null(row, template_type, "scheduled_frequency_type"),
    );
    result.insert(
        "scheduledFrequency".to_string(),
        recurring_value_or_null(row, template_type, "frequency"),
    );
    result.insert(
        "scheduledStartDate".to_string(),
        recurring_value_or_null(row, template_type, "start_date"),
    );
    result.insert(
        "scheduledEndDate".to_string(),
        recurring_value_or_null(row, template_type, "end_date"),
    );
    result.insert("scheduledAt".to_string(), Value::Null);
    result
}

fn template_table_name(template_type: i64) -> &'static str {
    if template_type == 2 {
        "recurring_bills"
    } else {
        "bill_templates"
    }
}

fn push_assignment(
    assignments: &mut Vec<&str>,
    values: &mut Vec<SqlValue>,
    column: &'static str,
    value: &Value,
) -> DbResult<()> {
    assignments.push(match column {
        "name" => "name = ?",
        "type" => "type = ?",
        "category" => "category = ?",
        "account" => "account = ?",
        "counterparty" => "counterparty = ?",
        "comment" => "comment = ?",
        "frequency" => "frequency = ?",
        "start_date" => "start_date = ?",
        "end_date" => "end_date = ?",
        _ => {
            return Err(DbError::InvalidOperation(format!(
                "unsupported template update field: {column}"
            )));
        }
    });
    values.push(json_value_to_sql(value)?);
    Ok(())
}

fn sql_value_or_null(value: Option<&Value>) -> DbResult<SqlValue> {
    match value {
        Some(value) => json_value_to_sql(value),
        None => Ok(SqlValue::Null),
    }
}

fn sql_value_or_default(value: Option<&Value>, default: SqlValue) -> DbResult<SqlValue> {
    match value {
        Some(value) => json_value_to_sql(value),
        None => Ok(default),
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
                    DbError::InvalidOperation("integer value out of range".to_string())
                })?)
            } else if let Some(float) = number.as_f64() {
                SqlValue::Real(float)
            } else {
                SqlValue::Null
            }
        }
        Value::String(text) => SqlValue::Text(text.clone()),
        Value::Array(_) | Value::Object(_) => SqlValue::Text(value.to_string()),
    })
}

fn next_date_value(payload: &Value, now: &str) -> DbResult<SqlValue> {
    match payload.get("scheduledStartDate") {
        Some(Value::Null) | None => Ok(SqlValue::Text(now.chars().take(10).collect())),
        Some(value) if !python_truthy(Some(value)) => {
            Ok(SqlValue::Text(now.chars().take(10).collect()))
        }
        Some(value) => json_value_to_sql(value),
    }
}

fn serialize_tag_ids(value: Option<&Value>) -> String {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .map(json_to_string)
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join(","),
        Some(value) => json_to_string(value),
        None => String::new(),
    }
}

fn deserialize_tag_ids(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| {
                let text = json_to_string(value);
                (!text.trim().is_empty()).then_some(Value::String(text))
            })
            .collect(),
        Some(value) => json_to_string(value)
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| Value::String(value.to_string()))
            .collect(),
        None => Vec::new(),
    }
}

fn normalize_template_transaction_type(value: Option<&Value>) -> i64 {
    let Some(value) = value else {
        return 3;
    };
    match json_to_string(value).trim().to_lowercase().as_str() {
        "2" | "income" | "收入" => 2,
        "3" | "expense" | "支出" => 3,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(number) => Value::Number(Number::from(number)),
        ValueRef::Real(number) => number_value(number),
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

fn number_value(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn recurring_value_or_null(row: &TemplateRecord, template_type: i64, key: &str) -> Value {
    if template_type == 2 {
        row.get(key).cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

fn recurring_number_or_null(row: &TemplateRecord, template_type: i64, key: &str) -> Value {
    if template_type == 2 {
        Value::Number(Number::from(int_field(row, key)))
    } else {
        Value::Null
    }
}

fn value_or_empty_string(row: &TemplateRecord, key: &str) -> Value {
    row.get(key)
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()))
}

fn default_string_field(row: &TemplateRecord, key: &str, default: &str) -> String {
    let value = string_field(row, key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

fn string_field(row: &TemplateRecord, key: &str) -> String {
    row.get(key).map(json_to_string).unwrap_or_default()
}

fn json_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn int_field(row: &TemplateRecord, key: &str) -> i64 {
    row.get(key).and_then(value_to_i64).unwrap_or(0)
}

fn float_field(row: &TemplateRecord, key: &str) -> f64 {
    row.get(key).and_then(value_to_f64).unwrap_or(0.0)
}

fn bool_field(row: &TemplateRecord, key: &str) -> bool {
    row.get(key).is_some_and(value_truthy)
}

fn python_truthy(value: Option<&Value>) -> bool {
    value.is_some_and(value_truthy)
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn int_or_default(value: Option<&Value>, default: i64) -> DbResult<i64> {
    match value {
        Some(value) if python_truthy(Some(value)) => value_to_i64(value)
            .ok_or_else(|| DbError::InvalidOperation("value must be integer".to_string())),
        _ => Ok(default),
    }
}

fn float_or_zero(value: Option<&Value>) -> DbResult<f64> {
    match value {
        Some(value) if python_truthy(Some(value)) => value_to_f64(value)
            .ok_or_else(|| DbError::InvalidOperation("value must be number".to_string())),
        _ => Ok(0.0),
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::String(text) => text.parse::<i64>().ok(),
        Value::Bool(flag) => Some(i64::from(*flag)),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE bill_templates (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    name TEXT NOT NULL,
                    description TEXT,
                    type TEXT NOT NULL,
                    category TEXT,
                    amount REAL,
                    account TEXT,
                    counterparty TEXT,
                    tag TEXT,
                    comment TEXT,
                    is_favorite BOOLEAN DEFAULT 0,
                    use_count INTEGER DEFAULT 0,
                    last_used_at TEXT,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    destination_amount REAL DEFAULT 0,
                    hide_amount INTEGER DEFAULT 0,
                    display_order INTEGER DEFAULT 0,
                    hidden INTEGER DEFAULT 0,
                    utc_offset INTEGER DEFAULT 0
                );
                CREATE TABLE recurring_bills (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL DEFAULT 1,
                    template_id INTEGER,
                    name TEXT NOT NULL,
                    description TEXT,
                    type TEXT NOT NULL,
                    category TEXT,
                    amount REAL NOT NULL,
                    account TEXT,
                    counterparty TEXT,
                    tag TEXT,
                    comment TEXT,
                    frequency TEXT NOT NULL,
                    start_date TEXT NOT NULL,
                    end_date TEXT,
                    next_date TEXT NOT NULL,
                    enabled BOOLEAN DEFAULT 1,
                    auto_create BOOLEAN DEFAULT 0,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    destination_amount REAL DEFAULT 0,
                    hide_amount INTEGER DEFAULT 0,
                    display_order INTEGER DEFAULT 0,
                    hidden INTEGER DEFAULT 0,
                    utc_offset INTEGER DEFAULT 0,
                    scheduled_frequency_type INTEGER DEFAULT 0
                );
            "#,
            )
            .unwrap();
        connection
    }

    #[test]
    fn taxonomy_templates_crud_serializes_normal_and_recurring_templates() {
        let mut connection = create_connection();
        let mut repository = TemplatesRepository::new(&mut connection);

        let normal_id = repository
            .create_template(
                &serde_json::json!({
                    "templateType": 1,
                    "name": "普通模板",
                    "type": "支出",
                    "categoryId": "101",
                    "sourceAccountId": "11",
                    "destinationAccountId": "0",
                    "sourceAmount": 12345,
                    "destinationAmount": 0,
                    "hideAmount": false,
                    "tagIds": ["1", "2"],
                    "comment": "普通备注",
                    "hidden": false,
                    "utcOffset": 480
                }),
                7,
            )
            .unwrap();
        let recurring_id = repository
            .create_template(
                &serde_json::json!({
                    "templateType": 2,
                    "name": "定时模板",
                    "type": 3,
                    "categoryId": "201",
                    "sourceAccountId": "21",
                    "destinationAccountId": "0",
                    "sourceAmount": 1000,
                    "destinationAmount": 0,
                    "hideAmount": true,
                    "tagIds": ["9"],
                    "comment": "定时备注",
                    "scheduledFrequencyType": 2,
                    "scheduledFrequency": "1,15",
                    "scheduledStartDate": "2026-03-01",
                    "scheduledEndDate": "2026-12-31",
                    "utcOffset": 480
                }),
                7,
            )
            .unwrap();

        let normal = repository
            .get_template_by_id(normal_id, 7, Some(1))
            .unwrap()
            .unwrap();
        assert_eq!(
            normal.get("id"),
            Some(&Value::String(normal_id.to_string()))
        );
        assert_eq!(
            normal.get("templateType"),
            Some(&Value::Number(Number::from(1)))
        );
        assert_eq!(normal.get("type"), Some(&Value::Number(Number::from(3))));
        assert_eq!(normal.get("tagIds"), Some(&serde_json::json!(["1", "2"])));
        assert_eq!(normal.get("scheduledFrequencyType"), Some(&Value::Null));

        let recurring = repository
            .get_template_by_id(recurring_id, 7, Some(2))
            .unwrap()
            .unwrap();
        assert_eq!(
            recurring.get("templateType"),
            Some(&Value::Number(Number::from(2)))
        );
        assert_eq!(recurring.get("hideAmount"), Some(&Value::Bool(true)));
        assert_eq!(
            recurring.get("scheduledFrequency"),
            Some(&Value::String("1,15".to_string()))
        );
        assert_eq!(
            recurring.get("scheduledStartDate"),
            Some(&Value::String("2026-03-01".to_string()))
        );

        assert_eq!(repository.list_templates(7, Some(1)).unwrap().len(), 1);
        assert_eq!(repository.list_templates(7, Some(2)).unwrap().len(), 1);
        assert_eq!(repository.list_templates(7, None).unwrap().len(), 2);
        assert!(repository.list_templates(8, None).unwrap().is_empty());
    }

    #[test]
    fn taxonomy_templates_update_delete_orders_and_enabled_reads_are_user_scoped() {
        let mut connection = create_connection();
        let mut repository = TemplatesRepository::new(&mut connection);

        let first_id = repository
            .create_template(
                &serde_json::json!({"templateType": 1, "name": "A", "type": 3, "sourceAmount": 1}),
                7,
            )
            .unwrap();
        let second_id = repository
            .create_template(
                &serde_json::json!({"templateType": 1, "name": "B", "type": 3, "sourceAmount": 2}),
                7,
            )
            .unwrap();
        assert!(repository
            .update_template(
                first_id,
                &serde_json::json!({"name": "A2", "sourceAmount": 42, "tagIds": ["7"], "hidden": true}),
                7,
                Some(1),
            )
            .unwrap());
        assert!(!repository
            .update_template(first_id, &serde_json::json!({"name": "other"}), 8, Some(1))
            .unwrap());
        assert!(repository
            .update_display_orders(
                &[
                    TemplateDisplayOrder {
                        template_id: first_id,
                        display_order: 2,
                    },
                    TemplateDisplayOrder {
                        template_id: second_id,
                        display_order: 1,
                    },
                ],
                1,
                7,
            )
            .unwrap());

        let ordered = repository.list_templates(7, Some(1)).unwrap();
        assert_eq!(
            ordered[0].get("name"),
            Some(&Value::String("B".to_string()))
        );
        assert_eq!(
            ordered[1].get("name"),
            Some(&Value::String("A2".to_string()))
        );
        assert!(repository.delete_template(first_id, 7, Some(1)).unwrap());
        assert!(!repository.delete_template(first_id, 7, Some(1)).unwrap());

        let recurring_id = repository
            .create_template(
                &serde_json::json!({
                    "templateType": 2,
                    "name": "R",
                    "type": 3,
                    "sourceAmount": 1200,
                    "scheduledFrequencyType": 1,
                    "scheduledFrequency": "1",
                    "scheduledStartDate": "2026-03-02"
                }),
                7,
            )
            .unwrap();
        assert!(repository
            .update_template(
                recurring_id,
                &serde_json::json!({"scheduledStartDate": "2026-03-09", "scheduledFrequency": "2"}),
                7,
                Some(2),
            )
            .unwrap());
        let enabled = repository.list_enabled_recurring_templates(7).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(
            enabled[0].get("next_date"),
            Some(&Value::String("2026-03-09".to_string()))
        );
        assert_eq!(
            enabled[0].get("frequency"),
            Some(&Value::String("2".to_string()))
        );
    }

    #[test]
    fn taxonomy_templates_display_order_parser_validates_shape() {
        assert_eq!(
            parse_template_display_orders(&serde_json::json!([[1, 2]])).unwrap(),
            vec![TemplateDisplayOrder {
                template_id: 1,
                display_order: 2,
            }]
        );
        assert!(parse_template_display_orders(&serde_json::json!({"bad": true})).is_err());
        assert!(parse_template_display_orders(&serde_json::json!([[1]])).is_err());
    }
}
