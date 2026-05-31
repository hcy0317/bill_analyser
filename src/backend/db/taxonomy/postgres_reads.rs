// 中文导读：PostgreSQL taxonomy 读仓储，提供硬切换后的账户、分类和标签列表投影。
// 维护重点：这里把 PostgreSQL authoritative schema 映射回现有 HTTP/前端兼容 DTO，避免 handler 复制 SQL。
// 不变式：只读路径必须按 user_id 过滤；金额字段从 Postgres 分显式转换为 legacy repository 的元投影。

use std::collections::BTreeSet;

use bill_analyser_core::{
    account::parse_aliases_text,
    account_rules::{
        normalize_account_role_scope, normalize_account_rule_field_scope,
        normalize_transaction_type_scope, AccountRuleCandidate, AccountRuleMatch,
        AccountRuleMatchContext, DEFAULT_FIELD_SCOPES,
    },
    category_rules::escape_rule_expression_term,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Number, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row, Transaction};

use crate::{
    auth_registration::{
        RegisterDefaultSeedSummary, DEFAULT_DAILY_CATEGORIES, DEFAULT_DAILY_CATEGORY_RULES,
    },
    taxonomy::{
        account_rules::AccountRuleRecord,
        accounts::{AccountDisplayOrder, AccountRecord},
        categories::{CategoryRecord, CategoryStatistic},
        category_rules::{
            convert_old_keyword_syntax, CategoryRuleMigrationSummary, CategoryRuleRecord,
        },
        tags::{TagDisplayOrder, TagRecord},
        templates::{TemplateDisplayOrder, TemplateRecord},
    },
    DbError, DbResult, PostgresPool,
};

pub async fn list_postgres_accounts(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<AccountRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, account_type, payment_method, currency,
            balance_cents, is_active, display_order, metadata, created_at, updated_at
        FROM accounts
        WHERE user_id = $1
        ORDER BY display_order ASC, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(account_from_postgres_row).collect()
}

pub async fn get_postgres_account_by_id(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<Option<AccountRecord>> {
    let row = sqlx::query(&account_select_sql("WHERE id = $1 AND user_id = $2"))
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(account_from_postgres_row).transpose()
}

pub async fn get_postgres_sub_accounts(
    pool: &PostgresPool,
    parent_id: i64,
    user_id: i64,
) -> DbResult<Vec<AccountRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, account_type, payment_method, currency,
            balance_cents, is_active, display_order, metadata, created_at, updated_at
        FROM accounts
        WHERE user_id = $1 AND metadata->>'parent_id' = $2
        ORDER BY display_order ASC, name ASC
        "#,
    )
    .bind(user_id)
    .bind(parent_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(account_from_postgres_row).collect()
}

pub async fn create_postgres_account(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<i64> {
    let mut transaction = pool.begin().await?;
    let account_id = insert_postgres_account(&mut transaction, payload, user_id, None).await?;
    transaction.commit().await?;
    Ok(account_id)
}

pub async fn update_postgres_account(
    pool: &PostgresPool,
    account_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let existing = sqlx::query(
        r#"
        SELECT metadata
        FROM accounts
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let metadata = account_metadata_from_payload(Some(&existing.try_get("metadata")?), payload);
    let changed = sqlx::query(
        r#"
        UPDATE accounts
        SET name = $1,
            account_type = $2,
            currency = $3,
            balance_cents = $4,
            is_active = $5,
            display_order = $6,
            metadata = $7,
            updated_at = now(),
            version = version + 1
        WHERE id = $8 AND user_id = $9
        "#,
    )
    .bind(value_text(payload.get("name")).unwrap_or_default())
    .bind(value_text(payload.get("type")))
    .bind(value_text(payload.get("currency")).unwrap_or_else(|| "CNY".to_string()))
    .bind(yuan_value_to_cents(payload.get("balance")).unwrap_or_default())
    .bind(!payload.get("hidden").is_some_and(value_truthy))
    .bind(i64_to_i32(
        int_value(payload.get("display_order")).unwrap_or_default(),
    ))
    .bind(metadata)
    .bind(account_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn delete_postgres_account(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

pub async fn update_postgres_account_display_orders(
    pool: &PostgresPool,
    orders: &[AccountDisplayOrder],
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for order in orders {
        sqlx::query(
            r#"
            UPDATE accounts
            SET display_order = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(order.display_order))
        .bind(order.account_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

pub async fn list_postgres_categories(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<CategoryRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at
        FROM categories
        WHERE user_id = $1
        ORDER BY display_order ASC, path ASC NULLS LAST, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(category_from_postgres_row).collect()
}

pub async fn get_postgres_category_by_id(
    pool: &PostgresPool,
    category_id: i64,
    user_id: i64,
) -> DbResult<Option<CategoryRecord>> {
    let row = sqlx::query(&category_select_sql("WHERE id = $1 AND user_id = $2"))
        .bind(category_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(category_from_postgres_row).transpose()
}

pub async fn get_postgres_category_by_name(
    pool: &PostgresPool,
    main_category: &str,
    sub_category: &str,
    user_id: i64,
) -> DbResult<Option<CategoryRecord>> {
    let path = category_path(main_category, sub_category);
    let row = sqlx::query(&category_select_sql("WHERE user_id = $1 AND path = $2"))
        .bind(user_id)
        .bind(path)
        .fetch_optional(pool)
        .await?;
    row.map(category_from_postgres_row).transpose()
}

pub async fn create_postgres_category(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let values = category_values_from_payload(payload, None);
    if values.name.trim().is_empty() {
        return Err(DbError::InvalidOperation(
            "category name is required".to_string(),
        ));
    }
    if get_postgres_category_by_name(pool, &values.main_category, &values.sub_category, user_id)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    let parent_id = parent_category_id_for_values(pool, user_id, &values).await?;
    let row = sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(values.name)
    .bind(values.category_type)
    .bind(values.path)
    .bind(values.icon)
    .bind(values.color)
    .bind(i64_to_i32(values.display_order))
    .bind(!values.hidden)
    .bind(values.metadata)
    .fetch_one(pool)
    .await?;
    Ok(Some(row.try_get("id")?))
}

pub async fn update_postgres_category(
    pool: &PostgresPool,
    category_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let row = sqlx::query(&category_select_sql("WHERE id = $1 AND user_id = $2"))
        .bind(category_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let existing = category_from_postgres_row(row)?;
    let values = category_values_from_payload(payload, Some(&existing));
    let parent_id = parent_category_id_for_values(pool, user_id, &values).await?;
    let changed = sqlx::query(
        r#"
        UPDATE categories
        SET parent_id = $1,
            name = $2,
            category_type = $3,
            path = $4,
            icon = $5,
            color = $6,
            display_order = $7,
            is_active = $8,
            metadata = $9,
            updated_at = now(),
            version = version + 1
        WHERE id = $10 AND user_id = $11
        "#,
    )
    .bind(parent_id)
    .bind(values.name)
    .bind(values.category_type)
    .bind(values.path)
    .bind(values.icon)
    .bind(values.color)
    .bind(i64_to_i32(values.display_order))
    .bind(!values.hidden)
    .bind(values.metadata)
    .bind(category_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn delete_postgres_category(
    pool: &PostgresPool,
    category_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let Some(category) = get_postgres_category_by_id(pool, category_id, user_id).await? else {
        return Ok(false);
    };
    let main_category = value_text(category.get("main_category")).unwrap_or_default();
    let sub_category = value_text(category.get("sub_category")).unwrap_or_default();
    if sub_category.trim().is_empty() {
        return delete_postgres_categories_by_main_category(pool, &main_category, user_id).await;
    }
    let changed = sqlx::query("DELETE FROM categories WHERE id = $1 AND user_id = $2")
        .bind(category_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

pub async fn delete_postgres_categories_by_main_category(
    pool: &PostgresPool,
    main_category: &str,
    user_id: i64,
) -> DbResult<bool> {
    let like_pattern = format!("{}/%", main_category.trim());
    let changed = sqlx::query(
        r#"
        DELETE FROM categories
        WHERE user_id = $1 AND (path = $2 OR path LIKE $3)
        "#,
    )
    .bind(user_id)
    .bind(main_category.trim())
    .bind(like_pattern)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn update_postgres_category_display_order(
    pool: &PostgresPool,
    category_id: i64,
    display_order: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query(
        r#"
        UPDATE categories
        SET display_order = $1, updated_at = now(), version = version + 1
        WHERE id = $2 AND user_id = $3
        "#,
    )
    .bind(i64_to_i32(display_order))
    .bind(category_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn query_postgres_category_statistics(
    pool: &PostgresPool,
    start_date: Option<&str>,
    end_date: Option<&str>,
    user_id: i64,
) -> DbResult<Vec<CategoryStatistic>> {
    const MAIN_CATEGORY_EXPR: &str = "COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(c.path, '/', 1), ''), c.name, '')";
    const SUB_CATEGORY_EXPR: &str = "COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), CASE WHEN position('/' in COALESCE(c.path, '')) > 0 THEN substring(c.path from position('/' in c.path) + 1) ELSE '' END, '')";

    let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
    builder.push(MAIN_CATEGORY_EXPR);
    builder.push(" AS main_category, ");
    builder.push(SUB_CATEGORY_EXPR);
    builder.push(
        r#" AS sub_category,
            COUNT(*)::BIGINT AS count,
            COALESCE(SUM(ABS(b.amount_cents)), 0)::BIGINT AS total_amount_cents
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false AND ");
    builder.push(MAIN_CATEGORY_EXPR);
    builder.push(" <> ''");

    if let Some(value) = start_date.filter(|value| !value.trim().is_empty()) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(value.trim().to_string());
        builder.push("::date");
    }
    if let Some(value) = end_date.filter(|value| !value.trim().is_empty()) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(value.trim().to_string());
        builder.push("::date + interval '1 day')");
    }

    builder.push(" GROUP BY ");
    builder.push(MAIN_CATEGORY_EXPR);
    builder.push(", ");
    builder.push(SUB_CATEGORY_EXPR);
    builder.push(" ORDER BY total_amount_cents DESC, main_category ASC, sub_category ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let total_amount_cents: i64 = row.try_get("total_amount_cents")?;
            Ok(CategoryStatistic {
                main_category: row
                    .try_get::<Option<String>, _>("main_category")?
                    .unwrap_or_default(),
                sub_category: row
                    .try_get::<Option<String>, _>("sub_category")?
                    .unwrap_or_default(),
                count: row.try_get("count")?,
                total_amount: total_amount_cents as f64 / 100.0,
            })
        })
        .collect()
}

pub async fn update_postgres_main_category_name(
    pool: &PostgresPool,
    old_name: &str,
    new_name: &str,
    user_id: i64,
) -> DbResult<bool> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    if old_name.is_empty() || new_name.is_empty() {
        return Ok(false);
    }
    if get_postgres_category_by_name(pool, new_name, "", user_id)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    let mut transaction = pool.begin().await?;
    let parent_changed = sqlx::query(
        r#"
        UPDATE categories
        SET name = $1,
            path = $1,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $2 AND path = $3
        "#,
    )
    .bind(new_name)
    .bind(user_id)
    .bind(old_name)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    let child_pattern = format!("{old_name}/%");
    let child_changed = sqlx::query(
        r#"
        UPDATE categories
        SET path = $1 || substring(path FROM char_length($2) + 1),
            updated_at = now(),
            version = version + 1
        WHERE user_id = $3 AND path LIKE $4
        "#,
    )
    .bind(new_name)
    .bind(old_name)
    .bind(user_id)
    .bind(child_pattern)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    transaction.commit().await?;
    Ok(parent_changed > 0 || child_changed > 0)
}

pub async fn list_postgres_category_rules(
    pool: &PostgresPool,
    user_id: i64,
    category_id: Option<i64>,
    enabled_only: bool,
) -> DbResult<Vec<CategoryRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(category_rule_select_sql());
    builder.push(" WHERE cr.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND c.user_id = ");
    builder.push_bind(user_id);
    if let Some(category_id) = category_id {
        builder.push(" AND cr.category_id = ");
        builder.push_bind(category_id);
    }
    if enabled_only {
        builder.push(" AND cr.enabled = true");
    }
    builder.push(" ORDER BY cr.priority ASC, cr.id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(category_rule_from_postgres_row)
        .collect()
}

pub async fn get_postgres_category_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<Option<CategoryRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(category_rule_select_sql());
    builder.push(" WHERE cr.id = ");
    builder.push_bind(rule_id);
    builder.push(" AND cr.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND c.user_id = ");
    builder.push_bind(user_id);
    let row = builder.build().fetch_optional(pool).await?;
    row.map(category_rule_from_postgres_row).transpose()
}

pub async fn create_postgres_category_rule(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("category rule payload must be an object".to_string())
    })?;
    let category_id = object
        .get("category_id")
        .and_then(|value| int_value(Some(value)))
        .ok_or_else(|| DbError::InvalidOperation("category_id is required".to_string()))?;
    if !postgres_category_belongs_to_user(pool, category_id, user_id).await? {
        return Ok(None);
    }
    let rule_expression = required_postgres_rule_expression(object.get("rule_expression"))?;
    let regex_enabled = object
        .get("regex_enabled")
        .map(payload_bool_value)
        .transpose()?
        .unwrap_or(false);
    let enabled = object
        .get("enabled")
        .map(payload_bool_value)
        .transpose()?
        .unwrap_or(true);
    let name = object
        .get("name")
        .map(payload_scalar_text)
        .transpose()?
        .unwrap_or_default();
    let priority = object
        .get("priority")
        .and_then(|value| int_value(Some(value)))
        .unwrap_or(100);

    let row = sqlx::query(
        r#"
        INSERT INTO category_rules (
            user_id, category_id, name, transaction_type_scope, field_scope,
            rule_expression, priority, enabled
        )
        VALUES ($1, $2, $3, 'all', $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(name)
    .bind(json_array(DEFAULT_FIELD_SCOPES))
    .bind(legacy_rule_expression_json(&rule_expression, regex_enabled))
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .fetch_one(pool)
    .await?;
    Ok(Some(row.try_get("id")?))
}

pub async fn update_postgres_category_rule(
    pool: &PostgresPool,
    rule_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("category rule update payload must be an object".to_string())
    })?;
    if object.is_empty() {
        return Ok(false);
    }
    let existing = sqlx::query(
        r#"
        SELECT category_id, name, rule_expression, priority, enabled
        FROM category_rules
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(rule_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let mut category_id = existing.try_get::<Option<i64>, _>("category_id")?;
    let mut name: String = existing.try_get("name")?;
    let mut rule_expression_json_value: Value = existing.try_get("rule_expression")?;
    let mut priority = i64::from(existing.try_get::<i32, _>("priority")?);
    let mut enabled: bool = existing.try_get("enabled")?;
    let mut changed = false;

    for (key, value) in object {
        match key.as_str() {
            "id" | "user_id" | "created_at" | "updated_at" | "applied_count"
            | "last_applied_at" | "main_category" | "sub_category" | "category_type" => {}
            "category_id" => {
                let next_category_id = required_postgres_i64(value, "category_id")?;
                if !postgres_category_belongs_to_user(pool, next_category_id, user_id).await? {
                    return Ok(false);
                }
                category_id = Some(next_category_id);
                changed = true;
            }
            "name" => {
                name = payload_scalar_text(value)?;
                changed = true;
            }
            "priority" => {
                priority = required_postgres_i64(value, "priority")?;
                changed = true;
            }
            "rule_expression" => {
                let rule_expression = required_postgres_rule_expression(Some(value))?;
                let regex_enabled = rule_expression_regex_enabled(&rule_expression_json_value);
                rule_expression_json_value =
                    legacy_rule_expression_json(&rule_expression, regex_enabled);
                changed = true;
            }
            "regex_enabled" => {
                let rule_expression = rule_expression_string(&rule_expression_json_value);
                rule_expression_json_value =
                    legacy_rule_expression_json(&rule_expression, payload_bool_value(value)?);
                changed = true;
            }
            "enabled" => {
                enabled = payload_bool_value(value)?;
                changed = true;
            }
            _ => {}
        }
    }
    if !changed {
        return Ok(false);
    }

    let affected = sqlx::query(
        r#"
        UPDATE category_rules
        SET category_id = $1,
            name = $2,
            rule_expression = $3,
            priority = $4,
            enabled = $5,
            updated_at = now(),
            version = version + 1
        WHERE id = $6 AND user_id = $7
        "#,
    )
    .bind(category_id)
    .bind(name)
    .bind(rule_expression_json_value)
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .bind(rule_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

pub async fn delete_postgres_category_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM category_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

pub async fn reorder_postgres_category_rules(
    pool: &PostgresPool,
    rule_ids: &[i64],
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for (priority, rule_id) in rule_ids.iter().enumerate() {
        sqlx::query(
            r#"
            UPDATE category_rules
            SET priority = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(i64::try_from(priority + 1).unwrap_or(i64::MAX)))
        .bind(rule_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

pub async fn ensure_postgres_category_rule_defaults(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<RegisterDefaultSeedSummary> {
    let mut summary = RegisterDefaultSeedSummary {
        categories_created: 0,
        categories_skipped: 0,
        rules_created: 0,
        rules_skipped: 0,
        rules_missing_categories: 0,
    };

    for category in DEFAULT_DAILY_CATEGORIES {
        let parent_payload = json!({
            "main_category": category.name,
            "sub_category": "",
            "type": category.type_code,
            "priority": category.priority,
            "icon": category.icon,
            "color": category.color,
            "hidden": false,
        });
        match create_postgres_category(pool, &parent_payload, user_id).await? {
            Some(_) => summary.categories_created += 1,
            None => summary.categories_skipped += 1,
        }

        for (offset, sub_category) in category.sub_categories.iter().enumerate() {
            let child_payload = json!({
                "main_category": category.name,
                "sub_category": sub_category.name,
                "type": category.type_code,
                "priority": category.priority + i64::try_from(offset + 1).unwrap_or(i64::MAX),
                "icon": sub_category.icon,
                "color": sub_category.color,
                "hidden": false,
            });
            match create_postgres_category(pool, &child_payload, user_id).await? {
                Some(_) => summary.categories_created += 1,
                None => summary.categories_skipped += 1,
            }
        }
    }

    for rule in DEFAULT_DAILY_CATEGORY_RULES {
        let Some(category) =
            get_postgres_category_by_name(pool, rule.main_category, rule.sub_category, user_id)
                .await?
        else {
            summary.rules_missing_categories += 1;
            continue;
        };
        if postgres_category_rule_name_exists(pool, user_id, rule.name).await? {
            summary.rules_skipped += 1;
            continue;
        }
        let Some(category_id) = int_value(category.get("id")) else {
            summary.rules_missing_categories += 1;
            continue;
        };
        let payload = json!({
            "category_id": category_id,
            "name": rule.name,
            "priority": rule.priority,
            "rule_expression": rule.rule_expression,
            "regex_enabled": false,
            "enabled": true,
        });
        match create_postgres_category_rule(pool, &payload, user_id).await? {
            Some(_) => summary.rules_created += 1,
            None => summary.rules_skipped += 1,
        }
    }

    Ok(summary)
}

pub async fn migrate_postgres_category_keywords_to_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<CategoryRuleMigrationSummary> {
    let rows = sqlx::query(
        r#"
        SELECT id, path, name, display_order, metadata
        FROM categories
        WHERE user_id = $1
          AND COALESCE(NULLIF(TRIM(metadata->>'keywords'), ''), '') <> ''
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut summary = CategoryRuleMigrationSummary {
        migrated: 0,
        skipped: 0,
    };
    for row in rows {
        let category_id: i64 = row.try_get("id")?;
        let path: Option<String> = row.try_get("path")?;
        let name: String = row.try_get("name")?;
        let priority: i32 = row.try_get("display_order")?;
        let metadata: Value = row.try_get("metadata")?;
        let keywords = metadata
            .get("keywords")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if keywords.is_empty() {
            summary.skipped += 1;
            continue;
        }
        let rule_expression = convert_old_keyword_syntax(keywords);
        if rule_expression.is_empty()
            || postgres_category_rule_expression_exists(
                pool,
                user_id,
                category_id,
                &rule_expression,
            )
            .await?
        {
            summary.skipped += 1;
            continue;
        }
        let (main_category, sub_category) =
            category_names_from_path(path.as_deref().unwrap_or_default(), &name);
        let payload = json!({
            "category_id": category_id,
            "name": format!("migrated:{main_category}/{sub_category}"),
            "priority": i64::from(priority),
            "rule_expression": rule_expression,
            "regex_enabled": false,
            "enabled": true,
        });
        match create_postgres_category_rule(pool, &payload, user_id).await? {
            Some(_) => summary.migrated += 1,
            None => summary.skipped += 1,
        }
    }

    Ok(summary)
}

pub async fn query_postgres_rules_overview_payload(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Value> {
    let learning_rules = list_postgres_rules_overview_learning_rules(pool, user_id).await?;
    let learning_count = i64::try_from(learning_rules.len()).unwrap_or(i64::MAX);
    let category_rule_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM category_rules WHERE user_id = $1 AND enabled = true",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let recurring_rules = list_postgres_rules_overview_recurring_rules(pool, user_id).await?;
    let recurring_rule_count = i64::try_from(recurring_rules.len()).unwrap_or(i64::MAX);

    Ok(json!({
        "learningRules": learning_rules,
        "learningRuleCount": learning_count,
        "categoryRuleCount": category_rule_count,
        "recurringRules": recurring_rules,
        "recurringRuleCount": recurring_rule_count,
        "totalRuleCount": learning_count + category_rule_count + recurring_rule_count,
    }))
}

async fn postgres_category_rule_name_exists(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM category_rules WHERE user_id = $1 AND name = $2 LIMIT 1",
    )
    .bind(user_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

async fn postgres_category_rule_expression_exists(
    pool: &PostgresPool,
    user_id: i64,
    category_id: i64,
    rule_expression: &str,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT 1
        FROM category_rules
        WHERE user_id = $1
          AND category_id = $2
          AND rule_expression = $3
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(legacy_rule_expression_json(rule_expression, false))
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

async fn list_postgres_rules_overview_learning_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, recommendation_type, recommendation_key, status,
               accepted_count, auto_applied_count, metadata
        FROM import_learning_lifecycle
        WHERE user_id = $1
        ORDER BY updated_at DESC, id DESC
        LIMIT 500
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id")?;
            let recommendation_type: String = row.try_get("recommendation_type")?;
            let recommendation_key: String = row.try_get("recommendation_key")?;
            let status: String = row.try_get("status")?;
            let accepted_count: i32 = row.try_get("accepted_count")?;
            let auto_applied_count: i32 = row.try_get("auto_applied_count")?;
            let metadata: Value = row.try_get("metadata")?;
            Ok(json!({
                "id": id,
                "matchType": metadata
                    .get("match_type")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_type.as_str()),
                "matchValue": metadata
                    .get("match_value")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_key.as_str()),
                "learnedType": metadata
                    .get("learned_type")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_type.as_str()),
                "learnedCategoryId": metadata.get("learned_category_id").cloned().unwrap_or(Value::Null),
                "enabled": status != "suppressed" && status != "disabled",
                "appliedCount": i64::from(accepted_count) + i64::from(auto_applied_count),
                "source": "learning",
            }))
        })
        .collect()
}

async fn list_postgres_rules_overview_recurring_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, source_amount_minor_units, scheduled_frequency,
               scheduled_frequency_type, enabled, scheduled_next_date
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = 2
        ORDER BY display_order, name
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id")?;
            let name: String = row.try_get("name")?;
            let amount_minor: i64 = row.try_get("source_amount_minor_units")?;
            let scheduled_frequency: Option<String> = row.try_get("scheduled_frequency")?;
            let scheduled_frequency_type: Option<i32> = row.try_get("scheduled_frequency_type")?;
            let enabled: bool = row.try_get("enabled")?;
            let next_date: Option<String> = row.try_get("scheduled_next_date")?;
            Ok(json!({
                "id": id,
                "name": name,
                "amount": amount_minor as f64 / 100.0,
                "frequency": scheduled_frequency
                    .or_else(|| scheduled_frequency_type.map(|value| value.to_string())),
                "enabled": enabled,
                "nextDate": next_date,
                "source": "recurring",
            }))
        })
        .collect()
}

pub async fn get_postgres_legacy_category_rules_setting(
    pool: &PostgresPool,
    key: &str,
    user_id: i64,
) -> DbResult<Option<Value>> {
    let row = sqlx::query("SELECT value FROM settings WHERE user_id = $1 AND key = $2")
        .bind(user_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("value"))
        .transpose()
        .map_err(Into::into)
}

pub async fn set_postgres_legacy_category_rules_setting(
    pool: &PostgresPool,
    key: &str,
    value: &Value,
    user_id: i64,
) -> DbResult<()> {
    sqlx::query(
        r#"
        INSERT INTO settings (user_id, key, value, sensitive)
        VALUES ($1, $2, $3, false)
        ON CONFLICT (user_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now(), version = settings.version + 1
        "#,
    )
    .bind(user_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_postgres_legacy_category_engine_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT
            cr.id,
            cr.category_id,
            cr.priority AS rule_priority,
            cr.rule_expression,
            c.path,
            c.name AS category_name,
            c.category_type,
            c.display_order AS category_priority
        FROM category_rules cr
        JOIN categories c ON cr.category_id = c.id
        WHERE cr.user_id = $1 AND c.user_id = $1 AND cr.enabled = true
        ORDER BY COALESCE(c.display_order, cr.priority, 999999), cr.category_id, cr.id
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut rules = Vec::new();
    for (load_order, row) in rows.into_iter().enumerate() {
        let category_type_text = row
            .try_get::<Option<String>, _>("category_type")?
            .unwrap_or_else(|| "3".to_string());
        let Some(rule_type) = normalize_legacy_category_rule_type(category_type_text.as_str())
        else {
            continue;
        };
        let category_id: i64 = row.try_get("category_id")?;
        let path = row
            .try_get::<Option<String>, _>("path")?
            .unwrap_or_default();
        let category_name: String = row.try_get("category_name")?;
        let (main_category, sub_category) = category_names_from_path(&path, &category_name);
        if category_id <= 0 || main_category.trim().is_empty() || sub_category.trim().is_empty() {
            continue;
        }
        let rule_expression: Value = row.try_get("rule_expression")?;
        let category_priority = i64::from(row.try_get::<i32, _>("category_priority")?);
        rules.push(serde_json::json!({
            "id": row.try_get::<i64, _>("id")?,
            "category_id": category_id,
            "main": main_category.trim(),
            "sub": sub_category.trim(),
            "priority": category_priority,
            "category_priority": category_priority,
            "rule_priority": i64::from(row.try_get::<i32, _>("rule_priority")?),
            "keywords": rule_expression_string(&rule_expression),
            "type": rule_type,
            "_load_order": load_order,
        }));
    }
    Ok(rules)
}

pub async fn list_postgres_account_rules(
    pool: &PostgresPool,
    user_id: i64,
    account_id: Option<i64>,
    enabled_only: bool,
    account_role_scope: Option<&str>,
    transaction_type_scope: Option<&str>,
) -> DbResult<Vec<AccountRuleRecord>> {
    let normalized_role_scope = account_role_scope
        .map(|scope| normalize_account_role_scope(Some(scope)).map_err(DbError::InvalidOperation))
        .transpose()?;
    let normalized_transaction_scope = transaction_type_scope
        .map(|scope| {
            normalize_transaction_type_scope(Some(scope)).map_err(DbError::InvalidOperation)
        })
        .transpose()?;

    let mut builder = QueryBuilder::<Postgres>::new(account_rule_select_sql());
    builder.push(" WHERE ar.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND a.user_id = ");
    builder.push_bind(user_id);
    if let Some(account_id) = account_id {
        builder.push(" AND ar.account_id = ");
        builder.push_bind(account_id);
    }
    if enabled_only {
        builder.push(" AND ar.enabled = true");
    }
    if let Some(scope) = normalized_role_scope {
        builder.push(" AND ar.account_role_scope = ");
        builder.push_bind(scope);
    }
    if let Some(scope) = normalized_transaction_scope {
        builder.push(" AND ar.transaction_type_scope = ");
        builder.push_bind(scope);
    }
    builder.push(" ORDER BY ar.priority ASC, ar.id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(account_rule_from_postgres_row)
        .collect()
}

pub async fn get_postgres_account_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<Option<AccountRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(account_rule_select_sql());
    builder.push(" WHERE ar.id = ");
    builder.push_bind(rule_id);
    builder.push(" AND ar.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND a.user_id = ");
    builder.push_bind(user_id);
    let row = builder.build().fetch_optional(pool).await?;
    row.map(account_rule_from_postgres_row).transpose()
}

pub async fn create_postgres_account_rule(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("account rule payload must be an object".to_string())
    })?;
    let account_id = object
        .get("account_id")
        .or_else(|| object.get("accountId"))
        .and_then(|value| int_value(Some(value)))
        .ok_or_else(|| DbError::InvalidOperation("account_id is required".to_string()))?;
    if !postgres_account_belongs_to_user(pool, account_id, user_id).await? {
        return Ok(None);
    }
    let rule_expression = required_postgres_rule_expression(
        object
            .get("rule_expression")
            .or_else(|| object.get("ruleExpression")),
    )?;
    let regex_enabled = object
        .get("regex_enabled")
        .or_else(|| object.get("regexEnabled"))
        .map(payload_bool_value)
        .transpose()?
        .unwrap_or(false);
    let enabled = object
        .get("enabled")
        .map(payload_bool_value)
        .transpose()?
        .unwrap_or(true);
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
    let name = object
        .get("name")
        .map(payload_scalar_text)
        .transpose()?
        .unwrap_or_default();
    let priority = object
        .get("priority")
        .and_then(|value| int_value(Some(value)))
        .unwrap_or(100);
    let source = object
        .get("source")
        .map(payload_scalar_text)
        .transpose()?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "manual".to_string());

    let row = sqlx::query(
        r#"
        INSERT INTO account_rules (
            user_id, account_id, name, account_role_scope, transaction_type_scope,
            field_scope, rule_expression, regex_enabled, priority, enabled, source
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(name)
    .bind(account_role_scope)
    .bind(transaction_type_scope)
    .bind(Value::Array(
        field_scope.into_iter().map(Value::String).collect(),
    ))
    .bind(legacy_rule_expression_json(&rule_expression, regex_enabled))
    .bind(regex_enabled)
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .bind(source)
    .fetch_one(pool)
    .await?;
    Ok(Some(row.try_get("id")?))
}

pub async fn update_postgres_account_rule(
    pool: &PostgresPool,
    rule_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("account rule update payload must be an object".to_string())
    })?;
    if object.is_empty() {
        return Ok(false);
    }
    let existing = sqlx::query(
        r#"
        SELECT account_id, name, account_role_scope, transaction_type_scope, field_scope,
            rule_expression, regex_enabled, priority, enabled, source
        FROM account_rules
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(rule_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let mut account_id = existing.try_get::<Option<i64>, _>("account_id")?;
    let mut name: String = existing.try_get("name")?;
    let mut account_role_scope: String = existing.try_get("account_role_scope")?;
    let mut transaction_type_scope: String = existing.try_get("transaction_type_scope")?;
    let mut field_scope: Value = existing.try_get("field_scope")?;
    let mut rule_expression_json_value: Value = existing.try_get("rule_expression")?;
    let mut regex_enabled: bool = existing.try_get("regex_enabled")?;
    let mut priority = i64::from(existing.try_get::<i32, _>("priority")?);
    let mut enabled: bool = existing.try_get("enabled")?;
    let mut source: String = existing.try_get("source")?;
    let mut changed = false;

    for (key, value) in object {
        match key.as_str() {
            "id" | "user_id" | "userId" | "created_at" | "createdAt" | "updated_at"
            | "updatedAt" | "applied_count" | "appliedCount" | "last_applied_at"
            | "lastAppliedAt" | "match_count" | "matchCount" | "last_matched_at"
            | "lastMatchedAt" | "source_key" | "sourceKey" | "account_name" | "accountName"
            | "account_type" | "accountType" | "account_hidden" | "accountHidden" => {}
            "account_id" | "accountId" => {
                let next_account_id = required_postgres_i64(value, "account_id")?;
                if !postgres_account_belongs_to_user(pool, next_account_id, user_id).await? {
                    return Ok(false);
                }
                account_id = Some(next_account_id);
                changed = true;
            }
            "name" => {
                name = payload_scalar_text(value)?;
                changed = true;
            }
            "priority" => {
                priority = required_postgres_i64(value, "priority")?;
                changed = true;
            }
            "rule_expression" | "ruleExpression" => {
                let rule_expression = required_postgres_rule_expression(Some(value))?;
                rule_expression_json_value =
                    legacy_rule_expression_json(&rule_expression, regex_enabled);
                changed = true;
            }
            "regex_enabled" | "regexEnabled" => {
                regex_enabled = payload_bool_value(value)?;
                let rule_expression = rule_expression_string(&rule_expression_json_value);
                rule_expression_json_value =
                    legacy_rule_expression_json(&rule_expression, regex_enabled);
                changed = true;
            }
            "enabled" => {
                enabled = payload_bool_value(value)?;
                changed = true;
            }
            "account_role_scope" | "accountRoleScope" => {
                account_role_scope = normalize_account_role_scope(value.as_str())
                    .map_err(DbError::InvalidOperation)?;
                changed = true;
            }
            "transaction_type_scope" | "transactionTypeScope" => {
                transaction_type_scope = normalize_transaction_type_scope(value.as_str())
                    .map_err(DbError::InvalidOperation)?;
                changed = true;
            }
            "field_scope" | "fieldScope" => {
                field_scope = Value::Array(
                    normalize_account_rule_field_scope(Some(value))
                        .map_err(DbError::InvalidOperation)?
                        .into_iter()
                        .map(Value::String)
                        .collect(),
                );
                changed = true;
            }
            "source" => {
                source = payload_scalar_text(value)?;
                changed = true;
            }
            _ => {}
        }
    }
    if !changed {
        return Ok(false);
    }

    let affected = sqlx::query(
        r#"
        UPDATE account_rules
        SET account_id = $1,
            name = $2,
            account_role_scope = $3,
            transaction_type_scope = $4,
            field_scope = $5,
            rule_expression = $6,
            regex_enabled = $7,
            priority = $8,
            enabled = $9,
            source = $10,
            updated_at = now(),
            version = version + 1
        WHERE id = $11 AND user_id = $12
        "#,
    )
    .bind(account_id)
    .bind(name)
    .bind(account_role_scope)
    .bind(transaction_type_scope)
    .bind(field_scope)
    .bind(rule_expression_json_value)
    .bind(regex_enabled)
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .bind(source)
    .bind(rule_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

pub async fn delete_postgres_account_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM account_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

pub async fn reorder_postgres_account_rules(
    pool: &PostgresPool,
    rule_ids: &[i64],
    user_id: i64,
) -> DbResult<bool> {
    let unique_ids = rule_ids.iter().copied().collect::<BTreeSet<_>>();
    if unique_ids.len() != rule_ids.len() {
        return Ok(false);
    }
    if !rule_ids.is_empty()
        && count_postgres_account_rules(pool, &unique_ids, user_id).await?
            != i64::try_from(unique_ids.len()).unwrap_or(i64::MAX)
    {
        return Ok(false);
    }
    let mut transaction = pool.begin().await?;
    for (priority, rule_id) in rule_ids.iter().enumerate() {
        sqlx::query(
            r#"
            UPDATE account_rules
            SET priority = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(i64::try_from(priority + 1).unwrap_or(i64::MAX)))
        .bind(rule_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

pub async fn migrate_postgres_account_aliases_to_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<crate::taxonomy::account_rules::AccountRuleMigrationSummary> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, is_active, metadata
        FROM accounts
        WHERE user_id = $1
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut summary = crate::taxonomy::account_rules::AccountRuleMigrationSummary {
        migrated: 0,
        skipped: 0,
    };
    for row in rows {
        let account_id: i64 = row.try_get("id")?;
        let account_name: String = row.try_get("name")?;
        let is_active: bool = row.try_get("is_active")?;
        let metadata: Value = row.try_get("metadata")?;
        let aliases = aliases_from_metadata(&metadata);
        if !is_active {
            summary.skipped += i64::try_from(aliases.len()).unwrap_or(i64::MAX);
            continue;
        }
        for alias in aliases {
            let normalized_alias = alias.trim().to_ascii_lowercase();
            if normalized_alias.is_empty() {
                summary.skipped += 1;
                continue;
            }
            let source_key = format!("legacy_alias:{normalized_alias}");
            if postgres_account_alias_rule_exists(pool, user_id, account_id, &source_key, &alias)
                .await?
            {
                summary.skipped += 1;
                continue;
            }
            let expression = format!("OR={{{}}}", escape_rule_expression_term(alias.trim()));
            sqlx::query(
                r#"
                INSERT INTO account_rules (
                    user_id, account_id, name, account_role_scope, transaction_type_scope,
                    field_scope, rule_expression, regex_enabled, priority, enabled,
                    source, source_key
                )
                VALUES ($1, $2, $3, 'any', 'all', $4, $5, false, 1000, true,
                    'alias_migration', $6)
                "#,
            )
            .bind(user_id)
            .bind(account_id)
            .bind(format!("migrated:{}/{}", account_name, alias.trim()))
            .bind(json_array(DEFAULT_FIELD_SCOPES))
            .bind(legacy_rule_expression_json(&expression, false))
            .bind(source_key)
            .execute(pool)
            .await?;
            summary.migrated += 1;
        }
    }
    Ok(summary)
}

pub async fn test_postgres_account_rule_match(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
    context: &AccountRuleMatchContext,
    requested_role_scope: &str,
    transaction_type_scope: &str,
) -> DbResult<Option<AccountRuleMatch>> {
    let Some(rule) = get_postgres_account_rule(pool, rule_id, user_id).await? else {
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

pub async fn list_postgres_tags(pool: &PostgresPool, user_id: i64) -> DbResult<Vec<TagRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, color, display_order, metadata, created_at, updated_at
        FROM tags
        WHERE user_id = $1
        ORDER BY display_order ASC, created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(tag_from_postgres_row).collect()
}

pub async fn get_postgres_tag(
    pool: &PostgresPool,
    tag_id: i64,
    user_id: i64,
) -> DbResult<Option<TagRecord>> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, name, color, display_order, metadata, created_at, updated_at
        FROM tags
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(tag_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(tag_from_postgres_row).transpose()
}

pub async fn create_postgres_tag(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<i64> {
    let name = value_text(payload.get("name"))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DbError::InvalidOperation("name is required".to_string()))?;
    let color = value_text(payload.get("color")).unwrap_or_else(|| "#000000".to_string());
    let display_order = i64_to_i32(int_value(payload.get("display_order")).unwrap_or_default());
    let metadata = tag_metadata_from_payload(None, payload);

    let row = sqlx::query(
        r#"
        INSERT INTO tags (user_id, name, color, display_order, metadata)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(color)
    .bind(display_order)
    .bind(metadata)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get("id")?)
}

pub async fn update_postgres_tag(
    pool: &PostgresPool,
    tag_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let existing = sqlx::query(
        r#"
        SELECT name, color, display_order, metadata
        FROM tags
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(tag_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let existing_metadata: Value = existing.try_get("metadata")?;
    let name = value_text(payload.get("name"))
        .unwrap_or_else(|| existing.try_get::<String, _>("name").unwrap_or_default());
    let color = if payload.get("color").is_some() {
        value_text(payload.get("color"))
    } else {
        existing.try_get("color")?
    };
    let display_order = payload
        .get("display_order")
        .and_then(|value| int_value(Some(value)))
        .map(i64_to_i32)
        .unwrap_or(existing.try_get("display_order")?);
    let metadata = tag_metadata_from_payload(Some(&existing_metadata), payload);

    let changed = sqlx::query(
        r#"
        UPDATE tags
        SET name = $1,
            color = $2,
            display_order = $3,
            metadata = $4,
            updated_at = now(),
            version = version + 1
        WHERE id = $5 AND user_id = $6
        "#,
    )
    .bind(name.trim())
    .bind(color)
    .bind(display_order)
    .bind(metadata)
    .bind(tag_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn delete_postgres_tag(pool: &PostgresPool, tag_id: i64, user_id: i64) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(tag_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

pub async fn update_postgres_tag_display_orders(
    pool: &PostgresPool,
    orders: &[TagDisplayOrder],
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for order in orders {
        sqlx::query(
            r#"
            UPDATE tags
            SET display_order = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(order.display_order))
        .bind(order.tag_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

pub async fn list_postgres_templates(
    pool: &PostgresPool,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<Vec<TemplateRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units::BIGINT AS source_amount_minor_units,
            destination_amount_minor_units::BIGINT AS destination_amount_minor_units,
            hide_amount,
            tag_ids, comment, scheduled_frequency_type, scheduled_frequency,
            scheduled_start_date, scheduled_end_date, scheduled_next_date,
            enabled, auto_create, display_order, hidden, utc_offset, created_at, updated_at
        FROM transaction_templates
        WHERE user_id = $1 AND ($2::BIGINT IS NULL OR template_type = $2)
        ORDER BY template_type ASC, display_order ASC, name ASC
        "#,
    )
    .bind(user_id)
    .bind(template_type)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(template_from_postgres_row).collect()
}

pub async fn get_postgres_template_by_id(
    pool: &PostgresPool,
    template_id: i64,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<Option<TemplateRecord>> {
    let sql = template_select_sql(
        "WHERE id = $1 AND user_id = $2 AND ($3::BIGINT IS NULL OR template_type = $3)",
    );
    let row = sqlx::query(&sql)
        .bind(template_id)
        .bind(user_id)
        .bind(template_type)
        .fetch_optional(pool)
        .await?;
    row.map(template_from_postgres_row).transpose()
}

pub async fn create_postgres_template(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<i64> {
    let template_type = int_value(payload.get("templateType")).unwrap_or(1);
    let display_order = next_template_display_order(pool, user_id, template_type).await?;
    let values = template_values_from_payload(payload, template_type, display_order, None);
    let row = sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units, destination_amount_minor_units,
            hide_amount, tag_ids, comment, scheduled_frequency_type,
            scheduled_frequency, scheduled_start_date, scheduled_end_date,
            scheduled_next_date, enabled, auto_create, display_order, hidden,
            utc_offset
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8,
            $9, $10,
            $11, $12, $13, $14,
            $15, $16, $17,
            $18, $19, $20, $21, $22,
            $23
        )
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(i64_to_i32(template_type))
    .bind(values.name)
    .bind(values.description)
    .bind(values.transaction_type)
    .bind(values.category_id)
    .bind(values.source_account_id)
    .bind(values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(values.tag_ids)
    .bind(values.comment)
    .bind(values.scheduled_frequency_type.map(i64_to_i32))
    .bind(values.scheduled_frequency)
    .bind(values.scheduled_start_date)
    .bind(values.scheduled_end_date)
    .bind(values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(i64_to_i32(values.utc_offset))
    .fetch_one(pool)
    .await?;
    Ok(row.try_get("id")?)
}

pub async fn update_postgres_template(
    pool: &PostgresPool,
    template_id: i64,
    payload: &Value,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<bool> {
    let Some(existing) =
        get_postgres_template_by_id(pool, template_id, user_id, template_type).await?
    else {
        return Ok(false);
    };
    let resolved_type = int_value(existing.get("templateType")).unwrap_or(1);
    let values = template_values_from_payload(payload, resolved_type, 0, Some(&existing));
    let changed = sqlx::query(
        r#"
        UPDATE transaction_templates
        SET name = $1,
            description = $2,
            transaction_type = $3,
            category_id = $4,
            source_account_id = $5,
            destination_account_id = $6,
            source_amount_minor_units = $7,
            destination_amount_minor_units = $8,
            hide_amount = $9,
            tag_ids = $10,
            comment = $11,
            scheduled_frequency_type = $12,
            scheduled_frequency = $13,
            scheduled_start_date = $14,
            scheduled_end_date = $15,
            scheduled_next_date = $16,
            enabled = $17,
            auto_create = $18,
            display_order = $19,
            hidden = $20,
            utc_offset = $21,
            updated_at = now(),
            version = version + 1
        WHERE id = $22 AND user_id = $23 AND template_type = $24
        "#,
    )
    .bind(values.name)
    .bind(values.description)
    .bind(values.transaction_type)
    .bind(values.category_id)
    .bind(values.source_account_id)
    .bind(values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(values.tag_ids)
    .bind(values.comment)
    .bind(values.scheduled_frequency_type.map(i64_to_i32))
    .bind(values.scheduled_frequency)
    .bind(values.scheduled_start_date)
    .bind(values.scheduled_end_date)
    .bind(values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(i64_to_i32(values.utc_offset))
    .bind(template_id)
    .bind(user_id)
    .bind(i64_to_i32(resolved_type))
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn delete_postgres_template(
    pool: &PostgresPool,
    template_id: i64,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<bool> {
    let changed = sqlx::query(
        r#"
        DELETE FROM transaction_templates
        WHERE id = $1 AND user_id = $2 AND ($3::BIGINT IS NULL OR template_type = $3)
        "#,
    )
    .bind(template_id)
    .bind(user_id)
    .bind(template_type)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn update_postgres_template_display_orders(
    pool: &PostgresPool,
    orders: &[TemplateDisplayOrder],
    template_type: i64,
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for order in orders {
        sqlx::query(
            r#"
            UPDATE transaction_templates
            SET display_order = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3 AND template_type = $4
            "#,
        )
        .bind(i64_to_i32(order.display_order))
        .bind(order.template_id)
        .bind(user_id)
        .bind(i64_to_i32(template_type))
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

fn category_rule_select_sql() -> &'static str {
    r#"
        SELECT
            cr.id, cr.user_id, cr.category_id, cr.name, cr.priority,
            cr.rule_expression, cr.enabled, cr.created_at, cr.updated_at,
            c.path, c.name AS category_name, c.category_type
        FROM category_rules cr
        JOIN categories c ON cr.category_id = c.id
    "#
}

fn account_rule_select_sql() -> &'static str {
    r#"
        SELECT
            ar.id, ar.user_id, ar.account_id, ar.name, ar.priority,
            ar.rule_expression, ar.regex_enabled, ar.enabled,
            ar.match_count, ar.last_matched_at,
            ar.account_role_scope, ar.transaction_type_scope,
            ar.field_scope, ar.source, ar.source_key, ar.created_at, ar.updated_at,
            a.name AS account_name, a.account_type AS account_type, NOT a.is_active AS account_hidden
        FROM account_rules ar
        JOIN accounts a ON ar.account_id = a.id
    "#
}

fn category_rule_from_postgres_row(row: PgRow) -> DbResult<CategoryRuleRecord> {
    let path = row
        .try_get::<Option<String>, _>("path")?
        .unwrap_or_default();
    let category_name: String = row.try_get("category_name")?;
    let (main_category, sub_category) = category_names_from_path(&path, &category_name);
    let rule_expression_json: Value = row.try_get("rule_expression")?;
    let enabled: bool = row.try_get("enabled")?;
    let mut record = Map::new();
    insert_i64(&mut record, "id", row.try_get("id")?);
    insert_i64(&mut record, "user_id", row.try_get("user_id")?);
    insert_i64(
        &mut record,
        "category_id",
        row.try_get::<Option<i64>, _>("category_id")?
            .unwrap_or_default(),
    );
    insert_string(&mut record, "name", row.try_get("name")?);
    insert_i64(
        &mut record,
        "priority",
        i64::from(row.try_get::<i32, _>("priority")?),
    );
    insert_string(
        &mut record,
        "rule_expression",
        rule_expression_string(&rule_expression_json),
    );
    insert_i64(
        &mut record,
        "regex_enabled",
        bool_as_i64(rule_expression_regex_enabled(&rule_expression_json)),
    );
    insert_i64(&mut record, "enabled", bool_as_i64(enabled));
    insert_i64(&mut record, "applied_count", 0);
    record.insert("last_applied_at".to_string(), Value::Null);
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    insert_string(&mut record, "main_category", main_category);
    insert_string(&mut record, "sub_category", sub_category);
    insert_number_or_string(
        &mut record,
        "category_type",
        row.try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    Ok(record)
}

fn account_rule_from_postgres_row(row: PgRow) -> DbResult<AccountRuleRecord> {
    let rule_expression_json: Value = row.try_get("rule_expression")?;
    let field_scope: Value = row.try_get("field_scope")?;
    let regex_enabled: bool = row.try_get("regex_enabled")?;
    let enabled: bool = row.try_get("enabled")?;
    let mut record = Map::new();
    insert_i64(&mut record, "id", row.try_get("id")?);
    insert_i64(&mut record, "user_id", row.try_get("user_id")?);
    insert_i64(
        &mut record,
        "account_id",
        row.try_get::<Option<i64>, _>("account_id")?
            .unwrap_or_default(),
    );
    insert_string(&mut record, "name", row.try_get("name")?);
    insert_i64(
        &mut record,
        "priority",
        i64::from(row.try_get::<i32, _>("priority")?),
    );
    insert_string(
        &mut record,
        "rule_expression",
        rule_expression_string(&rule_expression_json),
    );
    insert_i64(&mut record, "regex_enabled", bool_as_i64(regex_enabled));
    insert_i64(&mut record, "enabled", bool_as_i64(enabled));
    insert_i64(&mut record, "applied_count", 0);
    record.insert("last_applied_at".to_string(), Value::Null);
    insert_i64(&mut record, "match_count", row.try_get("match_count")?);
    insert_optional_timestamp(
        &mut record,
        "last_matched_at",
        row.try_get("last_matched_at")?,
    );
    insert_string(
        &mut record,
        "account_role_scope",
        row.try_get("account_role_scope")?,
    );
    insert_string(
        &mut record,
        "transaction_type_scope",
        row.try_get("transaction_type_scope")?,
    );
    record.insert(
        "field_scope".to_string(),
        field_scope_json_array(&field_scope),
    );
    insert_string(&mut record, "source", row.try_get("source")?);
    record.insert(
        "source_key".to_string(),
        row.try_get::<Option<String>, _>("source_key")?
            .map_or(Value::Null, Value::String),
    );
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    insert_string(&mut record, "account_name", row.try_get("account_name")?);
    insert_number_or_string(
        &mut record,
        "account_type",
        row.try_get::<Option<String>, _>("account_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    insert_i64(
        &mut record,
        "account_hidden",
        bool_as_i64(row.try_get("account_hidden")?),
    );
    Ok(record)
}

fn account_from_postgres_row(row: PgRow) -> DbResult<AccountRecord> {
    let metadata: Value = row.try_get("metadata")?;
    let mut account = Map::new();
    let balance_cents: i64 = row.try_get("balance_cents")?;
    let is_active: bool = row.try_get("is_active")?;

    insert_i64(&mut account, "id", row.try_get("id")?);
    insert_i64(&mut account, "user_id", row.try_get("user_id")?);
    insert_string(&mut account, "name", row.try_get("name")?);
    insert_number_or_string(
        &mut account,
        "type",
        row.try_get::<Option<String>, _>("account_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    account.insert(
        "category".to_string(),
        metadata_value_or_default(&metadata, "category", Value::Number(Number::from(0))),
    );
    insert_string(
        &mut account,
        "currency",
        row.try_get::<Option<String>, _>("currency")?
            .unwrap_or_else(|| "CNY".to_string()),
    );
    account.insert(
        "icon".to_string(),
        metadata_value_or_default(&metadata, "icon", Value::String(String::new())),
    );
    account.insert(
        "color".to_string(),
        metadata_value_or_default(&metadata, "color", Value::String(String::new())),
    );
    account.insert(
        "balance".to_string(),
        json_number(balance_cents as f64 / 100.0),
    );
    account.insert(
        "initial_balance".to_string(),
        metadata_value_or_default(
            &metadata,
            "initial_balance",
            json_number(balance_cents as f64 / 100.0),
        ),
    );
    account.insert("hidden".to_string(), Value::Bool(!is_active));
    insert_i64(
        &mut account,
        "display_order",
        i64::from(row.try_get::<i32, _>("display_order")?),
    );
    account.insert(
        "comment".to_string(),
        metadata_value_or_default(&metadata, "comment", Value::String(String::new())),
    );
    account.insert(
        "aliases".to_string(),
        metadata_value_or_default(&metadata, "aliases", Value::Array(Vec::new())),
    );
    account.insert(
        "parent_id".to_string(),
        metadata_value_or_default(&metadata, "parent_id", Value::Number(Number::from(0))),
    );
    account.insert(
        "credit_card_statement_date".to_string(),
        metadata_value_or_default(&metadata, "credit_card_statement_date", Value::Null),
    );
    insert_timestamp(&mut account, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut account, "updated_at", row.try_get("updated_at")?);
    if !row
        .try_get::<Option<String>, _>("payment_method")?
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        account.insert(
            "payment_method".to_string(),
            Value::String(row.try_get::<Option<String>, _>("payment_method")?.unwrap()),
        );
    }
    Ok(account)
}

fn category_from_postgres_row(row: PgRow) -> DbResult<CategoryRecord> {
    let metadata: Value = row.try_get("metadata")?;
    let path = row
        .try_get::<Option<String>, _>("path")?
        .unwrap_or_default();
    let name: String = row.try_get("name")?;
    let (main_category, sub_category) = category_names_from_path(&path, &name);
    let is_active: bool = row.try_get("is_active")?;

    let mut category = Map::new();
    insert_i64(&mut category, "id", row.try_get("id")?);
    insert_i64(&mut category, "user_id", row.try_get("user_id")?);
    insert_number_or_string(
        &mut category,
        "type",
        row.try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .unwrap_or_default(),
    );
    insert_string(&mut category, "main_category", main_category);
    insert_string(&mut category, "sub_category", sub_category);
    category.insert(
        "description".to_string(),
        metadata_value_or_default(&metadata, "description", Value::String(String::new())),
    );
    insert_i64(
        &mut category,
        "priority",
        i64::from(row.try_get::<i32, _>("display_order")?),
    );
    category.insert(
        "keywords".to_string(),
        metadata_value_or_default(&metadata, "keywords", Value::String(String::new())),
    );
    category.insert("hidden".to_string(), Value::Bool(!is_active));
    category.insert(
        "icon".to_string(),
        row.try_get::<Option<String>, _>("icon")?
            .map_or(Value::String(String::new()), Value::String),
    );
    category.insert(
        "color".to_string(),
        row.try_get::<Option<String>, _>("color")?
            .map_or(Value::String(String::new()), Value::String),
    );
    insert_timestamp(&mut category, "created_at", row.try_get("created_at")?);
    Ok(category)
}

fn tag_from_postgres_row(row: PgRow) -> DbResult<TagRecord> {
    let metadata: Value = row.try_get("metadata")?;
    Ok(TagRecord {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        name: row.try_get("name")?,
        color: row.try_get("color")?,
        icon: metadata
            .get("icon")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        display_order: i64::from(row.try_get::<i32, _>("display_order")?),
        hidden: metadata_bool(&metadata, "hidden").map_or(0, i64::from),
        created_at: timestamp_to_string(row.try_get("created_at")?),
        updated_at: timestamp_to_string(row.try_get("updated_at")?),
    })
}

fn template_from_postgres_row(row: PgRow) -> DbResult<TemplateRecord> {
    let template_type: i32 = row.try_get("template_type")?;
    let tag_ids: Value = row.try_get("tag_ids")?;
    let mut template = Map::new();
    template.insert(
        "id".to_string(),
        Value::String(row.try_get::<i64, _>("id")?.to_string()),
    );
    template.insert(
        "templateType".to_string(),
        Value::Number(Number::from(i64::from(template_type))),
    );
    template.insert("name".to_string(), Value::String(row.try_get("name")?));
    template.insert(
        "description".to_string(),
        optional_string_value(row.try_get("description")?),
    );
    template.insert(
        "type".to_string(),
        Value::Number(Number::from(normalize_template_transaction_type(
            row.try_get::<Option<String>, _>("transaction_type")?
                .as_deref(),
        ))),
    );
    template.insert(
        "categoryId".to_string(),
        string_or_default(row.try_get("category_id")?, ""),
    );
    template.insert(
        "sourceAccountId".to_string(),
        string_or_default(row.try_get("source_account_id")?, "0"),
    );
    template.insert(
        "destinationAccountId".to_string(),
        string_or_default(row.try_get("destination_account_id")?, "0"),
    );
    template.insert(
        "sourceAmount".to_string(),
        Value::Number(Number::from(
            row.try_get::<i64, _>("source_amount_minor_units")?,
        )),
    );
    template.insert(
        "destinationAmount".to_string(),
        Value::Number(Number::from(
            row.try_get::<i64, _>("destination_amount_minor_units")?,
        )),
    );
    template.insert(
        "hideAmount".to_string(),
        Value::Bool(row.try_get("hide_amount")?),
    );
    template.insert(
        "tagIds".to_string(),
        tag_ids
            .as_array()
            .cloned()
            .map_or(Value::Array(Vec::new()), Value::Array),
    );
    template.insert(
        "comment".to_string(),
        optional_string_value(row.try_get("comment")?),
    );
    template.insert("editable".to_string(), Value::Bool(true));
    template.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(i64::from(
            row.try_get::<i32, _>("display_order")?,
        ))),
    );
    template.insert("hidden".to_string(), Value::Bool(row.try_get("hidden")?));
    template.insert(
        "scheduledFrequencyType".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<i32>, _>("scheduled_frequency_type")?
                .map(|value| Value::Number(Number::from(i64::from(value)))),
        ),
    );
    template.insert(
        "scheduledFrequency".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<String>, _>("scheduled_frequency")?
                .map(Value::String),
        ),
    );
    template.insert(
        "scheduledStartDate".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<String>, _>("scheduled_start_date")?
                .map(Value::String),
        ),
    );
    template.insert(
        "scheduledEndDate".to_string(),
        scheduled_value(
            template_type,
            row.try_get::<Option<String>, _>("scheduled_end_date")?
                .map(Value::String),
        ),
    );
    template.insert("scheduledAt".to_string(), Value::Null);
    template.insert("enabled".to_string(), Value::Bool(row.try_get("enabled")?));
    template.insert(
        "autoCreate".to_string(),
        Value::Bool(row.try_get("auto_create")?),
    );
    template.insert(
        "utcOffset".to_string(),
        Value::Number(Number::from(i64::from(
            row.try_get::<i32, _>("utc_offset")?,
        ))),
    );
    Ok(template)
}

fn category_names_from_path(path: &str, name: &str) -> (String, String) {
    let parts = path
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

fn metadata_value_or_default(metadata: &Value, key: &str, default: Value) -> Value {
    metadata.get(key).cloned().unwrap_or(default)
}

fn metadata_bool(metadata: &Value, key: &str) -> Option<bool> {
    metadata.get(key).and_then(|value| match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" | "" => Some(false),
                _ => None,
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    })
}

fn insert_i64(record: &mut Map<String, Value>, key: &str, value: i64) {
    record.insert(key.to_string(), Value::Number(Number::from(value)));
}

fn insert_string(record: &mut Map<String, Value>, key: &str, value: String) {
    record.insert(key.to_string(), Value::String(value));
}

fn insert_number_or_string(record: &mut Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if let Ok(number) = trimmed.parse::<i64>() {
        record.insert(key.to_string(), Value::Number(Number::from(number)));
    } else {
        record.insert(key.to_string(), Value::String(trimmed.to_string()));
    }
}

fn insert_timestamp(record: &mut Map<String, Value>, key: &str, value: DateTime<Utc>) {
    record.insert(key.to_string(), Value::String(timestamp_to_string(value)));
}

fn insert_optional_timestamp(
    record: &mut Map<String, Value>,
    key: &str,
    value: Option<DateTime<Utc>>,
) {
    record.insert(
        key.to_string(),
        value.map_or(Value::Null, |value| {
            Value::String(timestamp_to_string(value))
        }),
    );
}

fn timestamp_to_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map_or_else(|| Value::String(String::new()), Value::String)
}

fn string_or_default(value: Option<String>, default: &str) -> Value {
    Value::String(value.unwrap_or_else(|| default.to_string()))
}

fn scheduled_value(template_type: i32, value: Option<Value>) -> Value {
    if template_type == 2 {
        value.unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

fn normalize_template_transaction_type(value: Option<&str>) -> i64 {
    match value.unwrap_or_default().trim().to_lowercase().as_str() {
        "2" | "income" | "收入" => 2,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn json_number(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn template_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units::BIGINT AS source_amount_minor_units,
            destination_amount_minor_units::BIGINT AS destination_amount_minor_units,
            hide_amount,
            tag_ids, comment, scheduled_frequency_type, scheduled_frequency,
            scheduled_start_date, scheduled_end_date, scheduled_next_date,
            enabled, auto_create, display_order, hidden, utc_offset, created_at, updated_at
        FROM transaction_templates
        {where_clause}
        "#
    )
}

fn category_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at
        FROM categories
        {where_clause}
        "#
    )
}

async fn next_template_display_order(
    pool: &PostgresPool,
    user_id: i64,
    template_type: i64,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(MAX(display_order), 0)::BIGINT AS max_order
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = $2
        "#,
    )
    .bind(user_id)
    .bind(i64_to_i32(template_type))
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("max_order")? + 1)
}

async fn insert_postgres_account(
    transaction: &mut Transaction<'_, Postgres>,
    payload: &Value,
    user_id: i64,
    parent_id: Option<i64>,
) -> DbResult<i64> {
    let name = value_text(payload.get("name"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DbError::InvalidOperation("account name is required".to_string()))?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE user_id = $1 AND name = $2)",
    )
    .bind(user_id)
    .bind(&name)
    .fetch_one(&mut **transaction)
    .await?;
    if exists {
        return Err(DbError::InvalidOperation(format!(
            "account name already exists: {name}"
        )));
    }
    let metadata = account_metadata_from_payload(None, payload);
    let row = sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents, is_active,
            display_order, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
    "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(value_text(payload.get("type")))
    .bind(value_text(payload.get("currency")).unwrap_or_else(|| "CNY".to_string()))
    .bind(yuan_value_to_cents(payload.get("balance")).unwrap_or_default())
    .bind(!payload.get("hidden").is_some_and(value_truthy))
    .bind(i64_to_i32(
        int_value(payload.get("display_order")).unwrap_or_default(),
    ))
    .bind(metadata_with_parent_id(metadata, parent_id))
    .fetch_one(&mut **transaction)
    .await?;
    let account_id: i64 = row.try_get("id")?;

    if let Some(sub_accounts) = payload.get("subAccounts").and_then(Value::as_array) {
        for sub_account in sub_accounts {
            Box::pin(insert_postgres_account(
                transaction,
                sub_account,
                user_id,
                Some(account_id),
            ))
            .await?;
        }
    }

    Ok(account_id)
}

#[derive(Debug)]
struct TemplateValues {
    name: String,
    description: Option<String>,
    transaction_type: Option<String>,
    category_id: Option<String>,
    source_account_id: Option<String>,
    destination_account_id: Option<String>,
    source_amount_minor_units: i64,
    destination_amount_minor_units: i64,
    hide_amount: bool,
    tag_ids: Value,
    comment: Option<String>,
    scheduled_frequency_type: Option<i64>,
    scheduled_frequency: Option<String>,
    scheduled_start_date: Option<String>,
    scheduled_end_date: Option<String>,
    scheduled_next_date: Option<String>,
    enabled: bool,
    auto_create: bool,
    display_order: i64,
    hidden: bool,
    utc_offset: i64,
}

fn template_values_from_payload(
    payload: &Value,
    template_type: i64,
    create_display_order: i64,
    existing: Option<&TemplateRecord>,
) -> TemplateValues {
    let display_order = payload
        .get("displayOrder")
        .and_then(|value| int_value(Some(value)))
        .or_else(|| existing.and_then(|record| int_value(record.get("displayOrder"))))
        .unwrap_or(create_display_order);
    let scheduled = template_type == 2;
    TemplateValues {
        name: value_text(payload.get("name"))
            .or_else(|| existing.and_then(|record| value_text(record.get("name"))))
            .unwrap_or_default(),
        description: optional_text_field(payload, existing, "description"),
        transaction_type: optional_text_field(payload, existing, "type"),
        category_id: optional_text_field(payload, existing, "categoryId"),
        source_account_id: optional_text_field(payload, existing, "sourceAccountId")
            .or_else(|| Some("0".to_string())),
        destination_account_id: optional_text_field(payload, existing, "destinationAccountId")
            .or_else(|| Some("0".to_string())),
        source_amount_minor_units: payload
            .get("sourceAmount")
            .map(rounded_minor_units)
            .or_else(|| {
                existing.and_then(|record| record.get("sourceAmount").map(rounded_minor_units))
            })
            .unwrap_or_default(),
        destination_amount_minor_units: payload
            .get("destinationAmount")
            .map(rounded_minor_units)
            .or_else(|| {
                existing.and_then(|record| record.get("destinationAmount").map(rounded_minor_units))
            })
            .unwrap_or_default(),
        hide_amount: payload
            .get("hideAmount")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("hideAmount").map(value_truthy)))
            .unwrap_or(false),
        tag_ids: payload
            .get("tagIds")
            .cloned()
            .or_else(|| existing.and_then(|record| record.get("tagIds").cloned()))
            .filter(Value::is_array)
            .unwrap_or_else(|| Value::Array(Vec::new())),
        comment: optional_text_field(payload, existing, "comment"),
        scheduled_frequency_type: scheduled
            .then(|| {
                payload
                    .get("scheduledFrequencyType")
                    .and_then(|value| int_value(Some(value)))
                    .or_else(|| {
                        existing
                            .and_then(|record| record.get("scheduledFrequencyType"))
                            .and_then(|value| int_value(Some(value)))
                    })
            })
            .flatten(),
        scheduled_frequency: scheduled
            .then(|| optional_text_field(payload, existing, "scheduledFrequency"))
            .flatten(),
        scheduled_start_date: scheduled
            .then(|| optional_text_field(payload, existing, "scheduledStartDate"))
            .flatten(),
        scheduled_end_date: scheduled
            .then(|| optional_text_field(payload, existing, "scheduledEndDate"))
            .flatten(),
        scheduled_next_date: scheduled
            .then(|| {
                optional_text_field(payload, existing, "scheduledNextDate")
                    .or_else(|| optional_text_field(payload, existing, "scheduledStartDate"))
            })
            .flatten(),
        enabled: payload
            .get("enabled")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("enabled").map(value_truthy)))
            .unwrap_or(true),
        auto_create: payload
            .get("autoCreate")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("autoCreate").map(value_truthy)))
            .unwrap_or(false),
        display_order,
        hidden: payload
            .get("hidden")
            .map(value_truthy)
            .or_else(|| existing.and_then(|record| record.get("hidden").map(value_truthy)))
            .unwrap_or(false),
        utc_offset: payload
            .get("utcOffset")
            .and_then(|value| int_value(Some(value)))
            .or_else(|| existing.and_then(|record| int_value(record.get("utcOffset"))))
            .unwrap_or_default(),
    }
}

fn optional_text_field(
    payload: &Value,
    existing: Option<&TemplateRecord>,
    key: &str,
) -> Option<String> {
    if payload.get(key).is_some() {
        return value_text(payload.get(key));
    }
    existing.and_then(|record| value_text(record.get(key)))
}

fn tag_metadata_from_payload(existing: Option<&Value>, payload: &Value) -> Value {
    let mut metadata = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if payload.get("icon").is_some() || existing.is_none() {
        metadata.insert(
            "icon".to_string(),
            value_text(payload.get("icon"))
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
    }
    if payload.get("hidden").is_some() || existing.is_none() {
        metadata.insert(
            "hidden".to_string(),
            Value::Bool(payload.get("hidden").is_some_and(value_truthy)),
        );
    }
    Value::Object(metadata)
}

fn account_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, name, account_type, payment_method, currency,
            balance_cents, is_active, display_order, metadata, created_at, updated_at
        FROM accounts
        {where_clause}
        "#
    )
}

fn account_metadata_from_payload(existing: Option<&Value>, payload: &Value) -> Value {
    let mut metadata = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for key in [
        "category",
        "icon",
        "color",
        "comment",
        "aliases",
        "parent_id",
        "initial_balance",
        "credit_card_statement_date",
    ] {
        if let Some(value) = payload.get(key) {
            metadata.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(metadata)
}

fn metadata_with_parent_id(metadata: Value, parent_id: Option<i64>) -> Value {
    let Some(parent_id) = parent_id else {
        return metadata;
    };
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    object.insert(
        "parent_id".to_string(),
        Value::Number(Number::from(parent_id)),
    );
    Value::Object(object)
}

#[derive(Debug)]
struct CategoryValues {
    main_category: String,
    sub_category: String,
    name: String,
    category_type: Option<String>,
    path: String,
    icon: Option<String>,
    color: Option<String>,
    display_order: i64,
    hidden: bool,
    metadata: Value,
}

fn category_values_from_payload(
    payload: &Value,
    existing: Option<&CategoryRecord>,
) -> CategoryValues {
    let main_category = value_text(payload.get("main_category"))
        .or_else(|| existing.and_then(|record| value_text(record.get("main_category"))))
        .unwrap_or_default();
    let sub_category = value_text(payload.get("sub_category"))
        .or_else(|| existing.and_then(|record| value_text(record.get("sub_category"))))
        .unwrap_or_default();
    let name = if sub_category.trim().is_empty() {
        main_category.trim().to_string()
    } else {
        sub_category.trim().to_string()
    };
    let category_type = value_text(payload.get("type"))
        .or_else(|| existing.and_then(|record| value_text(record.get("type"))));
    let display_order = int_value(payload.get("priority"))
        .or_else(|| int_value(payload.get("displayOrder")))
        .or_else(|| existing.and_then(|record| int_value(record.get("priority"))))
        .unwrap_or_default();
    let hidden = payload
        .get("hidden")
        .map(value_truthy)
        .or_else(|| existing.and_then(|record| record.get("hidden").map(value_truthy)))
        .unwrap_or(false);
    let icon = optional_text_for_category(payload, existing, "icon");
    let color = optional_text_for_category(payload, existing, "color");
    let metadata = category_metadata_from_payload(existing, payload);
    CategoryValues {
        path: category_path(&main_category, &sub_category),
        main_category,
        sub_category,
        name,
        category_type,
        icon,
        color,
        display_order,
        hidden,
        metadata,
    }
}

async fn parent_category_id_for_values(
    pool: &PostgresPool,
    user_id: i64,
    values: &CategoryValues,
) -> DbResult<Option<i64>> {
    if values.sub_category.trim().is_empty() {
        return Ok(None);
    }
    let row = sqlx::query("SELECT id FROM categories WHERE user_id = $1 AND path = $2")
        .bind(user_id)
        .bind(values.main_category.trim())
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("id"))
        .transpose()
        .map_err(Into::into)
}

fn category_path(main_category: &str, sub_category: &str) -> String {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if sub_category.is_empty() {
        main_category.to_string()
    } else {
        format!("{main_category}/{sub_category}")
    }
}

fn category_metadata_from_payload(existing: Option<&CategoryRecord>, payload: &Value) -> Value {
    let mut metadata = Map::new();
    if let Some(existing) = existing {
        for key in ["description", "keywords"] {
            if let Some(value) = existing.get(key) {
                metadata.insert(key.to_string(), value.clone());
            }
        }
    }
    for key in ["description", "keywords"] {
        if let Some(value) = payload.get(key) {
            metadata.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(metadata)
}

fn optional_text_for_category(
    payload: &Value,
    existing: Option<&CategoryRecord>,
    key: &str,
) -> Option<String> {
    if payload.get(key).is_some() {
        return value_text(payload.get(key)).filter(|value| !value.is_empty());
    }
    existing
        .and_then(|record| value_text(record.get(key)))
        .filter(|value| !value.is_empty())
}

fn value_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        Some(Value::Null) | None => None,
        Some(value @ (Value::Array(_) | Value::Object(_))) => Some(value.to_string()),
    }
}

fn int_value(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(flag)) => Some(i64::from(*flag)),
        Some(Value::Null) | None | Some(Value::Array(_) | Value::Object(_)) => None,
    }
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "" | "0" | "false" | "none" | "null")
        }
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Null => false,
    }
}

fn bool_as_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn payload_scalar_text(value: &Value) -> DbResult<String> {
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

fn payload_bool_value(value: &Value) -> DbResult<bool> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        Value::Number(number) => number.as_i64().map(|value| value != 0).ok_or_else(|| {
            DbError::InvalidOperation("boolean field must be an integer".to_string())
        }),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" | "" => Ok(false),
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

fn required_postgres_i64(value: &Value, field: &str) -> DbResult<i64> {
    int_value(Some(value)).ok_or_else(|| {
        DbError::InvalidOperation(format!("{field} must be an integer-compatible value"))
    })
}

fn required_postgres_rule_expression(value: Option<&Value>) -> DbResult<String> {
    match value {
        Some(Value::Null) | None => Err(DbError::InvalidOperation(
            "rule_expression is required".to_string(),
        )),
        Some(value) => {
            let expression = payload_scalar_text(value)?;
            if expression.trim().is_empty() {
                Err(DbError::InvalidOperation(
                    "rule_expression is required".to_string(),
                ))
            } else {
                Ok(expression)
            }
        }
    }
}

fn legacy_rule_expression_json(expression: &str, regex_enabled: bool) -> Value {
    serde_json::json!({
        "legacy_expression": expression,
        "regex_enabled": regex_enabled,
    })
}

fn rule_expression_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(object) => object
            .get("legacy_expression")
            .or_else(|| object.get("rule_expression"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| contains_any_expression(object))
            .unwrap_or_default(),
        Value::Null => String::new(),
        Value::Number(_) | Value::Bool(_) | Value::Array(_) => value.to_string(),
    }
}

fn rule_expression_regex_enabled(value: &Value) -> bool {
    value
        .get("regex_enabled")
        .map(value_truthy)
        .unwrap_or(false)
}

fn contains_any_expression(object: &Map<String, Value>) -> Option<String> {
    let operator = object
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !operator.eq_ignore_ascii_case("contains_any") {
        return None;
    }
    let values = object
        .get("values")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|value| match value {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        })
        .filter(|value| !value.is_empty())
        .map(|value| escape_rule_expression_term(&value))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| format!("OR={{{}}}", values.join(",")))
}

fn json_array(values: &[&str]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| Value::String((*value).to_string()))
            .collect(),
    )
}

fn field_scope_json_array(value: &Value) -> Value {
    Value::Array(
        normalize_account_rule_field_scope(Some(value))
            .unwrap_or_else(|_| {
                DEFAULT_FIELD_SCOPES
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect()
            })
            .into_iter()
            .map(Value::String)
            .collect(),
    )
}

async fn postgres_category_belongs_to_user(
    pool: &PostgresPool,
    category_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM categories WHERE id = $1 AND user_id = $2)",
    )
    .bind(category_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn postgres_account_belongs_to_user(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = $1 AND user_id = $2)",
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

fn normalize_legacy_category_rule_type(raw_type: &str) -> Option<i64> {
    match raw_type.trim().parse::<i64>().ok()? {
        1 => Some(3),
        2..=5 => Some(raw_type.trim().parse::<i64>().ok()?),
        _ => None,
    }
}

async fn count_postgres_account_rules(
    pool: &PostgresPool,
    rule_ids: &BTreeSet<i64>,
    user_id: i64,
) -> DbResult<i64> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT AS count FROM account_rules WHERE user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" AND id IN (");
    let mut separated = builder.separated(", ");
    for rule_id in rule_ids {
        separated.push_bind(rule_id);
    }
    separated.push_unseparated(")");
    let row = builder.build().fetch_one(pool).await?;
    row.try_get("count").map_err(Into::into)
}

fn account_rule_candidate_from_record(record: AccountRuleRecord) -> DbResult<AccountRuleCandidate> {
    Ok(AccountRuleCandidate {
        rule_id: int_value(record.get("id")).unwrap_or_default(),
        account_id: int_value(record.get("account_id")).unwrap_or_default(),
        account_role_scope: normalize_account_role_scope(
            record.get("account_role_scope").and_then(Value::as_str),
        )
        .map_err(DbError::InvalidOperation)?,
        transaction_type_scope: normalize_transaction_type_scope(
            record.get("transaction_type_scope").and_then(Value::as_str),
        )
        .map_err(DbError::InvalidOperation)?,
        field_scope: normalize_account_rule_field_scope(record.get("field_scope"))
            .map_err(DbError::InvalidOperation)?,
        rule_expression: record
            .get("rule_expression")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        regex_enabled: record.get("regex_enabled").is_some_and(value_truthy),
        enabled: record.get("enabled").map(value_truthy).unwrap_or(true),
        priority: int_value(record.get("priority")).unwrap_or(100),
    })
}

fn aliases_from_metadata(metadata: &Value) -> Vec<String> {
    match metadata.get("aliases") {
        Some(Value::Array(values)) => values
            .iter()
            .flat_map(|value| match value {
                Value::String(text) => parse_aliases_text(text),
                Value::Number(number) => vec![number.to_string()],
                Value::Bool(flag) => vec![flag.to_string()],
                Value::Null | Value::Array(_) | Value::Object(_) => Vec::new(),
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        Some(Value::String(text)) => parse_aliases_text(text),
        Some(Value::Number(number)) => vec![number.to_string()],
        Some(Value::Bool(flag)) => vec![flag.to_string()],
        Some(Value::Null | Value::Object(_)) | None => Vec::new(),
    }
}

async fn postgres_account_alias_rule_exists(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
    source_key: &str,
    alias: &str,
) -> DbResult<bool> {
    if sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM account_rules
            WHERE user_id = $1
              AND account_id = $2
              AND source IN ('alias_migration', 'legacy_account_aliases')
              AND source_key = $3
        )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(source_key)
    .fetch_one(pool)
    .await?
    {
        return Ok(true);
    }

    let rows = sqlx::query(
        r#"
        SELECT rule_expression
        FROM account_rules
        WHERE user_id = $1
          AND account_id = $2
          AND source IN ('alias_migration', 'legacy_account_aliases')
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_all(pool)
    .await?;
    let normalized_alias = alias.trim().to_ascii_lowercase();
    for row in rows {
        let expression: Value = row.try_get("rule_expression")?;
        if normalize_rule_expression_alias(&rule_expression_string(&expression)) == normalized_alias
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn normalize_rule_expression_alias(expression: &str) -> String {
    expression
        .trim()
        .strip_prefix("OR={")
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(expression)
        .replace('\\', "")
        .trim()
        .to_ascii_lowercase()
}

fn rounded_minor_units(value: &Value) -> i64 {
    let text = value_text(Some(value)).unwrap_or_default();
    round_decimal_text_to_i64(&text).unwrap_or_default()
}

fn yuan_value_to_cents(value: Option<&Value>) -> Option<i64> {
    let text = value_text(value)?;
    decimal_text_to_scaled_i64(&text, 2)
}

fn round_decimal_text_to_i64(text: &str) -> Option<i64> {
    decimal_text_to_scaled_i64(text, 0)
}

fn decimal_text_to_scaled_i64(text: &str, scale: usize) -> Option<i64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (negative, magnitude) = trimmed
        .strip_prefix('-')
        .map_or((false, trimmed), |rest| (true, rest));
    let magnitude = magnitude.strip_prefix('+').unwrap_or(magnitude);
    let mut parts = magnitude.splitn(2, '.');
    let integer = parts.next()?.parse::<i64>().ok()?;
    let multiplier = 10_i64.checked_pow(u32::try_from(scale).ok()?)?;
    let fraction = parts.next().unwrap_or_default();
    let mut scaled_fraction = 0_i64;
    let mut consumed = 0_usize;
    let mut round_digit = '0';
    for digit in fraction
        .chars()
        .filter(|character| character.is_ascii_digit())
    {
        if consumed < scale {
            scaled_fraction = scaled_fraction
                .checked_mul(10)?
                .checked_add(i64::from(digit.to_digit(10)?))?;
            consumed += 1;
        } else {
            round_digit = digit;
            break;
        }
    }
    for _ in consumed..scale {
        scaled_fraction = scaled_fraction.checked_mul(10)?;
    }
    let scaled = integer
        .checked_mul(multiplier)?
        .checked_add(scaled_fraction)?
        .checked_add(i64::from(round_digit >= '5'))?;
    Some(if negative { -scaled } else { scaled })
}

fn i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_names_from_postgres_path_preserve_legacy_main_and_sub_fields() {
        assert_eq!(
            category_names_from_path("餐饮/午餐", "午餐"),
            ("餐饮".to_string(), "午餐".to_string())
        );
        assert_eq!(
            category_names_from_path("餐饮", "餐饮"),
            ("餐饮".to_string(), String::new())
        );
        assert_eq!(
            category_names_from_path("", "未分类"),
            ("未分类".to_string(), String::new())
        );
    }

    #[test]
    fn metadata_bool_accepts_legacy_json_encodings() {
        assert_eq!(
            metadata_bool(&serde_json::json!({"hidden": true}), "hidden"),
            Some(true)
        );
        assert_eq!(
            metadata_bool(&serde_json::json!({"hidden": 0}), "hidden"),
            Some(false)
        );
        assert_eq!(
            metadata_bool(&serde_json::json!({"hidden": "false"}), "hidden"),
            Some(false)
        );
    }

    #[test]
    fn postgres_template_type_normalization_matches_frontend_contract() {
        assert_eq!(normalize_template_transaction_type(Some("收入")), 2);
        assert_eq!(normalize_template_transaction_type(Some("transfer")), 4);
        assert_eq!(normalize_template_transaction_type(Some("投资")), 5);
        assert_eq!(normalize_template_transaction_type(None), 3);
    }

    #[test]
    fn postgres_template_minor_units_round_without_float_precision() {
        assert_eq!(round_decimal_text_to_i64("18.49"), Some(18));
        assert_eq!(round_decimal_text_to_i64("18.5"), Some(19));
        assert_eq!(round_decimal_text_to_i64("-18.5"), Some(-19));
        assert_eq!(round_decimal_text_to_i64("1999"), Some(1999));
    }

    #[test]
    fn postgres_rule_helpers_preserve_legacy_expression_and_alias_edges() {
        assert_eq!(
            rule_expression_string(&serde_json::json!({
                "operator": "contains_any",
                "values": ["招商", "余额+宝", 123, true, null]
            })),
            r"OR={招商,余额\+宝,123,true}"
        );
        assert_eq!(
            rule_expression_string(&serde_json::json!({
                "legacy_expression": "OR={午餐}",
                "regex_enabled": "true"
            })),
            "OR={午餐}"
        );
        assert!(rule_expression_regex_enabled(&serde_json::json!({
            "regex_enabled": "true"
        })));
        assert!(payload_bool_value(&serde_json::json!("on")).expect("truthy string"));
        assert!(!payload_bool_value(&serde_json::json!(0)).expect("false numeric"));
        assert!(payload_bool_value(&serde_json::json!(1.5)).is_err());
        assert_eq!(
            required_postgres_i64(&serde_json::json!("42"), "field").expect("int text"),
            42
        );
        assert!(required_postgres_rule_expression(Some(&serde_json::json!(" "))).is_err());
        assert_eq!(
            field_scope_json_array(&serde_json::json!("parser,payment_method")),
            serde_json::json!(["parser", "payment_method"])
        );
        assert_eq!(
            aliases_from_metadata(&serde_json::json!({
                "aliases": ["主卡,工资", 123, true, null, {"bad": true}]
            })),
            vec!["主卡", "工资", "123", "true"]
        );
        assert_eq!(normalize_rule_expression_alias(r" OR={\子卡} "), "子卡");
        assert_eq!(normalize_legacy_category_rule_type("1"), Some(3));
        assert_eq!(normalize_legacy_category_rule_type("5"), Some(5));
        assert_eq!(normalize_legacy_category_rule_type("9"), None);
    }
}
