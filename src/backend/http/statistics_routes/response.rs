// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bill_analyser_core::{statistics::StatisticsContractError, UserId};
use bill_analyser_db::PostgresRepositoryRuntime;
use serde_json::{json, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

pub(super) const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

pub(super) type RouteResult<T> = Result<T, Box<Response>>;

pub(super) fn open_postgres_runtime(
    state: &HttpAppState,
) -> RouteResult<PostgresRepositoryRuntime> {
    state
        .open_postgres_repository_runtime("statistics")
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message(),
            ))
        })
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
                "error": error.error,
                "message": error.message,
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_value(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body bytes");
        (
            status,
            serde_json::from_slice(&body).expect("JSON response body"),
        )
    }

    #[tokio::test]
    async fn success_helpers_pin_result_and_data_envelopes() {
        let (status, result) =
            response_value(success_result(StatusCode::OK, json!({"items": [1]}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(result, json!({"success": true, "result": {"items": [1]}}));

        let (status, data) =
            response_value(success_data(StatusCode::CREATED, json!({"rows": [2]}))).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(data, json!({"success": true, "data": {"rows": [2]}}));
    }

    #[tokio::test]
    async fn error_helpers_pin_statistics_and_asset_error_shapes() {
        let (status, basic) = response_value(bad_request("bad query")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(basic, json!({"success": false, "error": "bad query"}));

        let contract_error = StatisticsContractError::new(
            "Invalid time range",
            "startTime must be less than or equal to endTime",
        );
        let (status, stats) =
            response_value(statistics_error_response(contract_error, false)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            stats,
            json!({
                "success": false,
                "error": "Invalid time range",
                "message": "startTime must be less than or equal to endTime"
            })
        );

        let invalid_year_month =
            StatisticsContractError::new("Invalid year-month format", "bad year month");
        let (status, year_month) =
            response_value(statistics_error_response(invalid_year_month, true)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            year_month,
            json!({"success": false, "error": "bad year month"})
        );

        let wide_asset = StatisticsContractError::new(
            "资产趋势查询最多支持365天范围，请缩小时间范围",
            "资产趋势查询最多支持365天范围，请缩小时间范围",
        );
        let (status, asset) = response_value(asset_trends_error_response(wide_asset)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(asset["errorCode"], 400);
        assert_eq!(
            asset["error"],
            "资产趋势查询最多支持365天范围，请缩小时间范围"
        );
    }

    #[test]
    fn invalid_status_falls_back_to_internal_server_error() {
        assert_eq!(status_or_internal(201), StatusCode::CREATED);
        assert_eq!(status_or_internal(42), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
