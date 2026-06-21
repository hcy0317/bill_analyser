#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_session_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_session_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(session)) => route_response(import_session_success(ImportSessionSummary {
            session_id: session.session_id,
            status: session.status,
            created_at: session.created_at,
            parsed_count: non_negative_usize(session.total_parsed),
            preview_count: non_negative_usize(session.total_preview),
            file_paths: json!([]),
        })),
        Ok(None) => route_response(import_session_not_found_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_session_cancel_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_session_cancel_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => match clear_session_data(runtime.connection_mut(), &session_id, user_id) {
            Ok(_) => route_response(import_session_cancel_success_response()),
            Err(error) => route_response(db_error_response(error)),
        },
        Ok(None) => route_response(import_session_cancel_missing_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}
