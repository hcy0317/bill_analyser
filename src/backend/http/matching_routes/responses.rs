fn matching_error_response(error: MatchingRuntimeError) -> Response {
    error_response(status_or_internal(error.status_code()), error.message())
}

fn preview_row_version_conflict_response(
    expected_row_version: i64,
    latest_row: bill_analyser_db::ImportPreviewRow,
) -> Response {
    let actual_row_version = latest_row.version;
    json_response(
        StatusCode::CONFLICT,
        json!({
            "success": false,
            "error": "Preview row changed, please refresh",
            "code": "PREVIEW_ROW_VERSION_CONFLICT",
            "data": {
                "expected_row_version": expected_row_version,
                "actual_row_version": actual_row_version,
                "previewItem": matching_preview_item_value(latest_row),
            },
        }),
    )
}

fn success_data(status: StatusCode, data: Value) -> Response {
    json_response(status, json!({ "success": true, "data": data }))
}

fn not_found(message: impl ToString) -> Response {
    message_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response(error: impl ToString) -> Response {
    message_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn message_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "message": message.to_string() }),
    )
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}
