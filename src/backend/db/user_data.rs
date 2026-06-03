// 中文导读：PostgreSQL user-data 仓储层，负责统计、导出、清空与审计事件。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑。

use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::{UserDataStatisticsContract, UserId};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, Row};

use crate::{bills, DbResult, PostgresPool, UserScope};

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
fn normalize_ids(values: &[i64]) -> Vec<i64> {
    values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
