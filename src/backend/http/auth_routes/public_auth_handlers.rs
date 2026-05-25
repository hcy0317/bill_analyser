// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn login_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "login_options_handler", "business operation entered");
    auth_options_handler().await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn register_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "register_options_handler", "business operation entered");
    auth_options_handler().await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn auth_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "auth_options_handler", "business operation entered");
    StatusCode::NO_CONTENT.into_response()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn auth_cors_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let origin = request.headers().get(header::ORIGIN).cloned();
    let requested_headers = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .cloned();
    let mut response = next.run(request).await;
    apply_auth_cors_headers(origin.as_ref(), response.headers_mut());
    if method == Method::OPTIONS {
        apply_auth_preflight_headers(requested_headers.as_ref(), response.headers_mut());
    }
    response
}

#[tracing::instrument(level = "debug", skip_all)]
async fn register_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "register_handler", "business operation entered");
    let body = request_body_object(&body);
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
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
    if username.is_empty() || email.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Username, email and password are required",
        ));
    }
    if !state.config.auth_enable_user_registration {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Registration disabled",
            "User registration is currently disabled",
        ));
    }
    if let Err(message) = state.config.auth_password_policy.validate(password) {
        return auth_rest_error_response(AuthRestError::new(400, "Invalid password", message));
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let username_exists = match auth_username_exists(runtime.connection(), &username) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if username_exists {
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: None,
                username: &username,
                event_type: "register_failed",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: false,
                error_message: Some("Username already exists".to_string()),
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            409,
            "Username exists",
            "Username already exists",
        ));
    }
    let email_exists = match auth_email_exists(runtime.connection(), &email) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if email_exists {
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: None,
                username: &username,
                event_type: "register_failed",
                ip_address: &ip_address,
                user_agent: &request_user_agent,
                success: false,
                error_message: Some("Email already exists".to_string()),
                metadata: None,
            },
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            409,
            "Email exists",
            "Email already exists",
        ));
    }

    let password_hash = match bcrypt::hash(password, bcrypt::DEFAULT_COST) {
        Ok(value) => value,
        Err(_) => {
            return auth_rest_error_response(AuthRestError::new(
                500,
                "Internal Server Error",
                "Rust auth register runtime password hashing error",
            ))
        }
    };
    let language = body
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("zh_Hans")
        .to_string();
    let default_currency = body
        .get("defaultCurrency")
        .and_then(Value::as_str)
        .unwrap_or("CNY")
        .to_string();
    let first_day_of_week = body
        .get("firstDayOfWeek")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let nickname = body
        .get("nickname")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let nickname = if nickname.is_empty() {
        username.clone()
    } else {
        nickname
    };
    let created_at = utc_now_text();
    let register_result = match create_registered_user_with_defaults(
        runtime.connection(),
        &RegisterUserDraft {
            username: username.clone(),
            email: email.clone(),
            password_hash,
            nickname,
            language,
            default_currency,
            first_day_of_week,
            email_verified: !state.config.auth_require_email_verification,
            created_at: created_at.clone(),
        },
        &register_preset_categories_from_body(&body),
        &AuthLogDraft {
            user_id: None,
            username: username.clone(),
            event_type: "register_success".to_string(),
            ip_address: ip_address.clone(),
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: None,
            created_at,
        },
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    success_result(
        StatusCode::OK,
        json!({
            "user_id": register_result.user_id,
            "username": username,
            "email": email,
            "needVerifyEmail": state.config.auth_require_email_verification,
            "presetCategoriesSaved": register_result.preset_categories_saved,
            "presetAccountsSaved": register_result.preset_accounts_saved,
            "message": "Registration successful",
        }),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn generate_api_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "generate_api_token_handler", "business operation entered");
    generate_personal_token(
        TokenKind::Api,
        state,
        headers,
        connect_info.map(|ConnectInfo(addr)| addr),
        body,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn generate_mcp_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "generate_mcp_token_handler", "business operation entered");
    generate_personal_token(
        TokenKind::Mcp,
        state,
        headers,
        connect_info.map(|ConnectInfo(addr)| addr),
        body,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn login_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "login_handler", "business operation entered");
    let body = request_body_object(&body);
    let login_name = body
        .get("loginName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if login_name.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Username and password are required",
        ));
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));

    let user = match get_login_user_by_login_name(runtime.connection(), &login_name) {
        Ok(Some(value)) => value,
        Ok(None) => {
            if log_auth_event(
                runtime.connection(),
                AuthEvent {
                    user_id: None,
                    username: &login_name,
                    event_type: "login_failed",
                    ip_address: &ip_address,
                    user_agent: &request_user_agent,
                    success: false,
                    error_message: Some("User not found".to_string()),
                    metadata: None,
                },
            )
            .is_err()
            {
                return db_error_response();
            }
            return invalid_login_credentials_response();
        }
        Err(_) => return db_error_response(),
    };

    if login_lock_is_active(&user.locked_until) {
        if log_login_failure(
            runtime.connection(),
            &user,
            &ip_address,
            &request_user_agent,
            "Account locked",
        )
        .is_err()
        {
            return db_error_response();
        }
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Account locked",
            "Account is temporarily locked due to multiple failed login attempts",
        ));
    }
    let expired_locked_until = if user.locked_until.trim().is_empty() {
        None
    } else {
        Some(user.locked_until.as_str())
    };

    if !bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        let lockout_until = login_lockout_until_text(state.config.auth_lockout_duration_minutes);
        if increment_failed_login(
            runtime.connection(),
            user.profile.id,
            state.config.auth_max_login_attempts,
            &lockout_until,
            expired_locked_until,
        )
        .is_err()
            || log_login_failure(
                runtime.connection(),
                &user,
                &ip_address,
                &request_user_agent,
                "Invalid password",
            )
            .is_err()
        {
            return db_error_response();
        }
        return invalid_login_credentials_response();
    }

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

    if user.two_factor_enabled {
        let pending_token = match issue_action_token(&user, "pending_2fa", 1, &state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        if log_auth_event(
            runtime.connection(),
            AuthEvent {
                user_id: Some(user.profile.id),
                username: &user.profile.username,
                event_type: "login_2fa_pending",
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
        return success_result(
            StatusCode::OK,
            json!({
                "token": pending_token,
                "need2FA": true,
            }),
        );
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
    let now = utc_now_text();
    let session_draft = CreateTokenSessionDraft {
        user_id: user.profile.id,
        token_hash: sha256_hex(&tokens.access_token),
        refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
        expires_at: tokens.expires_at.clone(),
        refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
        user_agent: request_user_agent.clone(),
        ip_address: ip_address.clone(),
        created_at: now.clone(),
    };
    if persist_login_success(
        runtime.connection(),
        &session_draft,
        user.profile.id,
        &user.profile.username,
        &now,
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

#[tracing::instrument(level = "debug", skip_all)]
async fn refresh_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "refresh_token_handler", "business operation entered");
    let body = request_body_object(&body);
    let refresh_token = body
        .get("refreshToken")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if refresh_token.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Refresh token is required",
        ));
    }

    let claims = match validate_refresh_jwt(refresh_token, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let refresh_token_hash = sha256_hex(refresh_token);
    let refresh_session =
        match get_active_refresh_session(runtime.connection(), &refresh_token_hash) {
            Ok(Some(value)) => value,
            Ok(None) => return invalid_refresh_token_response(),
            Err(_) => return db_error_response(),
        };
    if refresh_session.user_id != claims.user_id || !refresh_session.user_is_active {
        return invalid_refresh_token_response();
    }
    if refresh_session_is_expired(&refresh_session.refresh_expires_at) {
        return refresh_token_expired_response();
    }

    let user = match get_auth_user_profile(runtime.connection(), claims.user_id) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User does not exist",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let tokens = match issue_session_tokens(user.id, &user.username, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let session_id = match rotate_refresh_token_session(
        runtime.connection(),
        refresh_session.id,
        &refresh_token_hash,
        &CreateTokenSessionDraft {
            user_id: user.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent,
            ip_address,
            created_at: now_text(),
        },
    ) {
        Ok(Some(value)) => value,
        Ok(None) => return invalid_refresh_token_response(),
        Err(_) => return db_error_response(),
    };
    if session_id <= 0 {
        return db_error_response();
    }
    let cloud_settings = match list_application_cloud_settings(runtime.connection(), user.id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };

    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "newToken": tokens.access_token,
            "user": user_profile_payload(&user),
            "applicationCloudSettings": application_cloud_settings_payload(cloud_settings),
        }),
    )
}

