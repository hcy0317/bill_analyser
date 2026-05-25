// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn list_backup_files_response(
    state: &HttpAppState,
    headers: &HeaderMap,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let mut infos = Vec::new();
    for file_path in list_local_backup_files(&backup_dir).map_err(file_error_response)? {
        let info =
            build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
                .map_err(file_error_response)?;
        infos.push(info);
    }

    let records = list_backup_records(auth_runtime.runtime.connection())
        .map_err(|_| Box::new(db_error_response()))?;
    let record_map = records
        .into_iter()
        .map(|record| (record.backup_name.clone(), record))
        .collect::<BTreeMap<_, _>>();

    let mut payload = infos
        .into_iter()
        .map(|info| {
            let filename = info.filename.clone();
            backup_info_with_record(info, record_map.get(&filename))
        })
        .collect::<Vec<_>>();
    payload.sort_by(|left, right| {
        json_string_field(right, "created_at").cmp(&json_string_field(left, "created_at"))
    });

    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": payload }),
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn create_backup_response(
    state: &HttpAppState,
    headers: &HeaderMap,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
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
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                user_id,
                headers,
                "backup_created",
                json!({"filename": "", "path": ""}),
                0,
                "failed",
                Some("数据未变化，无需备份".to_string()),
            );
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "数据未变化，无需备份",
            ));
        }
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                user_id,
                headers,
                "backup_created",
                json!({"filename": "", "path": ""}),
                0,
                "failed",
                Some(error.message.clone()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };

    let backup_info =
        build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
            .map_err(|error| {
                write_backup_audit_event(
                    auth_runtime.runtime.connection(),
                    user_id,
                    headers,
                    "backup_created",
                    json!({"filename": backup_filename(&file_path), "path": public_backup_reference(&file_path)}),
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
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            user_id,
            headers,
            "backup_created",
            json!({"filename": backup_info.filename, "path": backup_info.path}),
            0,
            "failed",
            Some(error_message.clone()),
        );
        return Ok(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            error_message,
        ));
    }

    upsert_backup_record_from_info(auth_runtime.runtime.connection(), &backup_info)
        .map_err(|_| Box::new(db_error_response()))?;
    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        user_id,
        headers,
        "backup_created",
        json!({
            "filename": backup_info.filename,
            "path": backup_info.path,
            "checksum": backup_info.checksum,
        }),
        1,
        "success",
        None,
    );

    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": backup_info }),
    ))
}
