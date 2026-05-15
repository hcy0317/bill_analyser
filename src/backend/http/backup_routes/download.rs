use super::*;

pub(super) fn prepare_download_backup_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    filename: &str,
) -> RouteResult<(File, String)> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let file_path = match resolve_backup_path(&backup_dir, filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_downloaded",
                json!({"filename": audit_safe_value(&Value::String(filename.to_string()))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Err(Box::new(error_response(error.status, error.message)));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_downloaded",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Err(Box::new(error_response(
            StatusCode::NOT_FOUND,
            "文件不存在",
        )));
    }
    if !file_path.is_file() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_downloaded",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup path is not a file".to_string()),
        );
        return Err(Box::new(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "backup path is not a file",
        )));
    }

    let file = match File::open(&file_path) {
        Ok(file) => file,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_downloaded",
                json!({"filename": safe_filename}),
                0,
                "failed",
                Some(error.to_string()),
            );
            return Err(Box::new(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error,
            )));
        }
    };
    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        "backup_downloaded",
        json!({"filename": safe_filename, "path": public_backup_reference(&file_path)}),
        1,
        "success",
        None,
    );
    Ok((file, safe_filename))
}

pub(super) fn stream_backup_download_response(file: File, safe_filename: &str) -> Response {
    let stream = ReaderStream::new(tokio::fs::File::from_std(file));
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{safe_filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}
