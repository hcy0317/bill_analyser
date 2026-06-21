// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：校验登录阶段的 2FA recovery code，成功后签发完整登录 token。
async fn verify_two_factor_recovery_login_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "verify_two_factor_recovery_login_handler", "business operation entered");
    let token = match parse_logout_bearer_token(&headers) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };
    let body = request_body_object(&body);
    let recovery_code = body
        .get("recoveryCode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if recovery_code.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Recovery code is required",
        ));
    }

    let payload = match validate_pending_two_factor_jwt(&token, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = match pending_two_factor_user_id(&payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return verify_postgres_two_factor_recovery_login_response(
            state,
            headers,
            connect_info.map(|ConnectInfo(addr)| addr),
            recovery_code,
            user_id,
        )
        .await;
}

// 中文说明：执行 PostgreSQL recovery code 登录校验，命中后消费备份码并创建认证 session。
async fn verify_postgres_two_factor_recovery_login_response(
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    recovery_code: &str,
    user_id: UserId,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return json_response(
                StatusCode::NOT_FOUND,
                json!({
                    "success": false,
                    "error": "User not found"
                }),
            );
        }
        Err(_) => return db_error_response(),
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, peer_addr);
    if !user.is_active {
        let _ = create_postgres_auth_log(
            runtime.pool(),
            &AuthLogDraft {
                user_id: Some(user.profile.id),
                username: user.profile.username.clone(),
                event_type: "login_failed".to_string(),
                ip_address: ip_address.clone(),
                user_agent: request_user_agent.clone(),
                success: false,
                error_message: Some("Account not active".to_string()),
                metadata: None,
                created_at: utc_now_text(),
            },
        )
        .await;
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Account not active",
            "Your account has been deactivated",
        ));
    }
    if !user.two_factor_enabled {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Two-factor authentication is not enabled",
        ));
    }

    let cloud_settings =
        match list_postgres_application_cloud_settings(runtime.pool(), user.profile.id).await {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let tokens = match issue_session_tokens(user.profile.id, &user.profile.username, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let now = now_text();
    let consumed = match consume_postgres_two_factor_recovery_code(
        runtime.pool(),
        user.profile.id,
        recovery_code,
        &now,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if !consumed {
        return auth_rest_error_response(AuthRestError::new(
            401,
            "Invalid recovery code",
            "Recovery code is invalid or already used",
        ));
    }
    let session_id = match create_postgres_token_session(
        runtime.pool(),
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
            event_type: "login_2fa_recovery_success".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "session_id": session_id }).to_string()),
            created_at: now,
        },
    )
    .await;
    let mut user_payload = user_profile_payload(&user.profile);
    if let Value::Object(ref mut object) = user_payload {
        object.insert("id".to_string(), Value::from(user.profile.id.get()));
    }

    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "need2FA": false,
            "user": user_payload,
            "applicationCloudSettings": application_cloud_settings_payload(cloud_settings),
        }),
    )
}
