async fn get_two_factor_status_handler(
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

async fn verify_security_step_up_handler(
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

async fn verify_two_factor_login_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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

async fn request_two_factor_enable_handler(
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
    let user = match get_login_user_by_id(runtime.connection(), auth.user_id) {
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

async fn confirm_two_factor_enable_handler(
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

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_id(runtime.connection(), auth.user_id) {
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
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
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
    let persist_result = enable_two_factor_with_recovery_codes_and_session(
        runtime.connection(),
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
    );
    let (stored_count, _session_id) = match persist_result {
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
    create_user_audit_log_best_effort(
        runtime.connection(),
        UserAuditLogDraft {
            operation_type: "2fa_enabled",
            user_id: user.profile.id,
            details: json!({ "recovery_code_count": stored_count }),
            affected_count: 1_i64,
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            now: &now,
        },
    );

    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "recoveryCodes": recovery_codes
        }),
    )
}

async fn disable_two_factor_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_id(runtime.connection(), auth.user_id) {
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
        match resolve_sensitive_two_factor_auth(runtime.connection(), &body, &state, &user) {
            Ok(value) => value,
            Err(SensitiveTwoFactorAuthError::Missing) => return sensitive_auth_missing_response(),
            Err(SensitiveTwoFactorAuthError::Invalid) => return sensitive_auth_invalid_response(),
            Err(SensitiveTwoFactorAuthError::Db) => return db_error_response(),
        };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    let now = now_text();
    let cleared_count = match disable_two_factor_and_clear_recovery_codes(
        runtime.connection(),
        user.profile.id,
        &now,
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    create_user_audit_log_best_effort(
        runtime.connection(),
        UserAuditLogDraft {
            operation_type: "2fa_disabled",
            user_id: user.profile.id,
            details: json!({
                "cleared_recovery_code_count": cleared_count,
                "auth_mode": auth_mode.as_str()
            }),
            affected_count: 1_i64,
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            now: &now,
        },
    );

    success_result(StatusCode::OK, Value::Bool(true))
}

async fn regenerate_two_factor_recovery_codes_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_login_user_by_id(runtime.connection(), auth.user_id) {
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
        match resolve_sensitive_two_factor_auth(runtime.connection(), &body, &state, &user) {
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
    let stored_count = match bill_analyser_db::replace_two_factor_recovery_codes(
        runtime.connection(),
        user.profile.id,
        &recovery_code_refs,
        &now,
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if stored_count != recovery_codes.len() {
        return db_error_response();
    }

    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    create_user_audit_log_best_effort(
        runtime.connection(),
        UserAuditLogDraft {
            operation_type: "2fa_recovery_regenerated",
            user_id: user.profile.id,
            details: json!({
                "recovery_code_count": stored_count,
                "auth_mode": auth_mode.as_str()
            }),
            affected_count: 1_i64,
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            now: &now,
        },
    );

    success_result(
        StatusCode::OK,
        json!({
            "recoveryCodes": recovery_codes
        }),
    )
}

async fn verify_two_factor_recovery_login_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
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
    if !user.two_factor_enabled {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Two-factor authentication is not enabled",
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
    let persist_result = persist_two_factor_recovery_login_success(
        runtime.connection(),
        TwoFactorRecoveryLoginDraft {
            recovery_code,
            session_draft: &CreateTokenSessionDraft {
                user_id: user.profile.id,
                token_hash: sha256_hex(&tokens.access_token),
                refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
                expires_at: tokens.expires_at.clone(),
                refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
                user_agent: request_user_agent.clone(),
                ip_address: ip_address.clone(),
                created_at: now.clone(),
            },
            user_id: user.profile.id,
            username: &user.profile.username,
            ip_address: &ip_address,
            user_agent: &request_user_agent,
            now: &now,
        },
    );
    match persist_result {
        Ok(Some(_session_id)) => {}
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                401,
                "Invalid recovery code",
                "Recovery code is invalid or already used",
            ));
        }
        Err(_) => return db_error_response(),
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

