// 中文导读：PostgreSQL backup ops 仓储，负责 strict cutover 后的备份记录、任务和审计元数据。
// 维护重点：文件备份 I/O 留在 HTTP 层；这里仅集中 user-scoped PG 元数据读写。
// 不变式：backup_jobs 必须按 user_id 隔离；backup_audit_logs 是 best-effort 调用方可忽略失败。

use bill_analyser_core::UserId;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::Row;

use crate::{
    backup::{
        BackupAuditLogDraft, BackupJobDraft, BackupJobRow, BackupRecordDraft, BackupRecordRow,
    },
    DbError, DbResult, PostgresPool,
};

pub async fn list_postgres_backup_records(pool: &PostgresPool) -> DbResult<Vec<BackupRecordRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, backup_name, storage_type, file_path, checksum, encrypted,
               status, metadata, created_at, updated_at
        FROM backup_records
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(backup_record_from_row).collect()
}

pub async fn upsert_postgres_backup_record(
    pool: &PostgresPool,
    draft: BackupRecordDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO backup_records (
            backup_name, storage_type, file_path, checksum,
            encrypted, status, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
        ON CONFLICT(backup_name) DO UPDATE SET
            storage_type = excluded.storage_type,
            file_path = excluded.file_path,
            checksum = excluded.checksum,
            encrypted = excluded.encrypted,
            status = excluded.status,
            metadata = excluded.metadata,
            updated_at = excluded.updated_at,
            version = backup_records.version + 1
        RETURNING id
        "#,
    )
    .bind(draft.backup_name)
    .bind("local")
    .bind(draft.file_path)
    .bind(draft.checksum)
    .bind(draft.encrypted)
    .bind(draft.status)
    .bind(draft.metadata)
    .fetch_one(pool)
    .await?;

    Ok(row.try_get("id")?)
}

pub async fn update_postgres_backup_record_by_filename(
    pool: &PostgresPool,
    filename: &str,
    status: Option<&str>,
    metadata_update: Value,
) -> DbResult<bool> {
    if filename.trim().is_empty() {
        return Ok(false);
    }

    let existing = sqlx::query(
        r#"
        SELECT metadata
        FROM backup_records
        WHERE backup_name = $1
        LIMIT 1
        "#,
    )
    .bind(filename)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };
    let existing_metadata: Value = existing.try_get("metadata")?;
    let merged_metadata = merge_metadata(existing_metadata, metadata_update);

    let affected = if let Some(status) = status {
        sqlx::query(
            r#"
            UPDATE backup_records
            SET status = $1,
                metadata = $2,
                updated_at = now(),
                version = version + 1
            WHERE backup_name = $3
            "#,
        )
        .bind(status)
        .bind(merged_metadata)
        .bind(filename)
        .execute(pool)
        .await?
        .rows_affected()
    } else {
        sqlx::query(
            r#"
            UPDATE backup_records
            SET metadata = $1,
                updated_at = now(),
                version = version + 1
            WHERE backup_name = $2
            "#,
        )
        .bind(merged_metadata)
        .bind(filename)
        .execute(pool)
        .await?
        .rows_affected()
    };

    Ok(affected > 0)
}

pub async fn list_postgres_backup_jobs(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<BackupJobRow>> {
    let user_id = postgres_user_id(user_id)?;
    let rows = sqlx::query(
        r#"
        SELECT id, job_type, COALESCE(schedule_expr, '') AS schedule_expr,
               retention_days, retention_count, enabled, last_run_at, last_status,
               created_at, updated_at
        FROM backup_jobs
        WHERE user_id = $1
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(backup_job_from_row).collect()
}

pub async fn create_or_update_postgres_backup_job(
    pool: &PostgresPool,
    user_id: UserId,
    draft: BackupJobDraft,
) -> DbResult<i64> {
    let user_id = postgres_user_id(user_id)?;
    let retention_count = i64::try_from(draft.retention_count).unwrap_or(i64::MAX);

    if let Some(job_id) = draft.id {
        let row = sqlx::query(
            r#"
            UPDATE backup_jobs
            SET job_type = $1,
                schedule_expr = $2,
                retention_days = $3,
                retention_count = $4,
                enabled = $5,
                last_status = $6,
                updated_at = now(),
                version = version + 1
            WHERE id = $7
              AND user_id = $8
            RETURNING id
            "#,
        )
        .bind(draft.job_type)
        .bind(draft.schedule_expr)
        .bind(draft.retention_days)
        .bind(retention_count)
        .bind(draft.enabled)
        .bind(draft.last_status)
        .bind(job_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        let Some(row) = row else {
            return Err(DbError::InvalidOperation(
                "backup job not found".to_string(),
            ));
        };
        return Ok(row.try_get("id")?);
    }

    let row = sqlx::query(
        r#"
        INSERT INTO backup_jobs (
            user_id, job_type, schedule_expr, retention_days, retention_count,
            enabled, last_run_at, last_status, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, NULL, $7, now(), now())
        ON CONFLICT(user_id, job_type) DO UPDATE SET
            schedule_expr = excluded.schedule_expr,
            retention_days = excluded.retention_days,
            retention_count = excluded.retention_count,
            enabled = excluded.enabled,
            last_status = excluded.last_status,
            updated_at = excluded.updated_at,
            version = backup_jobs.version + 1
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(draft.job_type)
    .bind(draft.schedule_expr)
    .bind(draft.retention_days)
    .bind(retention_count)
    .bind(draft.enabled)
    .bind(draft.last_status)
    .fetch_one(pool)
    .await?;

    Ok(row.try_get("id")?)
}

pub async fn create_postgres_backup_audit_log_best_effort(
    pool: &PostgresPool,
    draft: BackupAuditLogDraft,
) -> DbResult<()> {
    let details = (!draft.details.is_null()).then_some(draft.details);
    sqlx::query(
        r#"
        INSERT INTO backup_audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now())
        "#,
    )
    .bind(draft.operation_type)
    .bind("backup")
    .bind(Option::<i64>::None)
    .bind(details)
    .bind(draft.affected_count)
    .bind(draft.ip_address)
    .bind(draft.user_agent)
    .bind(Option::<String>::None)
    .bind(draft.status)
    .bind(draft.error_message)
    .execute(pool)
    .await?;
    Ok(())
}

fn backup_record_from_row(row: sqlx::postgres::PgRow) -> DbResult<BackupRecordRow> {
    Ok(BackupRecordRow {
        id: row.try_get("id")?,
        backup_name: row.try_get("backup_name")?,
        storage_type: row.try_get("storage_type")?,
        file_path: row.try_get("file_path")?,
        checksum: row.try_get("checksum")?,
        encrypted: row.try_get("encrypted")?,
        status: row.try_get("status")?,
        metadata: row.try_get("metadata")?,
        created_at: timestamp_text(row.try_get("created_at")?),
        updated_at: timestamp_text(row.try_get("updated_at")?),
    })
}

fn backup_job_from_row(row: sqlx::postgres::PgRow) -> DbResult<BackupJobRow> {
    Ok(BackupJobRow {
        id: row.try_get("id")?,
        job_type: row.try_get("job_type")?,
        schedule_expr: row.try_get("schedule_expr")?,
        retention_days: row.try_get("retention_days")?,
        retention_count: row.try_get("retention_count")?,
        enabled: row.try_get("enabled")?,
        last_run_at: row
            .try_get::<Option<DateTime<Utc>>, _>("last_run_at")?
            .map(timestamp_text),
        last_status: row.try_get("last_status")?,
        created_at: timestamp_text(row.try_get("created_at")?),
        updated_at: timestamp_text(row.try_get("updated_at")?),
    })
}

fn merge_metadata(existing: Value, update: Value) -> Value {
    let mut existing = existing.as_object().cloned().unwrap_or_default();
    if let Some(update) = update.as_object() {
        for (key, value) in update {
            existing.insert(key.clone(), value.clone());
        }
    }
    Value::Object(existing)
}

fn postgres_user_id(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get()).map_err(|_| {
        DbError::InvalidOperation("user_id does not fit PostgreSQL BIGINT".to_string())
    })
}

fn timestamp_text(value: DateTime<Utc>) -> String {
    value.naive_utc().format("%Y-%m-%dT%H:%M:%S%.f").to_string()
}
