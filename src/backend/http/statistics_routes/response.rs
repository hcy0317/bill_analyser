use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bill_analyser_core::{statistics::StatisticsContractError, UserId};
use bill_analyser_db::{SqliteConnectionConfig, SqliteDbPath, SqliteRuntime};
use serde_json::{json, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

pub(super) const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

pub(super) type RouteResult<T> = Result<T, Box<Response>>;
pub(super) fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust statistics DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
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

pub(super) fn user_id_from_headers(
    headers: &HeaderMap,
    config: &HttpShellConfig,
) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

pub(super) fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

pub(super) fn success_data(status: StatusCode, data: Value) -> Response {
    json_response(status, json!({ "success": true, "data": data }))
}

pub(super) fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

pub(super) fn internal_error(message: impl ToString) -> Response {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, message)
}

pub(super) fn insights_error(message: impl ToString) -> Response {
    json_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "success": false, "message": message.to_string() }),
    )
}

pub(super) fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust statistics route runtime DB error",
    )
}

pub(super) fn statistics_error_response(
    error: StatisticsContractError,
    year_month: bool,
) -> Response {
    if year_month && error.error == "Invalid year-month format" {
        return bad_request(error.message);
    }
    json_response(
        StatusCode::BAD_REQUEST,
        json!({
            "success": false,
            "error": error.error,
            "message": error.message
        }),
    )
}

pub(super) fn asset_trends_error_response(error: StatisticsContractError) -> Response {
    if error.error == "资产趋势查询最多支持365天范围，请缩小时间范围" {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "success": false,
                "errorMessage": error.error,
                "error": error.error,
                "errorCode": 400
            }),
        );
    }
    statistics_error_response(error, false)
}

pub(super) fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

pub(super) fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

pub(super) fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}
