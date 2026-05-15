use bill_analyser_core::ops::{
    backup_archive_summary_from_entries, backup_encryption_secret_configured,
    backup_restore_verify_response, build_backup_file_info, build_cloud_backup_object_key,
    build_sync_config_contract, build_user_data_audit_contract, derive_backup_fernet_key,
    encryption_status_response, invalid_backup_archive_summary, is_safe_backup_archive_member,
    normalize_backup_job_payload, normalize_backup_sync_prefix, normalize_report_export_format,
    normalize_sqlcipher_status, normalize_sync_provider, normalize_user_data_statistics,
    parse_comma_separated_ints, plan_backup_cleanup, resolve_backup_filename,
    resolve_sensitive_auth_mode, secure_backup_filename, secure_report_filename,
    user_data_statistics_response, BackupFileCandidate, BackupFileInfoInput, BackupRecordContract,
    SensitiveAuthMode, UserDataClearKind, BACKUP_DEFAULT_RETENTION_COUNT,
    BACKUP_DEFAULT_RETENTION_DAYS,
};
use serde_json::json;

#[test]
fn backup_filename_archive_and_file_info_contracts_preserve_restore_safety_shape() {
    assert_eq!(
        secure_backup_filename("..\\backup_20260505_120000.zip.enc"),
        "backup_20260505_120000.zip.enc"
    );
    for unsafe_filename in [
        "../backup_20260505.zip",
        "..\\backup_20260505.zip",
        "backup_20260505.zip/..",
        "backup_20260505.zip:ads",
        "C:/backup_20260505.zip",
        "backup_20260505.zip\u{0007}",
    ] {
        assert!(resolve_backup_filename(unsafe_filename).is_err());
    }
    assert!(resolve_backup_filename("backup_20260505.zip/evil").is_err());
    assert!(resolve_backup_filename("not_a_backup.zip").is_err());
    assert!(resolve_backup_filename("backup_20260505.tar").is_err());

    let encrypted =
        resolve_backup_filename(" backup_20260505.zip.enc ").expect("safe encrypted backup");
    assert_eq!(encrypted.sanitized, "backup_20260505.zip.enc");
    assert!(encrypted.encrypted);

    let summary = backup_archive_summary_from_entries([
        "data/bills.db",
        "config/server_config.json",
        "data/config/settings.json",
    ]);
    assert!(summary.valid_zip);
    assert!(summary.contains_data_dir);
    assert_eq!(summary.entry_count, 3);
    assert_eq!(summary.top_level_entries, vec!["config", "data"]);
    assert!(summary.ready_to_restore);

    let flat_summary = backup_archive_summary_from_entries(["config/server_config.json"]);
    assert!(flat_summary.valid_zip);
    assert!(!flat_summary.contains_data_dir);
    assert!(!flat_summary.ready_to_restore);

    let data_dir_only_summary = backup_archive_summary_from_entries(["data/"]);
    assert!(data_dir_only_summary.valid_zip);
    assert!(data_dir_only_summary.contains_data_dir);
    assert!(!data_dir_only_summary.ready_to_restore);

    let empty_summary = backup_archive_summary_from_entries(Vec::<&str>::new());
    assert!(empty_summary.valid_zip);
    assert_eq!(empty_summary.entry_count, 0);
    assert!(!empty_summary.ready_to_restore);

    let invalid = invalid_backup_archive_summary("not a zip");
    assert!(!invalid.valid_zip);
    assert!(!invalid.ready_to_restore);
    assert_eq!(invalid.error, "not a zip");

    assert!(is_safe_backup_archive_member("data/bills.db"));
    assert!(!is_safe_backup_archive_member("../escape.txt"));
    assert!(!is_safe_backup_archive_member("data/../escape.txt"));
    assert!(!is_safe_backup_archive_member("C:/escape.txt"));
    assert!(!is_safe_backup_archive_member("/absolute/escape.txt"));

    let unsafe_summary = backup_archive_summary_from_entries(["data/bills.db", "..\\escape.txt"]);
    assert!(!unsafe_summary.valid_zip);
    assert!(!unsafe_summary.ready_to_restore);
    assert!(unsafe_summary.error.contains("备份文件包含不安全路径"));

    let info = build_backup_file_info(BackupFileInfoInput {
        filename: "backup_20260505.zip.enc".to_string(),
        size: 2048,
        created_at: "2026-05-05T12:00:00".to_string(),
        path: "C:/safe/backups/backup_20260505.zip.enc".to_string(),
        checksum: "sha256-current".to_string(),
        metadata_checksum: Some("sha256-current".to_string()),
        archive_summary: summary,
    });
    assert!(info.encrypted);
    assert!(info.valid_zip);
    assert!(info.metadata_checksum_matched);
    assert_eq!(info.error, "");
    assert_eq!(
        backup_restore_verify_response(&info)["success"],
        json!(true)
    );

    let empty_info = build_backup_file_info(BackupFileInfoInput {
        filename: "backup_empty.zip".to_string(),
        size: 22,
        created_at: "2026-05-05T12:00:00".to_string(),
        path: "C:/safe/backups/backup_empty.zip".to_string(),
        checksum: "sha256-empty".to_string(),
        metadata_checksum: Some("sha256-empty".to_string()),
        archive_summary: empty_summary,
    });
    assert_eq!(
        backup_restore_verify_response(&empty_info)["success"],
        json!(false)
    );
}

#[test]
fn backup_cleanup_plan_matches_record_first_retention_semantics() {
    let records = vec![
        BackupRecordContract {
            id: 1,
            backup_name: "backup_old.zip".to_string(),
            storage_type: "local".to_string(),
            status: "created".to_string(),
            created_at: "2026-05-01T00:00:00".to_string(),
            file_exists: true,
        },
        BackupRecordContract {
            id: 2,
            backup_name: "backup_new.zip.enc".to_string(),
            storage_type: "local".to_string(),
            status: "created".to_string(),
            created_at: "2026-05-02T00:00:00".to_string(),
            file_exists: true,
        },
        BackupRecordContract {
            id: 3,
            backup_name: "backup_missing.zip".to_string(),
            storage_type: "local".to_string(),
            status: "created".to_string(),
            created_at: "2026-05-03T00:00:00".to_string(),
            file_exists: false,
        },
        BackupRecordContract {
            id: 4,
            backup_name: "backup_remote.zip".to_string(),
            storage_type: "s3".to_string(),
            status: "created".to_string(),
            created_at: "2026-05-04T00:00:00".to_string(),
            file_exists: true,
        },
        BackupRecordContract {
            id: 5,
            backup_name: "backup_deleted.zip".to_string(),
            storage_type: "local".to_string(),
            status: "deleted".to_string(),
            created_at: "2026-05-05T00:00:00".to_string(),
            file_exists: true,
        },
    ];
    let stray_files = vec![
        BackupFileCandidate {
            filename: "backup_stray_new.zip.enc".to_string(),
            modified_at: 200,
        },
        BackupFileCandidate {
            filename: "backup_stray_old.zip".to_string(),
            modified_at: 100,
        },
    ];

    let plan = plan_backup_cleanup(&records, &stray_files, 1);
    assert_eq!(plan.deleted_count, 3);
    assert_eq!(plan.kept_count, 1);
    assert_eq!(
        plan.decisions
            .iter()
            .map(|item| (
                item.filename.as_str(),
                item.action.as_str(),
                item.reason.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("backup_missing.zip", "mark_deleted", "missing_file"),
            (
                "backup_old.zip",
                "delete_file_and_mark_deleted",
                "retention_cleanup"
            ),
            (
                "backup_stray_new.zip.enc",
                "delete_stray_file",
                "retention_cleanup"
            ),
            (
                "backup_stray_old.zip",
                "delete_stray_file",
                "retention_cleanup"
            ),
        ]
    );
}

#[test]
fn backup_job_encryption_and_sqlcipher_contracts_match_python_defaults() {
    let job = normalize_backup_job_payload(&json!({
        "id": "7",
        "job_type": " daily ",
        "schedule_expr": " 0 2 * * * ",
        "retention_days": "45",
        "retention_count": "6",
        "enabled": false,
        "last_status": 200,
    }))
    .expect("valid backup job");
    assert_eq!(job.id, Some(7));
    assert_eq!(job.job_type, "daily");
    assert_eq!(job.schedule_expr, "0 2 * * *");
    assert_eq!(job.retention_days, 45);
    assert_eq!(job.retention_count, 6);
    assert!(!job.enabled);
    assert_eq!(job.last_status.as_deref(), Some("200"));

    let defaulted =
        normalize_backup_job_payload(&json!({"job_type": "manual"})).expect("defaulted job");
    assert_eq!(defaulted.retention_days, BACKUP_DEFAULT_RETENTION_DAYS);
    assert_eq!(defaulted.retention_count, BACKUP_DEFAULT_RETENTION_COUNT);
    assert!(defaulted.enabled);
    assert_eq!(
        normalize_backup_job_payload(&json!({}))
            .expect_err("missing job_type")
            .error,
        "job_type is required"
    );
    assert_eq!(
        normalize_backup_job_payload(&json!({"job_type": "daily", "retention_days": "bad"}))
            .expect_err("bad retention_days")
            .error,
        "retention_days must be an integer"
    );
    assert_eq!(
        normalize_backup_job_payload(&json!({"job_type": "daily", "retention_count": -1}))
            .expect_err("negative retention_count")
            .error,
        "retention_count must be greater than or equal to 0"
    );
    assert_eq!(
        normalize_backup_job_payload(&json!({"job_type": "daily", "retention_days": -1}))
            .expect_err("negative retention_days")
            .error,
        "retention_days must be between 0 and 3650"
    );
    assert_eq!(
        normalize_backup_job_payload(&json!({"job_type": "daily", "retention_count": 1001}))
            .expect_err("large retention_count")
            .error,
        "retention_count must be less than or equal to 1000"
    );
    assert_eq!(
        normalize_backup_job_payload(&json!({"job_type": "daily job"}))
            .expect_err("bad job_type")
            .error,
        "job_type is invalid"
    );
    assert_eq!(
        normalize_backup_job_payload(&json!({"job_type": "daily", "schedule_expr": "bad\ncron"}))
            .expect_err("bad schedule_expr")
            .error,
        "schedule_expr is invalid"
    );
    for invalid_id in [json!(0), json!(-1), json!("0"), json!("bad"), json!(1.5)] {
        assert_eq!(
            normalize_backup_job_payload(&json!({"job_type": "daily", "id": invalid_id}))
                .expect_err("invalid id")
                .error,
            "id must be a positive integer"
        );
    }
    let enabled_default = normalize_backup_job_payload(&json!({
        "job_type": "daily",
        "retention_count": null,
        "enabled": null,
    }))
    .expect("null fields default");
    assert_eq!(
        enabled_default.retention_count,
        BACKUP_DEFAULT_RETENTION_COUNT
    );
    assert!(enabled_default.enabled);

    assert!(!backup_encryption_secret_configured(None));
    assert!(!backup_encryption_secret_configured(Some("   ")));
    assert_eq!(
        derive_backup_fernet_key("secret").as_deref(),
        Some("K7gNU3sdo-OL0wNhqoVWhr3g6s1xYv72ol_pe_Unols=")
    );
    assert_eq!(derive_backup_fernet_key("   "), None);

    let encrypted = normalize_sqlcipher_status(Some("true"), Some("db-key"), true);
    assert!(encrypted.encrypted);
    assert!(encrypted.sqlcipher_available);
    assert_eq!(encrypted.kdf_iter, 256_000);
    assert_eq!(encrypted.cipher_page_size, 4096);
    assert_eq!(
        encryption_status_response(&encrypted)["data"]["encrypted"],
        true
    );
    assert!(!normalize_sqlcipher_status(Some("1"), Some("db-key"), false).encrypted);
    assert!(!normalize_sqlcipher_status(Some("yes"), Some(""), true).encrypted);
}

#[test]
fn user_data_statistics_export_and_audit_contracts_preserve_sensitive_auth_shape() {
    assert_eq!(parse_comma_separated_ints("1, bad, 2, ,3"), vec![1, 2, 3]);
    assert_eq!(
        resolve_sensitive_auth_mode(false, false)
            .expect_err("missing credentials")
            .message,
        "Current password or stepUpToken is required"
    );
    assert_eq!(
        resolve_sensitive_auth_mode(true, false).expect("current password"),
        SensitiveAuthMode::CurrentPassword
    );
    assert_eq!(
        resolve_sensitive_auth_mode(true, true).expect("step-up wins"),
        SensitiveAuthMode::StepUpToken
    );

    let transaction_audit = build_user_data_audit_contract(
        UserDataClearKind::Transactions,
        42,
        SensitiveAuthMode::StepUpToken,
        &json!({"success": true, "deleted_count": 9}),
    );
    assert_eq!(transaction_audit.operation_type, "clear_transactions");
    assert_eq!(transaction_audit.operation_target, "user_data");
    assert_eq!(transaction_audit.target_id, 42);
    assert_eq!(transaction_audit.affected_count, 9);
    assert_eq!(transaction_audit.status, "success");
    assert_eq!(transaction_audit.details["auth_mode"], "step_up_token");
    assert_eq!(transaction_audit.details["deleted_count"], 9);

    let all_data_audit = build_user_data_audit_contract(
        UserDataClearKind::All,
        42,
        SensitiveAuthMode::CurrentPassword,
        &json!({
            "success": false,
            "counts": {"bills": 3, "tags": 2},
            "message": "clear failed"
        }),
    );
    assert_eq!(all_data_audit.operation_type, "clear_all_user_data");
    assert_eq!(all_data_audit.status, "failed");
    assert_eq!(
        all_data_audit.error_message.as_deref(),
        Some("clear failed")
    );
    assert_eq!(all_data_audit.details["auth_mode"], "current_password");
    assert_eq!(all_data_audit.details["bills"], 3);

    let statistics = normalize_user_data_statistics(&json!({
        "bills": "11",
        "account_count": 4,
        "categoryCount": 5,
        "tags": 6,
        "template_count": "7",
    }));
    assert_eq!(statistics.bill_count, 11);
    assert_eq!(statistics.account_count, 4);
    assert_eq!(statistics.category_count, 5);
    assert_eq!(statistics.tag_count, 6);
    assert_eq!(statistics.template_count, 7);
    assert_eq!(
        user_data_statistics_response(&statistics)["result"]["billCount"],
        11
    );
}

#[test]
fn sync_config_secret_redaction_and_report_format_contracts_are_stable() {
    assert_eq!(normalize_sync_provider(" S3 ").as_deref(), Some("s3"));
    assert_eq!(normalize_sync_provider("dropbox"), None);
    assert_eq!(
        build_cloud_backup_object_key(Some("prefix/"), "backup_20260505.zip").expect("safe prefix"),
        "prefix/backup_20260505.zip"
    );
    assert_eq!(
        build_cloud_backup_object_key(Some("   "), "backup_20260505.zip").expect("default prefix"),
        "bill_analyser_backups/backup_20260505.zip"
    );
    for unsafe_filename in [
        "../backup_20260505.zip",
        "nested/backup_20260505.zip",
        "nested\\backup_20260505.zip",
        "backup_20260505.zip:ads",
        "backup_20260505?.zip",
        "backup_20260505.zip\u{0007}",
        "not_a_backup.zip",
    ] {
        assert!(build_cloud_backup_object_key(Some("prefix/"), unsafe_filename).is_err());
    }
    assert_eq!(
        normalize_backup_sync_prefix(Some("nested/path")).expect("normalized prefix"),
        "nested/path/"
    );
    for unsafe_prefix in [
        "/absolute",
        "C:/absolute",
        "nested\\path",
        "safe/../escape",
        "bad\u{0007}prefix",
    ] {
        assert!(normalize_backup_sync_prefix(Some(unsafe_prefix)).is_err());
        assert!(build_cloud_backup_object_key(Some(unsafe_prefix), "backup.zip").is_err());
    }

    let sync = build_sync_config_contract(
        &json!({
            "provider": "S3",
            "endpoint": "https://s3.example.test",
            "bucket": "backup-bucket",
            "prefix": "secure/",
            "access_key": "visible-key-is-redacted",
            "nested": {
                "secret_key": "secret-is-redacted",
                "safe_header": "kept"
            },
            "headers": {
                "Authorization": "Bearer hidden"
            }
        }),
        "backup_20260505.zip",
    )
    .expect("safe sync config");
    assert_eq!(sync.provider, "s3");
    assert!(sync.supported);
    assert_eq!(sync.object_key, "secure/backup_20260505.zip");
    assert_eq!(sync.safe_config["access_key"], "********");
    assert_eq!(sync.safe_config["nested"]["secret_key"], "********");
    assert_eq!(sync.safe_config["nested"]["safe_header"], "kept");
    assert_eq!(sync.safe_config["headers"]["Authorization"], "********");

    let pdf = normalize_report_export_format(" PDF ").expect("pdf");
    assert_eq!(pdf.format_type, "pdf");
    assert_eq!(pdf.extension, "pdf");
    assert_eq!(pdf.mimetype, "application/pdf");
    let excel = normalize_report_export_format("xlsx").expect("xlsx alias");
    assert_eq!(excel.format_type, "excel");
    assert_eq!(excel.extension, "xlsx");
    assert_eq!(
        excel.mimetype,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    let html = normalize_report_export_format("html").expect("html");
    assert_eq!(html.mimetype, "text/html");
    assert_eq!(secure_report_filename("../monthly.html"), "monthly.html");
    assert_eq!(secure_report_filename("..\\monthly.xlsx"), "monthly.xlsx");
    assert_eq!(secure_report_filename("evil:ads"), "evil_ads");
    assert_eq!(secure_report_filename("report\u{0007}name"), "report_name");
    assert_eq!(secure_report_filename("CON"), "report");
    assert_eq!(secure_report_filename("LPT1.txt"), "report");
    assert_eq!(secure_report_filename("   "), "report");
    assert_eq!(
        normalize_report_export_format("csv")
            .expect_err("csv unsupported")
            .status_code,
        400
    );
}
