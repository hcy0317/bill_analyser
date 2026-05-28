// 中文导读：PostgreSQL 向量派生 outbox，负责把权威业务变更投递给 Weaviate 消费器。
// 维护重点：outbox 状态在 PostgreSQL 中可审计；Weaviate 写入失败只能重试/降级，不能改变权威业务表。
// 不变式：claim 使用短事务和 SKIP LOCKED，避免多个消费者重复处理同一批事件。

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::Row;

use crate::{DbResult, PostgresPool};

pub const VECTOR_OUTBOX_STATUS_PENDING: &str = "pending";
pub const VECTOR_OUTBOX_STATUS_PROCESSING: &str = "processing";
pub const VECTOR_OUTBOX_STATUS_COMPLETED: &str = "completed";
pub const VECTOR_OUTBOX_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, PartialEq)]
pub struct VectorOutboxEventDraft {
    pub user_id: Option<i64>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub payload: Value,
    pub available_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorOutboxEvent {
    pub id: i64,
    pub user_id: Option<i64>,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub payload: Value,
    pub status: String,
    pub attempts: i32,
    pub available_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportLearningFeatureVectorSource {
    pub feature_id: i64,
    pub user_id: i64,
    pub sample_id: i64,
    pub sample_key: String,
    pub feature_key: String,
    pub feature_hash: String,
    pub feature_payload: Value,
    pub normalized_features: Value,
    pub target_payload: Value,
    pub source_payload: Value,
}

pub async fn enqueue_vector_outbox_event(
    pool: &PostgresPool,
    draft: &VectorOutboxEventDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO vector_outbox_events (
            user_id, aggregate_type, aggregate_id, event_type, payload, available_at
        )
        VALUES ($1, $2, $3, $4, $5, COALESCE($6, now()))
        RETURNING id
        "#,
    )
    .bind(draft.user_id)
    .bind(&draft.aggregate_type)
    .bind(&draft.aggregate_id)
    .bind(&draft.event_type)
    .bind(&draft.payload)
    .bind(draft.available_at)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get("id")?)
}

pub async fn claim_pending_vector_outbox_events(
    pool: &PostgresPool,
    limit: usize,
) -> DbResult<Vec<VectorOutboxEvent>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut transaction = pool.begin().await?;
    let rows = sqlx::query(
        r#"
        WITH candidate AS (
            SELECT id
            FROM vector_outbox_events
            WHERE status = 'pending' AND available_at <= now()
            ORDER BY available_at ASC, id ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
        )
        UPDATE vector_outbox_events AS event
        SET status = 'processing',
            locked_at = now(),
            attempts = event.attempts + 1,
            updated_at = now()
        FROM candidate
        WHERE event.id = candidate.id
        RETURNING event.id, event.user_id, event.aggregate_type, event.aggregate_id,
            event.event_type, event.payload, event.status, event.attempts, event.available_at,
            event.locked_at, event.last_error, event.created_at, event.updated_at
        "#,
    )
    .bind(limit as i64)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    rows.into_iter().map(vector_outbox_event_from_row).collect()
}

pub async fn mark_vector_outbox_event_succeeded(
    pool: &PostgresPool,
    event_id: i64,
) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE vector_outbox_events
        SET status = 'completed',
            locked_at = NULL,
            last_error = NULL,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_vector_outbox_event_failed(
    pool: &PostgresPool,
    event_id: i64,
    error: &str,
    retry_delay_seconds: i64,
    max_attempts: i32,
) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE vector_outbox_events
        SET status = CASE
                WHEN attempts >= $4 THEN 'failed'
                ELSE 'pending'
            END,
            locked_at = NULL,
            last_error = $2,
            available_at = CASE
                WHEN attempts >= $4 THEN available_at
                ELSE now() + ($3::double precision * interval '1 second')
            END,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(event_id)
    .bind(error)
    .bind(retry_delay_seconds.max(0) as f64)
    .bind(max_attempts.max(1))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_import_learning_feature_vector_sources(
    pool: &PostgresPool,
    user_id: Option<i64>,
    limit: usize,
) -> DbResult<Vec<ImportLearningFeatureVectorSource>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            feature.id AS feature_id,
            feature.user_id AS user_id,
            feature.sample_id AS sample_id,
            sample.sample_key AS sample_key,
            feature.feature_key AS feature_key,
            feature.feature_hash AS feature_hash,
            feature.feature_payload AS feature_payload,
            sample.normalized_features AS normalized_features,
            sample.target_payload AS target_payload,
            sample.source_payload AS source_payload
        FROM import_learning_features AS feature
        JOIN import_learning_samples AS sample
            ON sample.id = feature.sample_id
            AND sample.user_id = feature.user_id
        WHERE ($1::BIGINT IS NULL OR feature.user_id = $1)
        ORDER BY feature.user_id ASC, feature.id ASC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(ImportLearningFeatureVectorSource {
                feature_id: row.try_get("feature_id")?,
                user_id: row.try_get("user_id")?,
                sample_id: row.try_get("sample_id")?,
                sample_key: row.try_get("sample_key")?,
                feature_key: row.try_get("feature_key")?,
                feature_hash: row.try_get("feature_hash")?,
                feature_payload: row.try_get("feature_payload")?,
                normalized_features: row.try_get("normalized_features")?,
                target_payload: row.try_get("target_payload")?,
                source_payload: row.try_get("source_payload")?,
            })
        })
        .collect()
}

fn vector_outbox_event_from_row(row: sqlx::postgres::PgRow) -> DbResult<VectorOutboxEvent> {
    Ok(VectorOutboxEvent {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        aggregate_type: row.try_get("aggregate_type")?,
        aggregate_id: row.try_get("aggregate_id")?,
        event_type: row.try_get("event_type")?,
        payload: row.try_get("payload")?,
        status: row.try_get("status")?,
        attempts: row.try_get("attempts")?,
        available_at: row.try_get("available_at")?,
        locked_at: row.try_get("locked_at")?,
        last_error: row.try_get("last_error")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
