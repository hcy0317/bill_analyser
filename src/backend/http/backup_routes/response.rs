// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

pub(super) fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    state
        .open_sqlite_repository_runtime("backup ops")
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message("Rust backup ops route runtime DB error"),
            ))
        })
}

pub(super) fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

pub(super) fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust backup ops route runtime DB error",
    )
}

pub(super) fn auth_error_response(error: crate::auth::RustRouteAuthError) -> Response {
    error_response(status_or_internal(error.status), error.message)
}

pub(super) fn db_write_error_response(error: DbError) -> Response {
    match error {
        DbError::InvalidOperation(message) if message == "backup job not found" => {
            error_response(StatusCode::NOT_FOUND, message)
        }
        _ => db_error_response(),
    }
}

pub(super) fn file_error_response(error: BackupFileRuntimeError) -> Box<Response> {
    Box::new(error_response(error.status, error.message))
}

pub(super) fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({
            "success": false,
            "error": message.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_helpers_preserve_status_and_error_mapping() {
        assert_eq!(status_or_internal(404), StatusCode::NOT_FOUND);
        assert_eq!(status_or_internal(99), StatusCode::INTERNAL_SERVER_ERROR);

        let not_found = db_write_error_response(DbError::InvalidOperation(
            "backup job not found".to_string(),
        ));
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);

        let db_error = db_error_response();
        assert_eq!(db_error.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let file_error = file_error_response(BackupFileRuntimeError::bad_request("bad file"));
        assert_eq!(file_error.status(), StatusCode::BAD_REQUEST);
    }
}
