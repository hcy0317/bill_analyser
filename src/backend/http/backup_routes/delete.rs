// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn delete_backup_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    filename: &str,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let file_path = match resolve_backup_path(&backup_dir, filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                &auth_runtime.runtime,
                auth_runtime.user_id,
                headers,
                "backup_deleted",
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
            "backup_deleted",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Ok(error_response(StatusCode::NOT_FOUND, "文件不存在"));
    }

    let result = (|| -> FileRouteResult<()> {
        fs::remove_file(&file_path)?;
        remove_metadata_file(&file_path)?;
        update_backup_record_for_runtime(
            &auth_runtime.runtime,
            &safe_filename,
            Some("deleted"),
            json!({
                "deleted_reason": "manual_delete",
                "deleted_at": now_iso(),
            }),
        )
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        Ok(())
    })();

    if let Err(error) = result {
        write_backup_audit_event(
            &auth_runtime.runtime,
            auth_runtime.user_id,
            headers,
            "backup_deleted",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some(error.message.clone()),
        );
        return Ok(error_response(error.status, error.message));
    }

    write_backup_audit_event(
        &auth_runtime.runtime,
        auth_runtime.user_id,
        headers,
        "backup_deleted",
        json!({"filename": safe_filename}),
        1,
        "success",
        None,
    );
    Ok(json_response(
        StatusCode::OK,
        json!({"success": true, "data": {"filename": safe_filename}}),
    ))
}
