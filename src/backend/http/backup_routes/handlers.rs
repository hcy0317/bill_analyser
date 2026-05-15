use super::*;

pub(super) async fn list_backup_files_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    blocking_route(move || list_backup_files_response(&state, &headers)).await
}

pub(super) async fn create_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    blocking_route(move || create_backup_response(&state, &headers)).await
}

pub(super) async fn sync_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
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

pub(super) async fn verify_backup_restore_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    blocking_route(move || verify_backup_restore_response(&state, &headers, body)).await
}

pub(super) async fn download_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    match prepare_download_backup_response(&state, &headers, &filename) {
        Ok((file, safe_filename)) => stream_backup_download_response(file, &safe_filename),
        Err(response) => *response,
    }
}

pub(super) async fn delete_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    blocking_route(move || delete_backup_response(&state, &headers, &filename)).await
}

pub(super) async fn restore_backup_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    blocking_route(move || restore_backup_response(&state, &headers, &filename)).await
}

pub(super) async fn cleanup_backups_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    blocking_route(move || cleanup_backups_response(&state, &headers, body)).await
}

pub(super) async fn list_backup_jobs_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    blocking_route(move || list_backup_jobs_response(&state, &headers)).await
}

pub(super) async fn save_backup_job_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    blocking_route(move || save_backup_job_response(&state, &headers, body)).await
}
