// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::{UserDataStatisticsContract, UserId};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, Transaction};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, Row};

use crate::{bills, run_transaction, DbResult, PostgresPool, UserScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDataExportCategory {
    pub id: i64,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserDataExportBundle {
    pub bills: Vec<bills::BillRecord>,
    pub account_names: BTreeMap<i64, String>,
    pub tag_names_by_bill: BTreeMap<i64, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDataClearAllResult {
    pub counts: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresUserDataAuditEvent<'a> {
    pub operation_type: &'a str,
    pub user_id: UserId,
    pub details: Value,
    pub affected_count: i64,
    pub ip_address: &'a str,
    pub user_agent: &'a str,
    pub now: &'a str,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_user_data_statistics(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<UserDataStatisticsContract> {
    let user_id = UserScope::new(user_id).bind_value()?;
    Ok(UserDataStatisticsContract {
        bill_count: count_user_rows(connection, "bills", user_id)?,
        account_count: count_user_rows(connection, "accounts", user_id)?,
        category_count: count_user_rows(connection, "categories", user_id)?,
        tag_count: count_user_rows(connection, "tags", user_id)?,
        template_count: count_user_rows(connection, "bill_templates", user_id)?,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_user_data_statistics(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<UserDataStatisticsContract> {
    let user_id = UserScope::new(user_id).bind_value()?;
    Ok(UserDataStatisticsContract {
        bill_count: count_postgres_user_rows(pool, "bills", user_id).await?,
        account_count: count_postgres_user_rows(pool, "accounts", user_id).await?,
        category_count: count_postgres_user_rows(pool, "categories", user_id).await?,
        tag_count: count_postgres_user_rows(pool, "tags", user_id).await?,
        template_count: count_postgres_user_rows(pool, "transaction_templates", user_id).await?,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn count_user_rows(connection: &Connection, table_name: &str, user_id: i64) -> DbResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table_name} WHERE user_id = ?1");
    connection
        .query_row(&sql, params![user_id], |row| row.get::<_, i64>(0))
        .map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn count_postgres_user_rows(
    pool: &PostgresPool,
    table_name: &'static str,
    user_id: i64,
) -> DbResult<i64> {
    let sql = format!("SELECT COUNT(*)::BIGINT FROM {table_name} WHERE user_id = $1");
    sqlx::query_scalar(&sql)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_user_data_categories(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<UserDataExportCategory>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "list_user_data_categories",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut statement = connection.prepare(
        "SELECT id, main_category, sub_category
         FROM categories
         WHERE user_id = ?1
         ORDER BY priority ASC, main_category, sub_category",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(UserDataExportCategory {
            id: row.get::<_, i64>(0)?,
            main_category: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            sub_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_user_data_categories(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<UserDataExportCategory>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let rows = sqlx::query(
        r#"
        SELECT id, path, name
        FROM categories
        WHERE user_id = $1
        ORDER BY display_order ASC, path ASC, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id")?;
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) = postgres_category_names(path.as_deref(), &name);
            Ok(UserDataExportCategory {
                id,
                main_category,
                sub_category,
            })
        })
        .collect::<DbResult<Vec<_>>>()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn load_user_data_export(
    connection: &Connection,
    user_id: UserId,
    filters: &bills::BillFilters,
) -> DbResult<UserDataExportBundle> {
    let bills = bills::list_bills(connection, user_id, filters)?;
    let account_names = load_account_names(connection, user_id)?;
    let bill_ids = bills
        .iter()
        .filter_map(|bill| bill.get("id").and_then(serde_json::Value::as_i64))
        .collect::<Vec<_>>();
    let tag_names_by_bill = load_tag_names_for_bills(connection, user_id, &bill_ids)?;
    Ok(UserDataExportBundle {
        bills,
        account_names,
        tag_names_by_bill,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn load_postgres_user_data_export(
    pool: &PostgresPool,
    user_id: UserId,
    filters: &bills::BillFilters,
) -> DbResult<UserDataExportBundle> {
    let user_id_sql = UserScope::new(user_id).bind_value()?;
    let mut page = 1_usize;
    let mut bills = Vec::new();
    loop {
        let result =
            bills::postgres_reads::query_postgres_bills(pool, user_id_sql, page, 500, filters)
                .await?;
        let total = result.total.max(0) as usize;
        let page_len = result.bills.len();
        bills.extend(result.bills);
        if page_len == 0 || bills.len() >= total {
            break;
        }
        page += 1;
    }
    let account_names = load_postgres_account_names(pool, user_id).await?;
    let bill_ids = bills
        .iter()
        .filter_map(|bill| bill.get("id").and_then(Value::as_i64))
        .collect::<Vec<_>>();
    let tag_names_by_bill = load_postgres_tag_names_for_bills(pool, user_id, &bill_ids).await?;
    Ok(UserDataExportBundle {
        bills,
        account_names,
        tag_names_by_bill,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn clear_user_transactions(connection: &mut Connection, user_id: UserId) -> DbResult<i64> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "clear_user_transactions",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let bill_ids = list_user_bill_ids(tx, user_id)?;
        let deleted_count = i64::try_from(bill_ids.len()).unwrap_or(i64::MAX);
        if bill_ids.is_empty() {
            return Ok(0);
        }
        delete_bill_side_effects(tx, user_id, &bill_ids)?;
        delete_bill_tags(tx, &bill_ids)?;
        tx.execute("DELETE FROM bills WHERE user_id = ?1", params![user_id])?;
        let now = now_text();
        if table_exists(tx, "accounts")? {
            tx.execute(
                "UPDATE accounts SET balance = initial_balance, updated_at = ?1 WHERE user_id = ?2",
                params![now, user_id],
            )?;
        }
        Ok(deleted_count)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn clear_postgres_user_transactions(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<i64> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut transaction = pool.begin().await?;
    let deleted_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await?;
    sqlx::query("DELETE FROM bills WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE accounts SET balance_cents = 0, updated_at = now() WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(deleted_count)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn clear_user_data(
    connection: &mut Connection,
    user_id: UserId,
) -> DbResult<UserDataClearAllResult> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "clear_user_data",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let counts = clear_all_counts(tx, user_id)?;
        let bill_ids = list_user_bill_ids(tx, user_id)?;
        delete_bill_side_effects(tx, user_id, &bill_ids)?;
        delete_if_table_exists(tx, "bills_preview", user_id)?;
        delete_if_table_exists(tx, "bills_parser_template", user_id)?;
        delete_if_table_exists(tx, "import_annotation_samples", user_id)?;
        delete_if_table_exists(tx, "llm_memory_events", user_id)?;
        delete_if_table_exists(tx, "import_sessions", user_id)?;
        delete_bill_tags(tx, &bill_ids)?;
        delete_if_table_exists(tx, "bills", user_id)?;
        if table_exists(tx, "budget_history")? {
            tx.execute(
                "DELETE FROM budget_history WHERE budget_id IN (SELECT id FROM budgets WHERE user_id = ?1)",
                params![user_id],
            )?;
        }
        delete_if_table_exists(tx, "budgets", user_id)?;
        delete_if_table_exists(tx, "recurring_bills", user_id)?;
        delete_if_table_exists(tx, "bill_templates", user_id)?;
        delete_if_table_exists(tx, "saved_filters", user_id)?;
        delete_if_table_exists(tx, "account_transfers", user_id)?;
        delete_if_table_exists(tx, "tags", user_id)?;
        delete_if_table_exists(tx, "account_rules", user_id)?;
        delete_if_table_exists(tx, "category_rules", user_id)?;
        delete_if_table_exists(tx, "categories", user_id)?;
        delete_if_table_exists(tx, "account_types", user_id)?;
        delete_if_table_exists(tx, "accounts", user_id)?;
        Ok(UserDataClearAllResult { counts })
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn clear_postgres_user_data(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<UserDataClearAllResult> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut transaction = pool.begin().await?;
    let counts = postgres_clear_all_counts(&mut transaction, user_id).await?;
    for table_name in [
        "vector_outbox_events",
        "import_learning_feedback_events",
        "import_learning_features",
        "import_learning_suggestions",
        "import_learning_lifecycle",
        "import_learning_suppressions",
        "import_learning_samples",
        "matching_feedback_events",
        "matching_pairs",
        "matching_suppressions",
        "preview_matching_feedback",
        "recurring_suggestions",
        "import_confirm_operations",
        "import_history_materializations",
        "import_decision_groups",
        "import_sessions",
        "parser_templates",
        "transaction_templates",
        "budget_history",
        "budgets",
        "bill_tags",
        "bills",
        "account_rules",
        "category_rules",
        "tags",
        "account_aliases_legacy",
        "categories",
        "accounts",
    ] {
        delete_postgres_user_rows(&mut transaction, table_name, user_id).await?;
    }
    transaction.commit().await?;
    Ok(UserDataClearAllResult { counts })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_user_data_audit_event(
    pool: &PostgresPool,
    draft: PostgresUserDataAuditEvent<'_>,
) -> DbResult<i64> {
    let user_id = UserScope::new(draft.user_id).bind_value()?;
    let row = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor, metadata, created_at
        ) VALUES ($1, 'user_data', $2, $3, 'runtime', $4, $5::timestamptz)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(draft.user_id.get().to_string())
    .bind(draft.operation_type)
    .bind(serde_json::json!({
        "details": draft.details,
        "affected_count": draft.affected_count,
        "ip_address": draft.ip_address,
        "user_agent": draft.user_agent,
        "status": "success"
    }))
    .bind(draft.now)
    .fetch_one(pool)
    .await?;
    row.try_get("id").map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_account_names(connection: &Connection, user_id: UserId) -> DbResult<BTreeMap<i64, String>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut statement =
        connection.prepare("SELECT id, name FROM accounts WHERE user_id = ?1 ORDER BY id ASC")?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut account_names = BTreeMap::new();
    for row in rows {
        let (id, name) = row?;
        account_names.insert(id, name);
    }
    Ok(account_names)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_tag_names_for_bills(
    connection: &Connection,
    user_id: UserId,
    bill_ids: &[i64],
) -> DbResult<BTreeMap<i64, Vec<String>>> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    let placeholders = placeholders(bill_ids.len());
    let mut sql_params = vec![SqlValue::Integer(user_id)];
    sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    let mut statement = connection.prepare(&format!(
        "SELECT bt.bill_id, t.name
         FROM tags t
         JOIN bill_tags bt ON t.id = bt.tag_id
         WHERE t.user_id = ?1 AND bt.bill_id IN ({placeholders})
         ORDER BY t.name"
    ))?;
    let rows = statement.query_map(params_from_iter(sql_params), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut tags = BTreeMap::<i64, Vec<String>>::new();
    for row in rows {
        let (bill_id, name) = row?;
        tags.entry(bill_id).or_default().push(name);
    }
    Ok(tags)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_account_names(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<BTreeMap<i64, String>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let rows = sqlx::query("SELECT id, name FROM accounts WHERE user_id = $1 ORDER BY id ASC")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    let mut account_names = BTreeMap::new();
    for row in rows {
        account_names.insert(row.try_get("id")?, row.try_get("name")?);
    }
    Ok(account_names)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_tag_names_for_bills(
    pool: &PostgresPool,
    user_id: UserId,
    bill_ids: &[i64],
) -> DbResult<BTreeMap<i64, Vec<String>>> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT bt.bill_id, t.name FROM bill_tags bt JOIN tags t ON t.user_id = bt.user_id AND t.id = bt.tag_id WHERE bt.user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" AND bt.bill_id IN (");
    for (index, bill_id) in bill_ids.iter().enumerate() {
        if index > 0 {
            builder.push(", ");
        }
        builder.push_bind(*bill_id);
    }
    builder.push(") ORDER BY t.name ASC");
    let rows = builder.build().fetch_all(pool).await?;
    let mut tags = BTreeMap::<i64, Vec<String>>::new();
    for row in rows {
        let bill_id: i64 = row.try_get("bill_id")?;
        let name: String = row.try_get("name")?;
        tags.entry(bill_id).or_default().push(name);
    }
    Ok(tags)
}

async fn postgres_clear_all_counts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
) -> DbResult<BTreeMap<String, i64>> {
    let mut counts = BTreeMap::new();
    for (key, table_name) in [
        ("bills", "bills"),
        ("accounts", "accounts"),
        ("categories", "categories"),
        ("tags", "tags"),
        ("account_rules", "account_rules"),
        ("category_rules", "category_rules"),
        ("templates", "transaction_templates"),
        ("recurring_bills", "recurring_suggestions"),
        ("budgets", "budgets"),
    ] {
        let sql = format!("SELECT COUNT(*)::BIGINT FROM {table_name} WHERE user_id = $1");
        let count = sqlx::query_scalar(&sql)
            .bind(user_id)
            .fetch_one(&mut **transaction)
            .await?;
        counts.insert(key.to_string(), count);
    }
    Ok(counts)
}

async fn delete_postgres_user_rows(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table_name: &'static str,
    user_id: i64,
) -> DbResult<()> {
    let sql = format!("DELETE FROM {table_name} WHERE user_id = $1");
    sqlx::query(&sql)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

fn postgres_category_names(path: Option<&str>, name: &str) -> (String, String) {
    let parts = path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn clear_all_counts(tx: &Transaction<'_>, user_id: i64) -> DbResult<BTreeMap<String, i64>> {
    let mut counts = BTreeMap::new();
    for (key, table_name) in [
        ("bills", "bills"),
        ("accounts", "accounts"),
        ("categories", "categories"),
        ("tags", "tags"),
        ("account_rules", "account_rules"),
        ("category_rules", "category_rules"),
        ("templates", "bill_templates"),
        ("recurring_bills", "recurring_bills"),
        ("budgets", "budgets"),
    ] {
        let count = if table_exists(tx, table_name)? {
            tx.query_row(
                &format!("SELECT COUNT(*) FROM {table_name} WHERE user_id = ?1"),
                params![user_id],
                |row| row.get::<_, i64>(0),
            )?
        } else {
            0
        };
        counts.insert(key.to_string(), count);
    }
    Ok(counts)
}

#[tracing::instrument(level = "debug", skip_all)]
fn list_user_bill_ids(tx: &Transaction<'_>, user_id: i64) -> DbResult<Vec<i64>> {
    if !table_exists(tx, "bills")? {
        return Ok(Vec::new());
    }
    let mut statement = tx.prepare("SELECT id FROM bills WHERE user_id = ?1")?;
    let rows = statement.query_map(params![user_id], |row| row.get::<_, i64>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_bill_side_effects(tx: &Transaction<'_>, user_id: i64, bill_ids: &[i64]) -> DbResult<()> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(());
    }
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_pair_links")?;
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_transfer_pair_suppressions")?;
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_investment_pair_suppressions")?;
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_duplicate_pair_suppressions")?;
    if table_exists(tx, "bill_learning_rule_suppressions")? {
        let placeholders = placeholders(bill_ids.len());
        let mut sql_params = vec![SqlValue::Integer(user_id)];
        sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
        tx.execute(
            &format!(
                "DELETE FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id IN ({placeholders})"
            ),
            params_from_iter(sql_params),
        )?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_pair_table_by_pair_columns(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
    table_name: &str,
) -> DbResult<()> {
    if !table_exists(tx, table_name)? {
        return Ok(());
    }
    let placeholders = placeholders(bill_ids.len());
    let mut sql_params = vec![SqlValue::Integer(user_id)];
    sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    sql_params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    tx.execute(
        &format!(
            "DELETE FROM {table_name} WHERE user_id = ? AND (left_bill_id IN ({placeholders}) OR right_bill_id IN ({placeholders}))"
        ),
        params_from_iter(sql_params),
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_bill_tags(tx: &Transaction<'_>, bill_ids: &[i64]) -> DbResult<()> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() || !table_exists(tx, "bill_tags")? {
        return Ok(());
    }
    let placeholders = placeholders(bill_ids.len());
    let sql_params = bill_ids
        .iter()
        .copied()
        .map(SqlValue::Integer)
        .collect::<Vec<_>>();
    tx.execute(
        &format!("DELETE FROM bill_tags WHERE bill_id IN ({placeholders})"),
        params_from_iter(sql_params),
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_if_table_exists(tx: &Transaction<'_>, table_name: &str, user_id: i64) -> DbResult<()> {
    if table_exists(tx, table_name)? {
        tx.execute(
            &format!("DELETE FROM {table_name} WHERE user_id = ?1"),
            params![user_id],
        )?;
    }
    Ok(())
}

fn table_exists(tx: &Transaction<'_>, table_name: &str) -> DbResult<bool> {
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_ids(values: &[i64]) -> Vec<i64> {
    values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn now_text() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(value: u64) -> UserId {
        UserId::new(value).expect("positive user id")
    }

    #[test]
    fn user_data_statistics_are_user_scoped() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE bills (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE accounts (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE categories (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE category_rules (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE account_rules (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE tags (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE bill_templates (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);

            INSERT INTO bills(id, user_id) VALUES (1, 42), (2, 42), (3, 77);
            INSERT INTO accounts(id, user_id) VALUES (10, 42), (11, 77);
            INSERT INTO categories(id, user_id) VALUES (20, 42), (21, 42), (22, 77);
            INSERT INTO tags(id, user_id) VALUES (30, 42), (31, 42), (32, 42), (33, 77);
            INSERT INTO bill_templates(id, user_id) VALUES (40, 42), (41, 77);
            "#,
        )?;

        let statistics = get_user_data_statistics(&connection, user_id(42))?;

        assert_eq!(statistics.bill_count, 2);
        assert_eq!(statistics.account_count, 1);
        assert_eq!(statistics.category_count, 2);
        assert_eq!(statistics.tag_count, 3);
        assert_eq!(statistics.template_count, 1);
        Ok(())
    }

    #[test]
    fn user_data_export_is_user_scoped_and_includes_account_and_tag_names() -> DbResult<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL,
                main_category TEXT, sub_category TEXT, priority INTEGER DEFAULT 0
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, name TEXT NOT NULL
            );
            CREATE TABLE bills (
                id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, date TEXT NOT NULL,
                type TEXT NOT NULL, amount REAL NOT NULL, counterparty TEXT NOT NULL,
                description TEXT NOT NULL, payment_method TEXT DEFAULT '',
                main_category TEXT, sub_category TEXT, batch_id TEXT, hash TEXT,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                source_account_id INTEGER DEFAULT 0, destination_account_id INTEGER DEFAULT 0,
                destination_amount REAL DEFAULT 0, created_from_template INTEGER,
                created_from_recurring INTEGER, import_history_id INTEGER
            );
            CREATE TABLE tags (
                id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, name TEXT NOT NULL
            );
            CREATE TABLE bill_tags (
                bill_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, created_at TEXT NOT NULL
            );

            INSERT INTO categories(id, user_id, main_category, sub_category, priority)
            VALUES (7, 42, '餐饮', '早餐', 0), (8, 77, '餐饮', '早餐', 0);
            INSERT INTO accounts(id, user_id, name) VALUES (10, 42, '支付宝'), (11, 42, '现金'), (12, 77, '他人账户');
            INSERT INTO bills(
                id, user_id, date, type, amount, counterparty, description, payment_method,
                main_category, sub_category, created_at, updated_at, source_account_id,
                destination_account_id, destination_amount
            ) VALUES
                (100, 42, '2026-01-02 08:00:00', '支出', -12.34, '早餐店', '豆浆', '支付宝',
                 '餐饮', '早餐', '2026-01-02T00:00:00', '2026-01-02T00:00:00', 10, 11, 0),
                (101, 77, '2026-01-02 09:00:00', '支出', -88.00, '他人', '不应导出', '现金',
                 '餐饮', '早餐', '2026-01-02T00:00:00', '2026-01-02T00:00:00', 12, 0, 0);
            INSERT INTO tags(id, user_id, name) VALUES (20, 42, '工作'), (21, 42, '早餐'), (22, 77, '他人标签');
            INSERT INTO bill_tags(bill_id, tag_id, created_at)
            VALUES (100, 20, 'now'), (100, 21, 'now'), (100, 22, 'now'), (101, 22, 'now');
            "#,
        )?;

        let categories = list_user_data_categories(&connection, user_id(42))?;
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].main_category, "餐饮");

        let bundle = load_user_data_export(
            &connection,
            user_id(42),
            &bills::BillFilters {
                categories: vec![bills::BillCategoryFilter {
                    main: "餐饮".to_string(),
                    sub: Some("早餐".to_string()),
                }],
                ..bills::BillFilters::default()
            },
        )?;

        assert_eq!(bundle.bills.len(), 1);
        assert_eq!(bundle.bills[0]["id"], 100);
        assert_eq!(
            bundle.account_names.get(&10).map(String::as_str),
            Some("支付宝")
        );
        assert_eq!(
            bundle.tag_names_by_bill.get(&100),
            Some(&vec!["工作".to_string(), "早餐".to_string()])
        );
        assert!(!bundle.tag_names_by_bill[&100].contains(&"他人标签".to_string()));
        Ok(())
    }

    #[test]
    fn user_data_clear_routes_remove_current_user_rows_and_side_effects() -> DbResult<()> {
        let mut connection = Connection::open_in_memory()?;
        connection.execute_batch(
            r#"
            CREATE TABLE bills (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE bill_tags (bill_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL,
                balance REAL DEFAULT 0, initial_balance REAL DEFAULT 0, updated_at TEXT
            );
            CREATE TABLE bill_pair_links (user_id INTEGER NOT NULL, left_bill_id INTEGER, right_bill_id INTEGER);
            CREATE TABLE bill_transfer_pair_suppressions (user_id INTEGER NOT NULL, left_bill_id INTEGER, right_bill_id INTEGER);
            CREATE TABLE bill_investment_pair_suppressions (user_id INTEGER NOT NULL, left_bill_id INTEGER, right_bill_id INTEGER);
            CREATE TABLE bill_learning_rule_suppressions (user_id INTEGER NOT NULL, bill_id INTEGER);

            INSERT INTO bills(id, user_id) VALUES (1, 42), (2, 42), (3, 77);
            INSERT INTO bill_tags(bill_id, tag_id, created_at) VALUES (1, 10, 'now'), (2, 11, 'now'), (3, 12, 'now');
            INSERT INTO accounts(id, user_id, balance, initial_balance, updated_at)
            VALUES (10, 42, 99.0, 5.0, 'old'), (11, 77, 88.0, 7.0, 'old');
            INSERT INTO bill_pair_links(user_id, left_bill_id, right_bill_id) VALUES (42, 1, 2), (77, 3, 4);
            INSERT INTO bill_transfer_pair_suppressions(user_id, left_bill_id, right_bill_id) VALUES (42, 1, 2), (77, 3, 4);
            INSERT INTO bill_investment_pair_suppressions(user_id, left_bill_id, right_bill_id) VALUES (42, 1, 2), (77, 3, 4);
            INSERT INTO bill_learning_rule_suppressions(user_id, bill_id) VALUES (42, 1), (77, 3);
            "#,
        )?;

        assert_eq!(clear_user_transactions(&mut connection, user_id(42))?, 2);
        assert_eq!(count_user_rows(&connection, "bills", 42)?, 0);
        assert_eq!(count_user_rows(&connection, "bills", 77)?, 1);
        assert_eq!(
            connection.query_row(
                "SELECT COUNT(*) FROM bill_tags WHERE bill_id IN (1, 2)",
                [],
                |row| row.get::<_, i64>(0),
            )?,
            0
        );
        assert_eq!(
            connection.query_row("SELECT balance FROM accounts WHERE id = 10", [], |row| {
                row.get::<_, f64>(0)
            })?,
            5.0
        );
        assert_eq!(count_user_rows(&connection, "bill_pair_links", 42)?, 0);
        assert_eq!(count_user_rows(&connection, "bill_pair_links", 77)?, 1);

        connection.execute_batch(
            r#"
            CREATE TABLE categories (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE category_rules (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE account_rules (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE tags (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE bill_templates (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE recurring_bills (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE budgets (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE budget_history (budget_id INTEGER NOT NULL);
            CREATE TABLE bills_preview (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE bills_parser_template (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE import_annotation_samples (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE llm_memory_events (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE import_sessions (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE saved_filters (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE account_transfers (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);
            CREATE TABLE account_types (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL);

            INSERT INTO bills(id, user_id) VALUES (4, 42);
            INSERT INTO accounts(id, user_id, balance, initial_balance, updated_at) VALUES (12, 42, 8.0, 8.0, 'old');
            INSERT INTO categories(id, user_id) VALUES (20, 42);
            INSERT INTO category_rules(id, user_id) VALUES (21, 42), (22, 77);
            INSERT INTO account_rules(id, user_id) VALUES (23, 42), (24, 77);
            INSERT INTO tags(id, user_id) VALUES (30, 42);
            INSERT INTO bill_templates(id, user_id) VALUES (40, 42);
            INSERT INTO recurring_bills(id, user_id) VALUES (50, 42);
            INSERT INTO budgets(id, user_id) VALUES (60, 42);
            INSERT INTO budget_history(budget_id) VALUES (60);
            INSERT INTO bills_preview(id, user_id) VALUES (70, 42);
            INSERT INTO bills_parser_template(id, user_id) VALUES (80, 42);
            INSERT INTO import_annotation_samples(id, user_id) VALUES (81, 42);
            INSERT INTO llm_memory_events(id, user_id) VALUES (82, 42);
            INSERT INTO import_sessions(id, user_id) VALUES ('session-42', 42);
            INSERT INTO saved_filters(id, user_id) VALUES (90, 42);
            INSERT INTO account_transfers(id, user_id) VALUES (100, 42);
            INSERT INTO account_types(id, user_id) VALUES (110, 42);
            "#,
        )?;

        let result = clear_user_data(&mut connection, user_id(42))?;
        assert_eq!(result.counts["bills"], 1);
        assert_eq!(result.counts["accounts"], 2);
        assert_eq!(result.counts["templates"], 1);
        assert_eq!(result.counts["category_rules"], 1);
        assert_eq!(result.counts["account_rules"], 1);
        assert_eq!(count_user_rows(&connection, "accounts", 42)?, 0);
        assert_eq!(count_user_rows(&connection, "categories", 42)?, 0);
        assert_eq!(count_user_rows(&connection, "category_rules", 42)?, 0);
        assert_eq!(count_user_rows(&connection, "category_rules", 77)?, 1);
        assert_eq!(count_user_rows(&connection, "account_rules", 42)?, 0);
        assert_eq!(count_user_rows(&connection, "account_rules", 77)?, 1);
        assert_eq!(count_user_rows(&connection, "budgets", 42)?, 0);
        assert_eq!(
            count_user_rows(&connection, "import_annotation_samples", 42)?,
            0
        );
        assert_eq!(count_user_rows(&connection, "llm_memory_events", 42)?, 0);
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM budget_history", [], |row| {
                row.get::<_, i64>(0)
            })?,
            0
        );
        assert_eq!(count_user_rows(&connection, "bills", 77)?, 1);
        Ok(())
    }
}
