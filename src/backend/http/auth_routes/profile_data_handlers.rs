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

async fn authorize_oauth2_callback_handler(State(state): State<HttpAppState>) -> Response {
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

async fn verify_email_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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

async fn resend_public_verification_email_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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

async fn forgot_password_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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

async fn reset_password_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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

async fn unlink_profile_external_auth_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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

async fn system_version_handler() -> Response {
    success_result(
        StatusCode::OK,
        json!({
            "version": BILL_ANALYSER_APP_VERSION,
            "commitHash": "",
            "buildTime": ""
        }),
    )
}

async fn get_user_data_statistics_handler(
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
    match get_db_user_data_statistics(runtime.connection(), auth.user_id) {
        Ok(statistics) => json_response(StatusCode::OK, user_data_statistics_response(&statistics)),
        Err(_) => db_error_response(),
    }
}

async fn export_user_data_handler(
    State(state): State<HttpAppState>,
    Path(file_type): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let export_type = match normalize_user_data_export_type(&file_type) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let categories = match list_user_data_categories(runtime.connection(), auth.user_id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let filters = build_user_data_export_filters(&query, &categories);
    let bundle = match load_user_data_export(runtime.connection(), auth.user_id, &filters) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let export_text = match render_user_data_export(&bundle, export_type.delimiter()) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let filename = format!(
        "bill_analyser_export_{}.{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        export_type.extension()
    );
    let mut response = Response::new(Body::from(format!("\u{feff}{export_text}")));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(export_type.content_type()),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename={filename}")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

async fn clear_user_transactions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    clear_user_data_handler(
        state,
        headers,
        connect_info,
        body,
        UserDataClearKind::Transactions,
    )
}

async fn clear_all_user_data_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    clear_user_data_handler(state, headers, connect_info, body, UserDataClearKind::All)
}

fn clear_user_data_handler(
    state: HttpAppState,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
    kind: UserDataClearKind,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let mut runtime = match open_runtime(&state) {
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
        USER_DATA_CLEAR_AUTH_FAILED_EVENT,
    ) {
        return *response;
    }
    let auth_mode =
        match resolve_destructive_user_data_auth(runtime.connection(), &body, &state, &user) {
            Ok(value) => value,
            Err(SensitiveTwoFactorAuthError::Missing) => {
                if let Err(response) = record_sensitive_auth_failure(
                    runtime.connection(),
                    &user,
                    USER_DATA_CLEAR_AUTH_FAILED_EVENT,
                    &ip_address,
                    &request_user_agent,
                    "Missing current password or step-up token",
                    Some(json!({
                        "operation_type": kind.operation_type(),
                        "reason": "missing_credentials"
                    })),
                ) {
                    return *response;
                }
                return sensitive_auth_missing_response();
            }
            Err(SensitiveTwoFactorAuthError::Invalid) => {
                if let Err(response) = record_sensitive_auth_failure(
                    runtime.connection(),
                    &user,
                    USER_DATA_CLEAR_AUTH_FAILED_EVENT,
                    &ip_address,
                    &request_user_agent,
                    "Invalid current password or step-up token",
                    Some(json!({
                        "operation_type": kind.operation_type(),
                        "reason": "invalid_credentials"
                    })),
                ) {
                    return *response;
                }
                return sensitive_auth_invalid_response();
            }
            Err(SensitiveTwoFactorAuthError::Db) => return db_error_response(),
        };
    let now = utc_now_text();
    match kind {
        UserDataClearKind::Transactions => {
            let deleted_count =
                match clear_user_transactions(runtime.connection_mut(), auth.user_id) {
                    Ok(value) => value,
                    Err(_) => return db_error_response(),
                };
            create_user_data_audit_log_best_effort(
                runtime.connection(),
                UserDataAuditLogDraft {
                    operation_type: kind.operation_type(),
                    user_id: auth.user_id,
                    details: json!({
                        "deleted_count": deleted_count,
                        "auth_mode": auth_mode.as_str()
                    }),
                    affected_count: deleted_count,
                    ip_address: &ip_address,
                    user_agent: &request_user_agent,
                    now: &now,
                },
            );
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": true,
                    "deletedCount": deleted_count
                }),
            )
        }
        UserDataClearKind::All => {
            let result = match clear_user_data(runtime.connection_mut(), auth.user_id) {
                Ok(value) => value,
                Err(_) => return db_error_response(),
            };
            let affected_count = result.counts.values().copied().sum::<i64>();
            let details = user_data_clear_all_audit_details(&result.counts, auth_mode.as_str());
            create_user_data_audit_log_best_effort(
                runtime.connection(),
                UserDataAuditLogDraft {
                    operation_type: kind.operation_type(),
                    user_id: auth.user_id,
                    details,
                    affected_count,
                    ip_address: &ip_address,
                    user_agent: &request_user_agent,
                    now: &now,
                },
            );
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": true,
                    "counts": result.counts
                }),
            )
        }
    }
}

