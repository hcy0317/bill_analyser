// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：生成 2FA 启用草稿，返回 secret/QR code 供用户绑定认证器。
async fn request_two_factor_enable_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "request_two_factor_enable_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return request_postgres_two_factor_enable_response(state, auth).await;
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：确认启用 2FA，校验当前密码和 passcode 后写入 recovery codes。
async fn confirm_two_factor_enable_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "confirm_two_factor_enable_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let secret = body
        .get("secret")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .replace(' ', "");
    let passcode = body
        .get("passcode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if secret.is_empty() || passcode.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Secret and passcode are required",
        ));
    }
    if !verify_totp_passcode(&secret, passcode, Utc::now().timestamp()) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Invalid passcode",
            "The current passcode is incorrect",
        ));
    }

        return confirm_postgres_two_factor_enable_response(
            state,
            headers,
            connect_info.map(|ConnectInfo(addr)| addr),
            auth,
            secret,
        )
        .await;
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：禁用当前用户 2FA，校验当前密码后清理 secret 和 recovery codes。
async fn disable_two_factor_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "disable_two_factor_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return disable_postgres_two_factor_response(
            state,
            headers,
            connect_info.map(|ConnectInfo(addr)| addr),
            auth,
            body,
        )
        .await;
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：重新生成当前用户 2FA recovery codes，校验当前密码后替换旧备份码。
async fn regenerate_two_factor_recovery_codes_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "regenerate_two_factor_recovery_codes_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return regenerate_postgres_two_factor_recovery_codes_response(
            state,
            headers,
            connect_info.map(|ConnectInfo(addr)| addr),
            auth,
            body,
        )
        .await;
}

// 中文说明：执行 PostgreSQL 2FA 启用草稿响应，生成 secret、issuer 和二维码 payload。
async fn request_postgres_two_factor_enable_response(
    state: HttpAppState,
    auth: AuthenticatedUser,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), auth.user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };

    let secret = match random_base32_secret() {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let provisioning_uri = two_factor_provisioning_uri(&user.profile.username, &secret);
    let qrcode = match qrcode_png_data_url(&provisioning_uri) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    success_result(
        StatusCode::OK,
        json!({
            "secret": secret,
            "qrcode": qrcode
        }),
    )
}

// 中文说明：执行 PostgreSQL 2FA 启用确认，写入 secret 与 recovery code hash 并返回明文备份码。
async fn confirm_postgres_two_factor_enable_response(
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    auth: AuthenticatedUser,
    secret: String,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), auth.user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    if user.two_factor_enabled {
        return two_factor_already_enabled_response();
    }
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, peer_addr);
    let tokens = match issue_session_tokens(user.profile.id, &user.profile.username, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let recovery_codes = match generate_two_factor_recovery_codes() {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let recovery_code_refs = recovery_codes
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let now = now_text();
    let persist_result = enable_postgres_two_factor_with_recovery_codes_and_session(
        runtime.pool(),
        user.profile.id,
        &secret,
        &recovery_code_refs,
        &CreateTokenSessionDraft {
            user_id: user.profile.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent.clone(),
            ip_address: ip_address.clone(),
            created_at: now.clone(),
        },
        &now,
    )
    .await;
    let (stored_count, session_id) = match persist_result {
        Ok(value) => value,
        Err(DbError::InvalidOperation(message))
            if message == "two-factor authentication is already enabled" =>
        {
            return two_factor_already_enabled_response();
        }
        Err(_) => return db_error_response(),
    };
    if stored_count != recovery_codes.len() {
        return db_error_response();
    }
    let _ = create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: "2fa_enabled".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "recovery_code_count": stored_count,
                    "session_id": session_id
                })
                .to_string(),
            ),
            created_at: now,
        },
    )
    .await;

    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "recoveryCodes": recovery_codes
        }),
    )
}

// 中文说明：执行 PostgreSQL 2FA 禁用流程，清理认证器 secret、备份码和 session 状态。
async fn disable_postgres_two_factor_response(
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    auth: AuthenticatedUser,
    body: Bytes,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), auth.user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let body = request_body_object(&body);
    let auth_mode =
        match resolve_sensitive_two_factor_auth_postgres(runtime.pool(), &body, &state, &user)
            .await
        {
            Ok(value) => value,
            Err(SensitiveTwoFactorAuthError::Missing) => return sensitive_auth_missing_response(),
            Err(SensitiveTwoFactorAuthError::Invalid) => return sensitive_auth_invalid_response(),
            Err(SensitiveTwoFactorAuthError::Db) => return db_error_response(),
        };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, peer_addr);
    let now = now_text();
    let cleared_count = match disable_postgres_two_factor_and_clear_recovery_codes(
        runtime.pool(),
        user.profile.id,
        &now,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let _ = create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: "2fa_disabled".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "cleared_recovery_code_count": cleared_count,
                    "auth_mode": auth_mode.as_str()
                })
                .to_string(),
            ),
            created_at: now,
        },
    )
    .await;

    success_result(StatusCode::OK, Value::Bool(true))
}

// 中文说明：执行 PostgreSQL recovery code 重新生成流程，返回新的明文备份码给前端一次性展示。
async fn regenerate_postgres_two_factor_recovery_codes_response(
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    auth: AuthenticatedUser,
    body: Bytes,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), auth.user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let body = request_body_object(&body);
    let auth_mode =
        match resolve_sensitive_two_factor_auth_postgres(runtime.pool(), &body, &state, &user)
            .await
        {
            Ok(value) => value,
            Err(SensitiveTwoFactorAuthError::Missing) => return sensitive_auth_missing_response(),
            Err(SensitiveTwoFactorAuthError::Invalid) => return sensitive_auth_invalid_response(),
            Err(SensitiveTwoFactorAuthError::Db) => return db_error_response(),
        };
    if !user.two_factor_enabled {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Two-factor authentication is not enabled",
        ));
    }

    let recovery_codes = match generate_two_factor_recovery_codes() {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let recovery_code_refs = recovery_codes
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let now = now_text();
    let stored_count = match replace_postgres_two_factor_recovery_codes(
        runtime.pool(),
        user.profile.id,
        &recovery_code_refs,
        &now,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if stored_count != recovery_codes.len() {
        return db_error_response();
    }

    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, peer_addr);
    let _ = create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: "2fa_recovery_regenerated".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "recovery_code_count": stored_count,
                    "auth_mode": auth_mode.as_str()
                })
                .to_string(),
            ),
            created_at: now,
        },
    )
    .await;

    success_result(
        StatusCode::OK,
        json!({
            "recoveryCodes": recovery_codes
        }),
    )
}
