// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：生成备份任务列表响应，从运行时仓库读取当前用户的任务配置。
pub(super) fn list_backup_jobs_response(
    state: &HttpAppState,
    headers: &HeaderMap,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let jobs = list_backup_jobs_for_runtime(&auth_runtime.runtime, auth_runtime.user_id)
        .map_err(|_| Box::new(db_error_response()))?;
    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": jobs }),
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：保存备份任务配置，校验用户认证后写入任务仓库并记录审计。
pub(super) fn save_backup_job_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_job_audit(
                &auth_runtime.runtime,
                auth_runtime.user_id,
                headers,
                json!({}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };
    let normalized = match normalize_backup_job_payload(&payload) {
        Ok(job) => job,
        Err(error) => {
            write_backup_job_audit(
                &auth_runtime.runtime,
                auth_runtime.user_id,
                headers,
                validation_audit_details(&payload),
                0,
                "failed",
                Some(error.error.clone()),
            );
            return Ok(error_response(
                status_or_internal(error.status_code),
                error.message,
            ));
        }
    };
    let response_payload = json!({
        "id": normalized.id,
        "job_type": normalized.job_type,
        "schedule_expr": normalized.schedule_expr,
        "retention_days": normalized.retention_days,
        "retention_count": normalized.retention_count,
        "enabled": normalized.enabled,
        "last_status": normalized.last_status,
    });

    let job_id = create_or_update_backup_job_for_runtime(
        &auth_runtime.runtime,
        auth_runtime.user_id,
        BackupJobDraft::from(normalized.clone()),
    )
    .map_err(|error| {
        write_backup_job_audit(
            &auth_runtime.runtime,
            auth_runtime.user_id,
            headers,
            validation_audit_details(&payload),
            0,
            "failed",
            Some(error.to_string()),
        );
        Box::new(db_write_error_response(error))
    })?;

    write_backup_job_audit(
        &auth_runtime.runtime,
        auth_runtime.user_id,
        headers,
        json!({
            "user_id": auth_runtime.user_id.get(),
            "job_id": job_id,
            "job_type": normalized.job_type,
            "retention_days": normalized.retention_days,
            "retention_count": normalized.retention_count,
        }),
        1,
        "success",
        None,
    );

    let mut response_payload = response_payload;
    if let Some(object) = response_payload.as_object_mut() {
        object.insert("id".to_string(), json!(job_id));
    }
    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": response_payload }),
    ))
}
