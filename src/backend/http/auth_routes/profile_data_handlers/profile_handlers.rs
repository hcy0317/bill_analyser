// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

async fn get_profile_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_auth_user_profile(runtime.connection(), auth.user_id) {
        Ok(Some(user)) => success_result(StatusCode::OK, user_profile_payload(&user)),
        Ok(None) => {
            auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
        }
        Err(_) => db_error_response(),
    }
}

async fn update_profile_handler(
    State(state): State<HttpAppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    if body.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Request body is required",
        ));
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let current_user = match get_auth_user_profile(runtime.connection(), auth.user_id) {
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
    let updates =
        match validated_profile_updates(runtime.connection(), auth.user_id, &current_user, &body) {
            Ok(value) => value,
            Err(error) => return auth_rest_error_response(error),
        };
    let email_changed = profile_email_changed(&updates, &current_user.email);
    let updated_at = utc_now_text();
    let update_result = if email_changed.is_some() {
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
        update_auth_user_profile_with_auth_log(
            runtime.connection(),
            auth.user_id,
            &updates,
            &updated_at,
            &AuthLogDraft {
                user_id: Some(auth.user_id),
                username: current_user.username.clone(),
                event_type: "profile_email_changed".to_string(),
                ip_address,
                user_agent: request_user_agent,
                success: true,
                error_message: None,
                metadata: Some(
                    json!({
                        "email_changed": true,
                        "email_verified_reset": true
                    })
                    .to_string(),
                ),
                created_at: updated_at.clone(),
            },
        )
    } else {
        update_auth_user_profile(runtime.connection(), auth.user_id, &updates, &updated_at)
    };
    if !updates.is_empty() && !matches!(update_result, Ok(true)) {
        return auth_rest_error_response(AuthRestError::new(
            500,
            "Update failed",
            "Failed to update user profile",
        ));
    }

    match get_auth_user_profile(runtime.connection(), auth.user_id) {
        Ok(Some(user)) => success_result(
            StatusCode::OK,
            json!({ "user": user_profile_payload(&user) }),
        ),
        Ok(None) => auth_rest_error_response(AuthRestError::new(
            404,
            "User not found after update",
            "User not found after update",
        )),
        Err(_) => db_error_response(),
    }
}

async fn update_profile_avatar_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let avatar = match avatar_data_url_from_multipart(&headers, &body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    update_profile_avatar_value(&state, auth.user_id, avatar).await
}

async fn remove_profile_avatar_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    update_profile_avatar_value(&state, auth.user_id, String::new()).await
}

async fn update_profile_avatar_value(
    state: &HttpAppState,
    user_id: UserId,
    avatar: String,
) -> Response {
    let runtime = match open_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let updates = [AuthUserProfileUpdate::Avatar(avatar)];
    if !matches!(
        update_auth_user_profile(runtime.connection(), user_id, &updates, &utc_now_text()),
        Ok(true)
    ) {
        return auth_rest_error_response(AuthRestError::new(
            500,
            "Update failed",
            "Failed to update avatar",
        ));
    }
    match get_auth_user_profile(runtime.connection(), user_id) {
        Ok(Some(user)) => success_result(StatusCode::OK, user_profile_payload(&user)),
        Ok(None) => {
            auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
        }
        Err(_) => db_error_response(),
    }
}

async fn resend_profile_verification_email_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_auth_user_profile(runtime.connection(), auth.user_id) {
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
    if user.email.trim().is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Email is required to resend verification email",
        ));
    }
    let since = (Utc::now() - ChronoDuration::minutes(PROFILE_VERIFICATION_RESEND_WINDOW_MINUTES))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let created_at = utc_now_text();
    match create_auth_log_under_event_limit(
        runtime.connection(),
        auth.user_id,
        "verification_email_resend_requested",
        &since,
        PROFILE_VERIFICATION_RESEND_LIMIT,
        &AuthLogDraft {
            user_id: Some(user.id),
            username: user.username.clone(),
            event_type: "verification_email_resend_requested".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(
                json!({
                    "email_present": true,
                    "email_verified": user.email_verified,
                    "require_email_verification": state.config.auth_require_email_verification,
                    "delivery": "not_configured_mock_success"
                })
                .to_string(),
            ),
            created_at,
        },
    ) {
        Ok(true) => {}
        Ok(false) => {
            return auth_rest_error_response(AuthRestError::new(
                429,
                "Too Many Requests",
                "Too many verification email resend requests",
            ));
        }
        Err(_) => return db_error_response(),
    }
    success_result(StatusCode::OK, Value::Bool(true))
}

async fn get_profile_cloud_settings_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match list_application_cloud_settings(runtime.connection(), auth.user_id) {
        Ok(settings) if settings.is_empty() => success_result(StatusCode::OK, Value::Bool(false)),
        Ok(settings) => {
            success_result(StatusCode::OK, application_cloud_settings_payload(settings))
        }
        Err(_) => db_error_response(),
    }
}

async fn update_profile_cloud_settings_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let empty_settings = Vec::new();
    let settings = match body.get("settings") {
        Some(Value::Array(values)) => values,
        Some(_) => {
            return auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "settings must be an array",
            ));
        }
        None => &empty_settings,
    };
    let mut drafts = Vec::with_capacity(settings.len());
    for setting in settings {
        match validate_application_cloud_setting(setting) {
            Ok(value) => drafts.push(value),
            Err(error) => return auth_rest_error_response(error),
        }
    }
    let full_update = body
        .get("fullUpdate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match update_application_cloud_settings(
        runtime.connection(),
        auth.user_id,
        &drafts,
        full_update,
        &utc_now_text(),
    ) {
        Ok(_) => success_result(StatusCode::OK, Value::Bool(true)),
        Err(_) => db_error_response(),
    }
}

async fn delete_profile_cloud_settings_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match delete_application_cloud_settings(runtime.connection(), auth.user_id) {
        Ok(_) => success_result(StatusCode::OK, Value::Bool(true)),
        Err(_) => db_error_response(),
    }
}

async fn list_profile_external_auths_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let rows = match list_user_external_auths(runtime.connection(), auth.user_id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let mut result: Vec<ExternalAuthInfo> =
        rows.into_iter().map(ExternalAuthInfo::linked).collect();

    let oauth2_provider = state.config.auth_oauth2_provider.trim();
    if state.config.auth_enable_oauth2
        && !oauth2_provider.is_empty()
        && !result
            .iter()
            .any(|item| item.external_auth_type == oauth2_provider)
    {
        result.push(ExternalAuthInfo {
            external_auth_category: "oauth2".to_string(),
            external_auth_type: oauth2_provider.to_string(),
            linked: false,
            external_username: String::new(),
            created_at: 0,
        });
    }

    result.sort_by(|left, right| {
        right
            .linked
            .cmp(&left.linked)
            .then_with(|| left.external_auth_type.cmp(&right.external_auth_type))
            .then_with(|| right.created_at.cmp(&left.created_at))
    });

    success_result(StatusCode::OK, external_auths_payload(result))
}
