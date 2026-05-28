// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn verify_backup_restore_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_audit_event(
                &auth_runtime.runtime,
                auth_runtime.user_id,
                headers,
                "backup_restore_verified",
                json!({}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, Some(&payload))?;
    let filename = payload
        .get("filename")
        .and_then(value_to_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if filename.is_empty() {
        write_backup_audit_event(
            &auth_runtime.runtime,
            auth_runtime.user_id,
            headers,
            "backup_restore_verified",
            json!({"filename": audit_safe_json_field(&payload, "filename")}),
            0,
            "failed",
            Some("filename is required".to_string()),
        );
        return Ok(error_response(
            StatusCode::BAD_REQUEST,
            "filename is required",
        ));
    }

    let backup_dir = backup_dir(&state.config)?;
    let file_path = match resolve_backup_path(&backup_dir, &filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                &auth_runtime.runtime,
                auth_runtime.user_id,
                headers,
                "backup_restore_verified",
                json!({"filename": audit_safe_value(&Value::String(filename))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            &auth_runtime.runtime,
            auth_runtime.user_id,
            headers,
            "backup_restore_verified",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Ok(error_response(StatusCode::NOT_FOUND, "文件不存在"));
    }

    let backup_info =
        build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
            .map_err(file_error_response)?;
    let restore_ready = backup_info.valid_zip && backup_info.ready_to_restore;
    let restore_error = if backup_info.error.is_empty() {
        "backup archive is not ready to restore".to_string()
    } else {
        backup_info.error.clone()
    };
    write_backup_audit_event(
        &auth_runtime.runtime,
        auth_runtime.user_id,
        headers,
        "backup_restore_verified",
        json!({
            "filename": safe_filename,
            "checksum": backup_info.checksum,
            "valid_zip": backup_info.valid_zip,
            "ready_to_restore": backup_info.ready_to_restore,
            "metadata_checksum_matched": backup_info.metadata_checksum_matched,
        }),
        0,
        if restore_ready { "success" } else { "failed" },
        (!restore_ready).then_some(restore_error.clone()),
    );

    Ok(json_response(
        if restore_ready {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        },
        backup_restore_verify_response(&backup_info),
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn restore_backup_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    filename: &str,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let data_dir = data_dir(&state.config);
    let file_path = match resolve_backup_path(&backup_dir, filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                &auth_runtime.runtime,
                auth_runtime.user_id,
                headers,
                "backup_restored",
                json!({"filename": audit_safe_value(&Value::String(filename.to_string()))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            &auth_runtime.runtime,
            auth_runtime.user_id,
            headers,
            "backup_restored",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Ok(error_response(StatusCode::NOT_FOUND, "文件不存在"));
    }

    let restored_at = now_iso();
    let user_id = auth_runtime.user_id;
    drop(auth_runtime);
    let result = restore_data_dir_from_backup(
        &file_path,
        &backup_dir,
        &data_dir,
        state.config.backup_encryption_key.as_deref(),
    );
    let runtime = open_backup_ops_runtime(state)?;

    if let Err(error) = result {
        write_backup_audit_event(
            &runtime,
            user_id,
            headers,
            "backup_restored",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some(error.message.clone()),
        );
        return Ok(error_response(error.status, error.message));
    }
    update_backup_record_for_runtime(
        &runtime,
        &safe_filename,
        Some("restored"),
        json!({"restored_at": restored_at}),
    )
    .map_err(|error| Box::new(db_write_error_response(error)))?;

    write_backup_audit_event(
        &runtime,
        user_id,
        headers,
        "backup_restored",
        json!({"filename": safe_filename, "restored_at": restored_at}),
        1,
        "success",
        None,
    );
    Ok(json_response(
        StatusCode::OK,
        json!({"success": true, "data": {"filename": safe_filename, "restored_at": restored_at}}),
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn restore_data_dir_from_backup(
    file_path: &Path,
    backup_dir: &Path,
    data_dir: &Path,
    encryption_key: Option<&str>,
) -> FileRouteResult<()> {
    validate_data_dir_for_restore(data_dir)?;
    if is_encrypted_backup_storage_path(file_path) {
        let Some(secret) = encryption_key.filter(|value| !value.trim().is_empty()) else {
            return Err(BackupFileRuntimeError::bad_request("备份加密密钥未配置"));
        };
        let bytes = decrypted_backup_bytes(file_path, secret)?;
        restore_data_dir_from_archive(io::Cursor::new(bytes), backup_dir, data_dir)
    } else {
        restore_data_dir_from_archive(File::open(file_path)?, backup_dir, data_dir)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn restore_data_dir_from_archive<R: Read + Seek>(
    reader: R,
    backup_dir: &Path,
    data_dir: &Path,
) -> FileRouteResult<()> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|error| BackupFileRuntimeError::bad_request(error.to_string()))?;
    let names = archive.file_names().map(str::to_string).collect::<Vec<_>>();
    if let Some(unsafe_name) = names
        .iter()
        .find(|name| !is_safe_backup_archive_member(name))
    {
        return Err(BackupFileRuntimeError::bad_request(format!(
            "备份文件包含不安全路径: {unsafe_name}"
        )));
    }
    let summary = backup_archive_summary_from_entries(names);
    if !(summary.valid_zip && summary.ready_to_restore) {
        let error = if summary.error.is_empty() {
            "backup archive is not ready to restore".to_string()
        } else {
            summary.error
        };
        return Err(BackupFileRuntimeError::bad_request(error));
    }

    let temp_restore_dir = TempFileBuilder::new()
        .prefix("restore_temp_")
        .tempdir_in(backup_dir)?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let member_name = file.name().replace('\\', "/");
        if !is_safe_backup_archive_member(&member_name) {
            return Err(BackupFileRuntimeError::bad_request(format!(
                "备份文件包含不安全路径: {member_name}"
            )));
        }
        let out_path = temp_restore_dir.path().join(&member_name);
        if file.is_dir() || member_name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&out_path)?;
        io::copy(&mut file, &mut output)?;
    }

    let extracted_data_dir = temp_restore_dir.path().join("data");
    if !extracted_data_dir.exists() {
        return Err(BackupFileRuntimeError::bad_request(
            "备份文件缺少 data/ 目录",
        ));
    }
    if !directory_contains_file(&extracted_data_dir) {
        return Err(BackupFileRuntimeError::bad_request(
            "备份文件缺少可恢复的 data/ 文件",
        ));
    }

    let data_parent = data_dir
        .parent()
        .ok_or_else(|| BackupFileRuntimeError::internal("data directory has no parent"))?;
    let staged_parent = TempFileBuilder::new()
        .prefix("restore_stage_")
        .tempdir_in(data_parent)?;
    let staged_data_dir = staged_parent.path().join("data");
    copy_dir_all(&extracted_data_dir, &staged_data_dir)?;

    let rollback_dir = unique_directory_path(data_parent, "data_rollback_");
    if data_dir.exists() {
        let backup_current = unique_directory_path(backup_dir, "before_restore_");
        copy_dir_all(data_dir, &backup_current)?;
        fs::rename(data_dir, &rollback_dir).map_err(|error| {
            io_context_error(
                error,
                format!(
                    "stage current data directory {}",
                    data_dir.to_string_lossy()
                ),
            )
        })?;
    }
    if let Err(error) = fs::rename(&staged_data_dir, data_dir) {
        if rollback_dir.exists() && !data_dir.exists() {
            let _ = fs::rename(&rollback_dir, data_dir);
        }
        return Err(io_context_error(
            error,
            format!("activate restored data directory {}", data_dir.display()),
        ));
    }
    if rollback_dir.exists() {
        let _ = fs::remove_dir_all(&rollback_dir);
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn validate_data_dir_for_restore(data_dir: &Path) -> FileRouteResult<()> {
    if data_dir.file_name().and_then(|value| value.to_str()) != Some("data") {
        return Err(BackupFileRuntimeError::internal(
            "Rust backup restore data dir must end with data",
        ));
    }
    if data_dir.parent().is_none() {
        return Err(BackupFileRuntimeError::internal(
            "Rust backup restore data dir must have a parent",
        ));
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn copy_dir_all(from: &Path, to: &Path) -> FileRouteResult<()> {
    copy_dir_all_filtered(from, to, &|_| false)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn copy_dir_all_filtered(
    from: &Path,
    to: &Path,
    should_skip_relative: &dyn Fn(&Path) -> bool,
) -> FileRouteResult<()> {
    fs::create_dir_all(to)?;
    for entry in WalkDir::new(from) {
        let entry = entry.map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(from)
            .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if should_skip_relative(relative) {
            continue;
        }
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn unique_directory_path(parent: &Path, prefix: &str) -> PathBuf {
    let counter = BACKUP_FILENAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        "{}{}_{}",
        prefix,
        Local::now().format("%Y%m%d_%H%M%S_%f"),
        counter
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn directory_contains_file(directory: &Path) -> bool {
    directory.exists()
        && WalkDir::new(directory)
            .into_iter()
            .filter_map(Result::ok)
            .any(|entry| entry.file_type().is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_bytes(entries: &[(&str, &[u8])]) -> FileRouteResult<Vec<u8>> {
        let cursor = io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, bytes) in entries {
            zip.start_file(name, options)
                .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
            zip.write_all(bytes)?;
        }
        let cursor = zip
            .finish()
            .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        Ok(cursor.into_inner())
    }

    fn write_zip_entries(path: &Path, entries: &[(&str, &[u8])]) -> FileRouteResult<()> {
        let bytes = zip_bytes(entries)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    #[test]
    fn restore_archive_rejects_invalid_unsafe_and_empty_archives() -> FileRouteResult<()> {
        let root = tempfile::tempdir()?;
        let backup_dir = root.path().join("backup");
        let data_dir = root.path().join("data");
        fs::create_dir_all(&backup_dir)?;
        fs::create_dir_all(&data_dir)?;

        let invalid_zip = restore_data_dir_from_archive(
            io::Cursor::new(b"not zip".as_slice()),
            &backup_dir,
            &data_dir,
        )
        .expect_err("invalid zip should fail");
        assert!(invalid_zip.message.contains("invalid Zip"));

        let unsafe_zip = zip_bytes(&[("../escape.txt", b"escape")])?;
        let unsafe_error =
            restore_data_dir_from_archive(io::Cursor::new(unsafe_zip), &backup_dir, &data_dir)
                .expect_err("unsafe member should fail");
        assert!(unsafe_error.message.contains("不安全路径"));

        let missing_data_zip = zip_bytes(&[("other/file.txt", b"other")])?;
        let missing_data = restore_data_dir_from_archive(
            io::Cursor::new(missing_data_zip),
            &backup_dir,
            &data_dir,
        )
        .expect_err("missing data directory should fail");
        assert!(
            missing_data.message.contains("not ready") || missing_data.message.contains("data")
        );

        let empty_data_zip = zip_bytes(&[("data/", b"")])?;
        let empty_data =
            restore_data_dir_from_archive(io::Cursor::new(empty_data_zip), &backup_dir, &data_dir)
                .expect_err("empty data directory should fail");
        assert!(empty_data.message.contains("not ready") || empty_data.message.contains("data"));
        Ok(())
    }

    #[test]
    fn restore_archive_replaces_data_and_copy_helpers_preserve_filters() -> FileRouteResult<()> {
        let root = tempfile::tempdir()?;
        let backup_dir = root.path().join("backup");
        let data_dir = root.path().join("data");
        fs::create_dir_all(&backup_dir)?;
        fs::create_dir_all(&data_dir)?;
        fs::write(data_dir.join("old.txt"), b"old")?;

        let restore_zip = zip_bytes(&[("data/new.txt", b"new")])?;
        restore_data_dir_from_archive(io::Cursor::new(restore_zip), &backup_dir, &data_dir)?;
        assert_eq!(fs::read(data_dir.join("new.txt"))?, b"new");
        assert!(!data_dir.join("old.txt").exists());

        let from = root.path().join("from");
        let to = root.path().join("to");
        fs::create_dir_all(from.join("nested"))?;
        fs::write(from.join("keep.txt"), b"keep")?;
        fs::write(from.join("nested").join("skip.txt"), b"skip")?;
        copy_dir_all_filtered(&from, &to, &|relative| relative.ends_with("skip.txt"))?;
        assert!(to.join("keep.txt").exists());
        assert!(!to.join("nested").join("skip.txt").exists());

        assert!(unique_directory_path(&backup_dir, "rollback_")
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .starts_with("rollback_"));
        assert!(directory_contains_file(&to));
        assert!(!directory_contains_file(&root.path().join("missing")));
        Ok(())
    }

    #[test]
    fn restore_data_dir_from_backup_handles_encrypted_paths_and_data_dir_validation(
    ) -> FileRouteResult<()> {
        let root = tempfile::tempdir()?;
        let backup_dir = root.path().join("backup");
        let data_dir = root.path().join("data");
        fs::create_dir_all(&backup_dir)?;
        fs::create_dir_all(&data_dir)?;

        assert!(validate_data_dir_for_restore(&root.path().join("not-data")).is_err());
        assert!(validate_data_dir_for_restore(&data_dir).is_ok());

        let plain = backup_dir.join("backup_plain.zip");
        write_zip_entries(&plain, &[("data/plain.txt", b"plain")])?;
        let encrypted = encrypt_backup_file(&plain, "local-backup-secret")?;
        assert!(restore_data_dir_from_backup(&encrypted, &backup_dir, &data_dir, None).is_err());
        restore_data_dir_from_backup(
            &encrypted,
            &backup_dir,
            &data_dir,
            Some("local-backup-secret"),
        )?;
        assert_eq!(fs::read(data_dir.join("plain.txt"))?, b"plain");
        Ok(())
    }
}
