// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。


fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust budget DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
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

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn invalid_budget_import_data_response() -> Response {
    bad_request("Invalid data format. Expected array of budgets.")
}

fn not_found(message: impl ToString) -> Response {
    error_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust budget route runtime DB error",
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

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
