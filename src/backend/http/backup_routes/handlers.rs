// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn list_backup_files_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "list_backup_files_handler",
        "business operation entered"
    );
    blocking_route(move || list_backup_files_response(&state, &headers)).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn create_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "create_backup_handler",
        "business operation entered"
    );
    blocking_route(move || create_backup_response(&state, &headers)).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn sync_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "sync_backup_handler",
        "business operation entered"
    );
    let prepare_state = state.clone();
    let prepare_headers = headers.clone();
    let prepared = match tokio::task::spawn_blocking(move || {
        prepare_sync_backup_response(&prepare_state, &prepare_headers, body)
    })
    .await
    {
        Ok(Ok(prepared)) => prepared,
        Ok(Err(response)) => return *response,
        Err(error) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("backup sync prepare task failed: {error}"),
            );
        }
    };

    let upload_result = upload_backup_to_cloud(
        &prepared.config,
        &prepared.contract,
        &prepared.file_path,
        state.config.timeout,
    )
    .await;
    let finish_state = state.clone();
    let finish_headers = headers.clone();
    match tokio::task::spawn_blocking(move || {
        finish_sync_backup_response(&finish_state, &finish_headers, prepared, upload_result)
    })
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(response)) => *response,
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("backup sync finish task failed: {error}"),
        ),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn download_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "download_backup_handler",
        "business operation entered"
    );
    match tokio::task::spawn_blocking(move || {
        prepare_download_backup_response(&state, &headers, &filename)
    })
    .await
    {
        Ok(Ok((file, safe_filename))) => stream_backup_download_response(file, &safe_filename),
        Ok(Err(response)) => *response,
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("backup download prepare task failed: {error}"),
        ),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn delete_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "delete_backup_handler",
        "business operation entered"
    );
    blocking_route(move || delete_backup_response(&state, &headers, &filename)).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn cleanup_backups_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "cleanup_backups_handler",
        "business operation entered"
    );
    blocking_route(move || cleanup_backups_response(&state, &headers, body)).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn list_backup_jobs_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "list_backup_jobs_handler",
        "business operation entered"
    );
    blocking_route(move || list_backup_jobs_response(&state, &headers)).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn save_backup_job_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "save_backup_job_handler",
        "business operation entered"
    );
    blocking_route(move || save_backup_job_response(&state, &headers, body)).await
}
