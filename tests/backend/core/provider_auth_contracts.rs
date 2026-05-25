use bill_analyser_core::ai_ocr_llm::{
    normalize_provider_auth_config, provider_auth_access_token,
    provider_auth_has_refresh_credential, provider_auth_is_expired, redact_provider_auth_config,
};
use chrono::{Duration, Utc};
use serde_json::json;

#[test]
fn provider_auth_parser_reads_common_json_shapes_and_manual_overrides() {
    let expires_at = (Utc::now() + Duration::minutes(30)).to_rfc3339();
    let normalized = normalize_provider_auth_config(Some(&json!({
        "credentialMode": "session_json",
        "session": {
            "accessToken": "access-one",
            "refresh_token": "refresh-one",
            "expiresAt": expires_at,
        },
        "tokenEndpoint": "https://llm.example.com/oauth/token",
        "refreshHeaders": {
            "authorization": "Bearer bootstrap-secret"
        },
        "refreshBody": {
            "client_id": "bill-analyser",
            "client_secret": "client-secret"
        },
        "requestHeaders": {
            "x-sub2api-key": "sub2api-secret"
        }
    })));

    assert_eq!(normalized["credential_mode"], "session_json");
    assert_eq!(normalized["access_token"], "access-one");
    assert_eq!(normalized["refresh_token"], "refresh-one");
    assert_eq!(
        normalized["token_endpoint"],
        "https://llm.example.com/oauth/token"
    );
    assert_eq!(
        normalized["refresh_headers"]["authorization"],
        "Bearer bootstrap-secret"
    );
    assert_eq!(
        provider_auth_access_token(&normalized).as_deref(),
        Some("access-one")
    );
    assert!(provider_auth_has_refresh_credential(&normalized));
    assert!(!provider_auth_is_expired(&normalized, Utc::now()));

    let redacted = redact_provider_auth_config(&normalized);
    let serialized = serde_json::to_string(&redacted).expect("redacted JSON");
    assert!(!serialized.contains("access-one"));
    assert!(!serialized.contains("refresh-one"));
    assert!(!serialized.contains("client-secret"));
    assert!(!serialized.contains("sub2api-secret"));
    assert_eq!(redacted["access_token"], "********");
    assert_eq!(redacted["refresh_body"]["client_secret"], "********");
}

#[test]
fn provider_auth_parser_accepts_raw_token_and_expired_refresh_state() {
    let expired_at = (Utc::now() - Duration::minutes(5)).to_rfc3339();
    let normalized = normalize_provider_auth_config(Some(&json!({
        "credential_mode": "access_token",
        "accessToken": "raw-access",
        "refreshToken": "raw-refresh",
        "expiresAt": expired_at
    })));

    assert_eq!(
        provider_auth_access_token(&normalized).as_deref(),
        Some("raw-access")
    );
    assert!(provider_auth_has_refresh_credential(&normalized));
    assert!(provider_auth_is_expired(&normalized, Utc::now()));

    let raw_string = normalize_provider_auth_config(Some(&json!("standalone-access-token")));
    assert_eq!(
        provider_auth_access_token(&raw_string).as_deref(),
        Some("standalone-access-token")
    );
}
