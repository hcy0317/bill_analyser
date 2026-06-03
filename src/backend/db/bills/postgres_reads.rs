// 中文导读：PostgreSQL bills 读仓储，负责 当前交易列表与详情投影。
// 维护重点：金额从 PostgreSQL 分转换成 current record 的元字段，handler 不复制 SQL。
// 不变式：所有查询必须按 user_id 过滤且忽略 is_deleted，不允许回退 non-Postgres。

use bill_analyser_core::adapters::transaction::{
    validate_batch_route_update_fields, ReconciliationCategoryRecord,
};
use bill_analyser_core::{parse_bill_datetime, Money};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Number, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row};

use crate::{
    calculate_bill_hash_from_record, BatchUpdateBillsResult, BillCategoryFilter, BillCreateDraft,
    BillFilters, BillPage, BillRecord, BillUpdateDraft, DbError, DbResult, PostgresPool,
};

const MAIN_CATEGORY_EXPR: &str = "COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(c.path, '/', 1), ''), c.name, '')";
const SUB_CATEGORY_EXPR: &str = "COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), CASE WHEN position('/' in COALESCE(c.path, '')) > 0 THEN substring(c.path from position('/' in c.path) + 1) ELSE '' END, '')";

#[derive(Debug, Clone)]
pub struct PostgresReconciliationAccount {
    pub name: String,
    pub initial_balance: Money,
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_category_filters_for_ids(
    pool: &PostgresPool,
    user_id: i64,
    category_ids: &[i64],
) -> DbResult<Vec<BillCategoryFilter>> {
    let category_ids = normalize_ids(category_ids);
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder =
        QueryBuilder::<Postgres>::new("SELECT path, name FROM categories WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND id IN (");
    push_bind_list(&mut builder, &category_ids);
    builder.push(") ORDER BY display_order ASC, id ASC");
    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main, sub) = category_names_from_path(path.as_deref(), &name);
            Ok(BillCategoryFilter {
                main,
                sub: (!sub.trim().is_empty()).then_some(sub),
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    page: usize,
    page_size: usize,
    filters: &BillFilters,
) -> DbResult<BillPage> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);
    let offset = (page - 1).saturating_mul(page_size);

    let mut count_builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT AS total FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ",
    );
    count_builder.push_bind(user_id);
    count_builder.push(" AND b.is_deleted = false");
    push_bill_filters(&mut count_builder, user_id, filters);
    let total = count_builder
        .build()
        .fetch_one(pool)
        .await?
        .try_get::<i64, _>("total")?;

    let mut list_builder = QueryBuilder::<Postgres>::new(
        "SELECT b.id, b.user_id, b.occurred_at, b.transaction_type, b.amount_cents, b.merchant, b.description, b.payment_method, ",
    );
    list_builder.push(MAIN_CATEGORY_EXPR);
    list_builder.push(" AS main_category, ");
    list_builder.push(SUB_CATEGORY_EXPR);
    list_builder.push(" AS sub_category, b.standard_payload, b.source_hash, b.created_at, b.updated_at, b.account_id, b.source_account_id, b.target_account_id, b.transfer_target_account_id, b.category_id FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ");
    list_builder.push_bind(user_id);
    list_builder.push(" AND b.is_deleted = false");
    push_bill_filters(&mut list_builder, user_id, filters);
    list_builder.push(" ORDER BY b.occurred_at DESC, b.id DESC LIMIT ");
    list_builder.push_bind(i64::try_from(page_size).unwrap_or(i64::MAX));
    list_builder.push(" OFFSET ");
    list_builder.push_bind(i64::try_from(offset).unwrap_or(i64::MAX));

    let rows = list_builder.build().fetch_all(pool).await?;
    let bills = rows
        .into_iter()
        .map(bill_record_from_postgres_row)
        .collect::<DbResult<Vec<_>>>()?;
    Ok(BillPage { bills, total })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_bill_by_id(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<BillRecord>> {
    let row = sqlx::query(&format!(
        r#"
        SELECT b.id, b.user_id, b.occurred_at, b.transaction_type, b.amount_cents,
            b.merchant, b.description, b.payment_method,
            {MAIN_CATEGORY_EXPR} AS main_category,
            {SUB_CATEGORY_EXPR} AS sub_category,
            b.standard_payload, b.source_hash, b.created_at, b.updated_at,
            b.account_id, b.source_account_id, b.target_account_id,
            b.transfer_target_account_id, b.category_id
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = $1 AND b.id = $2 AND b.is_deleted = false
        "#
    ))
    .bind(user_id)
    .bind(bill_id)
    .fetch_optional(pool)
    .await?;
    row.map(bill_record_from_postgres_row).transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_bill_tags(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.name, t.color, t.metadata
        FROM bill_tags bt
        JOIN tags t ON t.user_id = bt.user_id AND t.id = bt.tag_id
        WHERE bt.user_id = $1 AND bt.bill_id = $2
        ORDER BY t.display_order ASC, t.name ASC
        "#,
    )
    .bind(user_id)
    .bind(bill_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(tag_value_from_postgres_row).collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_first_postgres_account_id(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Option<i64>> {
    sqlx::query(
        r#"
        SELECT id
        FROM accounts
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.try_get("id").map_err(DbError::from))
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn resolve_postgres_category_by_id(
    pool: &PostgresPool,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<(String, String)>> {
    let row = sqlx::query(
        r#"
        SELECT path, name
        FROM categories
        WHERE user_id = $1 AND id = $2
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let path: Option<String> = row.try_get("path")?;
        let name: String = row.try_get("name")?;
        Ok(category_names_from_path(path.as_deref(), &name))
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_reconciliation_account(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
) -> DbResult<Option<PostgresReconciliationAccount>> {
    let row = sqlx::query(
        r#"
        SELECT name, balance_cents, metadata
        FROM accounts
        WHERE user_id = $1 AND id = $2
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let metadata: Value = row.try_get("metadata")?;
        let balance_cents: i64 = row.try_get("balance_cents")?;
        Ok(PostgresReconciliationAccount {
            name: row.try_get("name")?,
            initial_balance: postgres_initial_balance_money(&metadata, balance_cents)?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_reconciliation_categories(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ReconciliationCategoryRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, path, name
        FROM categories
        WHERE user_id = $1
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) = category_names_from_path(path.as_deref(), &name);
            Ok(ReconciliationCategoryRecord {
                id: row.try_get("id")?,
                main_category,
                sub_category,
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    draft: &BillCreateDraft,
) -> DbResult<i64> {
    let mutation = prepare_postgres_bill_mutation(pool, user_id, &draft.fields).await?;
    let mut tx = pool.begin().await?;
    let bill_id: i64 = sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, amount_cents, direction, transaction_type,
            account_id, source_account_id, target_account_id, transfer_target_account_id,
            category_id, merchant, description, payment_method, source_hash, standard_payload
        )
        VALUES ($1, $2, $3, $4, $5, $6, $6, $7, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(mutation.occurred_at)
    .bind(mutation.amount_cents)
    .bind(&mutation.direction)
    .bind(&mutation.transaction_type)
    .bind(mutation.source_account_id)
    .bind(mutation.destination_account_id)
    .bind(mutation.category_id)
    .bind(&mutation.merchant)
    .bind(&mutation.description)
    .bind(&mutation.payment_method)
    .bind(&mutation.source_hash)
    .bind(&mutation.standard_payload)
    .fetch_one(&mut *tx)
    .await?
    .try_get("id")?;
    replace_postgres_bill_tags(&mut tx, user_id, bill_id, &draft.tag_ids).await?;
    apply_postgres_balance_deltas(&mut tx, user_id, &mutation.balance_deltas()).await?;
    tx.commit().await?;
    Ok(bill_id)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn batch_create_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    drafts: &[BillCreateDraft],
) -> DbResult<Vec<i64>> {
    if drafts.is_empty() {
        return Ok(Vec::new());
    }
    let mut prepared = Vec::with_capacity(drafts.len());
    for draft in drafts {
        prepared.push(PreparedPostgresBillMutation {
            mutation: prepare_postgres_bill_mutation(pool, user_id, &draft.fields).await?,
            tag_ids: draft.tag_ids.clone(),
        });
    }

    let mut tx = pool.begin().await?;
    let mut bill_ids = Vec::with_capacity(prepared.len());
    let mut balance_deltas = Vec::new();
    for prepared_bill in prepared {
        let bill_id = insert_postgres_bill_on_tx(&mut tx, user_id, &prepared_bill.mutation).await?;
        replace_postgres_bill_tags(&mut tx, user_id, bill_id, &prepared_bill.tag_ids).await?;
        balance_deltas.extend(prepared_bill.mutation.balance_deltas());
        bill_ids.push(bill_id);
    }
    apply_postgres_balance_deltas(&mut tx, user_id, &balance_deltas).await?;
    tx.commit().await?;
    Ok(bill_ids)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
    draft: &BillUpdateDraft,
) -> DbResult<bool> {
    if draft.fields.is_empty() && draft.tag_ids.is_none() {
        return Ok(false);
    }
    let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
        return Ok(false);
    };
    let old_mutation = prepare_postgres_bill_mutation(pool, user_id, &existing).await?;
    let mut merged = existing;
    for (key, value) in &draft.fields {
        merged.insert(key.clone(), value.clone());
    }
    if update_requires_hash_recalculation(&draft.fields) && !draft.fields.contains_key("hash") {
        let hash = calculate_bill_hash_from_record(&merged)?;
        merged.insert("hash".to_string(), Value::String(hash));
    }
    let new_mutation = prepare_postgres_bill_mutation(pool, user_id, &merged).await?;

    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        r#"
        UPDATE bills
        SET occurred_at = $3,
            amount_cents = $4,
            direction = $5,
            transaction_type = $6,
            account_id = $7,
            source_account_id = $7,
            target_account_id = $8,
            transfer_target_account_id = $8,
            category_id = $9,
            merchant = $10,
            description = $11,
            payment_method = $12,
            source_hash = $13,
            standard_payload = $14,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id)
    .bind(bill_id)
    .bind(new_mutation.occurred_at)
    .bind(new_mutation.amount_cents)
    .bind(&new_mutation.direction)
    .bind(&new_mutation.transaction_type)
    .bind(new_mutation.source_account_id)
    .bind(new_mutation.destination_account_id)
    .bind(new_mutation.category_id)
    .bind(&new_mutation.merchant)
    .bind(&new_mutation.description)
    .bind(&new_mutation.payment_method)
    .bind(&new_mutation.source_hash)
    .bind(&new_mutation.standard_payload)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Ok(false);
    }
    if let Some(tag_ids) = &draft.tag_ids {
        replace_postgres_bill_tags(&mut tx, user_id, bill_id, tag_ids).await?;
    }
    let mut deltas = old_mutation
        .balance_deltas()
        .into_iter()
        .map(|(account_id, amount)| (account_id, -amount))
        .collect::<Vec<_>>();
    deltas.extend(new_mutation.balance_deltas());
    apply_postgres_balance_deltas(&mut tx, user_id, &deltas).await?;
    tx.commit().await?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn batch_update_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    bill_ids: &[i64],
    fields: &BillRecord,
) -> DbResult<BatchUpdateBillsResult> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() || fields.is_empty() {
        return Ok(BatchUpdateBillsResult::default());
    }
    validate_batch_route_update_fields(fields.keys().map(String::as_str))
        .map_err(|error| DbError::InvalidOperation(error.to_string()))?;

    let mut prepared = Vec::new();
    let mut failed_ids = Vec::new();
    for bill_id in bill_ids {
        let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
            failed_ids.push(bill_id);
            continue;
        };
        let old_mutation = prepare_postgres_bill_mutation(pool, user_id, &existing).await?;
        let mut merged = existing;
        for (key, value) in fields {
            merged.insert(key.clone(), value.clone());
        }
        if update_requires_hash_recalculation(fields) && !fields.contains_key("hash") {
            let hash = calculate_bill_hash_from_record(&merged)?;
            merged.insert("hash".to_string(), Value::String(hash));
        }
        let new_mutation = prepare_postgres_bill_mutation(pool, user_id, &merged).await?;
        prepared.push(PreparedPostgresBillUpdate {
            bill_id,
            old_mutation,
            new_mutation,
        });
    }

    let mut tx = pool.begin().await?;
    let mut success_count = 0_usize;
    let mut balance_deltas = Vec::new();
    for prepared_bill in prepared {
        let updated = update_postgres_bill_on_tx(
            &mut tx,
            user_id,
            prepared_bill.bill_id,
            &prepared_bill.new_mutation,
        )
        .await?;
        if updated == 0 {
            failed_ids.push(prepared_bill.bill_id);
            continue;
        }
        balance_deltas.extend(
            prepared_bill
                .old_mutation
                .balance_deltas()
                .into_iter()
                .map(|(account_id, amount)| (account_id, -amount)),
        );
        balance_deltas.extend(prepared_bill.new_mutation.balance_deltas());
        success_count += 1;
    }
    apply_postgres_balance_deltas(&mut tx, user_id, &balance_deltas).await?;
    tx.commit().await?;
    Ok(BatchUpdateBillsResult {
        success_count,
        failed_count: failed_ids.len(),
        failed_ids,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_bill(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<bool> {
    let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
        return Ok(false);
    };
    let old_mutation = prepare_postgres_bill_mutation(pool, user_id, &existing).await?;
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query("DELETE FROM bills WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(bill_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Ok(false);
    }
    let deltas = old_mutation
        .balance_deltas()
        .into_iter()
        .map(|(account_id, amount)| (account_id, -amount))
        .collect::<Vec<_>>();
    apply_postgres_balance_deltas(&mut tx, user_id, &deltas).await?;
    tx.commit().await?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn batch_delete_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<usize> {
    let bill_ids = normalize_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(0);
    }
    let mut prepared = Vec::new();
    for bill_id in bill_ids {
        let Some(existing) = get_postgres_bill_by_id(pool, user_id, bill_id).await? else {
            continue;
        };
        prepared.push((
            bill_id,
            prepare_postgres_bill_mutation(pool, user_id, &existing).await?,
        ));
    }
    if prepared.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;
    let mut deleted_count = 0_usize;
    let mut balance_deltas = Vec::new();
    for (bill_id, old_mutation) in prepared {
        let deleted = sqlx::query("DELETE FROM bills WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(bill_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if deleted == 0 {
            continue;
        }
        deleted_count += usize::try_from(deleted).unwrap_or(usize::MAX);
        balance_deltas.extend(
            old_mutation
                .balance_deltas()
                .into_iter()
                .map(|(account_id, amount)| (account_id, -amount)),
        );
    }
    apply_postgres_balance_deltas(&mut tx, user_id, &balance_deltas).await?;
    tx.commit().await?;
    Ok(deleted_count)
}

#[derive(Debug, Clone)]
struct PostgresBillMutation {
    occurred_at: DateTime<Utc>,
    amount_cents: i64,
    direction: String,
    transaction_type: String,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
    category_id: Option<i64>,
    merchant: Option<String>,
    description: Option<String>,
    payment_method: Option<String>,
    source_hash: Option<String>,
    standard_payload: Value,
}

#[derive(Debug, Clone)]
struct PreparedPostgresBillMutation {
    mutation: PostgresBillMutation,
    tag_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
struct PreparedPostgresBillUpdate {
    bill_id: i64,
    old_mutation: PostgresBillMutation,
    new_mutation: PostgresBillMutation,
}

impl PostgresBillMutation {
    fn balance_deltas(&self) -> Vec<(i64, i64)> {
        let mut deltas = Vec::new();
        let amount = self.amount_cents.abs();
        let destination_amount = destination_amount_yuan(&self.standard_payload)
            .map(yuan_to_cents)
            .unwrap_or(amount)
            .abs();
        match self.transaction_type.as_str() {
            "income" => {
                if let Some(account_id) = self.source_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, amount));
                }
            }
            "transfer" | "investment" => {
                if let Some(account_id) = self.source_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, -amount));
                }
                if let Some(account_id) = self.destination_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, destination_amount));
                }
            }
            _ => {
                if let Some(account_id) = self.source_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, -amount));
                }
            }
        }
        deltas
    }
}

async fn prepare_postgres_bill_mutation(
    pool: &PostgresPool,
    user_id: i64,
    fields: &BillRecord,
) -> DbResult<PostgresBillMutation> {
    let occurred_at = parse_postgres_bill_datetime(required_text(fields, "date")?)?;
    let transaction_type = canonical_transaction_type(&required_text(fields, "type")?);
    let direction = postgres_direction_for_type(&transaction_type).to_string();
    let amount_cents = amount_cents_from_record(fields, "amount")?.abs();
    let source_account_id = positive_value_i64(fields.get("source_account_id"));
    let destination_account_id = positive_value_i64(fields.get("destination_account_id"));
    let category_id = resolve_postgres_category_id_for_fields(pool, user_id, fields).await?;
    let mut standard_payload = Value::Object(fields.clone());
    if let Value::Object(payload) = &mut standard_payload {
        payload.insert(
            "main_category".to_string(),
            fields
                .get("main_category")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        payload.insert(
            "sub_category".to_string(),
            fields
                .get("sub_category")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        payload.insert(
            "destination_amount".to_string(),
            fields
                .get("destination_amount")
                .cloned()
                .unwrap_or_else(|| json_real(0.0)),
        );
    }
    let source_hash = match optional_value_string(fields.get("hash")) {
        Some(value) => Some(value),
        None => Some(calculate_bill_hash_from_record(fields)?),
    };
    Ok(PostgresBillMutation {
        occurred_at,
        amount_cents,
        direction,
        transaction_type,
        source_account_id,
        destination_account_id,
        category_id,
        merchant: optional_value_string(fields.get("counterparty")),
        description: optional_value_string(fields.get("description")),
        payment_method: optional_value_string(fields.get("payment_method")),
        source_hash,
        standard_payload,
    })
}

async fn resolve_postgres_category_id_for_fields(
    pool: &PostgresPool,
    user_id: i64,
    fields: &BillRecord,
) -> DbResult<Option<i64>> {
    let Some(main_category) = optional_value_string(fields.get("main_category")) else {
        return Ok(None);
    };
    if main_category.trim().is_empty() {
        return Ok(None);
    }
    let sub_category = optional_value_string(fields.get("sub_category")).unwrap_or_default();
    let path = if sub_category.trim().is_empty() {
        main_category.clone()
    } else {
        format!("{main_category}/{sub_category}")
    };
    sqlx::query(
        r#"
        SELECT id
        FROM categories
        WHERE user_id = $1
          AND (path = $2 OR name = $3)
        ORDER BY CASE WHEN path = $2 THEN 0 ELSE 1 END, display_order ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(path)
    .bind(if sub_category.trim().is_empty() {
        main_category
    } else {
        sub_category
    })
    .fetch_optional(pool)
    .await?
    .map(|row| row.try_get("id").map_err(DbError::from))
    .transpose()
}

async fn replace_postgres_bill_tags(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    bill_id: i64,
    tag_ids: &[i64],
) -> DbResult<()> {
    sqlx::query("DELETE FROM bill_tags WHERE user_id = $1 AND bill_id = $2")
        .bind(user_id)
        .bind(bill_id)
        .execute(&mut **tx)
        .await?;
    for tag_id in normalize_ids(tag_ids) {
        let exists = sqlx::query("SELECT 1 FROM tags WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(tag_id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some();
        if !exists {
            return Err(DbError::InvalidOperation(format!(
                "tag not found: {tag_id}"
            )));
        }
        sqlx::query(
            "INSERT INTO bill_tags (user_id, bill_id, tag_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(bill_id)
        .bind(tag_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn apply_postgres_balance_deltas(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    deltas: &[(i64, i64)],
) -> DbResult<()> {
    let mut combined: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for (account_id, delta) in deltas {
        if *account_id > 0 && *delta != 0 {
            *combined.entry(*account_id).or_default() += *delta;
        }
    }
    for (account_id, delta) in combined {
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance_cents = balance_cents + $3,
                updated_at = now(),
                version = version + 1
            WHERE user_id = $1 AND id = $2
            "#,
        )
        .bind(user_id)
        .bind(account_id)
        .bind(delta)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn insert_postgres_bill_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    mutation: &PostgresBillMutation,
) -> DbResult<i64> {
    sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, amount_cents, direction, transaction_type,
            account_id, source_account_id, target_account_id, transfer_target_account_id,
            category_id, merchant, description, payment_method, source_hash, standard_payload
        )
        VALUES ($1, $2, $3, $4, $5, $6, $6, $7, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(mutation.occurred_at)
    .bind(mutation.amount_cents)
    .bind(&mutation.direction)
    .bind(&mutation.transaction_type)
    .bind(mutation.source_account_id)
    .bind(mutation.destination_account_id)
    .bind(mutation.category_id)
    .bind(&mutation.merchant)
    .bind(&mutation.description)
    .bind(&mutation.payment_method)
    .bind(&mutation.source_hash)
    .bind(&mutation.standard_payload)
    .fetch_one(&mut **tx)
    .await?
    .try_get("id")
    .map_err(DbError::from)
}

async fn update_postgres_bill_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    bill_id: i64,
    mutation: &PostgresBillMutation,
) -> DbResult<u64> {
    sqlx::query(
        r#"
        UPDATE bills
        SET occurred_at = $3,
            amount_cents = $4,
            direction = $5,
            transaction_type = $6,
            account_id = $7,
            source_account_id = $7,
            target_account_id = $8,
            transfer_target_account_id = $8,
            category_id = $9,
            merchant = $10,
            description = $11,
            payment_method = $12,
            source_hash = $13,
            standard_payload = $14,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id)
    .bind(bill_id)
    .bind(mutation.occurred_at)
    .bind(mutation.amount_cents)
    .bind(&mutation.direction)
    .bind(&mutation.transaction_type)
    .bind(mutation.source_account_id)
    .bind(mutation.destination_account_id)
    .bind(mutation.category_id)
    .bind(&mutation.merchant)
    .bind(&mutation.description)
    .bind(&mutation.payment_method)
    .bind(&mutation.source_hash)
    .bind(&mutation.standard_payload)
    .execute(&mut **tx)
    .await
    .map(|result| result.rows_affected())
    .map_err(DbError::from)
}

fn parse_postgres_bill_datetime(value: String) -> DbResult<DateTime<Utc>> {
    let parsed = parse_bill_datetime(&value)
        .ok_or_else(|| DbError::InvalidOperation("invalid bill date time".to_string()))?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(
        parsed.inner(),
        Utc,
    ))
}

fn required_text(record: &BillRecord, key: &str) -> DbResult<String> {
    optional_value_string(record.get(key))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DbError::InvalidOperation(format!("missing required bill field: {key}")))
}

fn amount_cents_from_record(record: &BillRecord, key: &str) -> DbResult<i64> {
    let value = record
        .get(key)
        .and_then(value_to_f64)
        .ok_or_else(|| DbError::InvalidOperation(format!("missing required bill field: {key}")))?;
    Ok(yuan_to_cents(value))
}

fn positive_value_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(value_to_i64).filter(|value| *value > 0)
}

fn postgres_direction_for_type(transaction_type: &str) -> &'static str {
    if transaction_type == "income" {
        "income"
    } else {
        "expense"
    }
}

fn update_requires_hash_recalculation(fields: &BillRecord) -> bool {
    ["date", "type", "amount", "counterparty", "description"]
        .iter()
        .any(|key| fields.contains_key(*key))
}

fn push_bill_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    user_id: i64,
    filters: &BillFilters,
) {
    if let Some(id) = filters.id.filter(|value| *value > 0) {
        builder.push(" AND b.id = ");
        builder.push_bind(id);
    }
    if let Some(value) = text_filter(filters.date_from.as_deref()) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(value);
        builder.push("::date");
    }
    if let Some(value) = text_filter(filters.date_to.as_deref()) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(value);
        builder.push("::date + interval '1 day')");
    }
    if let Some(value) = text_filter(filters.transaction_type.as_deref()) {
        let canonical = canonical_transaction_type(&value);
        builder.push(" AND (b.transaction_type = ");
        builder.push_bind(canonical.clone());
        builder.push(" OR b.standard_payload->>'type' = ");
        builder.push_bind(value);
        builder.push(")");
    }
    if let Some(value) = text_filter(filters.main_category.as_deref()) {
        builder.push(" AND ");
        builder.push(MAIN_CATEGORY_EXPR);
        builder.push(" = ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.sub_category.as_deref()) {
        builder.push(" AND ");
        builder.push(SUB_CATEGORY_EXPR);
        builder.push(" = ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.batch_id.as_deref()) {
        builder.push(" AND b.standard_payload->>'batch_id' = ");
        builder.push_bind(value);
    }
    if let Some(value) = text_filter(filters.counterparty.as_deref()) {
        builder.push(" AND b.merchant ILIKE ");
        builder.push_bind(format!("%{value}%"));
    }
    if let Some(value) = text_filter(filters.description.as_deref()) {
        builder.push(" AND b.description ILIKE ");
        builder.push_bind(format!("%{value}%"));
    }
    if let Some(value) = text_filter(filters.keyword.as_deref()) {
        let pattern = format!("%{value}%");
        builder.push(" AND (b.description ILIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR b.merchant ILIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }
    let account_ids = normalize_ids(&filters.account_ids);
    if !account_ids.is_empty() {
        builder.push(" AND (b.account_id IN (");
        push_bind_list(builder, &account_ids);
        builder.push(") OR b.source_account_id IN (");
        push_bind_list(builder, &account_ids);
        builder.push(") OR b.target_account_id IN (");
        push_bind_list(builder, &account_ids);
        builder.push(") OR b.transfer_target_account_id IN (");
        push_bind_list(builder, &account_ids);
        builder.push("))");
    }
    push_category_filters(builder, filters);
    let tag_ids = normalize_ids(&filters.tag_ids);
    if !tag_ids.is_empty() {
        builder.push(" AND b.id IN (SELECT bill_id FROM bill_tags WHERE user_id = ");
        builder.push_bind(user_id);
        builder.push(" AND tag_id IN (");
        push_bind_list(builder, &tag_ids);
        builder.push("))");
    }
    if let Some(value) = filters.min_amount {
        builder.push(" AND ABS(b.amount_cents) >= ");
        builder.push_bind(yuan_to_cents(value));
    }
    if let Some(value) = filters.max_amount {
        builder.push(" AND ABS(b.amount_cents) <= ");
        builder.push_bind(yuan_to_cents(value));
    }
    if let Some(value) = text_filter(filters.amount_filter.as_deref()) {
        push_amount_filter(builder, &value);
    }
}

fn push_category_filters(builder: &mut QueryBuilder<'_, Postgres>, filters: &BillFilters) {
    let categories = filters
        .categories
        .iter()
        .filter_map(|category| {
            text_filter(Some(&category.main)).map(|main| {
                (
                    main,
                    category
                        .sub
                        .as_deref()
                        .and_then(|value| text_filter(Some(value))),
                )
            })
        })
        .collect::<Vec<_>>();
    if categories.is_empty() {
        return;
    }
    builder.push(" AND (");
    for (index, (main, sub)) in categories.into_iter().enumerate() {
        if index > 0 {
            builder.push(" OR ");
        }
        builder.push("(");
        builder.push(MAIN_CATEGORY_EXPR);
        builder.push(" = ");
        builder.push_bind(main);
        if let Some(sub) = sub {
            builder.push(" AND ");
            builder.push(SUB_CATEGORY_EXPR);
            builder.push(" = ");
            builder.push_bind(sub);
        }
        builder.push(")");
    }
    builder.push(")");
}

fn push_amount_filter(builder: &mut QueryBuilder<'_, Postgres>, amount_filter: &str) {
    let parts = amount_filter.split(':').collect::<Vec<_>>();
    if parts.len() < 2 {
        return;
    }
    let amount =
        |index: usize| -> Option<i64> { parts.get(index)?.parse::<f64>().ok().map(yuan_to_cents) };
    match parts[0].to_ascii_lowercase().as_str() {
        "eq" => push_amount_condition(builder, " = ", amount(1)),
        "ne" => push_amount_condition(builder, " != ", amount(1)),
        "gt" => push_amount_condition(builder, " > ", amount(1)),
        "lt" => push_amount_condition(builder, " < ", amount(1)),
        "gte" => push_amount_condition(builder, " >= ", amount(1)),
        "lte" => push_amount_condition(builder, " <= ", amount(1)),
        "between" => {
            if let (Some(minimum), Some(maximum)) = (amount(1), amount(2)) {
                builder.push(" AND ABS(b.amount_cents) BETWEEN ");
                builder.push_bind(minimum);
                builder.push(" AND ");
                builder.push_bind(maximum);
            }
        }
        _ => {}
    }
}

fn push_amount_condition(
    builder: &mut QueryBuilder<'_, Postgres>,
    operator: &'static str,
    amount: Option<i64>,
) {
    if let Some(amount) = amount {
        builder.push(" AND ABS(b.amount_cents)");
        builder.push(operator);
        builder.push_bind(amount);
    }
}

fn push_bind_list(builder: &mut QueryBuilder<'_, Postgres>, values: &[i64]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            builder.push(", ");
        }
        builder.push_bind(*value);
    }
}

fn bill_record_from_postgres_row(row: PgRow) -> DbResult<BillRecord> {
    let standard_payload: Value = row.try_get("standard_payload")?;
    let amount_cents: i64 = row.try_get("amount_cents")?;
    let destination_amount = destination_amount_yuan(&standard_payload);
    let source_account_id = first_positive([
        row.try_get::<Option<i64>, _>("source_account_id")?,
        row.try_get::<Option<i64>, _>("account_id")?,
    ]);
    let destination_account_id = first_positive([
        row.try_get::<Option<i64>, _>("target_account_id")?,
        row.try_get::<Option<i64>, _>("transfer_target_account_id")?,
    ]);

    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.try_get("id")?));
    record.insert("user_id".to_string(), json_i64(row.try_get("user_id")?));
    insert_timestamp(&mut record, "date", row.try_get("occurred_at")?);
    record.insert(
        "type".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("transaction_type")?),
    );
    record.insert("amount".to_string(), json_real(amount_cents as f64 / 100.0));
    record.insert(
        "counterparty".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("merchant")?),
    );
    record.insert(
        "description".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("description")?),
    );
    record.insert(
        "payment_method".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("payment_method")?),
    );
    record.insert(
        "main_category".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("main_category")?),
    );
    record.insert(
        "sub_category".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("sub_category")?),
    );
    record.insert(
        "batch_id".to_string(),
        payload_string_value(&standard_payload, "batch_id"),
    );
    record.insert(
        "hash".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("source_hash")?),
    );
    insert_timestamp(&mut record, "created_at", row.try_get("created_at")?);
    insert_timestamp(&mut record, "updated_at", row.try_get("updated_at")?);
    record.insert(
        "source_account_id".to_string(),
        json_i64(source_account_id.unwrap_or_default()),
    );
    record.insert(
        "destination_account_id".to_string(),
        json_i64(destination_account_id.unwrap_or_default()),
    );
    record.insert(
        "destination_amount".to_string(),
        json_real(destination_amount.unwrap_or_default()),
    );
    record.insert(
        "category_id".to_string(),
        row.try_get::<Option<i64>, _>("category_id")?
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );
    for key in [
        "created_from_template",
        "created_from_recurring",
        "import_history_id",
    ] {
        record.insert(key.to_string(), payload_i64_value(&standard_payload, key));
    }
    Ok(record)
}

fn tag_value_from_postgres_row(row: PgRow) -> DbResult<Value> {
    let metadata: Value = row.try_get("metadata")?;
    let mut tag = Map::new();
    tag.insert(
        "id".to_string(),
        row.try_get::<i64, _>("id")?.to_string().into(),
    );
    tag.insert("name".to_string(), row.try_get::<String, _>("name")?.into());
    tag.insert(
        "color".to_string(),
        optional_string_value(row.try_get::<Option<String>, _>("color")?),
    );
    tag.insert(
        "icon".to_string(),
        metadata
            .get("icon")
            .and_then(Value::as_str)
            .map_or(Value::Null, |value| Value::String(value.to_string())),
    );
    Ok(Value::Object(tag))
}

fn canonical_transaction_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => "income",
        "支出" | "expense" | "3" => "expense",
        "转账" | "transfer" | "4" => "transfer",
        "投资" | "investment" | "5" => "investment",
        other => other,
    }
    .to_string()
}

fn category_names_from_path(path: Option<&str>, name: &str) -> (String, String) {
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

fn destination_amount_yuan(payload: &Value) -> Option<f64> {
    payload
        .get("destination_amount")
        .or_else(|| payload.get("destinationAmount"))
        .and_then(value_to_f64)
}

fn payload_string_value(payload: &Value, key: &str) -> Value {
    payload
        .get(key)
        .and_then(value_string)
        .map_or(Value::Null, Value::String)
}

fn payload_i64_value(payload: &Value, key: &str) -> Value {
    payload
        .get(key)
        .and_then(value_to_i64)
        .map_or(Value::Null, |value| Value::Number(Number::from(value)))
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn optional_value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(value_string)
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn first_positive(values: [Option<i64>; 2]) -> Option<i64> {
    values.into_iter().flatten().find(|value| *value > 0)
}

fn normalize_ids(values: &[i64]) -> Vec<i64> {
    let mut values = values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn yuan_to_cents(value: f64) -> i64 {
    (value.abs() * 100.0).round() as i64
}

fn postgres_initial_balance_money(metadata: &Value, balance_cents: i64) -> DbResult<Money> {
    let fallback = Money::from_cents(balance_cents);
    match metadata.get("initial_balance") {
        Some(Value::Number(number)) => number
            .as_f64()
            .map(|value| {
                Money::from_yuan_str(&value.to_string())
                    .map_err(|error| DbError::InvalidOperation(error.to_string()))
            })
            .transpose()
            .map(|value| value.unwrap_or(fallback)),
        Some(Value::String(text)) if !text.trim().is_empty() => Money::from_yuan_str(text.trim())
            .map_err(|error| DbError::InvalidOperation(error.to_string())),
        _ => Ok(fallback),
    }
}

fn optional_string_value(value: Option<String>) -> Value {
    value
        .filter(|value| !value.trim().is_empty())
        .map_or(Value::Null, Value::String)
}

fn json_i64(value: i64) -> Value {
    json!(value)
}

fn json_real(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn insert_timestamp(record: &mut Map<String, Value>, key: &str, timestamp: DateTime<Utc>) {
    record.insert(
        key.to_string(),
        Value::String(
            timestamp
                .naive_utc()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        ),
    );
}
