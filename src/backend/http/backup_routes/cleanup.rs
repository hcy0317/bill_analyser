use super::*;

pub(super) fn cleanup_backups_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_cleanup",
                json!({"keep_count": "", "deleted_count": 0}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, Some(&payload))?;
    let keep_count = match parse_keep_count(&payload) {
        Ok(value) => value,
        Err(message) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_cleanup",
                json!({"keep_count": audit_safe_json_field(&payload, "keep_count"), "deleted_count": 0}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };

    let backup_dir = backup_dir(&state.config)?;
    let records = list_backup_records(auth_runtime.runtime.connection())
        .map_err(|_| Box::new(db_error_response()))?;
    let record_contracts = records
        .iter()
        .map(|record| {
            let file_exists = resolve_backup_path(&backup_dir, &record.backup_name)
                .map(|path| path.exists())
                .unwrap_or(false);
            BackupRecordContract {
                id: record.id,
                backup_name: record.backup_name.clone(),
                storage_type: record.storage_type.clone(),
                status: record.status.clone(),
                created_at: record.created_at.clone(),
                file_exists,
            }
        })
        .collect::<Vec<_>>();
    let stray_files = list_local_backup_files(&backup_dir)
        .map_err(file_error_response)?
        .into_iter()
        .map(|path| {
            Ok(BackupFileCandidate {
                filename: backup_filename(&path),
                modified_at: file_modified_at(&path)?,
            })
        })
        .collect::<FileRouteResult<Vec<_>>>()
        .map_err(file_error_response)?;

    let plan = plan_backup_cleanup(&record_contracts, &stray_files, keep_count);
    let result = apply_cleanup_plan(
        auth_runtime.runtime.connection(),
        &backup_dir,
        &plan.decisions,
    );
    if let Err(error) = result {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_cleanup",
            json!({"keep_count": keep_count, "deleted_count": plan.deleted_count}),
            i64::try_from(plan.deleted_count).unwrap_or(i64::MAX),
            "failed",
            Some(error.message.clone()),
        );
        return Ok(error_response(error.status, error.message));
    }

    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        "backup_cleanup",
        json!({"keep_count": keep_count, "deleted_count": plan.deleted_count}),
        i64::try_from(plan.deleted_count).unwrap_or(i64::MAX),
        "success",
        None,
    );

    Ok(json_response(
        StatusCode::OK,
        json!({"success": true, "data": {"deleted_count": plan.deleted_count, "kept_count": plan.kept_count}}),
    ))
}

pub(super) fn parse_keep_count(payload: &Value) -> Result<usize, String> {
    let Some(value) = payload.get("keep_count") else {
        return Ok(10);
    };
    let parsed = match value {
        Value::Null => 10,
        Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| "keep_count must be an integer".to_string())?,
        Value::String(text) => text
            .trim()
            .parse::<i64>()
            .map_err(|_| "keep_count must be an integer".to_string())?,
        _ => return Err("keep_count must be an integer".to_string()),
    };
    if parsed < 0 {
        return Err("keep_count must be greater than or equal to 0".to_string());
    }
    usize::try_from(parsed).map_err(|_| "keep_count must be an integer".to_string())
}

pub(super) fn apply_cleanup_plan(
    connection: &rusqlite::Connection,
    backup_dir: &Path,
    decisions: &[BackupCleanupDecision],
) -> FileRouteResult<()> {
    let mut seen = BTreeSet::new();
    for decision in decisions {
        if !seen.insert(decision.filename.clone()) {
            continue;
        }
        match decision.action.as_str() {
            "mark_deleted" => {
                update_backup_record_by_filename(
                    connection,
                    &decision.filename,
                    Some("deleted"),
                    json!({"deleted_reason": decision.reason, "deleted_at": now_iso()}),
                )
                .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
            }
            "delete_file_and_mark_deleted" => {
                let path = resolve_backup_path(backup_dir, &decision.filename)?;
                if path.exists() {
                    fs::remove_file(&path)?;
                }
                remove_metadata_file(&path)?;
                update_backup_record_by_filename(
                    connection,
                    &decision.filename,
                    Some("deleted"),
                    json!({"deleted_reason": decision.reason, "deleted_at": now_iso()}),
                )
                .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
            }
            "delete_stray_file" => {
                let path = resolve_backup_path(backup_dir, &decision.filename)?;
                if path.exists() {
                    fs::remove_file(&path)?;
                }
                remove_metadata_file(&path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_backup_record(
        connection: &rusqlite::Connection,
        filename: &str,
        status: &str,
    ) -> Result<i64, DbError> {
        upsert_backup_record(
            connection,
            BackupRecordDraft {
                backup_name: filename.to_string(),
                file_path: format!("backup/{filename}"),
                checksum: "checksum".to_string(),
                encrypted: filename.ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX),
                status: status.to_string(),
                metadata: json!({}),
            },
        )
    }

    #[test]
    fn parse_keep_count_accepts_defaults_strings_and_rejects_bad_shapes() {
        assert_eq!(parse_keep_count(&json!({})), Ok(10));
        assert_eq!(parse_keep_count(&json!({"keep_count": null})), Ok(10));
        assert_eq!(parse_keep_count(&json!({"keep_count": "2"})), Ok(2));
        assert_eq!(parse_keep_count(&json!({"keep_count": 3})), Ok(3));
        assert_eq!(
            parse_keep_count(&json!({"keep_count": "x"})),
            Err("keep_count must be an integer".to_string())
        );
        assert_eq!(
            parse_keep_count(&json!({"keep_count": -1})),
            Err("keep_count must be greater than or equal to 0".to_string())
        );
        assert_eq!(
            parse_keep_count(&json!({"keep_count": {}})),
            Err("keep_count must be an integer".to_string())
        );
    }

    #[test]
    fn apply_cleanup_plan_marks_records_deletes_files_and_ignores_duplicates() {
        let root = tempfile::tempdir().expect("temp dir");
        let backup_dir = root.path().join("backup");
        fs::create_dir_all(&backup_dir).expect("backup dir");
        let connection = rusqlite::Connection::open_in_memory().expect("connection");
        init_backup_ops_schema(&connection).expect("schema");

        seed_backup_record(&connection, "backup_mark.zip", "created").expect("mark record");
        seed_backup_record(&connection, "backup_delete.zip", "created").expect("delete record");
        fs::write(backup_dir.join("backup_delete.zip"), b"delete").expect("delete file");
        fs::write(backup_dir.join("backup_stray.zip"), b"stray").expect("stray file");

        let decisions = vec![
            BackupCleanupDecision {
                filename: "backup_mark.zip".to_string(),
                action: "mark_deleted".to_string(),
                reason: "old".to_string(),
            },
            BackupCleanupDecision {
                filename: "backup_mark.zip".to_string(),
                action: "mark_deleted".to_string(),
                reason: "duplicate should be ignored".to_string(),
            },
            BackupCleanupDecision {
                filename: "backup_delete.zip".to_string(),
                action: "delete_file_and_mark_deleted".to_string(),
                reason: "old".to_string(),
            },
            BackupCleanupDecision {
                filename: "backup_stray.zip".to_string(),
                action: "delete_stray_file".to_string(),
                reason: "stray".to_string(),
            },
            BackupCleanupDecision {
                filename: "backup_unknown.zip".to_string(),
                action: "keep".to_string(),
                reason: "ignored".to_string(),
            },
        ];

        apply_cleanup_plan(&connection, &backup_dir, &decisions).expect("cleanup plan applies");
        let records = list_backup_records(&connection).expect("records");
        let statuses = records
            .into_iter()
            .map(|record| (record.backup_name, record.status))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            statuses.get("backup_mark.zip").map(String::as_str),
            Some("deleted")
        );
        assert_eq!(
            statuses.get("backup_delete.zip").map(String::as_str),
            Some("deleted")
        );
        assert!(!backup_dir.join("backup_delete.zip").exists());
        assert!(!backup_metadata_path(&backup_dir.join("backup_delete.zip")).exists());
        assert!(!backup_dir.join("backup_stray.zip").exists());
        assert!(!backup_metadata_path(&backup_dir.join("backup_stray.zip")).exists());
    }
}
