// 中文导读：HTTP 认证路由的 Postgres-only runtime、请求与审计辅助。
// 维护重点：只打开 PostgreSQL runtime；认证状态以 Postgres 和 JWT 为权威。
// 不变式：敏感操作失败计数与用户数据审计必须按 user-scope 写入 Postgres。

fn open_postgres_runtime(state: &HttpAppState) -> RouteResult<PostgresRepositoryRuntime> {
    state
        .open_postgres_repository_runtime("auth")
        .map_err(|error| {
            let status = error.http_status_code();
            let title = if status == 503 {
                "Service Unavailable"
            } else {
                "Internal Server Error"
            };
            Box::new(auth_rest_error_response(AuthRestError::new(
                status,
                title,
                error.public_message(),
            )))
        })
}

fn request_body_object(body: &[u8]) -> Map<String, Value> {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    json_object_or_empty(parsed.as_ref())
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

#[tracing::instrument(level = "debug", skip_all)]
async fn ensure_postgres_sensitive_auth_failure_limit(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    event_type: &str,
) -> RouteResult<()> {
    let failure_count = count_postgres_auth_events_since(
        pool,
        user_id,
        event_type,
        &sensitive_auth_failure_window_start_text(),
    )
    .await
    .map_err(|_| Box::new(db_error_response()))?;
    if failure_count >= SENSITIVE_AUTH_FAILURE_LIMIT {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            429,
            "Too Many Requests",
            "Too many failed sensitive-operation authentication attempts, please try again later",
        ))));
    }
    Ok(())
}

async fn record_postgres_sensitive_auth_failure(
    pool: &bill_analyser_db::PostgresPool,
    user: &AuthLoginUserRow,
    event_type: &str,
    ip_address: &str,
    user_agent: &str,
    error_message: &str,
    metadata: Option<Value>,
) -> RouteResult<()> {
    create_postgres_auth_log(
        pool,
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: event_type.to_string(),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            success: false,
            error_message: Some(error_message.to_string()),
            metadata: metadata.map(|value| value.to_string()),
            created_at: utc_now_text(),
        },
    )
    .await
    .map(|_| ())
    .map_err(|_| Box::new(db_error_response()))
}

struct UserDataAuditLogDraft<'a> {
    operation_type: &'a str,
    user_id: UserId,
    details: Value,
    affected_count: i64,
    ip_address: &'a str,
    user_agent: &'a str,
    now: &'a str,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_postgres_user_data_audit_log_best_effort(
    pool: &bill_analyser_db::PostgresPool,
    draft: UserDataAuditLogDraft<'_>,
) {
    let _ = create_postgres_user_data_audit_event(
        pool,
        PostgresUserDataAuditEvent {
            operation_type: draft.operation_type,
            user_id: draft.user_id,
            details: draft.details,
            affected_count: draft.affected_count,
            ip_address: draft.ip_address,
            user_agent: draft.user_agent,
            now: draft.now,
        },
    )
    .await;
}
