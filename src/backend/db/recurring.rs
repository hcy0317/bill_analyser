// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::BTreeSet;

use bill_analyser_core::{matching::RecurringPattern, UserId};
use chrono::{SecondsFormat, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

use crate::{run_transaction, DbError, DbResult, UserScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurringSuggestionSaveSummary {
    pub created: i64,
    pub updated: i64,
    pub skipped: i64,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn init_recurring_runtime_schema(connection: &Connection) -> DbResult<()> {
    ensure_recurring_suggestions_schema(connection)?;
    ensure_recurring_bills_schema(connection)?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn count_recurring_suggestions(
    connection: &Connection,
    user_id: UserId,
    status: Option<&str>,
) -> DbResult<i64> {
    ensure_recurring_suggestions_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut values = vec![SqlValue::Integer(user_id)];
    let status_clause = if let Some(status) = non_empty_text(status) {
        values.push(SqlValue::Text(status.to_string()));
        " AND status = ?"
    } else {
        ""
    };
    let sql =
        format!("SELECT COUNT(*) FROM recurring_suggestions WHERE user_id = ?{status_clause}");
    connection
        .query_row(&sql, params_from_iter(values), |row| row.get::<_, i64>(0))
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_recurring_suggestions(
    connection: &Connection,
    user_id: UserId,
    status: Option<&str>,
    limit: usize,
    offset: usize,
) -> DbResult<Vec<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "list_recurring_suggestions",
        "business operation entered"
    );
    ensure_recurring_suggestions_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut values = vec![SqlValue::Integer(user_id)];
    let status_clause = if let Some(status) = non_empty_text(status) {
        values.push(SqlValue::Text(status.to_string()));
        " AND status = ?"
    } else {
        ""
    };
    values.push(SqlValue::Integer(limit as i64));
    values.push(SqlValue::Integer(offset as i64));
    let sql = format!(
        "
        SELECT id, user_id, pattern_hash, name, description, type, amount,
               source_account_id, destination_account_id, counterparty, frequency,
               detected_interval_days, confidence_score, sample_count,
               sample_bill_ids_json, first_occurrence, last_occurrence,
               suggested_next_date, status, created_at, updated_at
        FROM recurring_suggestions
        WHERE user_id = ?{status_clause}
        ORDER BY confidence_score DESC, last_occurrence DESC
        LIMIT ? OFFSET ?
        "
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), recurring_suggestion_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn detect_and_save_recurring_suggestions(
    connection: &Connection,
    user_id: UserId,
    patterns: &[RecurringPattern],
) -> DbResult<RecurringSuggestionSaveSummary> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "detect_and_save_recurring_suggestions",
        "business operation entered"
    );
    ensure_recurring_suggestions_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let now = utc_now_iso();
    let mut summary = RecurringSuggestionSaveSummary {
        created: 0,
        updated: 0,
        skipped: 0,
    };

    for pattern in patterns {
        let existing = connection
            .query_row(
                "SELECT id, status FROM recurring_suggestions WHERE user_id = ? AND pattern_hash = ?",
                params![user_id, pattern.pattern_hash],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let sample_ids_json =
            serde_json::to_string(&pattern.sample_bill_ids).unwrap_or_else(|_| "[]".to_string());

        if let Some((suggestion_id, status)) = existing {
            if matches!(status.as_str(), "accepted" | "rejected") {
                summary.skipped += 1;
                continue;
            }
            connection.execute(
                "
                UPDATE recurring_suggestions SET
                    name = ?, description = ?, type = ?, amount = ?,
                    source_account_id = ?, destination_account_id = ?,
                    counterparty = ?, frequency = ?, detected_interval_days = ?,
                    confidence_score = ?, sample_count = ?, sample_bill_ids_json = ?,
                    first_occurrence = ?, last_occurrence = ?, suggested_next_date = ?,
                    updated_at = ?
                WHERE id = ?
                ",
                params![
                    pattern.name,
                    pattern.description,
                    pattern.transaction_type,
                    pattern.amount,
                    pattern.source_account_id,
                    pattern.destination_account_id,
                    pattern.counterparty,
                    pattern.frequency,
                    pattern.detected_interval_days,
                    pattern.confidence_score,
                    pattern.sample_count as i64,
                    sample_ids_json,
                    pattern.first_occurrence,
                    pattern.last_occurrence,
                    pattern.suggested_next_date,
                    now,
                    suggestion_id,
                ],
            )?;
            summary.updated += 1;
        } else {
            connection.execute(
                "
                INSERT INTO recurring_suggestions
                    (user_id, pattern_hash, name, description, type, amount,
                     source_account_id, destination_account_id, counterparty,
                     frequency, detected_interval_days, confidence_score,
                     sample_count, sample_bill_ids_json, first_occurrence,
                     last_occurrence, suggested_next_date, status,
                     created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)
                ",
                params![
                    user_id,
                    pattern.pattern_hash,
                    pattern.name,
                    pattern.description,
                    pattern.transaction_type,
                    pattern.amount,
                    pattern.source_account_id,
                    pattern.destination_account_id,
                    pattern.counterparty,
                    pattern.frequency,
                    pattern.detected_interval_days,
                    pattern.confidence_score,
                    pattern.sample_count as i64,
                    sample_ids_json,
                    pattern.first_occurrence,
                    pattern.last_occurrence,
                    pattern.suggested_next_date,
                    now,
                    now,
                ],
            )?;
            summary.created += 1;
        }
    }

    Ok(summary)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn accept_recurring_suggestion(
    connection: &mut Connection,
    user_id: UserId,
    suggestion_id: i64,
) -> DbResult<Option<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "accept_recurring_suggestion",
        "business operation entered"
    );
    init_recurring_runtime_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let now = utc_now_iso();
    run_transaction(connection, |tx| {
        let suggestion = tx
            .query_row(
                "
                SELECT id, name, description, type, amount, source_account_id,
                       counterparty, frequency, first_occurrence, suggested_next_date
                FROM recurring_suggestions
                WHERE id = ? AND user_id = ? AND status = 'pending'
                ",
                params![suggestion_id, user_id],
                suggestion_accept_row,
            )
            .optional()?;
        let Some(suggestion) = suggestion else {
            return Ok(None);
        };
        let changed = tx.execute(
            "
            UPDATE recurring_suggestions
            SET status = 'accepted', updated_at = ?
            WHERE id = ? AND user_id = ? AND status = 'pending'
            ",
            params![now, suggestion_id, user_id],
        )?;
        if changed == 0 {
            return Ok(None);
        }

        tx.execute(
            "
            INSERT INTO recurring_bills
                (user_id, name, description, type, amount, account, counterparty,
                 frequency, start_date, next_date, enabled, auto_create,
                 created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 0, ?, ?)
            ",
            params![
                user_id,
                suggestion.name,
                suggestion.description.unwrap_or_default(),
                suggestion.transaction_type,
                suggestion.amount,
                suggestion
                    .source_account_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                suggestion.counterparty.unwrap_or_default(),
                suggestion.frequency,
                suggestion
                    .first_occurrence
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| now[..10].to_string()),
                suggestion
                    .suggested_next_date
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| now[..10].to_string()),
                now,
                now,
            ],
        )?;
        let recurring_id = tx.last_insert_rowid();

        Ok(Some(json!({
            "recurring_id": recurring_id,
            "suggestion_id": suggestion_id,
            "status": "accepted",
        })))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn reject_recurring_suggestion(
    connection: &mut Connection,
    user_id: UserId,
    suggestion_id: i64,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reject_recurring_suggestion",
        "business operation entered"
    );
    ensure_recurring_suggestions_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let now = utc_now_iso();
    run_transaction(connection, |tx| {
        let changed = tx.execute(
            "
            UPDATE recurring_suggestions
            SET status = 'rejected', updated_at = ?
            WHERE id = ? AND user_id = ? AND status = 'pending'
            ",
            params![now, suggestion_id, user_id],
        )?;
        Ok(changed > 0)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_bills_linked_to_recurring(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<BTreeSet<i64>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    if !table_exists(connection, "bills")?
        || !column_exists(connection, "bills", "created_from_recurring")?
    {
        return Ok(BTreeSet::new());
    }
    let mut statement = connection.prepare(
        "
        SELECT id
        FROM bills
        WHERE user_id = ? AND created_from_recurring IS NOT NULL AND created_from_recurring > 0
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| row.get::<_, i64>(0))?;
    rows.collect::<Result<BTreeSet<_>, _>>()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_recent_bills_for_recurring_detection(
    connection: &Connection,
    user_id: UserId,
    limit: usize,
) -> DbResult<Vec<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "list_recent_bills_for_recurring_detection",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    if !table_exists(connection, "bills")? {
        return Ok(Vec::new());
    }
    let expressions = BillColumnExpressions::for_table(connection)?;
    let sql = format!(
        "
        SELECT id, date, type, amount,
               {counterparty} AS counterparty,
               {description} AS description,
               {main_category} AS main_category,
               {sub_category} AS sub_category,
               {source_account_id} AS source_account_id,
               {destination_account_id} AS destination_account_id
        FROM bills
        WHERE user_id = ?
        ORDER BY date DESC, id DESC
        LIMIT ?
        ",
        counterparty = expressions.counterparty,
        description = expressions.description,
        main_category = expressions.main_category,
        sub_category = expressions.sub_category,
        source_account_id = expressions.source_account_id,
        destination_account_id = expressions.destination_account_id,
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![user_id, limit as i64], recent_bill_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[derive(Debug)]
struct SuggestionForAccept {
    name: String,
    description: Option<String>,
    transaction_type: String,
    amount: f64,
    source_account_id: Option<i64>,
    counterparty: Option<String>,
    frequency: String,
    first_occurrence: Option<String>,
    suggested_next_date: Option<String>,
}

fn suggestion_accept_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SuggestionForAccept> {
    Ok(SuggestionForAccept {
        name: row.get::<_, String>("name")?,
        description: row.get::<_, Option<String>>("description")?,
        transaction_type: row.get::<_, String>("type")?,
        amount: row.get::<_, f64>("amount")?,
        source_account_id: positive_i64(row.get::<_, Option<i64>>("source_account_id")?),
        counterparty: row.get::<_, Option<String>>("counterparty")?,
        frequency: row.get::<_, String>("frequency")?,
        first_occurrence: row.get::<_, Option<String>>("first_occurrence")?,
        suggested_next_date: row.get::<_, Option<String>>("suggested_next_date")?,
    })
}

#[derive(Debug)]
struct BillColumnExpressions {
    counterparty: String,
    description: String,
    main_category: String,
    sub_category: String,
    source_account_id: String,
    destination_account_id: String,
}

impl BillColumnExpressions {
    fn for_table(connection: &Connection) -> DbResult<Self> {
        Ok(Self {
            counterparty: optional_column_expr(connection, "bills", "counterparty", "''")?,
            description: optional_column_expr(connection, "bills", "description", "''")?,
            main_category: optional_column_expr(connection, "bills", "main_category", "''")?,
            sub_category: optional_column_expr(connection, "bills", "sub_category", "''")?,
            source_account_id: optional_column_expr(
                connection,
                "bills",
                "source_account_id",
                "NULL",
            )?,
            destination_account_id: optional_column_expr(
                connection,
                "bills",
                "destination_account_id",
                "NULL",
            )?,
        })
    }
}

fn recent_bill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, Option<i64>>("id")?.unwrap_or(0),
        "date": row.get::<_, Option<String>>("date")?.unwrap_or_default(),
        "type": row.get::<_, Option<String>>("type")?.unwrap_or_default(),
        "amount": row.get::<_, Option<f64>>("amount")?.unwrap_or(0.0),
        "counterparty": row.get::<_, Option<String>>("counterparty")?.unwrap_or_default(),
        "description": row.get::<_, Option<String>>("description")?.unwrap_or_default(),
        "main_category": row.get::<_, Option<String>>("main_category")?.unwrap_or_default(),
        "sub_category": row.get::<_, Option<String>>("sub_category")?.unwrap_or_default(),
        "source_account_id": row.get::<_, Option<i64>>("source_account_id")?,
        "destination_account_id": row.get::<_, Option<i64>>("destination_account_id")?,
    }))
}

fn recurring_suggestion_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut item = Map::new();
    for key in [
        "id",
        "user_id",
        "pattern_hash",
        "name",
        "description",
        "type",
        "amount",
        "source_account_id",
        "destination_account_id",
        "counterparty",
        "frequency",
        "detected_interval_days",
        "confidence_score",
        "sample_count",
        "sample_bill_ids_json",
        "first_occurrence",
        "last_occurrence",
        "suggested_next_date",
        "status",
        "created_at",
        "updated_at",
    ] {
        item.insert(key.to_string(), sql_value_ref_to_json(row.get_ref(key)?));
    }
    let sample_bill_ids = item
        .get("sample_bill_ids_json")
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .unwrap_or_else(|| json!([]));
    item.insert("sample_bill_ids".to_string(), sample_bill_ids);
    Ok(Value::Object(item))
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_recurring_suggestions_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS recurring_suggestions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            pattern_hash TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            source_account_id INTEGER,
            destination_account_id TEXT,
            counterparty TEXT,
            frequency TEXT NOT NULL,
            detected_interval_days REAL,
            confidence_score REAL NOT NULL DEFAULT 0,
            sample_count INTEGER NOT NULL DEFAULT 0,
            sample_bill_ids_json TEXT,
            first_occurrence TEXT,
            last_occurrence TEXT,
            suggested_next_date TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(user_id, pattern_hash),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_recurring_suggestions_user_status
            ON recurring_suggestions(user_id, status);
        ",
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_recurring_bills_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bill_templates (
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
            utc_offset INTEGER DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS recurring_bills (
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
            scheduled_frequency_type INTEGER DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY (template_id) REFERENCES bill_templates(id) ON DELETE SET NULL
        );
        ",
    )?;
    for (table, column, definition) in [
        ("bill_templates", "description", "TEXT"),
        ("bill_templates", "type", "TEXT DEFAULT ''"),
        ("bill_templates", "category", "TEXT"),
        ("bill_templates", "amount", "REAL"),
        ("bill_templates", "account", "TEXT"),
        ("bill_templates", "counterparty", "TEXT"),
        ("bill_templates", "tag", "TEXT"),
        ("bill_templates", "comment", "TEXT"),
        ("bill_templates", "is_favorite", "BOOLEAN DEFAULT 0"),
        ("bill_templates", "use_count", "INTEGER DEFAULT 0"),
        ("bill_templates", "last_used_at", "TEXT"),
        ("bill_templates", "created_at", "TEXT"),
        ("bill_templates", "updated_at", "TEXT"),
        ("bill_templates", "destination_amount", "REAL DEFAULT 0"),
        ("bill_templates", "hide_amount", "INTEGER DEFAULT 0"),
        ("bill_templates", "display_order", "INTEGER DEFAULT 0"),
        ("bill_templates", "hidden", "INTEGER DEFAULT 0"),
        ("bill_templates", "utc_offset", "INTEGER DEFAULT 0"),
        ("recurring_bills", "template_id", "INTEGER"),
        ("recurring_bills", "description", "TEXT"),
        ("recurring_bills", "category", "TEXT"),
        ("recurring_bills", "account", "TEXT"),
        ("recurring_bills", "counterparty", "TEXT"),
        ("recurring_bills", "tag", "TEXT"),
        ("recurring_bills", "comment", "TEXT"),
        ("recurring_bills", "end_date", "TEXT"),
        ("recurring_bills", "enabled", "BOOLEAN DEFAULT 1"),
        ("recurring_bills", "auto_create", "BOOLEAN DEFAULT 0"),
        ("recurring_bills", "created_at", "TEXT"),
        ("recurring_bills", "updated_at", "TEXT"),
        ("recurring_bills", "destination_amount", "REAL DEFAULT 0"),
        ("recurring_bills", "hide_amount", "INTEGER DEFAULT 0"),
        ("recurring_bills", "display_order", "INTEGER DEFAULT 0"),
        ("recurring_bills", "hidden", "INTEGER DEFAULT 0"),
        ("recurring_bills", "utc_offset", "INTEGER DEFAULT 0"),
        (
            "recurring_bills",
            "scheduled_frequency_type",
            "INTEGER DEFAULT 0",
        ),
    ] {
        add_column_if_missing(connection, table, column, definition)?;
    }
    connection.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_templates_user ON bill_templates(user_id);
        CREATE INDEX IF NOT EXISTS idx_templates_favorite ON bill_templates(is_favorite, use_count DESC);
        CREATE INDEX IF NOT EXISTS idx_templates_type ON bill_templates(type);
        CREATE INDEX IF NOT EXISTS idx_recurring_bills_user ON recurring_bills(user_id);
        CREATE INDEX IF NOT EXISTS idx_recurring_bills_next_date ON recurring_bills(next_date);
        CREATE INDEX IF NOT EXISTS idx_recurring_bills_enabled ON recurring_bills(enabled, next_date);
        ",
    )?;
    Ok(())
}

fn add_column_if_missing(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> DbResult<()> {
    if column_exists(connection, table, column)? {
        return Ok(());
    }
    connection.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
            params![table],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(DbError::from)
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> DbResult<bool> {
    Ok(table_column_names(connection, table)?
        .iter()
        .any(|name| name == column))
}

fn optional_column_expr(
    connection: &Connection,
    table: &str,
    column: &str,
    fallback: &str,
) -> DbResult<String> {
    Ok(if column_exists(connection, table, column)? {
        column.to_string()
    } else {
        fallback.to_string()
    })
}

fn table_column_names(connection: &Connection, table: &str) -> DbResult<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    rows.collect::<Result<BTreeSet<_>, _>>()
        .map_err(DbError::from)
}

fn sql_value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
    }
}

fn non_empty_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn positive_i64(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

fn utc_now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
