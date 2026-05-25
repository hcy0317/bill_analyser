// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        )))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.to_string(),
        )))
    })?;
    let runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: false,
        busy_timeout: state.config.timeout,
    })
    .map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            format!("Rust auth token runtime cannot open configured SQLite database: {error}"),
        )))
    })?;
    init_auth_security_schema(runtime.connection()).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            500,
            "Internal Server Error",
            format!("Rust auth token runtime schema initialization failed: {error}"),
        )))
    })?;
    Ok(runtime)
}

fn client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    if let Some(addr) = peer_addr {
        return addr.ip().to_string();
    }
    forwarded_header_ip(headers).unwrap_or_else(|| FALLBACK_CLIENT_IP.to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
fn login_client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    forwarded_header_ip(headers)
        .or_else(|| peer_addr.map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| FALLBACK_CLIENT_IP.to_string())
}

fn forwarded_header_ip(headers: &HeaderMap) -> Option<String> {
    let forwarded_for = header_value(headers, "x-forwarded-for");
    if !forwarded_for.is_empty() {
        return Some(
            forwarded_for
                .split(',')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
        );
    }
    let real_ip = header_value(headers, "x-real-ip");
    if !real_ip.is_empty() {
        return Some(real_ip);
    }
    None
}

fn request_origin(state: &HttpAppState) -> RouteResult<String> {
    if let Some(public_base_url) = state.config.public_base_url.as_deref() {
        return Ok(public_base_url.to_string());
    }
    Err(Box::new(auth_rest_error_response(AuthRestError::new(
        503,
        "Service Unavailable",
        "Rust auth token runtime requires BILL_ANALYSER_PUBLIC_BASE_URL",
    ))))
}

fn header_value(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}
