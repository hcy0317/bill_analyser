// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn prepare_sync_backup_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<PreparedCloudSync> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_sync_audit(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                json!({}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Err(Box::new(error_response(StatusCode::BAD_REQUEST, message)));
        }
    };
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, Some(&payload))?;
    let config = backup_sync_config_payload(&payload);
    let provisional_contract =
        match build_sync_config_contract(&config, "backup_19700101_000000.zip") {
            Ok(contract) => contract,
            Err(error) => {
                write_backup_sync_audit(
                    auth_runtime.runtime.connection(),
                    auth_runtime.user_id,
                    headers,
                    sync_config_validation_audit_details(&config),
                    0,
                    "failed",
                    Some(error.error.clone()),
                );
                return Err(Box::new(error_response(
                    status_or_internal(error.status_code),
                    error.message,
                )));
            }
        };
    if let Err(error) = validate_sync_upload_config(&config, &provisional_contract) {
        write_backup_sync_audit(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            sync_config_audit_details(&provisional_contract),
            0,
            "failed",
            Some(error.message.clone()),
        );
        return Err(Box::new(error_response(
            status_or_internal(error.status_code),
            error.message,
        )));
    }

    let backup_dir = backup_dir(&state.config)?;
    let data_dir = data_dir(&state.config);
    let sqlite_db_path = state.config.sqlite_db_path.as_deref().map(PathBuf::from);
    let user_id = auth_runtime.user_id;
    drop(auth_runtime);

    let result = create_backup_file(
        &data_dir,
        &backup_dir,
        sqlite_db_path.as_deref(),
        state.config.backup_encryption_key.as_deref(),
    );
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let file_path = match result {
        Ok(Some(path)) => path,
        Ok(None) => {
            write_backup_sync_audit(
                auth_runtime.runtime.connection(),
                user_id,
                headers,
                sync_config_audit_details(&provisional_contract),
                0,
                "failed",
                Some("数据未变化，无需备份".to_string()),
            );
            return Err(Box::new(error_response(
                StatusCode::BAD_REQUEST,
                "数据未变化，无需备份",
            )));
        }
        Err(error) => {
            write_backup_sync_audit(
                auth_runtime.runtime.connection(),
                user_id,
                headers,
                sync_config_audit_details(&provisional_contract),
                0,
                "failed",
                Some(error.message.clone()),
            );
            return Err(Box::new(error_response(error.status, error.message)));
        }
    };

    let backup_info =
        build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
            .map_err(|error| {
                write_backup_sync_audit(
                    auth_runtime.runtime.connection(),
                    user_id,
                    headers,
                    json!({
                        "filename": backup_filename(&file_path),
                        "path": public_backup_reference(&file_path),
                        "sync": sync_config_audit_details(&provisional_contract),
                    }),
                    0,
                    "failed",
                    Some(error.message.clone()),
                );
                Box::new(error_response(error.status, error.message))
            })?;

    if !(backup_info.valid_zip && backup_info.ready_to_restore) {
        let error_message = if backup_info.error.is_empty() {
            "backup archive is not ready to restore".to_string()
        } else {
            backup_info.error.clone()
        };
        write_backup_sync_audit(
            auth_runtime.runtime.connection(),
            user_id,
            headers,
            json!({
                "filename": backup_info.filename,
                "path": backup_info.path,
                "sync": sync_config_audit_details(&provisional_contract),
            }),
            0,
            "failed",
            Some(error_message.clone()),
        );
        return Err(Box::new(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            error_message,
        )));
    }

    let contract = build_sync_config_contract(&config, &backup_info.filename)
        .expect("validated sync config and generated backup filename must remain valid");
    validate_sync_upload_config(&config, &contract)
        .expect("validated sync upload config must remain valid after backup creation");

    upsert_backup_record_from_info(auth_runtime.runtime.connection(), &backup_info)
        .map_err(|_| Box::new(db_error_response()))?;
    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        user_id,
        headers,
        "backup_created",
        json!({
            "filename": backup_info.filename.clone(),
            "path": backup_info.path.clone(),
            "checksum": backup_info.checksum.clone(),
            "sync_provider": contract.provider.clone(),
            "sync_object_key": contract.object_key.clone(),
        }),
        1,
        "success",
        None,
    );

    Ok(PreparedCloudSync {
        config,
        contract,
        file_path,
        backup_info,
        user_id,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn finish_sync_backup_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    prepared: PreparedCloudSync,
    upload_result: Result<CloudBackupUploadResult, CloudBackupUploadError>,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let filename = prepared.backup_info.filename.clone();
    match upload_result {
        Ok(result) => {
            let safe_config = audit_safe_sync_config(&prepared.contract.safe_config);
            update_backup_record_by_filename(
                auth_runtime.runtime.connection(),
                &filename,
                None,
                json!({
                    "sync_provider": result.provider.clone(),
                    "sync_object_key": result.object_key.clone(),
                    "sync_status": "success",
                    "sync_status_code": result.status_code,
                    "sync_safe_config": safe_config.clone(),
                }),
            )
            .map_err(|_| Box::new(db_error_response()))?;
            write_backup_sync_audit(
                auth_runtime.runtime.connection(),
                prepared.user_id,
                headers,
                json!({
                    "filename": filename,
                    "path": prepared.backup_info.path.clone(),
                    "checksum": prepared.backup_info.checksum.clone(),
                    "provider": result.provider.clone(),
                    "object_key": result.object_key.clone(),
                    "status_code": result.status_code,
                    "safe_config": safe_config.clone(),
                }),
                1,
                "success",
                None,
            );
            Ok(json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "filename": prepared.backup_info.filename,
                        "provider": result.provider,
                        "object_key": result.object_key,
                        "status_code": result.status_code,
                        "safe_config": safe_config,
                        "backup": prepared.backup_info,
                    }
                }),
            ))
        }
        Err(error) => {
            let safe_config = audit_safe_sync_config(&prepared.contract.safe_config);
            update_backup_record_by_filename(
                auth_runtime.runtime.connection(),
                &filename,
                None,
                json!({
                    "sync_provider": prepared.contract.provider.clone(),
                    "sync_object_key": prepared.contract.object_key.clone(),
                    "sync_status": "failed",
                    "sync_error": error.message.clone(),
                    "sync_safe_config": safe_config.clone(),
                }),
            )
            .map_err(|_| Box::new(db_error_response()))?;
            write_backup_sync_audit(
                auth_runtime.runtime.connection(),
                prepared.user_id,
                headers,
                json!({
                    "filename": filename,
                    "path": prepared.backup_info.path.clone(),
                    "provider": prepared.contract.provider.clone(),
                    "object_key": prepared.contract.object_key.clone(),
                    "status_code": error.response_status,
                    "safe_config": safe_config.clone(),
                }),
                0,
                "failed",
                Some(error.message.clone()),
            );
            Ok(json_response(
                status_or_internal(error.status_code),
                json!({
                    "success": false,
                    "error": error.message,
                    "data": {
                        "filename": prepared.backup_info.filename,
                        "provider": prepared.contract.provider,
                        "object_key": prepared.contract.object_key,
                        "safe_config": safe_config,
                        "backup": prepared.backup_info,
                    }
                }),
            ))
        }
    }
}
