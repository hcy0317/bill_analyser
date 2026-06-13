// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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
    let default_package = match register_default_package_from_body(&body) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };

    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));

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

        return register_postgres_response(
            &state,
            &body,
            username,
            email,
            password_hash,
            nickname,
            language,
            default_currency,
            first_day_of_week,
            request_user_agent,
            ip_address,
            created_at,
            default_package,
        )
        .await;
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

    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));


        return login_postgres_response(
            &state,
            &login_name,
            password,
            &request_user_agent,
            &ip_address,
        )
        .await;
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

        return refresh_postgres_response(
            &state,
            claims.user_id,
            refresh_token,
            &headers,
            connect_info.map(|ConnectInfo(addr)| addr),
        )
        .await;
}

#[allow(clippy::too_many_arguments)]
async fn register_postgres_response(
    state: &HttpAppState,
    body: &Map<String, Value>,
    username: String,
    email: String,
    password_hash: String,
    nickname: String,
    language: String,
    default_currency: String,
    first_day_of_week: i64,
    request_user_agent: String,
    ip_address: String,
    created_at: String,
    default_package: RegisterDefaultSeedPackage,
) -> Response {
    let runtime = match open_postgres_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match postgres_auth_username_exists(runtime.pool(), &username).await {
        Ok(true) => {
            let _ = create_postgres_auth_log(
                runtime.pool(),
                &AuthLogDraft {
                    user_id: None,
                    username: username.clone(),
                    event_type: "register_failed".to_string(),
                    ip_address,
                    user_agent: request_user_agent,
                    success: false,
                    error_message: Some("Username already exists".to_string()),
                    metadata: None,
                    created_at,
                },
            )
            .await;
            return auth_rest_error_response(AuthRestError::new(
                409,
                "Username exists",
                "Username already exists",
            ));
        }
        Ok(false) => {}
        Err(_) => return db_error_response(),
    }
    match postgres_auth_email_exists(runtime.pool(), &email).await {
        Ok(true) => {
            let _ = create_postgres_auth_log(
                runtime.pool(),
                &AuthLogDraft {
                    user_id: None,
                    username: username.clone(),
                    event_type: "register_failed".to_string(),
                    ip_address,
                    user_agent: request_user_agent,
                    success: false,
                    error_message: Some("Email already exists".to_string()),
                    metadata: None,
                    created_at,
                },
            )
            .await;
            return auth_rest_error_response(AuthRestError::new(
                409,
                "Email exists",
                "Email already exists",
            ));
        }
        Ok(false) => {}
        Err(_) => return db_error_response(),
    }

    let register_result = match create_postgres_registered_user_with_defaults(
        runtime.pool(),
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
        &register_preset_categories_from_body(body),
        default_package,
        &AuthLogDraft {
            user_id: None,
            username: username.clone(),
            event_type: "register_success".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: None,
            created_at,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let mut result = json!({
        "user_id": register_result.user_id,
        "username": username,
        "email": email,
        "needVerifyEmail": state.config.auth_require_email_verification,
        "presetCategoriesSaved": register_result.preset_categories_saved,
        "presetAccountsSaved": register_result.preset_accounts_saved,
        "message": "Registration successful",
    });
    if let Some(default_seed) = register_default_seed_response(&register_result.default_seed) {
        if let Some(object) = result.as_object_mut() {
            object.insert("defaultSeed".to_string(), default_seed);
        }
    }
    success_result(
        StatusCode::OK,
        result,
    )
}

fn register_default_seed_response(summary: &RegisterDefaultSeedSummary) -> Option<Value> {
    let package = summary.package.as_ref()?;
    Some(json!({
        "package": package,
        "categoriesCreated": summary.categories_created,
        "categoriesSkipped": summary.categories_skipped,
        "categoryRulesCreated": summary.rules_created,
        "categoryRulesSkipped": summary.rules_skipped,
        "accountsCreated": summary.accounts_created,
        "accountsSkipped": summary.accounts_skipped,
        "accountRulesCreated": summary.account_rules_created,
        "accountRulesSkipped": summary.account_rules_skipped,
        "rulesMissingTargets": summary.rules_missing_targets,
    }))
}

async fn login_postgres_response(
    state: &HttpAppState,
    login_name: &str,
    password: &str,
    request_user_agent: &str,
    ip_address: &str,
) -> Response {
    let runtime = match open_postgres_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_login_name(runtime.pool(), login_name).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            let _ = create_postgres_auth_log(
                runtime.pool(),
                &AuthLogDraft {
                    user_id: None,
                    username: login_name.to_string(),
                    event_type: "login_failed".to_string(),
                    ip_address: ip_address.to_string(),
                    user_agent: request_user_agent.to_string(),
                    success: false,
                    error_message: Some("User not found".to_string()),
                    metadata: None,
                    created_at: utc_now_text(),
                },
            )
            .await;
            return invalid_login_credentials_response();
        }
        Err(_) => return db_error_response(),
    };

    if login_lock_is_active(&user.locked_until) {
        let _ = create_postgres_auth_log(
            runtime.pool(),
            &AuthLogDraft {
                user_id: Some(user.profile.id),
                username: user.profile.username.clone(),
                event_type: "login_failed".to_string(),
                ip_address: ip_address.to_string(),
                user_agent: request_user_agent.to_string(),
                success: false,
                error_message: Some("Account locked".to_string()),
                metadata: None,
                created_at: utc_now_text(),
            },
        )
        .await;
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
        if increment_postgres_failed_login(
            runtime.pool(),
            user.profile.id,
            state.config.auth_max_login_attempts,
            &lockout_until,
            expired_locked_until,
        )
        .await
        .is_err()
            || create_postgres_auth_log(
                runtime.pool(),
                &AuthLogDraft {
                    user_id: Some(user.profile.id),
                    username: user.profile.username.clone(),
                    event_type: "login_failed".to_string(),
                    ip_address: ip_address.to_string(),
                    user_agent: request_user_agent.to_string(),
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
        return invalid_login_credentials_response();
    }

    if !user.is_active {
        let _ = create_postgres_auth_log(
            runtime.pool(),
            &AuthLogDraft {
                user_id: Some(user.profile.id),
                username: user.profile.username.clone(),
                event_type: "login_failed".to_string(),
                ip_address: ip_address.to_string(),
                user_agent: request_user_agent.to_string(),
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

    if user.two_factor_enabled {
        let pending_token = match issue_action_token(&user, "pending_2fa", 1, state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let _ = create_postgres_auth_log(
            runtime.pool(),
            &AuthLogDraft {
                user_id: Some(user.profile.id),
                username: user.profile.username.clone(),
                event_type: "login_2fa_pending".to_string(),
                ip_address: ip_address.to_string(),
                user_agent: request_user_agent.to_string(),
                success: true,
                error_message: None,
                metadata: None,
                created_at: utc_now_text(),
            },
        )
        .await;
        return success_result(
            StatusCode::OK,
            json!({
                "token": pending_token,
                "need2FA": true,
            }),
        );
    }

    let cloud_settings =
        match list_postgres_application_cloud_settings(runtime.pool(), user.profile.id).await {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let tokens = match issue_session_tokens(user.profile.id, &user.profile.username, state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let now = utc_now_text();
    let session_id = match create_postgres_token_session(
        runtime.pool(),
        &CreateTokenSessionDraft {
            user_id: user.profile.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent.to_string(),
            ip_address: ip_address.to_string(),
            created_at: now.clone(),
        },
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if update_postgres_user_last_login(runtime.pool(), user.profile.id, &now, ip_address)
        .await
        .is_err()
        || create_postgres_auth_log(
            runtime.pool(),
            &AuthLogDraft {
                user_id: Some(user.profile.id),
                username: user.profile.username.clone(),
                event_type: "login_success".to_string(),
                ip_address: ip_address.to_string(),
                user_agent: request_user_agent.to_string(),
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

async fn refresh_postgres_response(
    state: &HttpAppState,
    user_id: UserId,
    refresh_token: &str,
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
) -> Response {
    let runtime = match open_postgres_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let refresh_token_hash = sha256_hex(refresh_token);
    let refresh_session =
        match get_postgres_active_refresh_session(runtime.pool(), &refresh_token_hash).await {
            Ok(Some(value)) => value,
            Ok(None) => return invalid_refresh_token_response(),
            Err(_) => return db_error_response(),
        };
    if refresh_session.user_id != user_id || !refresh_session.user_is_active {
        return invalid_refresh_token_response();
    }
    if refresh_session_is_expired(&refresh_session.refresh_expires_at) {
        return refresh_token_expired_response();
    }
    let user = match get_postgres_auth_user_profile(runtime.pool(), user_id).await {
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
    let tokens = match issue_session_tokens(user.id, &user.username, state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(headers, peer_addr);
    let session_id = match rotate_postgres_refresh_token_session(
        runtime.pool(),
        refresh_session.id,
        &refresh_token_hash,
        &CreateTokenSessionDraft {
            user_id: user.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent.clone(),
            ip_address: ip_address.clone(),
            created_at: now_text(),
        },
    )
    .await
    {
        Ok(Some(value)) => value,
        Ok(None) => return invalid_refresh_token_response(),
        Err(_) => return db_error_response(),
    };
    if session_id <= 0 {
        return db_error_response();
    }
    let _ = create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.id),
            username: user.username.clone(),
            event_type: "token_refresh".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "session_id": session_id }).to_string()),
            created_at: utc_now_text(),
        },
    )
    .await;
    let cloud_settings = match list_postgres_application_cloud_settings(runtime.pool(), user.id).await
    {
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
