// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn get_two_factor_status_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "get_two_factor_status_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match get_postgres_auth_user_two_factor_enabled(runtime.pool(), auth.user_id).await {
            Ok(Some(enabled)) => success_result(
                StatusCode::OK,
                json!({
                    "enable": enabled,
                    "isEnabled": enabled
                }),
            ),
            Ok(None) => auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            )),
            Err(_) => db_error_response(),
        };
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_auth_user_two_factor_enabled(runtime.connection(), auth.user_id) {
        Ok(Some(enabled)) => success_result(
            StatusCode::OK,
            json!({
                "enable": enabled,
                "isEnabled": enabled
            }),
        ),
        Ok(None) => {
            auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
        }
        Err(_) => db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn verify_security_step_up_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "verify_security_step_up_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let passcode = body
        .get("passcode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if password.is_empty() && passcode.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "password or passcode is required",
        ));
    }
    if state.config.database_backend.uses_postgres() {
        return verify_postgres_security_step_up_response(
            state,
            auth,
            headers,
            connect_info.map(|ConnectInfo(addr)| addr),
            password,
            passcode,
        )
        .await;
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_id(runtime.connection(), auth.user_id) {
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
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if let Err(response) = ensure_sensitive_auth_failure_limit(
        runtime.connection(),
        user.profile.id,
        STEP_UP_AUTH_FAILED_EVENT,
    ) {
        return *response;
    }

    let verified_via = if !password.is_empty() {
        match verify_sensitive_operation_password_with_policy(
            runtime.connection(),
            &user,
            password,
            OperationPasswordPolicy::RequireConfigured,
        ) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(response) = record_sensitive_auth_failure(
                    runtime.connection(),
                    &user,
                    STEP_UP_AUTH_FAILED_EVENT,
                    &ip_address,
                    &request_user_agent,
                    "Invalid password",
                    Some(json!({ "reason": "invalid_password" })),
                ) {
                    return *response;
                }
                return auth_rest_error_response(AuthRestError::new(
                    401,
                    "Invalid credentials",
                    "Current password is incorrect",
                ));
            }
            Err(_) => return db_error_response(),
        }
        "password"
    } else {
        let secret = user.two_factor_secret.trim().replace(' ', "");
        if !user.two_factor_enabled || secret.is_empty() {
            if let Err(response) = record_sensitive_auth_failure(
                runtime.connection(),
                &user,
                STEP_UP_AUTH_FAILED_EVENT,
                &ip_address,
                &request_user_agent,
                "Two-factor authentication is not enabled",
                Some(json!({ "reason": "totp_unavailable" })),
            ) {
                return *response;
            }
            return auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Two-factor authentication is not enabled",
            ));
        }
        if !verify_totp_passcode(&secret, passcode, Utc::now().timestamp()) {
            if let Err(response) = record_sensitive_auth_failure(
                runtime.connection(),
                &user,
                STEP_UP_AUTH_FAILED_EVENT,
                &ip_address,
                &request_user_agent,
                "Invalid passcode",
                Some(json!({ "reason": "invalid_passcode" })),
            ) {
                return *response;
            }
            return auth_rest_error_response(AuthRestError::new(
                401,
                "Invalid passcode",
                "The current passcode is incorrect",
            ));
        }
        "passcode"
    };

    let step_up_token = match issue_action_token(&user, "step_up", 1, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type: "step_up_verified",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "verified_via": verified_via }).to_string()),
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(
        StatusCode::OK,
        json!({
            "stepUpToken": step_up_token,
            "verifiedVia": verified_via
        }),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn verify_two_factor_login_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "verify_two_factor_login_handler", "business operation entered");
    let token = match parse_logout_bearer_token(&headers) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };
    let body = request_body_object(&body);
    let passcode = body
        .get("passcode")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if passcode.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "success": false,
                "error": "Bad Request",
                "errorCode": 203005,
                "message": "Passcode is required"
            }),
        );
    }

    let payload = match validate_pending_two_factor_jwt(&token, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = match pending_two_factor_user_id(&payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        return verify_postgres_two_factor_login_response(
            state,
            headers,
            connect_info.map(|ConnectInfo(addr)| addr),
            passcode,
            user_id,
        )
        .await;
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_id(runtime.connection(), user_id) {
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
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if !user.is_active {
        if log_login_failure(
            runtime.connection(),
            &user,
            &ip_address,
            &request_user_agent,
            "Account not active",
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Account not active",
            "Your account has been deactivated",
        ));
    }
    let secret = user.two_factor_secret.trim().replace(' ', "");
    if !user.two_factor_enabled || secret.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Two-factor authentication is not enabled",
        ));
    }
    if !verify_totp_passcode(&secret, passcode, Utc::now().timestamp()) {
        return auth_rest_error_response(AuthRestError::new(
            401,
            "Invalid passcode",
            "The current passcode is incorrect",
        ));
    }

    let cloud_settings =
        match list_application_cloud_settings(runtime.connection(), user.profile.id) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let tokens = match issue_session_tokens(user.profile.id, &user.profile.username, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let now = now_text();
    if persist_two_factor_login_success(
        runtime.connection(),
        &CreateTokenSessionDraft {
            user_id: user.profile.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent.clone(),
            ip_address: ip_address.clone(),
            created_at: now,
        },
        user.profile.id,
        &user.profile.username,
        &ip_address,
        &request_user_agent,
    )
    .is_err()
    {
        return db_error_response();
    }
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

async fn verify_postgres_security_step_up_response(
    state: HttpAppState,
    auth: AuthenticatedUser,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    password: &str,
    passcode: &str,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), auth.user_id).await {
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
    let ip_address = client_ip(&headers, peer_addr);
    if let Err(response) = ensure_postgres_sensitive_auth_failure_limit(
        runtime.pool(),
        user.profile.id,
        STEP_UP_AUTH_FAILED_EVENT,
    )
    .await
    {
        return *response;
    }

    let verified_via = if !password.is_empty() {
        match verify_postgres_sensitive_operation_password_with_policy(
            runtime.pool(),
            &user,
            password,
            OperationPasswordPolicy::RequireConfigured,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => {
                if let Err(response) = record_postgres_sensitive_auth_failure(
                    runtime.pool(),
                    &user,
                    STEP_UP_AUTH_FAILED_EVENT,
                    &ip_address,
                    &request_user_agent,
                    "Invalid password",
                    Some(json!({ "reason": "invalid_password" })),
                )
                .await
                {
                    return *response;
                }
                return auth_rest_error_response(AuthRestError::new(
                    401,
                    "Invalid credentials",
                    "Current password is incorrect",
                ));
            }
            Err(_) => return db_error_response(),
        }
        "password"
    } else {
        let secret = user.two_factor_secret.trim().replace(' ', "");
        if !user.two_factor_enabled || secret.is_empty() {
            if let Err(response) = record_postgres_sensitive_auth_failure(
                runtime.pool(),
                &user,
                STEP_UP_AUTH_FAILED_EVENT,
                &ip_address,
                &request_user_agent,
                "Two-factor authentication is not enabled",
                Some(json!({ "reason": "totp_unavailable" })),
            )
            .await
            {
                return *response;
            }
            return auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Two-factor authentication is not enabled",
            ));
        }
        if !verify_totp_passcode(&secret, passcode, Utc::now().timestamp()) {
            if let Err(response) = record_postgres_sensitive_auth_failure(
                runtime.pool(),
                &user,
                STEP_UP_AUTH_FAILED_EVENT,
                &ip_address,
                &request_user_agent,
                "Invalid passcode",
                Some(json!({ "reason": "invalid_passcode" })),
            )
            .await
            {
                return *response;
            }
            return auth_rest_error_response(AuthRestError::new(
                401,
                "Invalid passcode",
                "The current passcode is incorrect",
            ));
        }
        "passcode"
    };

    let step_up_token = match issue_action_token(&user, "step_up", 1, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: "step_up_verified".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "verified_via": verified_via }).to_string()),
            created_at: utc_now_text(),
        },
    )
    .await
    .is_err()
    {
        return db_error_response();
    }

    success_result(
        StatusCode::OK,
        json!({
            "stepUpToken": step_up_token,
            "verifiedVia": verified_via
        }),
    )
}

async fn verify_postgres_two_factor_login_response(
    state: HttpAppState,
    headers: HeaderMap,
    peer_addr: Option<SocketAddr>,
    passcode: &str,
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
    let secret = user.two_factor_secret.trim().replace(' ', "");
    if !user.two_factor_enabled || secret.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Two-factor authentication is not enabled",
        ));
    }
    if !verify_totp_passcode(&secret, passcode, Utc::now().timestamp()) {
        return auth_rest_error_response(AuthRestError::new(
            401,
            "Invalid passcode",
            "The current passcode is incorrect",
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
    if create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: "login_2fa_success".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "session_id": session_id }).to_string()),
            created_at: now,
        },
    )
    .await
    .is_err()
    {
        return db_error_response();
    }
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
