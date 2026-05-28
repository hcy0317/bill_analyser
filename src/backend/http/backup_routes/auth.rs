// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn authenticated_backup_runtime(
    state: &HttpAppState,
    headers: &HeaderMap,
) -> RouteResult<AuthenticatedBackupRuntime> {
    let authenticated =
        resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
            .map_err(|error| Box::new(auth_error_response(error)))?;
    let auth_kind = if authenticated.session_id.is_some() {
        BackupAuthKind::BearerSession
    } else {
        BackupAuthKind::TrustedHeader
    };
    let runtime = open_backup_ops_runtime(state)?;
    Ok(AuthenticatedBackupRuntime {
        runtime,
        user_id: authenticated.user_id,
        auth_kind,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn open_backup_ops_runtime(state: &HttpAppState) -> RouteResult<BackupOpsRuntime> {
    if state.config.database_backend.uses_postgres() {
        let runtime = state
            .open_postgres_repository_runtime("backup ops")
            .map_err(|error| {
                Box::new(error_response(
                    status_or_internal(error.http_status_code()),
                    error.public_message("Rust backup ops route runtime DB error"),
                ))
            })?;
        return Ok(BackupOpsRuntime::Postgres(runtime));
    }

    let runtime = open_runtime(state)?;
    init_backup_ops_schema(runtime.connection()).map_err(|_| Box::new(db_error_response()))?;
    Ok(BackupOpsRuntime::Sqlite(runtime))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn ensure_sensitive_backup_auth(
    auth_runtime: &AuthenticatedBackupRuntime,
    state: &HttpAppState,
    headers: &HeaderMap,
    payload: Option<&Value>,
) -> RouteResult<()> {
    if auth_runtime.auth_kind == BackupAuthKind::TrustedHeader {
        return Ok(());
    }
    let Some(token) = backup_step_up_token(headers, payload) else {
        return Err(Box::new(error_response(
            StatusCode::UNAUTHORIZED,
            "step-up token is required for backup file operation",
        )));
    };
    validate_backup_step_up_token(&token, state, auth_runtime.user_id)
        .map_err(|message| Box::new(error_response(StatusCode::UNAUTHORIZED, message)))
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn backup_step_up_token(headers: &HeaderMap, payload: Option<&Value>) -> Option<String> {
    header_text(headers, STEP_UP_TOKEN_HEADER).or_else(|| {
        payload
            .and_then(|value| {
                value
                    .get("stepUpToken")
                    .or_else(|| value.get("step_up_token"))
                    .and_then(Value::as_str)
            })
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn validate_backup_step_up_token(
    token: &str,
    state: &HttpAppState,
    expected_user_id: UserId,
) -> Result<(), &'static str> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or("step-up token validation is not configured")?;
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or("Invalid step-up token")?;
    let encoded_payload = parts.next().ok_or("Invalid step-up token")?;
    let encoded_signature = parts.next().ok_or("Invalid step-up token")?;
    if parts.next().is_some() {
        return Err("Invalid step-up token");
    }

    let header = decode_backup_jwt_part(encoded_header)?;
    let payload = decode_backup_jwt_part(encoded_payload)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err("Invalid step-up token");
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm)?;
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| "Invalid step-up token")?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input.as_bytes(), &signature)
        .map_err(|_| "Invalid step-up token")?;
    if payload.get("type").and_then(Value::as_str) != Some("step_up") {
        return Err("Invalid step-up token");
    }
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or("Invalid step-up token")?;
    if exp <= Utc::now().timestamp() {
        return Err("Invalid step-up token");
    }
    let user_id = payload
        .get("user_id")
        .and_then(Value::as_u64)
        .ok_or("Invalid step-up token")?;
    if user_id != expected_user_id.get() {
        return Err("Invalid step-up token");
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn decode_backup_jwt_part(encoded: &str) -> Result<Value, &'static str> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| "Invalid step-up token")?;
    serde_json::from_slice(&decoded).map_err(|_| "Invalid step-up token")
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn normalize_jwt_algorithm(algorithm: &str) -> String {
    algorithm.trim().to_ascii_uppercase()
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn jwt_hmac_algorithm(algorithm: &str) -> Result<hmac::Algorithm, &'static str> {
    match normalize_jwt_algorithm(algorithm).as_str() {
        "HS256" => Ok(hmac::HMAC_SHA256),
        "HS384" => Ok(hmac::HMAC_SHA384),
        "HS512" => Ok(hmac::HMAC_SHA512),
        _ => Err("Unsupported JWT algorithm for backup step-up token"),
    }
}
