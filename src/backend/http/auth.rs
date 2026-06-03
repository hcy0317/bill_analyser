// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::http::{header, HeaderMap};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{auth::parse_bearer_authorization_header, UserId};
use ring::hmac;
use serde_json::Value;

use crate::config::HttpShellConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub session_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustRouteAuthError {
    pub status: u16,
    pub message: String,
}

impl RustRouteAuthError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: 401,
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: 503,
            message: message.into(),
        }
    }
}

pub fn resolve_user_id_from_headers(
    headers: &HeaderMap,
    config: &HttpShellConfig,
    trusted_secret_header: &'static str,
) -> Result<UserId, RustRouteAuthError> {
    resolve_authenticated_user_from_headers(headers, config, trusted_secret_header)
        .map(|user| user.user_id)
}

pub fn resolve_authenticated_user_from_headers(
    headers: &HeaderMap,
    config: &HttpShellConfig,
    trusted_secret_header: &'static str,
) -> Result<AuthenticatedUser, RustRouteAuthError> {
    if headers.contains_key(trusted_secret_header)
        || headers.contains_key("x-user-id")
        || headers.contains_key("x-bill-analyser-user-id")
    {
        return resolve_trusted_header_user_id(headers, config, trusted_secret_header).map(
            |user_id| AuthenticatedUser {
                user_id,
                session_id: None,
            },
        );
    }

    resolve_bearer_user(headers, config)
}

fn resolve_trusted_header_user_id(
    headers: &HeaderMap,
    config: &HttpShellConfig,
    trusted_secret_header: &'static str,
) -> Result<UserId, RustRouteAuthError> {
    let expected_secret = config
        .trusted_user_header_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            RustRouteAuthError::unavailable(
                "Rust DB route runtime requires BILL_ANALYSER_TRUSTED_USER_HEADER_SECRET",
            )
        })?;
    let provided_secret = headers
        .get(trusted_secret_header)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RustRouteAuthError::unauthorized("Missing Rust route user trust header"))?;
    if provided_secret != expected_secret {
        return Err(RustRouteAuthError::unauthorized(
            "Invalid Rust route user trust header",
        ));
    }

    trusted_user_header_value(headers)
}

fn trusted_user_header_value(headers: &HeaderMap) -> Result<UserId, RustRouteAuthError> {
    let raw = headers
        .get("x-user-id")
        .or_else(|| headers.get("x-bill-analyser-user-id"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RustRouteAuthError::unauthorized("Missing Rust route user id"))?;
    parse_user_id(raw, "Invalid Rust route user id")
}

fn resolve_bearer_user(
    headers: &HeaderMap,
    config: &HttpShellConfig,
) -> Result<AuthenticatedUser, RustRouteAuthError> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let token = parse_bearer_authorization_header(auth_header)
        .map_err(|error| RustRouteAuthError::unauthorized(error.message))?;
    Ok(AuthenticatedUser {
        user_id: validate_access_jwt(&token, config)?,
        session_id: None,
    })
}

fn validate_access_jwt(
    token: &str,
    config: &HttpShellConfig,
) -> Result<UserId, RustRouteAuthError> {
    let secret = config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            RustRouteAuthError::unavailable(
                "Rust DB route runtime requires BILL_ANALYSER_AUTH_JWT_SECRET or JWT_SECRET_KEY",
            )
        })?;

    let mut parts = token.split('.');
    let encoded_header = parts
        .next()
        .ok_or_else(|| RustRouteAuthError::unauthorized("Invalid token"))?;
    let encoded_payload = parts
        .next()
        .ok_or_else(|| RustRouteAuthError::unauthorized("Invalid token"))?;
    let encoded_signature = parts
        .next()
        .ok_or_else(|| RustRouteAuthError::unauthorized("Invalid token"))?;
    if parts.next().is_some() {
        return Err(RustRouteAuthError::unauthorized("Invalid token"));
    }

    let header_value = decode_jwt_part(encoded_header)?;
    let payload_value = decode_jwt_part(encoded_payload)?;
    let header_alg = header_value
        .get("alg")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let header_alg = normalize_jwt_algorithm(header_alg);
    let configured_alg = normalize_jwt_algorithm(&config.auth_jwt_algorithm);
    let algorithm = jwt_hmac_algorithm(&configured_alg)?;
    if header_alg != configured_alg {
        return Err(RustRouteAuthError::unauthorized(
            "Unsupported JWT algorithm for Rust route runtime",
        ));
    }

    let signing_input = format!("{encoded_header}.{encoded_payload}");
    verify_hmac_signature(
        secret,
        algorithm,
        signing_input.as_bytes(),
        encoded_signature,
    )?;

    if payload_value.get("type").and_then(Value::as_str) != Some("access") {
        return Err(RustRouteAuthError::unauthorized("Invalid token"));
    }
    let exp = payload_value
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or_else(|| RustRouteAuthError::unauthorized("Invalid token"))?;
    if exp <= chrono::Utc::now().timestamp() {
        return Err(RustRouteAuthError::unauthorized("Token expired"));
    }
    let raw_user_id = payload_value
        .get("user_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| RustRouteAuthError::unauthorized("Invalid token"))?;
    UserId::new(raw_user_id).map_err(|_| RustRouteAuthError::unauthorized("Invalid token"))
}

fn decode_jwt_part(encoded: &str) -> Result<Value, RustRouteAuthError> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| RustRouteAuthError::unauthorized("Invalid token"))?;
    serde_json::from_slice(&decoded).map_err(|_| RustRouteAuthError::unauthorized("Invalid token"))
}

pub(crate) fn normalize_jwt_algorithm(algorithm: &str) -> String {
    algorithm.trim().to_ascii_uppercase()
}

pub(crate) fn jwt_hmac_algorithm(algorithm: &str) -> Result<hmac::Algorithm, RustRouteAuthError> {
    match normalize_jwt_algorithm(algorithm).as_str() {
        "HS256" => Ok(hmac::HMAC_SHA256),
        "HS384" => Ok(hmac::HMAC_SHA384),
        "HS512" => Ok(hmac::HMAC_SHA512),
        _ => Err(RustRouteAuthError::unauthorized(
            "Unsupported JWT algorithm for Rust route runtime",
        )),
    }
}

fn verify_hmac_signature(
    secret: &str,
    algorithm: hmac::Algorithm,
    signing_input: &[u8],
    encoded_signature: &str,
) -> Result<(), RustRouteAuthError> {
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| RustRouteAuthError::unauthorized("Invalid token"))?;
    let key = hmac::Key::new(algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input, &signature)
        .map_err(|_| RustRouteAuthError::unauthorized("Invalid token"))
}

fn parse_user_id(raw: &str, invalid_message: &'static str) -> Result<UserId, RustRouteAuthError> {
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| RustRouteAuthError::unauthorized(invalid_message))?;
    UserId::new(parsed).map_err(|_| RustRouteAuthError::unauthorized(invalid_message))
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::http::HeaderValue;
    use chrono::Duration as ChronoDuration;
    use serde_json::json;
    const TEST_SECRET: &str = "jwt-secret";
    const TRUSTED_SECRET_HEADER: &str = "x-bill-analyser-trusted-secret";

    #[test]
    fn bearer_auth_rejects_missing_jwt_secret() {
        let token = signed_token(7, "access", "HS256", TEST_SECRET, ChronoDuration::hours(1));
        let headers = bearer_headers(&token);
        let config = HttpShellConfig::default();

        let error = resolve_user_id_from_headers(&headers, &config, TRUSTED_SECRET_HEADER)
            .expect_err("missing JWT secret rejects frontend auth");

        assert_eq!(error.status, 503);
        assert!(error.message.contains("BILL_ANALYSER_AUTH_JWT_SECRET"));
    }

    #[test]
    fn bearer_auth_rejects_invalid_parts_unsupported_alg_and_wrong_type() {
        let config = HttpShellConfig::default().with_auth_jwt_secret(TEST_SECRET);
        let too_many_parts = resolve_user_id_from_headers(
            &bearer_headers("a.b.c.d"),
            &config,
            TRUSTED_SECRET_HEADER,
        )
        .expect_err("too many JWT parts rejects");
        assert_eq!(too_many_parts.status, 401);

        let unsupported_token =
            signed_token(7, "access", "none", TEST_SECRET, ChronoDuration::hours(1));
        let unsupported = resolve_user_id_from_headers(
            &bearer_headers(&unsupported_token),
            &config,
            TRUSTED_SECRET_HEADER,
        )
        .expect_err("unsupported JWT algorithm rejects");
        assert_eq!(unsupported.status, 401);
        assert_eq!(
            unsupported.message,
            "Unsupported JWT algorithm for Rust route runtime"
        );

        let refresh_token =
            signed_token(7, "refresh", "HS256", TEST_SECRET, ChronoDuration::hours(1));
        let wrong_type = resolve_user_id_from_headers(
            &bearer_headers(&refresh_token),
            &config,
            TRUSTED_SECRET_HEADER,
        )
        .expect_err("non-access token rejects");
        assert_eq!(wrong_type.status, 401);
        assert_eq!(wrong_type.message, "Invalid token");
    }

    #[test]
    fn bearer_auth_accepts_configured_hmac_algorithms() {
        for algorithm in ["HS256", "HS384", "HS512"] {
            let token = signed_token(
                7,
                "access",
                algorithm,
                TEST_SECRET,
                ChronoDuration::hours(1),
            );
            let config = HttpShellConfig::default()
                .with_auth_jwt_secret(TEST_SECRET)
                .with_auth_jwt_algorithm(algorithm);

            let user_id = resolve_user_id_from_headers(
                &bearer_headers(&token),
                &config,
                TRUSTED_SECRET_HEADER,
            )
            .expect("configured HMAC JWT accepts");

            assert_eq!(user_id, UserId::new(7).expect("positive user id"));
        }
    }

    fn bearer_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("valid bearer header"),
        );
        headers
    }

    fn signed_token(
        user_id: i64,
        token_type: &str,
        alg: &str,
        secret: &str,
        exp_offset: ChronoDuration,
    ) -> String {
        let now = chrono::Utc::now();
        let header = json!({"alg": alg, "typ": "JWT"});
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
        let key = hmac::Key::new(
            jwt_hmac_algorithm(alg).unwrap_or(hmac::HMAC_SHA256),
            secret.as_bytes(),
        );
        let signature = hmac::sign(&key, signing_input.as_bytes());
        let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
        format!("{signing_input}.{encoded_signature}")
    }
}
