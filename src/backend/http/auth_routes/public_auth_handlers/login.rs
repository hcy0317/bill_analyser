// 中文说明：处理公开登录请求，解析账号密码和 2FA 字段后交给 PostgreSQL 登录响应编排。
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


// 中文说明：校验密码、登录失败计数、锁定状态和 2FA 分支，并在成功时创建 token session。
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
