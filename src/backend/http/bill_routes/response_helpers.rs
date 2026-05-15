fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust bills DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        ))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
        ))
    })?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: state.config.timeout,
    })
    .map_err(|_| Box::new(db_error_response()))
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn route_contract_response(response: RouteResponseContract) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    json_response(status, response.body)
}

fn response_error_text(response: Response) -> Option<String> {
    let status = response.status();
    if status == StatusCode::BAD_REQUEST {
        Some("Invalid bill payload".to_string())
    } else {
        None
    }
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn not_found(message: impl ToString) -> Response {
    error_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust bills route runtime DB error",
    )
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod response_helper_tests {
    use super::*;

    #[test]
    fn response_helpers_cover_error_text_and_runtime_edges() {
        assert_eq!(
            response_error_text(bad_request("invalid")),
            Some("Invalid bill payload".to_string())
        );
        assert_eq!(response_error_text(not_found("missing")), None);
        assert_eq!(
            db_error_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            status_or_internal(0),
            StatusCode::INTERNAL_SERVER_ERROR
        );

        let config = HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            std::time::Duration::from_secs(1),
            1024,
            crate::config::ImportRouteMode::ImportDbRuntime,
        )
        .expect("config builds");
        let state = HttpAppState::new(config).expect("state builds");
        let response = match open_runtime(&state) {
            Ok(_) => panic!("runtime should require a sqlite db path"),
            Err(response) => response,
        };
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
