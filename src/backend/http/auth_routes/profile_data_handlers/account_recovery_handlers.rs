// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn authorize_oauth2_callback_handler(State(state): State<HttpAppState>) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "authorize_oauth2_callback_handler", "business operation entered");
    if !state.config.auth_enable_oauth2 {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "OAuth2 disabled",
            "OAuth2 login is currently disabled",
        ));
    }

    auth_rest_error_response(AuthRestError::new(
        501,
        "Not Implemented",
        "OAuth2 callback authorization is not implemented in this workspace build",
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn verify_email_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "verify_email_handler", "business operation entered");
    let peer_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let body = request_body_object(&body);
    let token = body
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if token.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Verification token is required",
        ));
    }
    let request_new_token = body
        .get("requestNewToken")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let payload = match validate_action_jwt(
        token,
        &state,
        "verify_email",
        "Verification token is invalid or expired",
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = match action_user_id(&payload, "Verification token is invalid") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_user_profile(runtime.connection(), user_id) {
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
    if payload.get("email").and_then(Value::as_str) != Some(user.email.trim()) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Invalid token",
            "Verification token does not match email",
        ));
    }
    if set_user_email_verified(runtime.connection(), user_id, true, &utc_now_text()).is_err() {
        return db_error_response();
    }
    let user = match get_auth_user_profile(runtime.connection(), user_id) {
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
    let mut new_token = Value::Null;
    if request_new_token {
        let tokens = match issue_session_tokens(user.id, &user.username, &state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let created_at = now_text();
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, peer_addr);
        if create_token_session(
            runtime.connection(),
            &CreateTokenSessionDraft {
                user_id: user.id,
                token_hash: sha256_hex(&tokens.access_token),
                refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
                expires_at: tokens.expires_at,
                refresh_expires_at: Some(tokens.refresh_expires_at),
                user_agent: request_user_agent,
                ip_address,
                created_at,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        new_token = Value::String(tokens.access_token);
    }
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, peer_addr);
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: "email_verified",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success: true,
            error_message: None,
            metadata: None,
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(
        StatusCode::OK,
        json!({
            "newToken": new_token,
            "user": user_profile_payload(&user),
            "notificationContent": "",
        }),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resend_public_verification_email_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "resend_public_verification_email_handler", "business operation entered");
    let body = request_body_object(&body);
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if email.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email and password are required",
        ));
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_email(runtime.connection(), &email) {
        Ok(Some(value)) if bcrypt::verify(password, &value.password_hash).unwrap_or(false) => value,
        Ok(_) => {
            return auth_rest_error_response(AuthRestError::new(
                401,
                "Invalid credentials",
                "Invalid email or password",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let verification_token = match issue_action_token(&user, "verify_email", 24, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type: "verification_email_resend_requested",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "email": email,
                    "delivery": "not_configured_mock_success",
                    "verification_token": verification_token,
                })
                .to_string(),
            ),
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn forgot_password_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "forgot_password_handler", "business operation entered");
    let body = request_body_object(&body);
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if email.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email is required",
        ));
    }
    if !state.config.auth_enable_user_forget_password {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Forget password disabled",
            "Forget password is currently disabled",
        ));
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_email(runtime.connection(), &email) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if let Some(user) = user {
        let reset_token = match issue_action_token(&user, "reset_password", 24, &state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: Some(user.profile.id),
                username: &user.profile.username,
                event_type: "password_reset_requested",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: true,
                error_message: None,
                metadata: Some(
                    json!({
                        "email": email,
                        "delivery": "not_configured_mock_success",
                        "reset_token": reset_token,
                    })
                    .to_string(),
                ),
            },
        )
        .is_err()
        {
            return db_error_response();
        }
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reset_password_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "reset_password_handler", "business operation entered");
    let body = request_body_object(&body);
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let token = body
        .get("token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if email.is_empty() || password.is_empty() || token.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email, password and token are required",
        ));
    }
    if !state.config.auth_enable_user_forget_password {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Forget password disabled",
            "Forget password is currently disabled",
        ));
    }
    if let Err(message) = state.config.auth_password_policy.validate(password) {
        return auth_rest_error_response(AuthRestError::new(400, "Invalid password", message));
    }
    let payload = match validate_action_jwt(
        token,
        &state,
        "reset_password",
        "Reset password token is invalid or expired",
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if payload.get("email").and_then(Value::as_str) != Some(email.as_str()) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Invalid token",
            "Reset password token does not match email",
        ));
    }
    let user_id = match action_user_id(&payload, "Invalid token") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_user_profile(runtime.connection(), user_id) {
        Ok(Some(value)) if value.email.trim() == email => value,
        Ok(_) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let password_hash = match bcrypt::hash(password, bcrypt::DEFAULT_COST) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if update_user_password_hash(
        runtime.connection(),
        user.id,
        &password_hash,
        &utc_now_text(),
    )
    .is_err()
    {
        return db_error_response();
    }
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: "password_reset_completed",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success: true,
            error_message: None,
            metadata: None,
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn unlink_profile_external_auth_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "unlink_profile_external_auth_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let external_auth_type = body
        .get("externalAuthType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if external_auth_type.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "externalAuthType is required",
        ));
    }
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if password.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "password is required",
        ));
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_token_user(runtime.connection(), auth.user_id) {
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
    if !bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Invalid password",
        ));
    }

    let existing =
        match get_user_external_auth(runtime.connection(), auth.user_id, &external_auth_type) {
            Ok(Some(value)) => value,
            Ok(None) => {
                return auth_rest_error_response(AuthRestError::new(
                    404,
                    "Not Found",
                    "Third-party login is not linked",
                ));
            }
            Err(_) => return db_error_response(),
        };
    let success =
        match delete_user_external_auth(runtime.connection(), auth.user_id, &external_auth_type) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if log_auth_event(
        runtime.connection(),
        AuthEvent {
            user_id: Some(user.id),
            username: &user.username,
            event_type: "external_auth_unlinked",
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            success,
            error_message: None,
            metadata: Some(
                json!({
                    "external_auth_type": external_auth_type,
                    "external_auth_category": existing.external_auth_category,
                })
                .to_string(),
            ),
        },
    )
    .is_err()
    {
        return db_error_response();
    }

    success_result(StatusCode::OK, Value::Bool(success))
}
