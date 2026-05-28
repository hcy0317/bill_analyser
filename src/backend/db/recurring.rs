// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::BTreeSet;

use bill_analyser_core::{matching::RecurringPattern, UserId};
use chrono::{Datelike, Duration, NaiveDate, SecondsFormat, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use sqlx::{postgres::PgRow, Row};

use crate::{
    get_postgres_bill_by_id, run_transaction, BillRecord, BillRecurringBindResult,
    BillRecurringCandidates, DbError, DbResult, PostgresPool, UserScope,
};

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
pub async fn count_postgres_recurring_suggestions(
    pool: &PostgresPool,
    user_id: UserId,
    status: Option<&str>,
) -> DbResult<i64> {
    let user_id = user_id_i64(user_id)?;
    let status = non_empty_text(status);
    let count = if let Some(status) = status {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM recurring_suggestions WHERE user_id = $1 AND status = $2",
        )
        .bind(user_id)
        .bind(status)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM recurring_suggestions WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?
    };
    Ok(count)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_recurring_suggestions(
    pool: &PostgresPool,
    user_id: UserId,
    status: Option<&str>,
    limit: usize,
    offset: usize,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let limit = i64::try_from(limit)
        .map_err(|_| DbError::InvalidOperation("invalid recurring limit".to_string()))?;
    let offset = i64::try_from(offset)
        .map_err(|_| DbError::InvalidOperation("invalid recurring offset".to_string()))?;
    let status = non_empty_text(status);
    let rows = if let Some(status) = status {
        let sql = recurring_suggestion_select_sql(
            "WHERE user_id = $1 AND status = $2",
            "LIMIT $3 OFFSET $4",
        );
        sqlx::query(&sql)
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
    } else {
        let sql = recurring_suggestion_select_sql("WHERE user_id = $1", "LIMIT $2 OFFSET $3");
        sqlx::query(&sql)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
    };
    rows.iter()
        .map(postgres_recurring_suggestion_from_row)
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_bill_recurring_candidates(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
    tolerance_days: i64,
) -> DbResult<Option<BillRecurringCandidates>> {
    let user_id_i64 = user_id_i64(user_id)?;
    let Some(bill) = get_postgres_bill_by_id(pool, user_id_i64, bill_id).await? else {
        return Ok(None);
    };
    let linked_recurring_id = pg_record_optional_i64(&bill, "created_from_recurring");
    let linked_recurring_name = match linked_recurring_id {
        Some(recurring_id) => get_postgres_recurring_template_name(pool, user_id_i64, recurring_id)
            .await?
            .unwrap_or_default(),
        None => String::new(),
    };
    let recurring_rows = list_enabled_postgres_recurring_templates(pool, user_id_i64).await?;
    let candidates = build_postgres_recurring_candidates_for_bill_data(
        &bill,
        &recurring_rows,
        linked_recurring_id,
        tolerance_days.clamp(0, 31),
    );
    Ok(Some(BillRecurringCandidates {
        linked_recurring_id,
        linked_recurring_name,
        candidates,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn bind_postgres_bill_to_recurring(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
    recurring_id: i64,
) -> DbResult<Option<BillRecurringBindResult>> {
    let user_id_i64 = user_id_i64(user_id)?;
    let Some(bill) = get_postgres_bill_by_id(pool, user_id_i64, bill_id).await? else {
        return Ok(None);
    };
    let previous_recurring_id = pg_record_optional_i64(&bill, "created_from_recurring");
    let mut tx = pool.begin().await?;
    let Some(recurring) =
        get_postgres_recurring_template_on_tx(&mut tx, user_id_i64, recurring_id).await?
    else {
        return Ok(None);
    };
    let bill_date_text = pg_record_text(&bill, "date");
    let next_occurrence = pg_parse_date_value(&bill_date_text)
        .and_then(|bill_date| pg_next_recurring_occurrence_after(&recurring, bill_date, 370))
        .map(|date| date.to_string());
    let fallback_next_date = pg_recurring_text(&recurring, "next_date");
    let stored_next_date = next_occurrence
        .or_else(|| non_empty_text(Some(fallback_next_date.as_str())).map(ToOwned::to_owned));
    sqlx::query(
        r#"
        UPDATE bills
        SET standard_payload = jsonb_set(
                COALESCE(standard_payload, '{}'::jsonb),
                '{created_from_recurring}',
                to_jsonb($3::bigint),
                true
            ),
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id_i64)
    .bind(bill_id)
    .bind(recurring_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE transaction_templates
        SET scheduled_next_date = $3,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND template_type = 2
        "#,
    )
    .bind(user_id_i64)
    .bind(recurring_id)
    .bind(stored_next_date.as_deref())
    .execute(&mut *tx)
    .await?;
    if previous_recurring_id.is_some_and(|previous| previous != recurring_id) {
        pg_recalculate_recurring_next_date_on_tx(
            &mut tx,
            user_id_i64,
            previous_recurring_id.unwrap_or_default(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Some(BillRecurringBindResult {
        bill_id,
        recurring_id,
        next_scheduled_date: stored_next_date,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn unbind_postgres_bill_from_recurring(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
) -> DbResult<Option<bool>> {
    let user_id_i64 = user_id_i64(user_id)?;
    let Some(bill) = get_postgres_bill_by_id(pool, user_id_i64, bill_id).await? else {
        return Ok(None);
    };
    let recurring_id = pg_record_optional_i64(&bill, "created_from_recurring");
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        r#"
        UPDATE bills
        SET standard_payload = COALESCE(standard_payload, '{}'::jsonb) - 'created_from_recurring',
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND is_deleted = false
        "#,
    )
    .bind(user_id_i64)
    .bind(bill_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if let Some(recurring_id) = recurring_id {
        pg_recalculate_recurring_next_date_on_tx(&mut tx, user_id_i64, recurring_id).await?;
    }
    tx.commit().await?;
    Ok(Some(updated > 0))
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
pub async fn detect_and_save_postgres_recurring_suggestions(
    pool: &PostgresPool,
    user_id: UserId,
    patterns: &[RecurringPattern],
) -> DbResult<RecurringSuggestionSaveSummary> {
    let user_id = user_id_i64(user_id)?;
    let mut summary = RecurringSuggestionSaveSummary {
        created: 0,
        updated: 0,
        skipped: 0,
    };

    for pattern in patterns {
        let existing = sqlx::query(
            "SELECT id, status FROM recurring_suggestions WHERE user_id = $1 AND pattern_hash = $2",
        )
        .bind(user_id)
        .bind(&pattern.pattern_hash)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = existing {
            let suggestion_id: i64 = row.try_get("id")?;
            let status: String = row.try_get("status")?;
            if matches!(status.as_str(), "accepted" | "rejected") {
                summary.skipped += 1;
                continue;
            }
            sqlx::query(
                r#"
                UPDATE recurring_suggestions SET
                    name = $1,
                    description = $2,
                    type = $3,
                    amount_cents = $4,
                    source_account_id = $5,
                    destination_account_id = $6,
                    counterparty = $7,
                    frequency = $8,
                    detected_interval_days = $9,
                    confidence_score = $10,
                    sample_count = $11,
                    sample_bill_ids = $12,
                    first_occurrence = $13,
                    last_occurrence = $14,
                    suggested_next_date = $15,
                    updated_at = now(),
                    version = version + 1
                WHERE id = $16 AND user_id = $17
                "#,
            )
            .bind(&pattern.name)
            .bind(&pattern.description)
            .bind(&pattern.transaction_type)
            .bind(yuan_to_cents(pattern.amount))
            .bind(pattern.source_account_id)
            .bind(optional_i64_from_text(&pattern.destination_account_id))
            .bind(&pattern.counterparty)
            .bind(&pattern.frequency)
            .bind(pattern.detected_interval_days)
            .bind(pattern.confidence_score)
            .bind(i32::try_from(pattern.sample_count).unwrap_or(i32::MAX))
            .bind(json!(pattern.sample_bill_ids))
            .bind(&pattern.first_occurrence)
            .bind(&pattern.last_occurrence)
            .bind(&pattern.suggested_next_date)
            .bind(suggestion_id)
            .bind(user_id)
            .execute(pool)
            .await?;
            summary.updated += 1;
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO recurring_suggestions (
                user_id, pattern_hash, name, description, type, amount_cents,
                source_account_id, destination_account_id, counterparty,
                frequency, detected_interval_days, confidence_score, sample_count,
                sample_bill_ids, first_occurrence, last_occurrence, suggested_next_date
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(user_id)
        .bind(&pattern.pattern_hash)
        .bind(&pattern.name)
        .bind(&pattern.description)
        .bind(&pattern.transaction_type)
        .bind(yuan_to_cents(pattern.amount))
        .bind(pattern.source_account_id)
        .bind(optional_i64_from_text(&pattern.destination_account_id))
        .bind(&pattern.counterparty)
        .bind(&pattern.frequency)
        .bind(pattern.detected_interval_days)
        .bind(pattern.confidence_score)
        .bind(i32::try_from(pattern.sample_count).unwrap_or(i32::MAX))
        .bind(json!(pattern.sample_bill_ids))
        .bind(&pattern.first_occurrence)
        .bind(&pattern.last_occurrence)
        .bind(&pattern.suggested_next_date)
        .execute(pool)
        .await?;
        summary.created += 1;
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
pub async fn accept_postgres_recurring_suggestion(
    pool: &PostgresPool,
    user_id: UserId,
    suggestion_id: i64,
) -> DbResult<Option<Value>> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await?;
    let suggestion = sqlx::query(
        r#"
        SELECT id, name, description, type, amount_cents, source_account_id,
               destination_account_id, counterparty, frequency, first_occurrence,
               suggested_next_date
        FROM recurring_suggestions
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(suggestion) = suggestion else {
        return Ok(None);
    };

    let changed = sqlx::query(
        r#"
        UPDATE recurring_suggestions
        SET status = 'accepted', updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed == 0 {
        return Ok(None);
    }

    let name: String = suggestion.try_get("name")?;
    let description: Option<String> = suggestion.try_get("description")?;
    let transaction_type: String = suggestion.try_get("type")?;
    let amount_cents: i64 = suggestion.try_get("amount_cents")?;
    let source_account_id: Option<i64> = suggestion.try_get("source_account_id")?;
    let destination_account_id: Option<i64> = suggestion.try_get("destination_account_id")?;
    let counterparty: Option<String> = suggestion.try_get("counterparty")?;
    let frequency: String = suggestion.try_get("frequency")?;
    let first_occurrence: Option<String> = suggestion.try_get("first_occurrence")?;
    let suggested_next_date: Option<String> = suggestion.try_get("suggested_next_date")?;
    let recurring_id: i64 = sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, description, transaction_type,
            source_account_id, destination_account_id, source_amount_minor_units,
            destination_amount_minor_units, comment, scheduled_frequency,
            scheduled_start_date, scheduled_next_date, enabled, auto_create
        )
        VALUES ($1, 2, $2, $3, $4, $5, $6, $7, 0, $8, $9, $10, $11, true, false)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(description.unwrap_or_default())
    .bind(transaction_type)
    .bind(
        source_account_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    )
    .bind(
        destination_account_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    )
    .bind(amount_cents)
    .bind(counterparty.unwrap_or_default())
    .bind(frequency)
    .bind(first_occurrence.unwrap_or_else(utc_today_text))
    .bind(suggested_next_date.unwrap_or_else(utc_today_text))
    .fetch_one(&mut *tx)
    .await?
    .try_get("id")?;

    tx.commit().await?;
    Ok(Some(json!({
        "recurring_id": recurring_id,
        "suggestion_id": suggestion_id,
        "status": "accepted",
    })))
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
pub async fn reject_postgres_recurring_suggestion(
    pool: &PostgresPool,
    user_id: UserId,
    suggestion_id: i64,
) -> DbResult<bool> {
    let user_id = user_id_i64(user_id)?;
    let changed = sqlx::query(
        r#"
        UPDATE recurring_suggestions
        SET status = 'rejected', updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2 AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
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
pub async fn get_postgres_bills_linked_to_recurring(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<BTreeSet<i64>> {
    let user_id = user_id_i64(user_id)?;
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM bills
        WHERE user_id = $1
          AND COALESCE(
              NULLIF(standard_payload->>'created_from_recurring', ''),
              NULLIF(raw_payload->>'created_from_recurring', '')
          ) IS NOT NULL
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| row.try_get::<i64, _>("id").map_err(DbError::from))
        .collect()
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_recent_bills_for_recurring_detection(
    pool: &PostgresPool,
    user_id: UserId,
    limit: usize,
) -> DbResult<Vec<Value>> {
    let user_id = user_id_i64(user_id)?;
    let rows = sqlx::query(
        r#"
        SELECT
            b.id,
            b.occurred_at,
            b.transaction_type,
            b.amount_cents,
            b.merchant,
            b.description,
            b.source_account_id,
            b.target_account_id,
            b.transfer_target_account_id,
            b.standard_payload,
            c.path AS category_path,
            c.name AS category_name
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = $1 AND b.is_deleted = false
        ORDER BY b.occurred_at DESC, b.id DESC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await?;
    rows.iter().map(postgres_recent_bill_from_row).collect()
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

fn recurring_suggestion_select_sql(where_clause: &str, page_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, pattern_hash, name, description, type, amount_cents,
               source_account_id, destination_account_id, counterparty, frequency,
               detected_interval_days::DOUBLE PRECISION AS detected_interval_days,
               confidence_score::DOUBLE PRECISION AS confidence_score,
               sample_count,
               sample_bill_ids, first_occurrence, last_occurrence,
               suggested_next_date, status, created_at, updated_at
        FROM recurring_suggestions
        {where_clause}
        ORDER BY confidence_score DESC, last_occurrence DESC, id DESC
        {page_clause}
        "#
    )
}

fn postgres_recurring_suggestion_from_row(row: &PgRow) -> DbResult<Value> {
    let amount_cents: i64 = row.try_get("amount_cents")?;
    let sample_bill_ids: Value = row.try_get("sample_bill_ids")?;
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "user_id": row.try_get::<i64, _>("user_id")?,
        "pattern_hash": row.try_get::<String, _>("pattern_hash")?,
        "name": row.try_get::<String, _>("name")?,
        "description": row.try_get::<Option<String>, _>("description")?,
        "type": row.try_get::<String, _>("type")?,
        "amount": amount_cents as f64 / 100.0,
        "source_account_id": row.try_get::<Option<i64>, _>("source_account_id")?,
        "destination_account_id": row.try_get::<Option<i64>, _>("destination_account_id")?,
        "counterparty": row.try_get::<Option<String>, _>("counterparty")?,
        "frequency": row.try_get::<String, _>("frequency")?,
        "detected_interval_days": row.try_get::<Option<f64>, _>("detected_interval_days")?,
        "confidence_score": row.try_get::<f64, _>("confidence_score")?,
        "sample_count": row.try_get::<i32, _>("sample_count")?,
        "sample_bill_ids_json": sample_bill_ids.to_string(),
        "sample_bill_ids": sample_bill_ids,
        "first_occurrence": row.try_get::<Option<String>, _>("first_occurrence")?,
        "last_occurrence": row.try_get::<Option<String>, _>("last_occurrence")?,
        "suggested_next_date": row.try_get::<Option<String>, _>("suggested_next_date")?,
        "status": row.try_get::<String, _>("status")?,
        "created_at": row.try_get::<chrono::DateTime<Utc>, _>("created_at")?.to_rfc3339_opts(SecondsFormat::Secs, true),
        "updated_at": row.try_get::<chrono::DateTime<Utc>, _>("updated_at")?.to_rfc3339_opts(SecondsFormat::Secs, true),
    }))
}

fn postgres_recent_bill_from_row(row: &PgRow) -> DbResult<Value> {
    let payload = row.try_get::<Value, _>("standard_payload")?;
    let category_path = row.try_get::<Option<String>, _>("category_path")?;
    let category_name = row.try_get::<Option<String>, _>("category_name")?;
    let (main_category, sub_category) = category_names_from_postgres_values(
        &payload,
        category_path.as_deref(),
        category_name.as_deref(),
    );
    let amount_cents: i64 = row.try_get("amount_cents")?;
    let destination_account_id =
        row.try_get::<Option<i64>, _>("target_account_id")?
            .or(row.try_get::<Option<i64>, _>("transfer_target_account_id")?);
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "date": row.try_get::<chrono::DateTime<Utc>, _>("occurred_at")?.to_rfc3339_opts(SecondsFormat::Secs, true),
        "type": row.try_get::<Option<String>, _>("transaction_type")?.unwrap_or_default(),
        "amount": amount_cents as f64 / 100.0,
        "counterparty": row.try_get::<Option<String>, _>("merchant")?.unwrap_or_default(),
        "description": row.try_get::<Option<String>, _>("description")?.unwrap_or_default(),
        "main_category": main_category,
        "sub_category": sub_category,
        "source_account_id": row.try_get::<Option<i64>, _>("source_account_id")?,
        "destination_account_id": destination_account_id,
    }))
}

fn category_names_from_postgres_values(
    payload: &Value,
    category_path: Option<&str>,
    category_name: Option<&str>,
) -> (String, String) {
    let payload_main = payload
        .get("main_category")
        .or_else(|| payload.get("mainCategory"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let payload_sub = payload
        .get("sub_category")
        .or_else(|| payload.get("subCategory"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if payload_main.is_some() || payload_sub.is_some() {
        return (
            payload_main.unwrap_or_default(),
            payload_sub.unwrap_or_default(),
        );
    }
    let parts = category_path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (category_name.unwrap_or_default().to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

async fn get_postgres_recurring_template_name(
    pool: &PostgresPool,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT name FROM transaction_templates WHERE user_id = $1 AND id = $2 AND template_type = 2",
    )
    .bind(user_id)
    .bind(recurring_id)
    .fetch_optional(pool)
    .await
    .map(|value| value.flatten())
    .map_err(DbError::from)
}

async fn list_enabled_postgres_recurring_templates(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<BillRecord>> {
    let rows = sqlx::query(&postgres_recurring_template_select_sql(
        "WHERE user_id = $1 AND template_type = 2 AND enabled = true",
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(postgres_recurring_template_from_row)
        .collect()
}

async fn get_postgres_recurring_template_on_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<Option<BillRecord>> {
    let row = sqlx::query(&postgres_recurring_template_select_sql(
        "WHERE user_id = $1 AND id = $2 AND template_type = 2",
    ))
    .bind(user_id)
    .bind(recurring_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref()
        .map(postgres_recurring_template_from_row)
        .transpose()
}

fn postgres_recurring_template_select_sql(where_clause: &str) -> String {
    format!(
        r#"
        SELECT id, user_id, legacy_id AS template_id, name, description, transaction_type,
               category_id, source_account_id, destination_account_id,
               source_amount_minor_units, destination_amount_minor_units,
               hide_amount, tag_ids, comment, scheduled_frequency,
               scheduled_frequency_type, scheduled_start_date, scheduled_end_date,
               scheduled_next_date, enabled, auto_create, display_order, hidden,
               utc_offset, created_at, updated_at
        FROM transaction_templates
        {where_clause}
        ORDER BY display_order ASC, name ASC, id ASC
        "#
    )
}

fn postgres_recurring_template_from_row(row: &PgRow) -> DbResult<BillRecord> {
    let tag_ids: Value = row.try_get("tag_ids")?;
    let tag_text = tag_ids
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(pg_value_string)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let mut record = Map::new();
    record.insert("id".to_string(), json!(row.try_get::<i64, _>("id")?));
    record.insert(
        "user_id".to_string(),
        json!(row.try_get::<i64, _>("user_id")?),
    );
    record.insert(
        "template_id".to_string(),
        optional_i64_json(row.try_get::<Option<i64>, _>("template_id")?),
    );
    record.insert(
        "name".to_string(),
        pg_optional_string_json(row.try_get("name")?),
    );
    record.insert(
        "description".to_string(),
        pg_optional_string_json(row.try_get("description")?),
    );
    record.insert(
        "type".to_string(),
        pg_optional_string_json(row.try_get("transaction_type")?),
    );
    record.insert(
        "category".to_string(),
        pg_optional_string_json(row.try_get("category_id")?),
    );
    record.insert(
        "account".to_string(),
        pg_optional_string_json(row.try_get("source_account_id")?),
    );
    record.insert(
        "counterparty".to_string(),
        pg_optional_string_json(row.try_get("destination_account_id")?),
    );
    record.insert(
        "amount".to_string(),
        json!(row.try_get::<i64, _>("source_amount_minor_units")?),
    );
    record.insert(
        "destination_amount".to_string(),
        json!(row.try_get::<i64, _>("destination_amount_minor_units")?),
    );
    record.insert(
        "hide_amount".to_string(),
        json!(i64::from(row.try_get::<bool, _>("hide_amount")?)),
    );
    record.insert("tag".to_string(), Value::String(tag_text));
    record.insert(
        "comment".to_string(),
        pg_optional_string_json(row.try_get("comment")?),
    );
    record.insert(
        "frequency".to_string(),
        pg_optional_string_json(row.try_get("scheduled_frequency")?),
    );
    record.insert(
        "scheduled_frequency_type".to_string(),
        optional_i64_json(
            row.try_get::<Option<i32>, _>("scheduled_frequency_type")?
                .map(i64::from),
        ),
    );
    record.insert(
        "start_date".to_string(),
        pg_optional_string_json(row.try_get("scheduled_start_date")?),
    );
    record.insert(
        "end_date".to_string(),
        pg_optional_string_json(row.try_get("scheduled_end_date")?),
    );
    record.insert(
        "next_date".to_string(),
        pg_optional_string_json(row.try_get("scheduled_next_date")?),
    );
    record.insert(
        "enabled".to_string(),
        json!(i64::from(row.try_get::<bool, _>("enabled")?)),
    );
    record.insert(
        "auto_create".to_string(),
        json!(i64::from(row.try_get::<bool, _>("auto_create")?)),
    );
    record.insert(
        "display_order".to_string(),
        json!(i64::from(row.try_get::<i32, _>("display_order")?)),
    );
    record.insert(
        "hidden".to_string(),
        json!(i64::from(row.try_get::<bool, _>("hidden")?)),
    );
    record.insert(
        "utc_offset".to_string(),
        json!(i64::from(row.try_get::<i32, _>("utc_offset")?)),
    );
    record.insert(
        "created_at".to_string(),
        Value::String(
            row.try_get::<chrono::DateTime<Utc>, _>("created_at")?
                .to_rfc3339_opts(SecondsFormat::Secs, true),
        ),
    );
    record.insert(
        "updated_at".to_string(),
        Value::String(
            row.try_get::<chrono::DateTime<Utc>, _>("updated_at")?
                .to_rfc3339_opts(SecondsFormat::Secs, true),
        ),
    );
    Ok(record)
}

fn build_postgres_recurring_candidates_for_bill_data(
    bill: &BillRecord,
    recurring_rows: &[BillRecord],
    linked_recurring_id: Option<i64>,
    tolerance_days: i64,
) -> Vec<Value> {
    let Some(bill_date) = pg_parse_date_value(&pg_record_text(bill, "date")) else {
        return Vec::new();
    };
    let bill_type = pg_normalize_template_transaction_type(bill.get("type"));
    let bill_amount_cents = (pg_record_f64(bill, "amount").abs() * 100.0).round() as i64;
    let bill_source_account = pg_record_text(bill, "source_account_id");
    let bill_destination_account = pg_record_text(bill, "destination_account_id");
    let mut candidates = Vec::new();

    for recurring in recurring_rows {
        if pg_normalize_template_transaction_type(recurring.get("type")) != bill_type {
            continue;
        }
        let recurring_amount_cents = pg_recurring_f64(recurring, "amount").abs().round() as i64;
        if recurring_amount_cents != bill_amount_cents {
            continue;
        }
        let Some(matched_occurrence) =
            pg_find_recurring_occurrence_near_date(recurring, bill_date, tolerance_days)
        else {
            continue;
        };

        let mut score = 80_i64;
        let mut reasons = vec![
            Value::String("type".to_string()),
            Value::String("amount".to_string()),
            Value::String("schedule".to_string()),
        ];
        if pg_recurring_text(recurring, "account") == bill_source_account {
            reasons.push(Value::String("source_account".to_string()));
            score += 10;
        }
        if !matches!(bill_destination_account.as_str(), "" | "0")
            && pg_recurring_text(recurring, "counterparty") == bill_destination_account
        {
            reasons.push(Value::String("destination_account".to_string()));
            score += 10;
        }
        let days_offset = (matched_occurrence - bill_date).num_days().abs();
        score += (10 - days_offset * 2).max(0);

        let mut candidate = serialize_postgres_recurring_template_row(recurring);
        candidate.insert("matchScore".to_string(), json!(score));
        candidate.insert("matchReasons".to_string(), Value::Array(reasons));
        candidate.insert(
            "matchedOccurrenceDate".to_string(),
            Value::String(matched_occurrence.to_string()),
        );
        candidate.insert("matchedDayOffset".to_string(), json!(days_offset));
        candidate.insert(
            "linked".to_string(),
            Value::Bool(
                linked_recurring_id
                    .zip(pg_recurring_i64(recurring, "id"))
                    .is_some_and(|(linked, recurring_id)| linked == recurring_id),
            ),
        );
        candidates.push(Value::Object(candidate));
    }

    candidates.sort_by(|left, right| {
        let left = left.as_object().expect("candidate object");
        let right = right.as_object().expect("candidate object");
        pg_recurring_i64(right, "matchScore")
            .cmp(&pg_recurring_i64(left, "matchScore"))
            .then_with(|| {
                pg_recurring_i64(left, "matchedDayOffset")
                    .unwrap_or(999)
                    .cmp(&pg_recurring_i64(right, "matchedDayOffset").unwrap_or(999))
            })
            .then_with(|| pg_recurring_text(left, "name").cmp(&pg_recurring_text(right, "name")))
    });
    candidates
}

fn serialize_postgres_recurring_template_row(row: &BillRecord) -> Map<String, Value> {
    let mut value = Map::new();
    value.insert("id".to_string(), pg_recurring_text(row, "id").into());
    value.insert("timeSequenceId".to_string(), String::new().into());
    value.insert("templateType".to_string(), json!(2));
    value.insert("name".to_string(), pg_recurring_text(row, "name").into());
    value.insert(
        "type".to_string(),
        json!(pg_normalize_template_transaction_type(row.get("type"))),
    );
    value.insert(
        "categoryId".to_string(),
        pg_recurring_text(row, "category").into(),
    );
    value.insert("time".to_string(), json!(0));
    value.insert(
        "utcOffset".to_string(),
        json!(pg_recurring_i64(row, "utc_offset").unwrap_or(0)),
    );
    value.insert(
        "sourceAccountId".to_string(),
        pg_recurring_text(row, "account").into(),
    );
    value.insert(
        "destinationAccountId".to_string(),
        pg_recurring_text(row, "counterparty").into(),
    );
    value.insert(
        "sourceAmount".to_string(),
        json!(pg_recurring_f64(row, "amount")),
    );
    value.insert(
        "destinationAmount".to_string(),
        json!(pg_recurring_f64(row, "destination_amount")),
    );
    value.insert(
        "hideAmount".to_string(),
        Value::Bool(pg_recurring_i64(row, "hide_amount").unwrap_or(0) != 0),
    );
    value.insert(
        "tagIds".to_string(),
        Value::Array(
            pg_recurring_text(row, "tag")
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect(),
        ),
    );
    value.insert(
        "comment".to_string(),
        pg_recurring_text(row, "comment").into(),
    );
    value.insert("editable".to_string(), Value::Bool(true));
    value.insert(
        "displayOrder".to_string(),
        json!(pg_recurring_i64(row, "display_order").unwrap_or(0)),
    );
    value.insert(
        "hidden".to_string(),
        Value::Bool(pg_recurring_i64(row, "hidden").unwrap_or(0) != 0),
    );
    value.insert(
        "scheduledFrequencyType".to_string(),
        json!(pg_recurring_i64(row, "scheduled_frequency_type").unwrap_or(0)),
    );
    value.insert(
        "scheduledFrequency".to_string(),
        pg_recurring_optional_text_json(row, "frequency"),
    );
    value.insert(
        "scheduledStartDate".to_string(),
        pg_recurring_optional_text_json(row, "start_date"),
    );
    value.insert(
        "scheduledEndDate".to_string(),
        pg_recurring_optional_text_json(row, "end_date"),
    );
    value.insert("scheduledAt".to_string(), Value::Null);
    value
}

async fn pg_recalculate_recurring_next_date_on_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<()> {
    let Some(recurring) = get_postgres_recurring_template_on_tx(tx, user_id, recurring_id).await?
    else {
        return Ok(());
    };
    let latest_linked_date = sqlx::query(
        r#"
        SELECT occurred_at
        FROM bills
        WHERE user_id = $1
          AND standard_payload->>'created_from_recurring' = $2
          AND is_deleted = false
        ORDER BY occurred_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(recurring_id.to_string())
    .fetch_optional(&mut **tx)
    .await?
    .map(|row| row.try_get::<chrono::DateTime<Utc>, _>("occurred_at"))
    .transpose()?
    .map(|value| value.date_naive());
    let fallback_next_date = pg_recurring_text(&recurring, "next_date");
    let next_occurrence = latest_linked_date
        .and_then(|date| pg_next_recurring_occurrence_after(&recurring, date, 370))
        .or_else(|| pg_first_recurring_occurrence(&recurring, 370))
        .map(|date| date.to_string())
        .or_else(|| non_empty_text(Some(fallback_next_date.as_str())).map(ToOwned::to_owned));
    sqlx::query(
        r#"
        UPDATE transaction_templates
        SET scheduled_next_date = $3,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND template_type = 2
        "#,
    )
    .bind(user_id)
    .bind(recurring_id)
    .bind(next_occurrence.as_deref())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn pg_parse_date_value(value: &str) -> Option<NaiveDate> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()
}

fn pg_parse_schedule_frequency_values(value: &str) -> Vec<u32> {
    let mut values = value
        .split(',')
        .filter_map(|item| item.trim().parse::<u32>().ok())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}

fn pg_weekday_sunday_first(date: NaiveDate) -> u32 {
    date.weekday().num_days_from_sunday()
}

fn pg_recurring_active_on_date(recurring: &BillRecord, target_date: NaiveDate) -> bool {
    if pg_parse_date_value(&pg_recurring_text(recurring, "start_date"))
        .is_some_and(|start_date| target_date < start_date)
    {
        return false;
    }
    if pg_parse_date_value(&pg_recurring_text(recurring, "end_date"))
        .is_some_and(|end_date| target_date > end_date)
    {
        return false;
    }
    true
}

fn pg_recurring_due_on_date(recurring: &BillRecord, target_date: NaiveDate) -> bool {
    if !pg_recurring_active_on_date(recurring, target_date) {
        return false;
    }
    let frequency_type = pg_recurring_i64(recurring, "scheduled_frequency_type").unwrap_or(0);
    let frequency_values =
        pg_parse_schedule_frequency_values(&pg_recurring_text(recurring, "frequency"));
    let start_date = pg_parse_date_value(&pg_recurring_text(recurring, "start_date"));
    let next_date = pg_parse_date_value(&pg_recurring_text(recurring, "next_date"));

    if frequency_type == 1 {
        let valid_weekdays = if frequency_values.is_empty() {
            start_date
                .map(pg_weekday_sunday_first)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            frequency_values
        };
        return valid_weekdays.contains(&pg_weekday_sunday_first(target_date));
    }
    if frequency_type == 2 {
        let valid_days = if frequency_values.is_empty() {
            start_date
                .map(|date| date.day())
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            frequency_values
        };
        return valid_days.contains(&target_date.day());
    }
    next_date.is_some_and(|date| date == target_date)
        || start_date.is_some_and(|date| date == target_date)
}

fn pg_find_recurring_occurrence_near_date(
    recurring: &BillRecord,
    target_date: NaiveDate,
    tolerance_days: i64,
) -> Option<NaiveDate> {
    let mut nearest_date = None;
    let mut nearest_diff = None;
    for offset in -tolerance_days..=tolerance_days {
        let current_date = target_date + Duration::days(offset);
        if !pg_recurring_due_on_date(recurring, current_date) {
            continue;
        }
        let diff = offset.abs();
        if nearest_date.is_none() || nearest_diff.is_some_and(|value| diff < value) {
            nearest_date = Some(current_date);
            nearest_diff = Some(diff);
        }
    }
    nearest_date
}

fn pg_next_recurring_occurrence_after(
    recurring: &BillRecord,
    after_date: NaiveDate,
    max_search_days: i64,
) -> Option<NaiveDate> {
    (1..=max_search_days)
        .map(|offset| after_date + Duration::days(offset))
        .find(|candidate| pg_recurring_due_on_date(recurring, *candidate))
}

fn pg_first_recurring_occurrence(
    recurring: &BillRecord,
    max_search_days: i64,
) -> Option<NaiveDate> {
    let Some(start_date) = pg_parse_date_value(&pg_recurring_text(recurring, "start_date")) else {
        return pg_parse_date_value(&pg_recurring_text(recurring, "next_date"));
    };
    (0..=max_search_days)
        .map(|offset| start_date + Duration::days(offset))
        .find(|candidate| pg_recurring_due_on_date(recurring, *candidate))
        .or_else(|| pg_parse_date_value(&pg_recurring_text(recurring, "next_date")))
}

fn pg_record_text(record: &BillRecord, key: &str) -> String {
    pg_value_string(record.get(key).unwrap_or(&Value::Null)).unwrap_or_default()
}

fn pg_record_optional_i64(record: &BillRecord, key: &str) -> Option<i64> {
    pg_value_i64(record.get(key)?)
}

fn pg_record_f64(record: &BillRecord, key: &str) -> f64 {
    pg_value_f64(record.get(key).unwrap_or(&Value::Null))
}

fn pg_recurring_text(record: &BillRecord, key: &str) -> String {
    pg_record_text(record, key)
}

fn pg_recurring_i64(record: &BillRecord, key: &str) -> Option<i64> {
    pg_record_optional_i64(record, key)
}

fn pg_recurring_f64(record: &BillRecord, key: &str) -> f64 {
    pg_record_f64(record, key)
}

fn pg_value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
    .filter(|value| !value.is_empty())
}

fn pg_value_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_f64().map(|value| value as i64)),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        Value::Bool(value) => Some(i64::from(*value)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn pg_value_f64(value: &Value) -> f64 {
    match value {
        Value::Number(value) => value.as_f64().unwrap_or(0.0),
        Value::String(value) => value.trim().parse::<f64>().unwrap_or(0.0),
        Value::Bool(value) => f64::from(*value as u8),
        Value::Null | Value::Array(_) | Value::Object(_) => 0.0,
    }
}

fn pg_normalize_template_transaction_type(value: Option<&Value>) -> i64 {
    let text = match value {
        Some(Value::String(value)) => value.trim().to_ascii_lowercase(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => return 3,
    };
    match text.as_str() {
        "2" | "income" | "收入" => 2,
        "3" | "expense" | "支出" => 3,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn pg_recurring_optional_text_json(record: &BillRecord, key: &str) -> Value {
    pg_value_string(record.get(key).unwrap_or(&Value::Null))
        .map(Value::String)
        .unwrap_or(Value::Null)
}

fn optional_i64_json(value: Option<i64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn pg_optional_string_json(value: Option<String>) -> Value {
    value
        .filter(|value| !value.trim().is_empty())
        .map(Value::String)
        .unwrap_or(Value::Null)
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

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}

fn yuan_to_cents(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

fn optional_i64_from_text(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|value| *value > 0)
}

fn utc_now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn utc_today_text() -> String {
    Utc::now().date_naive().to_string()
}
