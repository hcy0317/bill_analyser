use super::*;

fn write_zip_entries(path: &Path, entries: &[(&str, &[u8])]) -> FileRouteResult<()> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in entries {
        zip.start_file(name, options)
            .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        zip.write_all(bytes)?;
    }
    zip.finish()
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    Ok(())
}

#[test]
fn backup_source_and_sqlite_snapshot_helpers_cover_branch_edges() -> FileRouteResult<()> {
    let root = tempfile::tempdir()?;
    let backup_dir = root.path().join("backup");
    let data_dir = root.path().join("data");
    fs::create_dir_all(&backup_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::write(data_dir.join("note.txt"), b"note")?;

    assert!(create_backup_file(&root.path().join("missing"), &backup_dir, None, None)?.is_none());

    let borrowed = prepare_backup_source(&data_dir, &backup_dir, None)?;
    assert_eq!(borrowed.data_dir(), data_dir.as_path());

    let outside_db = root.path().join("outside.db");
    fs::write(&outside_db, b"not under data")?;
    let outside = prepare_backup_source(&data_dir, &backup_dir, Some(&outside_db))?;
    assert_eq!(outside.data_dir(), data_dir.as_path());

    let sqlite_path = data_dir.join("bills.db");
    let connection = rusqlite::Connection::open(&sqlite_path)
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    connection
        .execute_batch("CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('ok');")
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    drop(connection);

    let target = root.path().join("snapshot").join("bills.db");
    fs::create_dir_all(target.parent().expect("snapshot parent"))?;
    fs::write(&target, b"replace me")?;
    snapshot_sqlite_database(&sqlite_path, &target)?;
    assert!(target.metadata()?.len() > 0);

    let sidecars = sqlite_sidecar_relative_paths(Path::new("bills.db"));
    assert_eq!(
        sidecars,
        vec![PathBuf::from("bills.db-wal"), PathBuf::from("bills.db-shm")]
    );

    let staged = prepare_backup_source(&data_dir, &backup_dir, Some(&sqlite_path))?;
    assert!(staged.data_dir().join("bills.db").exists());
    assert!(staged.data_dir().join("note.txt").exists());
    Ok(())
}

#[test]
fn archive_metadata_encryption_and_resolution_helpers_cover_edge_branches() -> FileRouteResult<()> {
    let root = tempfile::tempdir()?;
    let backup_dir = root.path().join("backup");
    let data_dir = root.path().join("data");
    fs::create_dir_all(&backup_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::write(data_dir.join("records.json"), b"records")?;

    let backup_path =
        create_backup_file(&data_dir, &backup_dir, None, None)?.expect("plain backup path");
    assert!(backup_path.exists());
    assert!(backup_filename(&backup_path).ends_with(".zip"));
    assert!(public_backup_reference(&backup_path).starts_with("backup/backup_"));
    assert!(archive_summary_for_backup_file(&backup_path, None).valid_zip);

    let info = build_runtime_backup_info(&backup_path, None)?;
    assert!(info.valid_zip);
    assert_eq!(
        build_backup_metadata(
            &backup_path,
            &info.checksum,
            &archive_summary_for_backup_file(&backup_path, None),
        )?["filename"],
        info.filename
    );
    assert_eq!(
        backup_info_with_record(info.clone(), None)["filename"],
        info.filename
    );

    let encrypted_public_name = format!("backup_manual{PUBLIC_ENCRYPTED_BACKUP_SUFFIX}");
    let encrypted_storage_path = backup_dir.join(encrypted_storage_filename_for_public(
        &encrypted_public_name,
    ));
    assert!(is_encrypted_backup_storage_path(&encrypted_storage_path));
    assert_eq!(
        public_backup_filename(&encrypted_storage_path),
        encrypted_public_name
    );
    assert!(
        archive_summary_for_backup_file(&encrypted_storage_path, None)
            .error
            .contains("encryption key")
    );
    let resolved_encrypted = resolve_backup_path(&backup_dir, &encrypted_public_name)?;
    assert_eq!(
        public_backup_filename(&resolved_encrypted),
        encrypted_public_name
    );
    assert!(resolve_backup_path(&backup_dir, "../escape.zip").is_err());

    let bad_zip = backup_dir.join("backup_bad.zip");
    fs::write(&bad_zip, b"not a zip")?;
    assert!(!archive_summary_for_backup_file(&bad_zip, None).valid_zip);
    assert!(!inspect_zip_reader(io::Cursor::new(b"bad".as_slice())).valid_zip);
    assert!(relative_zip_name(&data_dir, &data_dir).is_err());

    let stale = backup_dir.join("bill-analyser-backup-stale.zip");
    fs::write(&stale, b"stale")?;
    remove_stale_plaintext_backup_temps(&backup_dir)?;
    assert!(!stale.exists());

    let local_files = list_local_backup_files(&backup_dir)?;
    assert!(local_files
        .iter()
        .any(|path| public_backup_filename(path) == public_backup_filename(&backup_path)));
    Ok(())
}

#[test]
fn archive_encryption_listing_and_error_helpers_cover_file_edges() -> FileRouteResult<()> {
    let root = tempfile::tempdir()?;
    let blocked_backup_dir = root.path().join("blocked-backup");
    fs::write(&blocked_backup_dir, b"not a directory")?;
    assert!(
        create_backup_file(root.path(), &blocked_backup_dir, None, None)
            .expect_err("blocked backup directory should fail")
            .message
            .contains("create backup directory")
    );

    let missing_stale_dir = root.path().join("missing-stale");
    remove_stale_plaintext_backup_temps(&missing_stale_dir)?;

    let backup_dir = root.path().join("backup");
    fs::create_dir_all(&backup_dir)?;
    fs::create_dir_all(backup_dir.join("backup_directory.zip"))?;

    let plain = backup_dir.join("backup_plain.zip");
    write_zip_entries(&plain, &[("data/plain.txt", b"plain")])?;
    let encrypted = encrypt_backup_file(&plain, "local-backup-secret")?;
    assert!(!plain.exists());
    assert!(encrypted.exists());
    let decrypted = decrypted_backup_bytes(&encrypted, "local-backup-secret")?;
    assert!(inspect_zip_reader(io::Cursor::new(decrypted)).ready_to_restore);
    assert!(
        archive_summary_for_backup_file(&encrypted, Some("local-backup-secret")).ready_to_restore
    );
    assert!(!archive_summary_for_backup_file(&root.path().join("missing.zip"), None).valid_zip);
    assert!(calculate_file_checksum(&root.path().join("missing.zip")).is_err());

    let metadata_path = backup_metadata_path(&encrypted);
    fs::write(&metadata_path, b"{}")?;
    remove_metadata_file(&encrypted)?;
    assert!(!metadata_path.exists());
    assert_eq!(list_local_backup_files(&backup_dir)?.len(), 1);
    Ok(())
}

#[test]
fn create_backup_zip_and_checksum_helpers_report_contextual_errors() -> FileRouteResult<()> {
    let root = tempfile::tempdir()?;
    let data_dir = root.path().join("data");
    fs::create_dir_all(&data_dir)?;
    fs::write(data_dir.join("records.json"), b"records")?;
    let backup_path = root.path().join("backup.zip");
    create_backup_zip(&data_dir, &backup_path)?;
    assert!(calculate_file_checksum(&backup_path)?.len() >= 64);

    let collision =
        create_backup_zip(&data_dir, &backup_path).expect_err("create_new should reject archive");
    assert!(collision.message.contains("create backup archive"));

    let zip_with_error = root.path().join("manual.zip");
    write_zip_entries(&zip_with_error, &[("data/records.json", b"records")])?;
    assert!(archive_summary_for_backup_file(&zip_with_error, None).ready_to_restore);
    Ok(())
}
