use bill_analyser_core::auth::{
    extract_bearer_token_or_empty, infer_token_type_from_user_agent, json_object_or_empty,
    normalize_recovery_code, parse_bearer_authorization_header, parse_user_agent_device_name,
    recovery_code_hash_input, validate_refresh_token_claims, AuthRestError, PasswordPolicy,
    TokenKind, TOKEN_TYPE_API, TOKEN_TYPE_DEFAULT, TOKEN_TYPE_MCP,
};
use serde_json::json;

#[test]
fn bearer_header_contract_matches_flask_auth_edges() {
    assert_eq!(
        parse_bearer_authorization_header(""),
        Err(AuthRestError::unauthorized("Missing authorization header"))
    );
    assert_eq!(
        parse_bearer_authorization_header("Token abc"),
        Err(AuthRestError::unauthorized(
            "Invalid authorization header format"
        ))
    );
    assert_eq!(
        parse_bearer_authorization_header("   "),
        Err(AuthRestError::unauthorized(
            "Invalid authorization header format"
        ))
    );
    assert_eq!(
        parse_bearer_authorization_header("Bearer token extra"),
        Err(AuthRestError::unauthorized(
            "Invalid authorization header format"
        ))
    );
    assert_eq!(
        parse_bearer_authorization_header("  bearer   demo-token  ").unwrap(),
        "demo-token"
    );

    assert_eq!(
        extract_bearer_token_or_empty("Bearer demo-token"),
        "demo-token"
    );
    assert_eq!(extract_bearer_token_or_empty("Token demo-token"), "");
}

#[test]
fn refresh_claim_validation_preserves_existing_error_contract() {
    assert_eq!(
        validate_refresh_token_claims(
            &json!({"type": "access", "user_id": 1, "username": "alice"})
        ),
        Err(AuthRestError::invalid_token(400, "Not a refresh token"))
    );
    assert_eq!(
        validate_refresh_token_claims(
            &json!({"type": "refresh", "user_id": "bad", "username": ""})
        ),
        Err(AuthRestError::invalid_token(401, "Invalid refresh token"))
    );

    let claims = validate_refresh_token_claims(
        &json!({"type": "refresh", "user_id": 42, "username": "alice"}),
    )
    .unwrap();
    assert_eq!(claims.user_id.get(), 42);
    assert_eq!(claims.username, "alice");
}

#[test]
fn json_body_object_contract_treats_null_and_scalars_as_empty() {
    assert!(json_object_or_empty(None).is_empty());
    assert!(json_object_or_empty(Some(&json!(null))).is_empty());
    assert!(json_object_or_empty(Some(&json!("plain-string"))).is_empty());

    let body = json_object_or_empty(Some(&json!({"refreshToken": "token"})));
    assert_eq!(body.get("refreshToken").unwrap(), "token");
}

#[test]
fn token_kind_and_user_agent_projection_match_token_routes() {
    assert_eq!(
        TokenKind::Api.user_agent("Browser"),
        "Bill Analyser API Token"
    );
    assert_eq!(
        TokenKind::Mcp.user_agent("Browser"),
        "Bill Analyser MCP Token"
    );
    assert_eq!(TokenKind::Session.user_agent("Browser"), "Browser");

    assert_eq!(
        infer_token_type_from_user_agent("Bill Analyser API Token"),
        TOKEN_TYPE_API
    );
    assert_eq!(
        infer_token_type_from_user_agent("Bill Analyser MCP Token"),
        TOKEN_TYPE_MCP
    );
    assert_eq!(
        infer_token_type_from_user_agent("browser"),
        TOKEN_TYPE_DEFAULT
    );

    assert_eq!(
        parse_user_agent_device_name(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit Chrome/120.0 Safari/537.36"
        ),
        "Windows 10 (Chrome)"
    );
    assert_eq!(
        parse_user_agent_device_name("Mozilla/5.0 (iPhone) Version/17.0 Safari/605.1.15"),
        "iOS (Safari)"
    );
    assert_eq!(parse_user_agent_device_name(""), "未知设备");
}

#[test]
fn password_policy_matches_python_registration_messages() {
    let policy = PasswordPolicy {
        min_length: 8,
        require_uppercase: true,
        require_lowercase: true,
        require_digit: true,
        require_special: true,
    };

    assert_eq!(
        policy.validate("short").unwrap_err(),
        "Password must be at least 8 characters long"
    );
    assert_eq!(
        policy.validate("lower123!").unwrap_err(),
        "Password must contain at least one uppercase letter"
    );
    assert_eq!(
        policy.validate("UPPER123!").unwrap_err(),
        "Password must contain at least one lowercase letter"
    );
    assert_eq!(
        policy.validate("NoDigits!!").unwrap_err(),
        "Password must contain at least one digit"
    );
    assert_eq!(
        policy.validate("NoSpecial1").unwrap_err(),
        "Password must contain at least one special character"
    );
    assert!(policy.validate("Valid1!A").is_ok());
}

#[test]
fn recovery_code_normalization_matches_persistent_db_contract() {
    assert_eq!(normalize_recovery_code(" abcd - 1234 "), "ABCD-1234");
    assert_eq!(normalize_recovery_code("a b c d-1 2 3 4"), "ABCD-1234");
    assert_eq!(normalize_recovery_code(" \t\n "), "");
    assert_eq!(
        recovery_code_hash_input(" abcd - 1234 ").unwrap(),
        "2fa-recovery:ABCD-1234"
    );
    assert!(recovery_code_hash_input("").is_none());
}
