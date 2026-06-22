#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportV2RouteResponse {
    pub status_code: u16,
    pub body: Value,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_v2_error_response(status_code: u16, error: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_v2_message_response(
    status_code: u16,
    success: bool,
    message: &str,
) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": success, "message": message}),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_v2_data_response<T>(data: T) -> ImportV2RouteResponse
where
    T: Serialize,
{
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_stage_parse_success(data: ImportStageParseData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_stage_parse_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_stage_dedup_success(data: ImportStageDedupData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_stage_dedup_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_stage_confirm_success(data: ImportStageConfirmData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_stage_confirm_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_preview_page_success(data: ImportPreviewPageData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_preview_page_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_preview_index_success(data: ImportPreviewIndexData) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_preview_index_success",
        "business operation entered"
    );
    import_v2_data_response(data)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_session_success(session: ImportSessionSummary) -> ImportV2RouteResponse {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_session_success",
        "business operation entered"
    );
    import_v2_data_response(session)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_session_not_found_response() -> ImportV2RouteResponse {
    import_v2_error_response(404, "Session not found or expired")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_session_cancel_missing_response() -> ImportV2RouteResponse {
    import_v2_message_response(200, false, "Session not found")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_session_cancel_success_response() -> ImportV2RouteResponse {
    import_v2_message_response(200, true, "Session cleared")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_v2_missing_session_id_response() -> ImportV2RouteResponse {
    import_v2_error_response(400, "Missing session_id")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_v2_invalid_request_response() -> ImportV2RouteResponse {
    import_v2_error_response(400, "Invalid request")
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn expected_preview_state_is_valid(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Object(_)))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn preview_state_conflict_response() -> ImportV2RouteResponse {
    import_v2_error_response(409, "Preview state changed, please refresh")
}
