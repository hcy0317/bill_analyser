// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn create_backup_file(
    data_dir: &Path,
    backup_dir: &Path,
    encryption_key: Option<&str>,
) -> FileRouteResult<Option<PathBuf>> {
    if !data_dir.exists() {
        return Ok(None);
    }
    fs::create_dir_all(backup_dir).map_err(|error| {
        io_context_error(
            error,
            format!("create backup directory {}", backup_dir.display()),
        )
    })?;
    remove_stale_plaintext_backup_temps(backup_dir)?;
    let backup_path = unique_backup_zip_path(backup_dir);
    create_backup_zip(data_dir, &backup_path)?;
    if let Some(secret) = encryption_key.filter(|value| !value.trim().is_empty()) {
        Ok(Some(encrypt_backup_file(&backup_path, secret)?))
    } else {
        Ok(Some(backup_path))
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn unique_backup_zip_path(backup_dir: &Path) -> PathBuf {
    let counter = BACKUP_FILENAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    backup_dir.join(format!(
        "backup_{}_{}.zip",
        Local::now().format("%Y%m%d_%H%M%S_%f"),
        counter
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn create_backup_zip(data_dir: &Path, backup_path: &Path) -> FileRouteResult<()> {
    let data_parent = data_dir
        .parent()
        .ok_or_else(|| BackupFileRuntimeError::internal("data directory has no parent"))?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup_path)
        .map_err(|error| {
            io_context_error(
                error,
                format!("create backup archive {}", backup_path.display()),
            )
        })?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let mut files = WalkDir::new(data_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort();

    for file_path in files {
        let archive_name = relative_zip_name(data_parent, &file_path)?;
        zip.start_file(&archive_name, options).map_err(|error| {
            BackupFileRuntimeError::internal(format!(
                "start backup archive member {archive_name}: {error}"
            ))
        })?;
        let mut source = File::open(&file_path).map_err(|error| {
            io_context_error(error, format!("open backup source {}", file_path.display()))
        })?;
        io::copy(&mut source, &mut zip).map_err(|error| {
            io_context_error(
                error,
                format!("write backup source {}", file_path.display()),
            )
        })?;
    }
    let archive_file = zip.finish().map_err(|error| {
        BackupFileRuntimeError::internal(format!(
            "finish backup archive {}: {error}",
            backup_path.display()
        ))
    })?;
    drop(archive_file);
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn relative_zip_name(base_dir: &Path, file_path: &Path) -> FileRouteResult<String> {
    let relative = file_path
        .strip_prefix(base_dir)
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    let name = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if name.is_empty() {
        Err(BackupFileRuntimeError::internal(
            "empty backup archive path",
        ))
    } else {
        Ok(name)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn encrypt_backup_file(file_path: &Path, secret: &str) -> FileRouteResult<PathBuf> {
    let key = derive_backup_fernet_key(secret)
        .ok_or_else(|| BackupFileRuntimeError::bad_request("备份加密密钥未配置"))?;
    let fernet = Fernet::new(&key)
        .ok_or_else(|| BackupFileRuntimeError::internal("invalid derived backup key"))?;
    let archive_bytes = fs::read(file_path).map_err(|error| {
        io_context_error(
            error,
            format!("read backup archive {}", file_path.display()),
        )
    })?;
    let encrypted = fernet.encrypt(&archive_bytes);
    let encrypted_path = encrypted_storage_path_for_zip(file_path);
    let mut encrypted_file = File::create(&encrypted_path).map_err(|error| {
        io_context_error(
            error,
            format!(
                "create encrypted backup archive {}",
                encrypted_path.display()
            ),
        )
    })?;
    encrypted_file
        .write_all(encrypted.as_bytes())
        .map_err(|error| {
            io_context_error(
                error,
                format!(
                    "write encrypted backup archive {}",
                    encrypted_path.display()
                ),
            )
        })?;
    encrypted_file.flush().map_err(|error| {
        io_context_error(
            error,
            format!(
                "flush encrypted backup archive {}",
                encrypted_path.display()
            ),
        )
    })?;
    drop(encrypted_file);
    if let Err(error) = fs::remove_file(file_path) {
        let _ = fs::remove_file(&encrypted_path);
        return Err(io_context_error(
            error,
            format!("remove plaintext backup {}", file_path.display()),
        ));
    }
    Ok(encrypted_path)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn build_runtime_backup_info(
    file_path: &Path,
    encryption_key: Option<&str>,
) -> FileRouteResult<BackupFileInfoContract> {
    let stat = fs::metadata(file_path)?;
    let filename = public_backup_filename(file_path);
    let checksum = calculate_file_checksum(file_path)?;
    let mut metadata = read_backup_metadata(file_path);
    let metadata_checksum = metadata
        .get("checksum")
        .and_then(Value::as_str)
        .map(str::to_string);
    let archive_summary = archive_summary_for_backup_file(file_path, encryption_key);

    if metadata
        .get("size")
        .and_then(Value::as_u64)
        .is_none_or(|size| size != stat.len())
    {
        metadata = build_backup_metadata(file_path, &checksum, &archive_summary)?;
    }

    Ok(build_backup_file_info(BackupFileInfoInput {
        filename,
        size: stat.len(),
        created_at: system_time_iso(stat.modified().unwrap_or_else(|_| SystemTime::now())),
        path: public_backup_reference(file_path),
        checksum,
        metadata_checksum: metadata
            .get("checksum")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(metadata_checksum),
        archive_summary,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn public_backup_reference(file_path: &Path) -> String {
    format!("backup/{}", public_backup_filename(file_path))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn archive_summary_for_backup_file(
    file_path: &Path,
    encryption_key: Option<&str>,
) -> bill_analyser_core::BackupArchiveSummary {
    if is_encrypted_backup_storage_path(file_path) {
        let Some(secret) = encryption_key.filter(|value| !value.trim().is_empty()) else {
            return invalid_backup_archive_summary("backup encryption key is not configured");
        };
        return match decrypted_backup_bytes(file_path, secret) {
            Ok(bytes) => inspect_zip_reader(io::Cursor::new(bytes)),
            Err(error) => invalid_backup_archive_summary(&error.message),
        };
    }
    match File::open(file_path) {
        Ok(file) => inspect_zip_reader(file),
        Err(error) => invalid_backup_archive_summary(&error.to_string()),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn decrypted_backup_bytes(file_path: &Path, secret: &str) -> FileRouteResult<Vec<u8>> {
    let key = derive_backup_fernet_key(secret)
        .ok_or_else(|| BackupFileRuntimeError::bad_request("备份加密密钥未配置"))?;
    let fernet = Fernet::new(&key)
        .ok_or_else(|| BackupFileRuntimeError::internal("invalid derived backup key"))?;
    let token = String::from_utf8(fs::read(file_path)?)
        .map_err(|_| BackupFileRuntimeError::bad_request("备份解密失败"))?;
    fernet
        .decrypt(&token)
        .map_err(|_| BackupFileRuntimeError::bad_request("备份解密失败"))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn inspect_zip_reader<R: Read + Seek>(
    reader: R,
) -> bill_analyser_core::BackupArchiveSummary {
    let mut archive = match ZipArchive::new(reader) {
        Ok(archive) => archive,
        Err(error) => return invalid_backup_archive_summary(&error.to_string()),
    };
    let mut names = Vec::new();
    for index in 0..archive.len() {
        match archive.by_index(index) {
            Ok(file) => names.push(file.name().to_string()),
            Err(error) => return invalid_backup_archive_summary(&error.to_string()),
        }
    }
    backup_archive_summary_from_entries(names)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn build_backup_metadata(
    file_path: &Path,
    checksum: &str,
    archive_summary: &bill_analyser_core::BackupArchiveSummary,
) -> FileRouteResult<Value> {
    let stat = fs::metadata(file_path)?;
    let metadata = json!({
        "filename": backup_filename(file_path),
        "checksum": checksum,
        "size": stat.len(),
        "created_at": system_time_iso(stat.modified().unwrap_or_else(|_| SystemTime::now())),
        "valid_zip": archive_summary.valid_zip,
        "contains_data_dir": archive_summary.contains_data_dir,
        "entry_count": archive_summary.entry_count,
        "top_level_entries": archive_summary.top_level_entries,
    });
    Ok(metadata)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn read_backup_metadata(file_path: &Path) -> Value {
    fs::read_to_string(backup_metadata_path(file_path))
        .ok()
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .unwrap_or_else(|| Value::Object(Map::new()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn backup_metadata_path(file_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta.json", file_path.to_string_lossy()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn calculate_file_checksum(file_path: &Path) -> FileRouteResult<String> {
    let mut file = File::open(file_path).map_err(|error| {
        io_context_error(
            error,
            format!("open backup checksum source {}", file_path.display()),
        )
    })?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn io_context_error(error: io::Error, context: impl ToString) -> BackupFileRuntimeError {
    BackupFileRuntimeError::internal(format!("{}: {error}", context.to_string()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn upsert_backup_record_from_info(
    runtime: &BackupOpsRuntime,
    backup_info: &BackupFileInfoContract,
) -> Result<i64, DbError> {
    upsert_backup_record_for_runtime(
        runtime,
        BackupRecordDraft {
            backup_name: backup_info.filename.clone(),
            file_path: backup_info.path.clone(),
            checksum: backup_info.checksum.clone(),
            encrypted: backup_info.encrypted,
            status: "created".to_string(),
            metadata: json!({
                "valid_zip": backup_info.valid_zip,
                "contains_data_dir": backup_info.contains_data_dir,
                "entry_count": backup_info.entry_count,
                "top_level_entries": backup_info.top_level_entries,
            }),
        },
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn backup_info_with_record(
    info: BackupFileInfoContract,
    record: Option<&BackupRecordRow>,
) -> Value {
    let mut value = serde_json::to_value(info).unwrap_or(Value::Null);
    if let (Value::Object(object), Some(record)) = (&mut value, record) {
        object.insert("recordId".to_string(), json!(record.id));
        object.insert("recordStatus".to_string(), json!(record.status));
        object.insert("recordCreatedAt".to_string(), json!(record.created_at));
    }
    value
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn list_local_backup_files(backup_dir: &Path) -> FileRouteResult<Vec<PathBuf>> {
    fs::create_dir_all(backup_dir)?;
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = backup_filename(&path);
        if filename.starts_with("backup_")
            && (filename.ends_with(".zip") || filename.ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX))
        {
            files.insert(filename, path);
        }
    }
    Ok(files.into_values().collect())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn remove_stale_plaintext_backup_temps(backup_dir: &Path) -> FileRouteResult<()> {
    if !backup_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        let filename = physical_backup_filename(&path);
        if path.is_file()
            && filename.starts_with("bill-analyser-backup-")
            && filename.ends_with(".zip")
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn remove_metadata_file(file_path: &Path) -> FileRouteResult<()> {
    let metadata_path = backup_metadata_path(file_path);
    if metadata_path.exists() {
        fs::remove_file(metadata_path)?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn resolve_backup_path(backup_dir: &Path, filename: &str) -> FileRouteResult<PathBuf> {
    let resolution = resolve_backup_filename(filename).map_err(|error| {
        BackupFileRuntimeError::new(status_or_internal(error.status_code), error.message)
    })?;
    let backup_dir = backup_dir
        .canonicalize()
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    let path = backup_dir.join(&resolution.sanitized);
    let path = if path.exists() {
        path
    } else if resolution
        .sanitized
        .ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX)
    {
        let storage_path =
            backup_dir.join(encrypted_storage_filename_for_public(&resolution.sanitized));
        if storage_path.exists() {
            storage_path
        } else {
            path
        }
    } else {
        path
    };
    if path.exists() {
        let canonical = path
            .canonicalize()
            .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        if !canonical.starts_with(&backup_dir) {
            return Err(BackupFileRuntimeError::bad_request("无效的文件名"));
        }
        Ok(canonical)
    } else {
        Ok(path)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn backup_dir(config: &HttpShellConfig) -> RouteResult<PathBuf> {
    let backup_dir = PathBuf::from(&config.backup_dir);
    fs::create_dir_all(&backup_dir)
        .map_err(|error| Box::new(error_response(StatusCode::INTERNAL_SERVER_ERROR, error)))?;
    Ok(backup_dir)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn data_dir(config: &HttpShellConfig) -> PathBuf {
    PathBuf::from(&config.data_dir)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn backup_filename(path: &Path) -> String {
    public_backup_filename(path)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn public_backup_filename(path: &Path) -> String {
    let filename = physical_backup_filename(path);
    if filename.ends_with(RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX) {
        return format!(
            "{}{}",
            filename.trim_end_matches(RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX),
            PUBLIC_ENCRYPTED_BACKUP_SUFFIX
        );
    }
    filename
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn physical_backup_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn is_encrypted_backup_storage_path(path: &Path) -> bool {
    let filename = physical_backup_filename(path);
    filename.ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX)
        || filename.ends_with(RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn encrypted_storage_path_for_zip(file_path: &Path) -> PathBuf {
    let raw = file_path.to_string_lossy();
    PathBuf::from(format!(
        "{}{}",
        raw.trim_end_matches(".zip"),
        RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn encrypted_storage_filename_for_public(filename: &str) -> String {
    format!(
        "{}{}",
        filename.trim_end_matches(PUBLIC_ENCRYPTED_BACKUP_SUFFIX),
        RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn file_modified_at(path: &Path) -> FileRouteResult<i64> {
    Ok(path
        .metadata()?
        .modified()
        .unwrap_or_else(|_| SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn system_time_iso(value: SystemTime) -> String {
    let datetime: chrono::DateTime<Local> = value.into();
    datetime.format("%Y-%m-%dT%H:%M:%S%.f").to_string()
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn now_iso() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}
