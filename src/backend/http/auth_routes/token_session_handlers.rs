// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn list_tokens_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "list_tokens_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return list_postgres_tokens_response(state, headers, auth).await;
}

#[tracing::instrument(level = "debug", skip_all)]
async fn generate_personal_token(
    token_kind: TokenKind,
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Current password is required",
        ));
    }
    let expires_in_seconds = match parse_expires_in_seconds(&body) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return generate_postgres_personal_token(
            token_kind,
            state,
            headers,
            peer_addr,
            auth,
            password,
            expires_in_seconds,
        )
        .await;
}

#[tracing::instrument(level = "debug", skip_all)]
async fn revoke_other_tokens_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "revoke_other_tokens_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return revoke_other_postgres_tokens_response(state, headers, auth).await;
}

#[tracing::instrument(level = "debug", skip_all)]
async fn revoke_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(token_id): Path<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "revoke_token_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let token_id = match token_id.trim().parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return auth_rest_error_response(AuthRestError::invalid_request(
                "tokenId must be a valid integer",
            ));
        }
    };

        return revoke_postgres_token_response(state, auth, token_id).await;
}

#[tracing::instrument(level = "debug", skip_all)]
async fn logout_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    _connect_info: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "logout_handler", "business operation entered");
    let token = match parse_logout_bearer_token(&headers) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };

        if let Ok(runtime) = open_postgres_runtime(&state) {
            let token_hash = sha256_hex(&token);
            let _ = invalidate_postgres_session_by_token_hash(runtime.pool(), &token_hash).await;
        }
        return json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "result": true,
                "message": "Logged out successfully"
            }),
        );
}

async fn list_postgres_tokens_response(
    state: HttpAppState,
    headers: HeaderMap,
    auth: AuthenticatedUser,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if cleanup_postgres_expired_sessions(runtime.pool(), &now_text())
        .await
        .is_err()
    {
        return db_error_response();
    }
    let current_session_id = current_postgres_session_id(runtime.pool(), &headers).await;
    match list_postgres_user_sessions(runtime.pool(), auth.user_id).await {
        Ok(sessions) => success_result(
            StatusCode::OK,
            Value::Array(
                sessions
                    .into_iter()
                    .map(|session| session_payload(session, current_session_id))
                    .collect(),
            ),
        ),
        Err(_) => db_error_response(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn generate_postgres_personal_token(
    token_kind: TokenKind,
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    auth: AuthenticatedUser,
    password: &str,
    expires_in_seconds: i64,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if current_postgres_session_id(runtime.pool(), &headers)
        .await
        .is_none()
    {
        return auth_error_response(RustRouteAuthError {
            status: 401,
            message: "Current bearer session is required".to_string(),
        });
    }
    let user = match get_postgres_auth_token_user(runtime.pool(), auth.user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let token_user_agent = token_kind.user_agent(&request_user_agent);
    let ip_address = client_ip(&headers, peer_addr);
    let response_origin = match request_origin(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let failure_count = match count_postgres_recent_token_password_failures(
        runtime.pool(),
        user.id,
        &token_failure_window_start_text(),
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if failure_count >= TOKEN_PASSWORD_FAILURE_LIMIT {
        return auth_rest_error_response(AuthRestError::new(
            429,
            "Too Many Requests",
            "Too many failed token password attempts, please try again later",
        ));
    }

    if !bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        if create_postgres_auth_log(
            runtime.pool(),
            &AuthLogDraft {
                user_id: Some(user.id),
                username: user.username.clone(),
                event_type: format!("{}_token_generate_failed", token_kind.as_str()),
                ip_address: ip_address.clone(),
                user_agent: request_user_agent.clone(),
                success: false,
                error_message: Some("Invalid password".to_string()),
                metadata: None,
                created_at: utc_now_text(),
            },
        )
        .await
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            401,
            "Invalid credentials",
            "Current password is incorrect",
        ));
    }

    let issued_token = match issue_access_token(
        user.id,
        &user.username,
        &state,
        token_kind,
        expires_in_seconds,
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let created_at = now_text();
    let session_id = match create_postgres_token_session(
        runtime.pool(),
        &CreateTokenSessionDraft {
            user_id: user.id,
            token_hash: sha256_hex(&issued_token.access_token),
            refresh_token_hash: None,
            expires_at: issued_token.expires_at.clone(),
            refresh_expires_at: None,
            user_agent: token_user_agent.clone(),
            ip_address: ip_address.clone(),
            created_at: created_at.clone(),
        },
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.id),
            username: user.username.clone(),
            event_type: format!("{}_token_generate_success", token_kind.as_str()),
            ip_address,
            user_agent: token_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "session_id": session_id }).to_string()),
            created_at,
        },
    )
    .await
    .is_err()
    {
        return db_error_response();
    }

    let mut result = Map::new();
    result.insert(
        "token".to_string(),
        Value::String(issued_token.access_token),
    );
    match token_kind {
        TokenKind::Api => {
            result.insert(
                "apiBaseUrl".to_string(),
                Value::String(format!("{response_origin}/api")),
            );
        }
        TokenKind::Mcp => {
            result.insert(
                "mcpUrl".to_string(),
                Value::String(format!("{response_origin}/mcp")),
            );
        }
        TokenKind::Session => {}
    }
    success_result(StatusCode::OK, Value::Object(result))
}

async fn revoke_other_postgres_tokens_response(
    state: HttpAppState,
    headers: HeaderMap,
    auth: AuthenticatedUser,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(current_session_id) = current_postgres_session_id(runtime.pool(), &headers).await
    else {
        return auth_error_response(RustRouteAuthError {
            status: 401,
            message: "Current bearer session is required".to_string(),
        });
    };
    match invalidate_other_postgres_user_sessions(runtime.pool(), auth.user_id, current_session_id)
        .await
    {
        Ok(revoked_count) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "result": true,
                "revokedCount": revoked_count
            }),
        ),
        Err(_) => db_error_response(),
    }
}

async fn revoke_postgres_token_response(
    state: HttpAppState,
    auth: AuthenticatedUser,
    token_id: i64,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match invalidate_postgres_session_by_id(runtime.pool(), token_id, auth.user_id).await {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => {
            auth_rest_error_response(AuthRestError::new(404, "Not Found", "Token not found"))
        }
        Err(_) => db_error_response(),
    }
}

async fn current_postgres_session_id(
    pool: &bill_analyser_db::PostgresPool,
    headers: &HeaderMap,
) -> Option<i64> {
    let token = parse_logout_bearer_token(headers).ok()?;
    get_postgres_active_session_id_by_token_hash(pool, &sha256_hex(&token))
        .await
        .ok()
        .flatten()
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_logout_bearer_token(headers: &HeaderMap) -> Result<String, AuthRestError> {
    let auth_header = header_value(headers, header::AUTHORIZATION.as_str());
    if auth_header.is_empty() {
        return Err(AuthRestError::unauthorized("Missing authorization header"));
    }

    let mut parts = auth_header.split_whitespace();
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default();
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(AuthRestError::unauthorized("Invalid authorization header"));
    }
    Ok(token.to_string())
}

#[cfg(test)]
#[tracing::instrument(level = "debug", skip_all)]
fn logout_session_not_found_warning_payload(token_hash: &str) -> Value {
    json!({
        "level": "warn",
        "target": "bill_analyser_http::auth_routes",
        "event": "logout_session_not_found",
        "token_hash_prefix": token_hash.chars().take(16).collect::<String>(),
    })
}
