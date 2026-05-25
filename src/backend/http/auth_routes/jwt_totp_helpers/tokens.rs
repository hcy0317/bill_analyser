// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn invalid_login_credentials_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Invalid credentials",
        "Invalid username or password",
    ))
}

fn log_login_failure(
    connection: &rusqlite::Connection,
    user: &AuthLoginUserRow,
    ip_address: &str,
    user_agent: &str,
    error_message: &str,
) -> bill_analyser_db::DbResult<i64> {
    log_auth_event(
        connection,
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type: "login_failed",
            ip_address,
            user_agent,
            success: false,
            error_message: Some(error_message.to_string()),
            metadata: None,
        },
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn login_lock_is_active(locked_until: &str) -> bool {
    let value = locked_until.trim();
    if value.is_empty() {
        return false;
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|datetime| Utc::now().naive_utc() < datetime)
        .unwrap_or(false)
}

struct IssuedAccessToken {
    access_token: String,
    expires_at: String,
}

struct IssuedSessionTokens {
    access_token: String,
    refresh_token: String,
    expires_at: String,
    refresh_expires_at: String,
}

#[tracing::instrument(level = "debug", skip_all)]
fn validate_refresh_jwt(
    token: &str,
    state: &HttpAppState,
) -> RouteResult<bill_analyser_core::auth::RefreshTokenClaims> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or_else(invalid_refresh_token_box)?;
    let encoded_payload = parts.next().ok_or_else(invalid_refresh_token_box)?;
    let encoded_signature = parts.next().ok_or_else(invalid_refresh_token_box)?;
    if parts.next().is_some() {
        return Err(invalid_refresh_token_box());
    }

    let header = decode_jwt_part(encoded_header)?;
    let payload = decode_jwt_part(encoded_payload)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err(invalid_refresh_token_box());
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    verify_hmac_signature(
        secret,
        hmac_algorithm,
        signing_input.as_bytes(),
        encoded_signature,
    )?;
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_refresh_token_box)?;
    if exp <= Utc::now().timestamp() {
        return Err(Box::new(refresh_token_expired_response()));
    }
    validate_refresh_token_claims(&payload)
        .map_err(|error| Box::new(auth_rest_error_response(error)))
}

#[tracing::instrument(level = "debug", skip_all)]
fn validate_action_jwt(
    token: &str,
    state: &HttpAppState,
    expected_type: &str,
    invalid_message: &'static str,
) -> RouteResult<Value> {
    validate_action_jwt_with_invalid(token, state, expected_type, || {
        invalid_action_token_box(invalid_message)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn validate_pending_two_factor_jwt(token: &str, state: &HttpAppState) -> RouteResult<Value> {
    validate_action_jwt_with_invalid(token, state, "pending_2fa", invalid_pending_two_factor_box)
}

#[tracing::instrument(level = "debug", skip_all)]
fn validate_action_jwt_with_invalid<F>(
    token: &str,
    state: &HttpAppState,
    expected_type: &str,
    invalid_response: F,
) -> RouteResult<Value>
where
    F: Fn() -> Box<Response> + Copy,
{
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or_else(invalid_response)?;
    let encoded_payload = parts.next().ok_or_else(invalid_response)?;
    let encoded_signature = parts.next().ok_or_else(invalid_response)?;
    if parts.next().is_some() {
        return Err(invalid_response());
    }

    let header = decode_action_jwt_part(encoded_header, invalid_response)?;
    let payload = decode_action_jwt_part(encoded_payload, invalid_response)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err(invalid_response());
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| invalid_response())?;
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input.as_bytes(), &signature).map_err(|_| invalid_response())?;
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_response)?;
    if exp <= Utc::now().timestamp() {
        return Err(invalid_response());
    }
    if payload.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err(invalid_response());
    }
    Ok(payload)
}

fn decode_action_jwt_part<F>(encoded: &str, invalid_response: F) -> RouteResult<Value>
where
    F: Fn() -> Box<Response> + Copy,
{
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| invalid_response())?;
    serde_json::from_slice(&decoded).map_err(|_| invalid_response())
}

fn action_user_id(payload: &Value, invalid_message: &'static str) -> RouteResult<UserId> {
    action_user_id_with_invalid(payload, || invalid_action_token_box(invalid_message))
}

fn pending_two_factor_user_id(payload: &Value) -> RouteResult<UserId> {
    action_user_id_with_invalid(payload, invalid_pending_two_factor_box)
}

fn action_user_id_with_invalid<F>(payload: &Value, invalid_response: F) -> RouteResult<UserId>
where
    F: Fn() -> Box<Response> + Copy,
{
    let raw_user_id = payload
        .get("user_id")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_response)?;
    UserId::new(raw_user_id).map_err(|_| invalid_response())
}

#[tracing::instrument(level = "debug", skip_all)]
fn issue_session_tokens(
    user_id: UserId,
    username: &str,
    state: &HttpAppState,
) -> RouteResult<IssuedSessionTokens> {
    let now = Local::now();
    let access_expires_at = now + ChronoDuration::days(state.config.auth_jwt_expiration_days);
    let refresh_expires_at =
        now + ChronoDuration::days(state.config.auth_refresh_token_expiration_days);
    let access_payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "iat": now.timestamp(),
        "exp": access_expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });
    let refresh_payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "refresh",
        "iat": now.timestamp(),
        "exp": refresh_expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });

    Ok(IssuedSessionTokens {
        access_token: sign_jwt(&access_payload, state)?,
        refresh_token: sign_jwt(&refresh_payload, state)?,
        expires_at: access_expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
        refresh_expires_at: refresh_expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn issue_action_token(
    user: &AuthLoginUserRow,
    token_type: &str,
    expires_in_hours: i64,
    state: &HttpAppState,
) -> RouteResult<String> {
    let now = Local::now();
    let expires_at = now + ChronoDuration::hours(expires_in_hours);
    let payload = json!({
        "user_id": user.profile.id.get(),
        "username": user.profile.username.clone(),
        "email": user.profile.email.clone(),
        "type": token_type,
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": random_nonce_hex()?,
    });
    sign_jwt(&payload, state)
}

#[tracing::instrument(level = "debug", skip_all)]
fn issue_access_token(
    user_id: bill_analyser_core::UserId,
    username: &str,
    state: &HttpAppState,
    token_kind: TokenKind,
    expires_in_seconds: i64,
) -> RouteResult<IssuedAccessToken> {
    let now = Local::now();
    let expires_at = if expires_in_seconds > 0 {
        now + ChronoDuration::seconds(expires_in_seconds)
    } else {
        now + ChronoDuration::days(365 * 100)
    };
    let nonce = random_nonce_hex()?;
    let payload = json!({
        "user_id": user_id.get(),
        "username": username,
        "type": "access",
        "token_kind": token_kind.as_str(),
        "iat": now.timestamp(),
        "exp": expires_at.timestamp(),
        "nonce": nonce,
    });
    Ok(IssuedAccessToken {
        access_token: sign_jwt(&payload, state)?,
        expires_at: expires_at
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string(),
    })
}

fn sign_jwt(payload: &Value, state: &HttpAppState) -> RouteResult<String> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Box::new(auth_rest_error_response(AuthRestError::new(
                503,
                "Service Unavailable",
                "Rust auth token runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )))
        })?;
    let algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let hmac_algorithm = jwt_hmac_algorithm(&algorithm).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.message,
        )))
    })?;
    let header = json!({ "alg": algorithm, "typ": "JWT" });
    let encoded_header = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header).map_err(|_| Box::new(db_error_response()))?);
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(payload).map_err(|_| Box::new(db_error_response()))?);
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    Ok(format!("{signing_input}.{encoded_signature}"))
}

fn decode_jwt_part(encoded: &str) -> RouteResult<Value> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| invalid_refresh_token_box())?;
    serde_json::from_slice(&decoded).map_err(|_| invalid_refresh_token_box())
}

#[tracing::instrument(level = "debug", skip_all)]
fn verify_hmac_signature(
    secret: &str,
    algorithm: hmac::Algorithm,
    signing_input: &[u8],
    encoded_signature: &str,
) -> RouteResult<()> {
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| invalid_refresh_token_box())?;
    let key = hmac::Key::new(algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input, &signature).map_err(|_| invalid_refresh_token_box())
}

fn invalid_refresh_token_box() -> Box<Response> {
    Box::new(invalid_refresh_token_response())
}

fn invalid_action_token_box(message: &'static str) -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::new(
        400,
        "Invalid token",
        message,
    )))
}

fn invalid_pending_two_factor_box() -> Box<Response> {
    Box::new(auth_rest_error_response(AuthRestError::new(
        401,
        "Unauthorized",
        "Invalid or expired 2FA token",
    )))
}

fn invalid_refresh_token_response() -> Response {
    auth_rest_error_response(AuthRestError::invalid_token(401, "Invalid refresh token"))
}

#[tracing::instrument(level = "debug", skip_all)]
fn refresh_token_expired_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        401,
        "Token expired",
        "Refresh token has expired",
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
fn refresh_session_is_expired(expires_at: &str) -> bool {
    let normalized = expires_at.trim();
    if normalized.is_empty() {
        return true;
    }
    NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(normalized, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|datetime| Local::now().naive_local() > datetime)
        .unwrap_or(true)
}
