// 中文导读：导入 Stage 2 的 PostgreSQL 上下文仓库，一次加载单批评估所需的用户域事实。
// 维护重点：本模块只负责 user-scoped SQL 与 typed row mapping，不执行分类或学习投影。
// 不变式：每次调用读取当前权威数据；返回值仅供一次 Stage 2 批次构建不可变快照。

use bill_analyser_core::{
    account_rules::AccountRuleCandidate, learning_lifecycle_signal_state, UserId,
};
use serde_json::Value;
use sqlx::Row;

use crate::{import_staging::ImportLearningLifecycleView, DbError, DbResult, PostgresPool};

#[derive(Debug, Clone)]
pub struct ImportStage2CategoryRecord {
    pub id: i64,
    pub category_type: Option<String>,
    pub path: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ImportStage2CategoryRuleRecord {
    pub id: i64,
    pub category_id: i64,
    pub priority: i32,
    pub rule_expression: Value,
}

#[derive(Debug, Clone)]
pub struct ImportStage2AccountRecord {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ImportStage2LearningRuleRecord {
    pub id: i64,
    pub recommendation_type: String,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct ImportStage2RecurringTemplateRecord {
    pub id: i64,
    pub name: String,
    pub transaction_type: Option<String>,
    pub source_amount_minor_units: i64,
    pub source_account_id: Option<i64>,
    pub scheduled_next_date: Option<String>,
    pub scheduled_start_date: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct ImportStage2ContextRows {
    pub user_id: i64,
    pub categories: Vec<ImportStage2CategoryRecord>,
    pub category_rules: Vec<ImportStage2CategoryRuleRecord>,
    pub accounts: Vec<ImportStage2AccountRecord>,
    pub account_rules: Vec<AccountRuleCandidate>,
    pub learning_rules: Vec<ImportStage2LearningRuleRecord>,
    pub recurring_templates: Vec<ImportStage2RecurringTemplateRecord>,
    pub cash_transfer_category_id: Option<i64>,
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn load_import_stage2_context(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<ImportStage2ContextRows> {
    let user_id = i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))?;
    let categories = load_import_stage2_category_records(pool, user_id).await?;
    let category_rules = load_import_stage2_category_rule_records(pool, user_id).await?;
    let accounts = load_import_stage2_account_records(pool, user_id).await?;
    let account_rules = load_import_stage2_account_rule_candidates(pool, user_id).await?;
    let learning_rules = load_learning_rules(pool, user_id).await?;
    let recurring_templates = load_import_stage2_recurring_template_records(pool, user_id).await?;
    let cash_transfer_category_id = load_cash_transfer_category_id(pool, user_id).await?;
    Ok(ImportStage2ContextRows {
        user_id,
        categories,
        category_rules,
        accounts,
        account_rules,
        learning_rules,
        recurring_templates,
        cash_transfer_category_id,
    })
}

pub async fn load_import_stage2_learning_lifecycle_views(
    pool: &PostgresPool,
    user_id: UserId,
    recommendation_keys: &[String],
) -> DbResult<Vec<ImportLearningLifecycleView>> {
    if recommendation_keys.is_empty() {
        return Ok(Vec::new());
    }
    let user_id = i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))?;
    let rows = sqlx::query(
        r#"
        SELECT recommendation_key, recommendation_type, status, accepted_count,
               rejected_count, auto_applied_count, auto_apply_enabled,
               suppressed_until IS NOT NULL AS suppressed
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND recommendation_key = ANY($2)
        ORDER BY recommendation_key ASC
        "#,
    )
    .bind(user_id)
    .bind(recommendation_keys)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let status: String = row.try_get("status")?;
            Ok(ImportLearningLifecycleView {
                recommendation_key: row.try_get("recommendation_key")?,
                recommendation_type: row.try_get("recommendation_type")?,
                signal_state: learning_lifecycle_signal_state(&status).to_string(),
                status,
                accepted_count: i64::from(row.try_get::<i32, _>("accepted_count")?),
                rejected_count: i64::from(row.try_get::<i32, _>("rejected_count")?),
                auto_applied_count: i64::from(row.try_get::<i32, _>("auto_applied_count")?),
                auto_apply_enabled: row.try_get("auto_apply_enabled")?,
                suppressed: row.try_get("suppressed")?,
            })
        })
        .collect()
}

pub async fn load_import_stage2_category_records(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportStage2CategoryRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_type, path, name
        FROM categories
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportStage2CategoryRecord {
                id: row.try_get("id")?,
                category_type: row.try_get("category_type")?,
                path: row.try_get("path")?,
                name: row.try_get("name")?,
            })
        })
        .collect()
}

pub async fn load_import_stage2_category_rule_records(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportStage2CategoryRuleRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_id, priority, rule_expression
        FROM category_rules
        WHERE user_id = $1 AND enabled = true
        ORDER BY priority ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(ImportStage2CategoryRuleRecord {
                id: row.try_get("id").ok()?,
                category_id: row.try_get("category_id").ok()?,
                priority: row.try_get("priority").ok()?,
                rule_expression: row.try_get("rule_expression").ok()?,
            })
        })
        .collect())
}

pub async fn load_import_stage2_account_records(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportStage2AccountRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name
        FROM accounts
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportStage2AccountRecord {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
            })
        })
        .collect()
}

pub async fn load_import_stage2_account_rule_candidates(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<AccountRuleCandidate>> {
    let rows = sqlx::query(
        r#"
        SELECT ar.id, ar.account_id, ar.rule_expression, ar.regex_enabled, ar.enabled, ar.priority
        FROM account_rules ar
        JOIN accounts a ON a.id = ar.account_id AND a.user_id = ar.user_id
        WHERE ar.user_id = $1 AND ar.enabled = true AND a.is_active = true
        ORDER BY ar.priority ASC, ar.id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let expression: Value = row.try_get("rule_expression")?;
            Ok(AccountRuleCandidate {
                rule_id: row.try_get("id")?,
                account_id: row.try_get("account_id")?,
                rule_expression: rule_expression_string(&expression),
                regex_enabled: row.try_get("regex_enabled")?,
                enabled: row.try_get("enabled")?,
                priority: i64::from(row.try_get::<i32, _>("priority")?),
            })
        })
        .collect()
}

async fn load_learning_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportStage2LearningRuleRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, recommendation_type, metadata
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND status IN ('accepted', 'auto_applied', 'green')
        ORDER BY updated_at DESC, id DESC
        LIMIT 1000
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportStage2LearningRuleRecord {
                id: row.try_get("id")?,
                recommendation_type: row.try_get("recommendation_type")?,
                metadata: row.try_get("metadata")?,
            })
        })
        .collect()
}

pub async fn load_import_stage2_recurring_template_records(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportStage2RecurringTemplateRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, transaction_type, source_amount_minor_units,
               source_account_id, scheduled_next_date, scheduled_start_date,
               metadata
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = 2
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportStage2RecurringTemplateRecord {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                transaction_type: row.try_get("transaction_type")?,
                source_amount_minor_units: row
                    .try_get("source_amount_minor_units")
                    .unwrap_or_default(),
                source_account_id: row.try_get("source_account_id").ok().flatten(),
                scheduled_next_date: row.try_get("scheduled_next_date")?,
                scheduled_start_date: row.try_get("scheduled_start_date")?,
                metadata: row
                    .try_get("metadata")
                    .unwrap_or_else(|_| serde_json::json!({})),
            })
        })
        .collect()
}

async fn load_cash_transfer_category_id(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Option<i64>> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM categories
        WHERE user_id = $1
          AND is_active = true
          AND category_type IN ('4', 'transfer', '转账')
          AND (path ILIKE '%现金%' OR name ILIKE '%现金%')
        ORDER BY display_order ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

fn rule_expression_string(expression: &Value) -> String {
    expression
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| {
            expression
                .get("expression")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
}
