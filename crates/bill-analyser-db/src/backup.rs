use bill_analyser_core::{ops::BackupJobContract, UserId};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

use crate::{schema::migrate_user_id_field, user_scope::UserScope, DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupJobDraft {
    pub id: Option<i64>,
    pub job_type: String,
    pub schedule_expr: String,
    pub retention_days: i64,
    pub retention_count: usize,
    pub enabled: bool,
    pub last_status: Option<String>,
}

impl From<BackupJobContract> for BackupJobDraft {
    fn from(value: BackupJobContract) -> Self {
        Self {
            id: value.id,
            job_type: value.job_type,
            schedule_expr: value.schedule_expr,
            retention_days: value.retention_days,
            retention_count: value.retention_count,
            enabled: value.enabled,
            last_status: value.last_status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupJobRow {
    pub id: i64,
    pub job_type: String,
    pub schedule_expr: String,
    pub retention_days: i64,
    pub retention_count: i64,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct BackupAuditLogDraft {
    pub operation_type: String,
    pub details: Value,
    pub affected_count: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
}

pub fn init_backup_ops_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_type TEXT NOT NULL,
            operation_target TEXT NOT NULL,
            target_id INTEGER,
            details TEXT,
            affected_count INTEGER DEFAULT 0,
            ip_address TEXT,
            user_agent TEXT,
            session_id TEXT,
            status TEXT NOT NULL,
            error_message TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS backup_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            backup_name TEXT NOT NULL UNIQUE,
            storage_type TEXT NOT NULL DEFAULT 'local',
            file_path TEXT NOT NULL,
            checksum TEXT,
            encrypted BOOLEAN DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'created',
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS backup_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            job_type TEXT NOT NULL,
            schedule_expr TEXT,
            retention_days INTEGER DEFAULT 30,
            retention_count INTEGER DEFAULT 10,
            enabled BOOLEAN DEFAULT 1,
            last_run_at TEXT,
            last_status TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_audit_logs_type ON audit_logs(operation_type, created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_target
            ON audit_logs(operation_target, target_id);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_status ON audit_logs(status);
        CREATE INDEX IF NOT EXISTS idx_backup_records_status_created
            ON backup_records(status, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_backup_jobs_type_enabled
            ON backup_jobs(job_type, enabled);
        "#,
    )?;
    migrate_user_id_field(connection, "backup_jobs")?;
    dedupe_backup_jobs_by_user_and_type(connection)?;
    connection.execute(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_backup_jobs_user_job_type
            ON backup_jobs(user_id, job_type)
        "#,
        [],
    )?;
    connection.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_backup_jobs_user_type_enabled
            ON backup_jobs(user_id, job_type, enabled)
        "#,
        [],
    )?;
    Ok(())
}

pub fn list_backup_jobs(connection: &Connection, user_id: UserId) -> DbResult<Vec<BackupJobRow>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut statement = connection.prepare(
        r#"
        SELECT id, job_type, COALESCE(schedule_expr, ''), retention_days, retention_count,
               enabled, last_run_at, last_status, created_at, updated_at
        FROM backup_jobs
        WHERE user_id = ?1
        ORDER BY created_at DESC, id DESC
        "#,
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(BackupJobRow {
            id: row.get(0)?,
            job_type: row.get(1)?,
            schedule_expr: row.get(2)?,
            retention_days: row.get(3)?,
            retention_count: row.get(4)?,
            enabled: row.get::<_, i64>(5)? != 0,
            last_run_at: row.get(6)?,
            last_status: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;

    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row?);
    }
    Ok(jobs)
}

pub fn create_or_update_backup_job(
    connection: &Connection,
    user_id: UserId,
    draft: BackupJobDraft,
) -> DbResult<i64> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let now = utc_now_iso();
    let retention_count = i64::try_from(draft.retention_count).unwrap_or(i64::MAX);

    if let Some(job_id) = draft.id {
        let affected = connection.execute(
            r#"
            UPDATE backup_jobs
            SET job_type = ?1,
                schedule_expr = ?2,
                retention_days = ?3,
                retention_count = ?4,
                enabled = ?5,
                last_status = ?6,
                updated_at = ?7
            WHERE id = ?8
              AND user_id = ?9
            "#,
            params![
                draft.job_type,
                draft.schedule_expr,
                draft.retention_days,
                retention_count,
                if draft.enabled { 1 } else { 0 },
                draft.last_status,
                now,
                job_id,
                user_id,
            ],
        )?;
        if affected == 0 {
            return Err(DbError::InvalidOperation(
                "backup job not found".to_string(),
            ));
        }
        return Ok(job_id);
    }

    let job_type = draft.job_type;
    let schedule_expr = draft.schedule_expr;
    let last_status = draft.last_status;

    connection.execute(
        r#"
        INSERT INTO backup_jobs (
            user_id, job_type, schedule_expr, retention_days, retention_count,
            enabled, last_run_at, last_status, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(user_id, job_type) DO UPDATE SET
            schedule_expr = excluded.schedule_expr,
            retention_days = excluded.retention_days,
            retention_count = excluded.retention_count,
            enabled = excluded.enabled,
            last_status = excluded.last_status,
            updated_at = excluded.updated_at
        "#,
        params![
            user_id,
            &job_type,
            schedule_expr,
            draft.retention_days,
            retention_count,
            if draft.enabled { 1 } else { 0 },
            Option::<String>::None,
            last_status,
            now,
            now,
        ],
    )?;
    find_backup_job_id_by_type(connection, user_id, &job_type)?.ok_or_else(|| {
        DbError::InvalidOperation("backup job upsert did not return a row".to_string())
    })
}

fn dedupe_backup_jobs_by_user_and_type(connection: &Connection) -> DbResult<()> {
    connection.execute(
        r#"
        DELETE FROM backup_jobs
        WHERE id NOT IN (
            SELECT MAX(id)
            FROM backup_jobs
            GROUP BY user_id, job_type
        )
        "#,
        [],
    )?;
    Ok(())
}

fn find_backup_job_id_by_type(
    connection: &Connection,
    user_id: i64,
    job_type: &str,
) -> DbResult<Option<i64>> {
    let mut statement = connection.prepare(
        r#"
        SELECT id
        FROM backup_jobs
        WHERE user_id = ?1
          AND job_type = ?2
        ORDER BY id DESC
        LIMIT 1
        "#,
    )?;
    let mut rows = statement.query(params![user_id, job_type])?;
    Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
}

pub fn create_backup_audit_log_best_effort(connection: &Connection, draft: BackupAuditLogDraft) {
    let details = if draft.details.is_null() {
        None
    } else {
        Some(draft.details.to_string())
    };
    let _ = connection.execute(
        r#"
        INSERT INTO audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        params![
            draft.operation_type,
            "backup",
            Option::<i64>::None,
            details,
            draft.affected_count,
            draft.ip_address,
            draft.user_agent,
            Option::<String>::None,
            draft.status,
            draft.error_message,
            utc_now_iso(),
        ],
    );
}

fn utc_now_iso() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}
