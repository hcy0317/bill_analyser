fn open_postgres_runtime(
    state: &HttpAppState,
    label: &'static str,
) -> RouteResult<PostgresRepositoryRuntime> {
    state
        .open_postgres_repository_runtime(label)
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message(),
            ))
        })
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(message_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn required_date(value: Option<&str>, name: &str) -> RouteResult<NaiveDate> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(Box::new(message_response(
            StatusCode::BAD_REQUEST,
            "start_date and end_date are required",
        )));
    };
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        Box::new(message_response(
            StatusCode::BAD_REQUEST,
            format!("{name} must use YYYY-MM-DD format"),
        ))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_limit(value: Option<&str>) -> RouteResult<usize> {
    let limit = match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.parse::<usize>().map_err(|error| {
            Box::new(message_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            ))
        })?,
        None => 200,
    };
    Ok(limit.min(1000))
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_offset(value: Option<&str>) -> RouteResult<usize> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.parse::<usize>().map_err(|error| {
            Box::new(message_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            ))
        }),
        None => Ok(0),
    }
}
