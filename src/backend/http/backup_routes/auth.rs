// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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
    let auth_kind = backup_auth_kind_from_headers(headers);
    let runtime = open_backup_ops_runtime(state)?;
    Ok(AuthenticatedBackupRuntime {
        runtime,
        user_id: authenticated.user_id,
        auth_kind,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn open_backup_ops_runtime(state: &HttpAppState) -> RouteResult<BackupOpsRuntime> {
    let runtime = state
        .open_postgres_repository_runtime("backup ops")
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message(),
            ))
        })?;
    Ok(BackupOpsRuntime::Postgres(runtime))
}

#[tracing::instrument(level = "debug", skip_all)]
fn backup_auth_kind_from_headers(headers: &HeaderMap) -> BackupAuthKind {
    if headers.contains_key(TRUSTED_USER_SECRET_HEADER)
        || headers.contains_key("x-user-id")
        || headers.contains_key("x-bill-analyser-user-id")
    {
        BackupAuthKind::TrustedHeader
    } else {
        BackupAuthKind::BearerSession
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) fn ensure_sensitive_backup_auth(
    auth_runtime: &AuthenticatedBackupRuntime,
    state: &HttpAppState,
    headers: &HeaderMap,
    payload: Option<&Value>,
) -> RouteResult<()> {
    ensure_sensitive_backup_auth_for_kind(
        auth_runtime.auth_kind,
        state,
        headers,
        payload,
        auth_runtime.user_id,
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_sensitive_backup_auth_for_kind(
    auth_kind: BackupAuthKind,
    state: &HttpAppState,
    headers: &HeaderMap,
    payload: Option<&Value>,
    user_id: UserId,
) -> RouteResult<()> {
    if auth_kind == BackupAuthKind::TrustedHeader {
        return Ok(());
    }
    let Some(token) = backup_step_up_token(headers, payload) else {
        return Err(Box::new(error_response(
            StatusCode::UNAUTHORIZED,
            "step-up token is required for backup file operation",
        )));
    };
    validate_backup_step_up_token(&token, state, user_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    use axum::http::HeaderValue;
    use chrono::Duration as ChronoDuration;

    const TEST_SECRET: &str = "backup-step-up-secret";

    #[test]
    fn backup_bearer_auth_requires_step_up_token() {
        let state = test_state();
        let headers = HeaderMap::new();
        let user_id = UserId::new(7).expect("positive user id");

        let error = ensure_sensitive_backup_auth_for_kind(
            BackupAuthKind::BearerSession,
            &state,
            &headers,
            None,
            user_id,
        )
        .expect_err("bearer-backed backup operations require step-up");

        assert_eq!(error.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn backup_trusted_header_skips_step_up_only_with_trusted_header_shape() {
        let state = test_state();
        let mut trusted_headers = HeaderMap::new();
        trusted_headers.insert(
            TRUSTED_USER_SECRET_HEADER,
            HeaderValue::from_static("trusted-secret"),
        );
        trusted_headers.insert("x-user-id", HeaderValue::from_static("7"));

        assert_eq!(
            backup_auth_kind_from_headers(&trusted_headers),
            BackupAuthKind::TrustedHeader
        );
        assert!(
            ensure_sensitive_backup_auth_for_kind(
                BackupAuthKind::TrustedHeader,
                &state,
                &trusted_headers,
                None,
                UserId::new(7).expect("positive user id"),
            )
            .is_ok(),
            "trusted header runtime remains the only step-up bypass"
        );

        let mut bearer_headers = HeaderMap::new();
        bearer_headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer opaque-access-token"),
        );
        assert_eq!(
            backup_auth_kind_from_headers(&bearer_headers),
            BackupAuthKind::BearerSession
        );
    }

    #[test]
    fn backup_step_up_rejects_wrong_user_wrong_type_and_expired_token() {
        let state = test_state();
        let headers = HeaderMap::new();
        let user_id = UserId::new(7).expect("positive user id");
        let valid = signed_step_up_token(7, "step_up", ChronoDuration::minutes(5));

        assert!(
            ensure_sensitive_backup_auth_for_kind(
                BackupAuthKind::BearerSession,
                &state,
                &headers,
                Some(&json!({ "stepUpToken": valid })),
                user_id,
            )
            .is_ok(),
            "matching fresh step-up token is accepted"
        );

        for token in [
            signed_step_up_token(8, "step_up", ChronoDuration::minutes(5)),
            signed_step_up_token(7, "access", ChronoDuration::minutes(5)),
            signed_step_up_token(7, "step_up", ChronoDuration::minutes(-1)),
        ] {
            assert!(
                ensure_sensitive_backup_auth_for_kind(
                    BackupAuthKind::BearerSession,
                    &state,
                    &headers,
                    Some(&json!({ "step_up_token": token })),
                    user_id,
                )
                .is_err(),
                "wrong-user, wrong-type, and expired step-up tokens fail closed"
            );
        }
    }

    fn test_state() -> HttpAppState {
        HttpAppState::new(
            HttpShellConfig::default()
                .with_auth_jwt_secret(TEST_SECRET)
                .with_auth_jwt_algorithm("HS256"),
        )
        .expect("state")
    }

    fn signed_step_up_token(user_id: u64, token_type: &str, exp_offset: ChronoDuration) -> String {
        let now = Utc::now();
        let header = json!({"alg": "HS256", "typ": "JWT"});
        let payload = json!({
            "user_id": user_id,
            "type": token_type,
            "iat": now.timestamp(),
            "exp": (now + exp_offset).timestamp(),
        });
        let encoded_header = general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&header).expect("header json"));
        let encoded_payload = general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).expect("payload json"));
        let signing_input = format!("{encoded_header}.{encoded_payload}");
        let key = hmac::Key::new(hmac::HMAC_SHA256, TEST_SECRET.as_bytes());
        let signature = hmac::sign(&key, signing_input.as_bytes());
        let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
        format!("{signing_input}.{encoded_signature}")
    }
}
