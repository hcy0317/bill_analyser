use super::*;
use std::sync::{Mutex, MutexGuard, OnceLock};

const TOKEN_ALLOWLIST_ENV: &str = "BILL_ANALYSER_LLM_TOKEN_URL_ALLOWLIST";
static TOKEN_ALLOWLIST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct TokenAllowlistGuard {
    previous: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl TokenAllowlistGuard {
    fn set(value: Option<&str>) -> Self {
        let lock = TOKEN_ALLOWLIST_LOCK.get_or_init(|| Mutex::new(()));
        let guard = lock.lock().expect("token allowlist lock");
        let previous = std::env::var(TOKEN_ALLOWLIST_ENV).ok();
        match value {
            Some(value) => std::env::set_var(TOKEN_ALLOWLIST_ENV, value),
            None => std::env::remove_var(TOKEN_ALLOWLIST_ENV),
        }
        Self {
            previous,
            _lock: guard,
        }
    }
}

impl Drop for TokenAllowlistGuard {
    fn drop(&mut self) {
        match self.previous.as_deref() {
            Some(value) => std::env::set_var(TOKEN_ALLOWLIST_ENV, value),
            None => std::env::remove_var(TOKEN_ALLOWLIST_ENV),
        }
    }
}

#[test]
fn token_endpoint_policy_preserves_same_origin_and_fails_closed_for_metadata() {
    let _guard = TokenAllowlistGuard::set(None);
    assert!(validate_provider_token_endpoint(
        "https://llm.example.com/oauth/token",
        "https://llm.example.com/v1",
    )
    .is_ok());
    assert_eq!(
        validate_provider_token_endpoint(
            "http://public.example.com/oauth/token",
            "http://public.example.com/v1",
        ),
        Err("token endpoint must use https unless it is local".to_string())
    );
    assert_eq!(
        validate_provider_token_endpoint(
            "https://user:secret@llm.example.com/oauth/token",
            "https://llm.example.com/v1",
        ),
        Err("token endpoint must not contain credentials".to_string())
    );
    for (endpoint, message) in [
        ("", "provider url is not allowed"),
        ("https:\\llm.example.com/token", "provider url is not allowed"),
        ("not a URL", "provider url must be valid"),
        (
            "ftp://llm.example.com/token",
            "provider url must use http or https with a host",
        ),
        (
            "https://?query",
            "provider url must use http or https with a host",
        ),
    ] {
        assert_eq!(
            validate_provider_token_endpoint(endpoint, "https://llm.example.com/v1"),
            Err(message.to_string()),
            "{endpoint:?}"
        );
    }

    std::env::set_var(
        TOKEN_ALLOWLIST_ENV,
        "https://tokens.example.test;http://127.0.0.1:9999;https://metadata.google.internal;https://169.254.169.254;https://0.0.0.0;https://224.0.0.1",
    );
    assert!(validate_provider_token_endpoint(
        "https://tokens.example.test/oauth/token",
        "https://llm.example.com/v1",
    )
    .is_ok());
    assert!(validate_provider_token_endpoint(
        "http://127.0.0.1:9999/oauth/token",
        "https://llm.example.com/v1",
    )
    .is_ok());
    for endpoint in [
        "https://metadata.google.internal/token",
        "https://169.254.169.254/token",
        "https://0.0.0.0/token",
        "https://224.0.0.1/token",
    ] {
        assert_eq!(
            validate_provider_token_endpoint(endpoint, "https://llm.example.com/v1"),
            Err("token endpoint is not allowed".to_string()),
            "{endpoint}"
        );
    }
}

#[tokio::test]
async fn refresh_fails_before_network_for_a_never_allowed_token_endpoint() {
    let _guard = TokenAllowlistGuard::set(Some("https://metadata.google.internal"));
    let client = reqwest::Client::new();
    let result = refresh_provider_auth_profile(
        &client,
        &json!({
            "token_endpoint": "https://metadata.google.internal/token",
            "refresh_token": "synthetic-refresh-token",
        }),
        "https://llm.example.com/v1",
    )
    .await;

    assert!(result.is_err());
}
