use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_backup_audit_log_best_effort, create_or_update_backup_job, init_backup_ops_schema,
    list_backup_jobs, list_backup_records, update_backup_record_by_filename, upsert_backup_record,
    BackupAuditLogDraft, BackupJobDraft, BackupRecordDraft,
};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn backup_ops_repository_roundtrips_jobs_and_audit_logs() -> Result<(), Box<dyn Error>> {
    let connection = Connection::open_in_memory()?;
    init_backup_ops_schema(&connection)?;
    let user_id = UserId::new(42)?;
    let other_user_id = UserId::new(77)?;
    assert!(list_backup_jobs(&connection, user_id)?.is_empty());

    let first_id = create_or_update_backup_job(
        &connection,
        user_id,
        BackupJobDraft {
            id: None,
            job_type: "manual".to_string(),
            schedule_expr: String::new(),
            retention_days: 30,
            retention_count: 10,
            enabled: true,
            last_status: None,
        },
    )?;
    assert_eq!(first_id, 1);

    let second_id = create_or_update_backup_job(
        &connection,
        user_id,
        BackupJobDraft {
            id: None,
            job_type: "weekly".to_string(),
            schedule_expr: "0 2 * * 1".to_string(),
            retention_days: 60,
            retention_count: 4,
            enabled: false,
            last_status: Some("200".to_string()),
        },
    )?;
    assert_eq!(second_id, 2);

    let other_user_job_id = create_or_update_backup_job(
        &connection,
        other_user_id,
        BackupJobDraft {
            id: None,
            job_type: "manual".to_string(),
            schedule_expr: "0 5 * * *".to_string(),
            retention_days: 15,
            retention_count: 3,
            enabled: true,
            last_status: None,
        },
    )?;
    assert_eq!(other_user_job_id, 3);

    let jobs = list_backup_jobs(&connection, user_id)?;
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].id, second_id);
    assert_eq!(jobs[0].job_type, "weekly");
    assert_eq!(jobs[0].retention_days, 60);
    assert_eq!(jobs[0].retention_count, 4);
    assert!(!jobs[0].enabled);
    assert_eq!(jobs[0].last_status.as_deref(), Some("200"));

    let updated_id = create_or_update_backup_job(
        &connection,
        user_id,
        BackupJobDraft {
            id: Some(first_id),
            job_type: "daily".to_string(),
            schedule_expr: "0 3 * * *".to_string(),
            retention_days: 45,
            retention_count: 6,
            enabled: true,
            last_status: Some("ok".to_string()),
        },
    )?;
    assert_eq!(updated_id, first_id);
    let refreshed = list_backup_jobs(&connection, user_id)?;
    let first = refreshed
        .iter()
        .find(|job| job.id == first_id)
        .expect("updated first job");
    assert_eq!(first.job_type, "daily");
    assert_eq!(first.schedule_expr, "0 3 * * *");
    assert_eq!(first.retention_days, 45);
    assert_eq!(first.retention_count, 6);
    assert_eq!(first.last_status.as_deref(), Some("ok"));

    let upserted_id = create_or_update_backup_job(
        &connection,
        user_id,
        BackupJobDraft {
            id: None,
            job_type: "weekly".to_string(),
            schedule_expr: "0 4 * * 1".to_string(),
            retention_days: 90,
            retention_count: 8,
            enabled: true,
            last_status: Some("upserted".to_string()),
        },
    )?;
    assert_eq!(upserted_id, second_id);
    assert_eq!(list_backup_jobs(&connection, user_id)?.len(), 2);
    let foreign_update = create_or_update_backup_job(
        &connection,
        other_user_id,
        BackupJobDraft {
            id: Some(first_id),
            job_type: "manual".to_string(),
            schedule_expr: String::new(),
            retention_days: 30,
            retention_count: 10,
            enabled: true,
            last_status: None,
        },
    )
    .expect_err("foreign job id is not updated");
    assert_eq!(
        foreign_update.to_string(),
        "invalid database operation: backup job not found"
    );
    assert_eq!(list_backup_jobs(&connection, other_user_id)?.len(), 1);

    create_backup_audit_log_best_effort(
        &connection,
        BackupAuditLogDraft {
            operation_type: "backup_job_saved".to_string(),
            details: json!({"job_id": first_id, "job_type": "daily"}),
            affected_count: 1,
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("db-test".to_string()),
            status: "success".to_string(),
            error_message: None,
        },
    );
    let audit_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM audit_logs WHERE operation_type = 'backup_job_saved' AND status = 'success'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(audit_count, 1);

    Ok(())
}

#[test]
fn backup_ops_schema_migrates_legacy_jobs_to_user_scoped_unique_rows() -> Result<(), Box<dyn Error>>
{
    let connection = Connection::open_in_memory()?;
    connection.execute_batch(
        r#"
        CREATE TABLE backup_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
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
        INSERT INTO backup_jobs (
            job_type, schedule_expr, retention_days, retention_count,
            enabled, last_run_at, last_status, created_at, updated_at
        ) VALUES
            ('daily', '0 1 * * *', 30, 10, 1, NULL, NULL,
             '2026-05-15T01:00:00', '2026-05-15T01:00:00'),
            ('daily', '0 2 * * *', 60, 4, 0, NULL, 'legacy',
             '2026-05-15T02:00:00', '2026-05-15T02:00:00');
        "#,
    )?;

    init_backup_ops_schema(&connection)?;

    let user_id = UserId::new(1)?;
    let migrated_jobs = list_backup_jobs(&connection, user_id)?;
    assert_eq!(migrated_jobs.len(), 1);
    let migrated_job = &migrated_jobs[0];
    assert_eq!(migrated_job.job_type, "daily");
    assert_eq!(migrated_job.schedule_expr, "0 2 * * *");
    assert_eq!(migrated_job.retention_days, 60);
    assert_eq!(migrated_job.retention_count, 4);
    assert!(!migrated_job.enabled);
    assert_eq!(migrated_job.last_status.as_deref(), Some("legacy"));

    let upserted_id = create_or_update_backup_job(
        &connection,
        user_id,
        BackupJobDraft {
            id: None,
            job_type: "daily".to_string(),
            schedule_expr: "0 3 * * *".to_string(),
            retention_days: 90,
            retention_count: 8,
            enabled: true,
            last_status: Some("upserted".to_string()),
        },
    )?;
    assert_eq!(upserted_id, migrated_job.id);

    let refreshed = list_backup_jobs(&connection, user_id)?;
    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].schedule_expr, "0 3 * * *");
    assert_eq!(refreshed[0].last_status.as_deref(), Some("upserted"));

    let other_user_id = UserId::new(2)?;
    let other_user_job_id = create_or_update_backup_job(
        &connection,
        other_user_id,
        BackupJobDraft {
            id: None,
            job_type: "daily".to_string(),
            schedule_expr: "0 4 * * *".to_string(),
            retention_days: 15,
            retention_count: 2,
            enabled: true,
            last_status: None,
        },
    )?;
    assert_ne!(other_user_job_id, upserted_id);
    assert_eq!(list_backup_jobs(&connection, other_user_id)?.len(), 1);

    Ok(())
}

#[test]
fn backup_ops_repository_roundtrips_backup_records() -> Result<(), Box<dyn Error>> {
    let connection = Connection::open_in_memory()?;
    init_backup_ops_schema(&connection)?;

    let record_id = upsert_backup_record(
        &connection,
        BackupRecordDraft {
            backup_name: "backup_20260515_120000.zip".to_string(),
            file_path: "C:/repo/backup/backup_20260515_120000.zip".to_string(),
            checksum: "abc123".to_string(),
            encrypted: false,
            status: "created".to_string(),
            metadata: json!({
                "valid_zip": true,
                "ready_to_restore": true,
                "entry_count": 1,
            }),
        },
    )?;
    assert_eq!(record_id, 1);

    let same_record_id = upsert_backup_record(
        &connection,
        BackupRecordDraft {
            backup_name: "backup_20260515_120000.zip".to_string(),
            file_path: "C:/repo/backup/backup_20260515_120000.zip".to_string(),
            checksum: "def456".to_string(),
            encrypted: true,
            status: "created".to_string(),
            metadata: json!({
                "valid_zip": true,
                "ready_to_restore": true,
                "entry_count": 2,
            }),
        },
    )?;
    assert_eq!(same_record_id, record_id);

    let records = list_backup_records(&connection)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].backup_name, "backup_20260515_120000.zip");
    assert_eq!(records[0].checksum.as_deref(), Some("def456"));
    assert!(records[0].encrypted);
    assert_eq!(records[0].metadata["entry_count"], 2);

    assert!(update_backup_record_by_filename(
        &connection,
        "backup_20260515_120000.zip",
        Some("deleted"),
        json!({"deleted_reason": "manual_delete"})
    )?);
    assert!(!update_backup_record_by_filename(
        &connection,
        "missing.zip",
        Some("deleted"),
        json!({"deleted_reason": "manual_delete"})
    )?);

    let updated = list_backup_records(&connection)?;
    assert_eq!(updated[0].status, "deleted");
    assert_eq!(updated[0].metadata["entry_count"], 2);
    assert_eq!(updated[0].metadata["deleted_reason"], "manual_delete");

    Ok(())
}
