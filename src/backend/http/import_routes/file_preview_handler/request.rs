use super::super::*;
use super::payload::import_file_preview_payload;

pub(in crate::import_routes) const IMPORT_FILE_PREVIEW_HARD_MAX_BYTES: usize = 10 * 1024 * 1024;

#[tracing::instrument(level = "debug", skip_all)]
pub(in crate::import_routes) async fn import_file_preview_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let content_type = content_type_from_headers(&headers);
    if !content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        return route_response(import_v2_error_response(
            415,
            "Import file preview requires multipart form data",
        ));
    }
    let form = match parse_multipart_form_data(&content_type, &body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let delimiter_hint = form.text_value(&["delimiter"]);
    let encoding = form
        .text_value(&["fileEncoding", "file_encoding"])
        .unwrap_or_else(|| "auto".to_string());
    let max_preview_bytes = state
        .config
        .body_limit_bytes
        .min(IMPORT_FILE_PREVIEW_HARD_MAX_BYTES);

    let (filename, bytes) = if let Some(temp_path) = form.text_value(&["temp_path", "tempPath"]) {
        let Some(session_id) = form.text_value(&["session_id", "sessionId"]) else {
            return route_response(import_v2_error_response(
                400,
                "session_id is required for temp_path preview",
            ));
        };
        let runtime = match open_runtime(&state) {
            Ok(runtime) => runtime,
            Err(response) => return route_response(response),
        };
        if let Err(response) = init_import_runtime_schema(&runtime) {
            return route_response(response);
        }
        match get_import_session(runtime.connection(), &session_id, user_id) {
            Ok(Some(_)) => {}
            Ok(None) => return route_response(import_session_not_found_response()),
            Err(error) => return route_response(db_error_response(error)),
        }
        let path = match validate_import_temp_path(&temp_path, user_id, Some(&session_id)) {
            Ok(path) => path,
            Err(response) => return route_response(response),
        };
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("import.csv")
            .to_string();
        let bytes = match tokio::task::spawn_blocking(move || {
            read_bounded_preview_file(&path, max_preview_bytes)
        })
        .await
        {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(response)) => return route_response(response),
            Err(_) => {
                return route_response(import_v2_error_response(
                    500,
                    "Unable to execute temp import file read",
                ))
            }
        };
        (filename, bytes)
    } else if let Some(file) = form.first_file_part_named(&["file", "files"]) {
        let Some(filename) = file
            .filename
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return route_response(import_v2_error_response(400, "No file selected"));
        };
        if file.body.len() > max_preview_bytes {
            return route_response(import_v2_error_response(
                413,
                "Import preview file exceeds the size limit",
            ));
        }
        (filename.to_string(), file.body.clone())
    } else {
        return route_response(import_v2_error_response(
            400,
            "No file or temp_path provided",
        ));
    };

    let preview = match tokio::task::spawn_blocking(move || {
        import_file_preview_payload(&filename, &bytes, delimiter_hint.as_deref(), &encoding)
    })
    .await
    {
        Ok(preview) => preview,
        Err(_) => Err(import_v2_error_response(
            500,
            "Unable to execute import file preview",
        )),
    };

    match preview {
        Ok(result) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({ "success": true, "result": result }),
        }),
        Err(response) => route_response(response),
    }
}

pub(in crate::import_routes) fn read_bounded_preview_file(
    path: &FsPath,
    max_bytes: usize,
) -> Result<Vec<u8>, ImportV2RouteResponse> {
    let metadata = fs::metadata(path)
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    if !metadata.is_file() {
        return Err(import_v2_error_response(
            400,
            "Temp import path is not a file",
        ));
    }
    if metadata.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(import_v2_error_response(
            413,
            "Import preview file exceeds the size limit",
        ));
    }
    let file = fs::File::open(path)
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    let read_limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(metadata.len().try_into().unwrap_or(max_bytes));
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| import_v2_error_response(500, "Unable to read temp import file"))?;
    if bytes.len() > max_bytes {
        return Err(import_v2_error_response(
            413,
            "Import preview file exceeds the size limit",
        ));
    }
    Ok(bytes)
}
