// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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
