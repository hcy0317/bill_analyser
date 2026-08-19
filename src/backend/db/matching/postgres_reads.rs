use bill_analyser_core::{
    matching::{normalize_transfer_pair_bill_ids, MANUAL_PAIR_SOURCE, TRANSFER_PAIR_TYPE},
    UserId,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Value};
use sqlx::{postgres::PgRow, Row};

use crate::{category_path::category_names_from_postgres_path, DbError, DbResult, PostgresPool};

use super::{MatchingResult, MatchingRuntimeError};

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_matching_pairs_payload(
    pool: &PostgresPool,
    user_id: UserId,
) -> MatchingResult<Value> {
    let user_id = i64::try_from(user_id.get())
        .map_err(|_| MatchingRuntimeError::BadRequest("invalid user id".to_string()))?;
    let rows = sqlx::query(
        r#"
        SELECT
            p.id,
            p.pair_type,
            COALESCE(NULLIF(p.metadata->>'source', ''), $2) AS source,
            p.left_bill_id,
            p.right_bill_id,
            p.created_at,
            p.updated_at,
            l.id AS left_bill_id_value,
            l.occurred_at AS left_bill_occurred_at,
            l.transaction_type AS left_bill_transaction_type,
            l.amount_cents AS left_bill_amount_cents,
            l.merchant AS left_bill_merchant,
            l.description AS left_bill_description,
            l.payment_method AS left_bill_payment_method,
            l.standard_payload AS left_bill_standard_payload,
            l.account_id AS left_bill_account_id,
            l.source_account_id AS left_bill_source_account_id,
            l.target_account_id AS left_bill_target_account_id,
            l.transfer_target_account_id AS left_bill_transfer_target_account_id,
            lc.path AS left_bill_category_path,
            lc.name AS left_bill_category_name,
            r.id AS right_bill_id_value,
            r.occurred_at AS right_bill_occurred_at,
            r.transaction_type AS right_bill_transaction_type,
            r.amount_cents AS right_bill_amount_cents,
            r.merchant AS right_bill_merchant,
            r.description AS right_bill_description,
            r.payment_method AS right_bill_payment_method,
            r.standard_payload AS right_bill_standard_payload,
            r.account_id AS right_bill_account_id,
            r.source_account_id AS right_bill_source_account_id,
            r.target_account_id AS right_bill_target_account_id,
            r.transfer_target_account_id AS right_bill_transfer_target_account_id,
            rc.path AS right_bill_category_path,
            rc.name AS right_bill_category_name
        FROM matching_pairs p
        JOIN bills l ON l.user_id = p.user_id AND l.id = p.left_bill_id
        JOIN bills r ON r.user_id = p.user_id AND r.id = p.right_bill_id
        LEFT JOIN categories lc ON lc.user_id = p.user_id AND lc.id = l.category_id
        LEFT JOIN categories rc ON rc.user_id = p.user_id AND rc.id = r.category_id
        WHERE p.user_id = $1
          AND p.status = 'active'
          AND COALESCE(NULLIF(p.metadata->>'source', ''), $2) = $2
          AND l.is_deleted = false
          AND r.is_deleted = false
        ORDER BY p.updated_at DESC, p.id DESC
        "#,
    )
    .bind(user_id)
    .bind(MANUAL_PAIR_SOURCE)
    .fetch_all(pool)
    .await
    .map_err(DbError::from)?;

    let pairs = rows
        .iter()
        .map(pair_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "pairs": pairs }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_matching_bill_candidates_payload(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
) -> MatchingResult<Option<Value>> {
    let user_id = user_id_i64(user_id)?;
    if !postgres_bill_exists(pool, user_id, bill_id).await? {
        return Ok(None);
    }
    let linked_pair = query_postgres_pair_for_bill(pool, user_id, bill_id).await?;
    Ok(Some(json!({
        "billId": bill_id,
        "linkedPair": linked_pair,
        "candidates": [],
        "reconciliation": Value::Null,
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_matching_session_candidates_payload(
    pool: &PostgresPool,
    user_id: UserId,
    session_id: &str,
) -> MatchingResult<Option<Value>> {
    let user_id = user_id_i64(user_id)?;
    let Some(session_id) = session_id
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
    else {
        return Ok(None);
    };
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM import_sessions WHERE id = $1 AND user_id = $2)",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(DbError::from)?;
    if !exists {
        return Ok(None);
    }
    Ok(Some(json!({
        "session_id": session_id.to_string(),
        "summary": {
            "preview_count": 0,
            "candidate_count": 0,
            "counts_by_kind": {},
        },
        "candidates": [],
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_matching_bill_feedback_payload(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
) -> MatchingResult<Option<Value>> {
    let user_id = user_id_i64(user_id)?;
    if !postgres_bill_exists(pool, user_id, bill_id).await? {
        return Ok(None);
    }
    let rows = sqlx::query(
        r#"
        SELECT id, event_type, payload, created_at
        FROM matching_feedback_events
        WHERE user_id = $1
          AND (
              payload->>'billId' = $2
              OR payload->>'candidateBillId' = $2
              OR payload->>'leftBillId' = $2
              OR payload->>'rightBillId' = $2
          )
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(user_id)
    .bind(bill_id.to_string())
    .fetch_all(pool)
    .await
    .map_err(DbError::from)?;
    let events = rows
        .into_iter()
        .map(|row| {
            let payload: Value = row.try_get("payload").map_err(DbError::from)?;
            Ok(json!({
                "id": row.try_get::<i64, _>("id").map_err(DbError::from)?,
                "candidateId": payload.get("candidateId").and_then(Value::as_str).unwrap_or_default(),
                "action": row.try_get::<String, _>("event_type").map_err(DbError::from)?,
                "createdAt": timestamp_string(row.try_get("created_at").map_err(DbError::from)?),
                "payload": payload,
            }))
        })
        .collect::<DbResult<Vec<_>>>()?;
    Ok(Some(json!({ "billId": bill_id, "events": events })))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn query_postgres_reconciliation_candidates_payload(
    _pool: &PostgresPool,
    _user_id: UserId,
) -> MatchingResult<Value> {
    Ok(json!({ "candidates": [] }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_manual_matching_pair(
    pool: &PostgresPool,
    user_id: UserId,
    bill_id: i64,
    candidate_bill_id: i64,
    pair_type: &str,
) -> MatchingResult<Value> {
    let user_id = user_id_i64(user_id)?;
    let (left_bill_id, right_bill_id) =
        normalize_transfer_pair_bill_ids(bill_id, candidate_bill_id)
            .map_err(|message| MatchingRuntimeError::BadRequest(message.to_string()))?;
    let mut tx = pool.begin().await.map_err(DbError::from)?;
    let bill_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM bills
        WHERE user_id = $1
          AND is_deleted = false
          AND id = ANY($2)
        "#,
    )
    .bind(user_id)
    .bind(vec![left_bill_id, right_bill_id])
    .fetch_one(&mut *tx)
    .await
    .map_err(DbError::from)?;
    if bill_count != 2 {
        return Err(MatchingRuntimeError::NotFound("Bill not found".to_string()));
    }
    let existing_pair = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM matching_pairs
            WHERE user_id = $1
              AND status = 'active'
              AND ($2 IN (left_bill_id, right_bill_id)
                   OR $3 IN (left_bill_id, right_bill_id))
        )
        "#,
    )
    .bind(user_id)
    .bind(left_bill_id)
    .bind(right_bill_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(DbError::from)?;
    if existing_pair {
        return Err(MatchingRuntimeError::Conflict(
            "Bills already belong to an existing transfer pair".to_string(),
        ));
    }
    let row = sqlx::query(
        r#"
        INSERT INTO matching_pairs (
            user_id, left_bill_id, right_bill_id, pair_type, status, metadata
        )
        VALUES ($1, $2, $3, $4, 'active', jsonb_build_object('source', $5::TEXT))
        RETURNING id, pair_type, metadata, left_bill_id, right_bill_id, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(left_bill_id)
    .bind(right_bill_id)
    .bind(pair_type)
    .bind(MANUAL_PAIR_SOURCE)
    .fetch_one(&mut *tx)
    .await
    .map_err(DbError::from)?;
    tx.commit().await.map_err(DbError::from)?;
    Ok(json!({ "pair": postgres_pair_summary_from_row(&row, None)? }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_manual_matching_pair(
    pool: &PostgresPool,
    user_id: UserId,
    pair_id: i64,
) -> MatchingResult<Value> {
    let user_id = user_id_i64(user_id)?;
    let mut tx = pool.begin().await.map_err(DbError::from)?;
    let row = sqlx::query(
        r#"
        SELECT id, pair_type, metadata, left_bill_id, right_bill_id, created_at, updated_at
        FROM matching_pairs
        WHERE id = $1
          AND user_id = $2
          AND status = 'active'
          AND COALESCE(NULLIF(metadata->>'source', ''), $3) = $3
        FOR UPDATE
        "#,
    )
    .bind(pair_id)
    .bind(user_id)
    .bind(MANUAL_PAIR_SOURCE)
    .fetch_optional(&mut *tx)
    .await
    .map_err(DbError::from)?;
    let Some(row) = row else {
        return Err(MatchingRuntimeError::NotFound("Pair not found".to_string()));
    };
    sqlx::query(
        "UPDATE matching_pairs SET status = 'deleted', updated_at = now(), version = version + 1 WHERE id = $1 AND user_id = $2",
    )
    .bind(pair_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(DbError::from)?;
    tx.commit().await.map_err(DbError::from)?;
    Ok(json!({ "pair": postgres_pair_summary_from_row(&row, None)? }))
}

fn pair_from_row(row: &PgRow) -> MatchingResult<Value> {
    Ok(json!({
        "id": row.try_get::<i64, _>("id").map_err(DbError::from)?,
        "pairType": row.try_get::<Option<String>, _>("pair_type")
            .map_err(DbError::from)?
            .unwrap_or_else(|| TRANSFER_PAIR_TYPE.to_string()),
        "source": row.try_get::<Option<String>, _>("source")
            .map_err(DbError::from)?
            .unwrap_or_else(|| MANUAL_PAIR_SOURCE.to_string()),
        "leftBillId": row.try_get::<i64, _>("left_bill_id").map_err(DbError::from)?,
        "rightBillId": row.try_get::<i64, _>("right_bill_id").map_err(DbError::from)?,
        "createdAt": timestamp_string(row.try_get("created_at").map_err(DbError::from)?),
        "updatedAt": timestamp_string(row.try_get("updated_at").map_err(DbError::from)?),
        "leftBill": bill_snapshot_from_row(row, "left_bill")?,
        "rightBill": bill_snapshot_from_row(row, "right_bill")?,
    }))
}

async fn postgres_bill_exists(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> MatchingResult<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM bills WHERE id = $1 AND user_id = $2 AND is_deleted = false)",
    )
    .bind(bill_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(DbError::from)
    .map_err(MatchingRuntimeError::from)
}

async fn query_postgres_pair_for_bill(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> MatchingResult<Value> {
    let row = sqlx::query(
        r#"
        SELECT id, pair_type, metadata, left_bill_id, right_bill_id, created_at, updated_at
        FROM matching_pairs
        WHERE user_id = $1
          AND status = 'active'
          AND $2 IN (left_bill_id, right_bill_id)
        ORDER BY updated_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(bill_id)
    .fetch_optional(pool)
    .await
    .map_err(DbError::from)?;
    row.map(|row| postgres_pair_summary_from_row(&row, Some(bill_id)))
        .transpose()
        .map(|value| value.unwrap_or(Value::Null))
}

fn postgres_pair_summary_from_row(
    row: &PgRow,
    anchor_bill_id: Option<i64>,
) -> MatchingResult<Value> {
    let left_bill_id: i64 = row.try_get("left_bill_id").map_err(DbError::from)?;
    let right_bill_id: i64 = row.try_get("right_bill_id").map_err(DbError::from)?;
    let source = row
        .try_get::<Value, _>("metadata")
        .ok()
        .and_then(|metadata| {
            metadata
                .get("source")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| MANUAL_PAIR_SOURCE.to_string());
    let mut pair = serde_json::Map::new();
    pair.insert(
        "id".to_string(),
        json!(row.try_get::<i64, _>("id").map_err(DbError::from)?),
    );
    pair.insert(
        "pairType".to_string(),
        json!(row
            .try_get::<Option<String>, _>("pair_type")
            .map_err(DbError::from)?
            .unwrap_or_else(|| TRANSFER_PAIR_TYPE.to_string())),
    );
    pair.insert("source".to_string(), json!(source));
    pair.insert("leftBillId".to_string(), json!(left_bill_id));
    pair.insert("rightBillId".to_string(), json!(right_bill_id));
    pair.insert(
        "createdAt".to_string(),
        json!(timestamp_string(
            row.try_get("created_at").map_err(DbError::from)?
        )),
    );
    pair.insert(
        "updatedAt".to_string(),
        json!(timestamp_string(
            row.try_get("updated_at").map_err(DbError::from)?
        )),
    );
    if let Some(anchor_bill_id) = anchor_bill_id {
        pair.insert(
            "otherBillId".to_string(),
            json!(if anchor_bill_id == left_bill_id {
                right_bill_id
            } else {
                left_bill_id
            }),
        );
    }
    Ok(Value::Object(pair))
}

fn user_id_i64(user_id: UserId) -> MatchingResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| MatchingRuntimeError::BadRequest("invalid user id".to_string()))
}

fn bill_snapshot_from_row(row: &PgRow, prefix: &str) -> MatchingResult<Value> {
    let standard_payload: Value =
        try_get_required(row, &format!("{prefix}_standard_payload")).map_err(DbError::from)?;
    let category_path: Option<String> =
        try_get_optional(row, &format!("{prefix}_category_path")).map_err(DbError::from)?;
    let category_name: Option<String> =
        try_get_optional(row, &format!("{prefix}_category_name")).map_err(DbError::from)?;
    let (main_category, sub_category) =
        category_names_from_payload(&standard_payload, category_path, category_name);
    let source_account_id = optional_account_id(row, prefix, "source_account_id")?
        .or_else(|| {
            optional_account_id(row, prefix, "account_id")
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    let destination_account_id = optional_account_id(row, prefix, "target_account_id")?
        .or_else(|| {
            optional_account_id(row, prefix, "transfer_target_account_id")
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    let amount_cents: i64 =
        try_get_required(row, &format!("{prefix}_amount_cents")).map_err(DbError::from)?;

    Ok(json!({
        "id": try_get_required::<i64>(row, &format!("{prefix}_id_value")).map_err(DbError::from)?,
        "date": timestamp_string(try_get_required(row, &format!("{prefix}_occurred_at")).map_err(DbError::from)?),
        "type": try_get_optional::<String>(row, &format!("{prefix}_transaction_type"))
            .map_err(DbError::from)?
            .unwrap_or_default(),
        "amountCents": amount_cents,
        "counterparty": try_get_optional::<String>(row, &format!("{prefix}_merchant"))
            .map_err(DbError::from)?
            .unwrap_or_default(),
        "description": try_get_optional::<String>(row, &format!("{prefix}_description"))
            .map_err(DbError::from)?
            .unwrap_or_default(),
        "paymentMethod": try_get_optional::<String>(row, &format!("{prefix}_payment_method"))
            .map_err(DbError::from)?
            .unwrap_or_default(),
        "mainCategory": main_category,
        "subCategory": sub_category,
        "sourceAccountId": source_account_id,
        "destinationAccountId": destination_account_id,
    }))
}

fn category_names_from_payload(
    payload: &Value,
    category_path: Option<String>,
    category_name: Option<String>,
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

    category_names_from_postgres_path(
        category_path.as_deref(),
        category_name.as_deref().unwrap_or_default(),
    )
}

fn optional_account_id(row: &PgRow, prefix: &str, field: &str) -> MatchingResult<Option<i64>> {
    let value: Option<i64> =
        try_get_optional(row, &format!("{prefix}_{field}")).map_err(DbError::from)?;
    Ok(value.filter(|value| *value > 0))
}

fn try_get_required<T>(row: &PgRow, column: &str) -> Result<T, sqlx::Error>
where
    for<'r> T: sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    row.try_get(column)
}

fn try_get_optional<T>(row: &PgRow, column: &str) -> Result<Option<T>, sqlx::Error>
where
    for<'r> T: sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    row.try_get(column)
}

fn timestamp_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_names_prefer_payload_then_path() {
        assert_eq!(
            category_names_from_payload(
                &json!({"main_category": "Transfer", "sub_category": "Internal"}),
                Some("Ignored/Path".to_string()),
                Some("Ignored".to_string()),
            ),
            ("Transfer".to_string(), "Internal".to_string())
        );
        assert_eq!(
            category_names_from_payload(&json!({}), Some("Food/Lunch".to_string()), None),
            ("Food".to_string(), "Lunch".to_string())
        );
        assert_eq!(
            category_names_from_payload(&json!({}), None, Some("Standalone".to_string())),
            ("Standalone".to_string(), String::new())
        );
    }
}
