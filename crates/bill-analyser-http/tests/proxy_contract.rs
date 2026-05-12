use std::{env, net::SocketAddr, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use bill_analyser_core::auth::PasswordPolicy;
use bill_analyser_http::{
    bind_addr_from_env, bind_addr_from_env_with, build_router, build_upstream_url,
    filter_proxy_request_headers, http_shell_health, run_http_server, HttpShellConfig,
    HttpShellConfigError, ImportRouteMode, ProxyState, DEFAULT_HTTP_BIND, REQUEST_ID_HEADER,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[test]
fn config_defaults_keep_python_as_proxy_fallback() {
    let config = HttpShellConfig::default();
    let health = http_shell_health(&config);

    assert_eq!(config.python_upstream, "http://127.0.0.1:5001");
    assert_eq!(config.timeout, Duration::from_millis(30_000));
    assert_eq!(config.body_limit_bytes, 10 * 1024 * 1024);
    assert_eq!(config.uploads_dir, "data/uploads");
    assert_eq!(config.import_route_mode, ImportRouteMode::ProxyOnly);
    assert_eq!(config.public_base_url, None);
    assert_eq!(config.auth_max_login_attempts, 5);
    assert_eq!(config.auth_lockout_duration_minutes, 15);
    assert!(config.auth_enable_user_registration);
    assert!(!config.auth_require_email_verification);
    assert!(!config.auth_enable_user_forget_password);
    assert!(!config.auth_enable_oauth2);
    assert_eq!(config.auth_oauth2_provider, "");
    assert_eq!(config.auth_password_policy, PasswordPolicy::default());
    let tuned_config = config
        .clone()
        .with_auth_max_login_attempts(8)
        .with_auth_lockout_duration_minutes(60)
        .with_auth_enable_user_registration(false)
        .with_auth_require_email_verification(true)
        .with_auth_enable_user_forget_password(true)
        .with_auth_enable_oauth2(true)
        .with_auth_oauth2_provider(" github ")
        .with_uploads_dir(" C:/temp/uploads ")
        .with_auth_password_policy(PasswordPolicy {
            min_length: 12,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
        });
    assert_eq!(tuned_config.auth_max_login_attempts, 8);
    assert_eq!(tuned_config.auth_lockout_duration_minutes, 60);
    assert!(!tuned_config.auth_enable_user_registration);
    assert!(tuned_config.auth_require_email_verification);
    assert!(tuned_config.auth_enable_user_forget_password);
    assert!(tuned_config.auth_enable_oauth2);
    assert_eq!(tuned_config.auth_oauth2_provider, "github");
    assert_eq!(tuned_config.uploads_dir, "C:/temp/uploads");
    assert_eq!(tuned_config.auth_password_policy.min_length, 12);
    assert_eq!(
        config.clone().with_uploads_dir("  ").uploads_dir,
        "data/uploads"
    );
    assert_eq!(
        health.identity.runtime_boundary,
        "rust-http-shell:proxy-only"
    );
    assert_eq!(health.identity.business_migration, "none");
    assert!(health.identity.api_takeover);
    assert_eq!(
        health.details.get("proxied_routes"),
        Some(&"unowned /api/*".to_string())
    );
}

#[test]
fn config_from_env_reads_explicit_proxy_values() {
    let config = HttpShellConfig::from_env_with(|name| match name {
        "BILL_ANALYSER_PYTHON_UPSTREAM" => Some("http://127.0.0.1:5999/".to_string()),
        "BILL_ANALYSER_HTTP_TIMEOUT_MS" => Some("1234".to_string()),
        "BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES" => Some("4096".to_string()),
        "BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE" => Some("import_route_skeleton".to_string()),
        _ => None,
    })
    .expect("env config parses");

    assert_eq!(config.python_upstream, "http://127.0.0.1:5999");
    assert_eq!(config.timeout, Duration::from_millis(1234));
    assert_eq!(config.body_limit_bytes, 4096);
    assert_eq!(
        config.import_route_mode,
        ImportRouteMode::ImportRouteSkeleton
    );
}

#[test]
fn config_from_env_reads_import_db_runtime_and_sqlite_path() {
    let config = HttpShellConfig::from_env_with(|name| match name {
        "BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE" => Some("import_db_runtime".to_string()),
        "BILL_ANALYSER_SQLITE_DB_PATH" => Some("  C:/temp/bill-runtime.db  ".to_string()),
        "BILL_ANALYSER_UPLOADS_DIR" => Some("  C:/temp/uploads  ".to_string()),
        "BILL_ANALYSER_TRUSTED_USER_HEADER_SECRET" => Some("  route-secret  ".to_string()),
        "BILL_ANALYSER_AUTH_JWT_SECRET" => Some("  jwt-secret  ".to_string()),
        "BILL_ANALYSER_AUTH_JWT_ALGORITHM" => Some("HS256".to_string()),
        "BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS" => Some("3".to_string()),
        "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS" => Some("9".to_string()),
        "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS" => Some("7".to_string()),
        "BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES" => Some("45".to_string()),
        "BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION" => Some("false".to_string()),
        "BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION" => Some("true".to_string()),
        "BILL_ANALYSER_AUTH_ENABLE_USER_FORGET_PASSWORD" => Some("true".to_string()),
        "BILL_ANALYSER_AUTH_ENABLE_OAUTH2" => Some("true".to_string()),
        "BILL_ANALYSER_AUTH_OAUTH2_PROVIDER" => Some(" gitlab ".to_string()),
        "BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH" => Some("10".to_string()),
        "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_UPPERCASE" => Some("true".to_string()),
        "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_LOWERCASE" => Some("true".to_string()),
        "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_DIGIT" => Some("true".to_string()),
        "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_SPECIAL" => Some("true".to_string()),
        "BILL_ANALYSER_PUBLIC_BASE_URL" => Some("  https://api.example.test/  ".to_string()),
        _ => None,
    })
    .expect("env config parses");

    assert_eq!(config.import_route_mode, ImportRouteMode::ImportDbRuntime);
    assert_eq!(
        config.sqlite_db_path.as_deref(),
        Some("C:/temp/bill-runtime.db")
    );
    assert_eq!(config.uploads_dir, "C:/temp/uploads");
    assert_eq!(
        config.trusted_user_header_secret.as_deref(),
        Some("route-secret")
    );
    assert_eq!(config.auth_jwt_secret.as_deref(), Some("jwt-secret"));
    assert_eq!(config.auth_jwt_algorithm, "HS256");
    assert_eq!(config.auth_jwt_expiration_days, 3);
    assert_eq!(config.auth_refresh_token_expiration_days, 9);
    assert_eq!(config.auth_max_login_attempts, 7);
    assert_eq!(config.auth_lockout_duration_minutes, 45);
    assert!(!config.auth_enable_user_registration);
    assert!(config.auth_require_email_verification);
    assert!(config.auth_enable_user_forget_password);
    assert!(config.auth_enable_oauth2);
    assert_eq!(config.auth_oauth2_provider, "gitlab");
    assert_eq!(
        config.auth_password_policy,
        PasswordPolicy {
            min_length: 10,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
        }
    );
    assert_eq!(
        config.public_base_url.as_deref(),
        Some("https://api.example.test")
    );
}

#[test]
fn config_from_env_reads_python_compatible_jwt_env_aliases() {
    let config = HttpShellConfig::from_env_with(|name| match name {
        "JWT_SECRET_KEY" => Some("  dotenv-secret  ".to_string()),
        "JWT_ALGORITHM" => Some("hs512".to_string()),
        "JWT_EXPIRATION_DAYS" => Some("5".to_string()),
        "REFRESH_TOKEN_EXPIRATION_DAYS" => Some("11".to_string()),
        "MAX_LOGIN_ATTEMPTS" => Some("4".to_string()),
        "LOCKOUT_DURATION_MINUTES" => Some("30".to_string()),
        "ENABLE_USER_REGISTRATION" => Some("0".to_string()),
        "REQUIRE_EMAIL_VERIFICATION" => Some("1".to_string()),
        "ENABLE_USER_FORGET_PASSWORD" => Some("1".to_string()),
        "ENABLE_OAUTH2" => Some("1".to_string()),
        "OAUTH2_PROVIDER" => Some("google".to_string()),
        "PASSWORD_MIN_LENGTH" => Some("9".to_string()),
        "PASSWORD_REQUIRE_UPPERCASE" => Some("yes".to_string()),
        "PASSWORD_REQUIRE_LOWERCASE" => Some("on".to_string()),
        "PASSWORD_REQUIRE_DIGIT" => Some("true".to_string()),
        "PASSWORD_REQUIRE_SPECIAL" => Some("true".to_string()),
        _ => None,
    })
    .expect("env config parses");

    assert_eq!(config.auth_jwt_secret.as_deref(), Some("dotenv-secret"));
    assert_eq!(config.auth_jwt_algorithm, "hs512");
    assert_eq!(config.auth_jwt_expiration_days, 5);
    assert_eq!(config.auth_refresh_token_expiration_days, 11);
    assert_eq!(config.auth_max_login_attempts, 4);
    assert_eq!(config.auth_lockout_duration_minutes, 30);
    assert!(!config.auth_enable_user_registration);
    assert!(config.auth_require_email_verification);
    assert!(config.auth_enable_user_forget_password);
    assert!(config.auth_enable_oauth2);
    assert_eq!(config.auth_oauth2_provider, "google");
    assert_eq!(config.auth_password_policy.min_length, 9);
    assert!(config.auth_password_policy.require_uppercase);
    assert!(config.auth_password_policy.require_lowercase);
    assert!(config.auth_password_policy.require_digit);
    assert!(config.auth_password_policy.require_special);
}

#[test]
fn server_bind_addr_reads_rust_primary_http_env() {
    let default_addr = bind_addr_from_env_with(|_| None).expect("default bind parses");
    assert_eq!(default_addr.to_string(), "127.0.0.1:5000");

    let custom_addr = bind_addr_from_env_with(|name| match name {
        "BILL_ANALYSER_HTTP_BIND" => Some("127.0.0.1:5010".to_string()),
        _ => None,
    })
    .expect("custom bind parses");
    assert_eq!(custom_addr.to_string(), "127.0.0.1:5010");
}

#[test]
fn server_bind_addr_reads_process_env_without_mutating_it() {
    let result = bind_addr_from_env();
    if let Ok(raw) = env::var("BILL_ANALYSER_HTTP_BIND") {
        assert_eq!(result.is_ok(), raw.trim().parse::<SocketAddr>().is_ok());
    } else {
        assert_eq!(
            result.expect("default process env bind"),
            DEFAULT_HTTP_BIND.parse().expect("default bind parses")
        );
    }
}

#[tokio::test]
async fn rust_primary_http_server_serves_router() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");
    let config = HttpShellConfig::default();
    let state = ProxyState::new(config).expect("proxy state");
    let server = tokio::spawn(async move { run_http_server(listener, state).await });
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/api/health");
    let mut last_error = None;
    for _ in 0..20 {
        match client.get(&url).send().await {
            Ok(response) => {
                assert_eq!(response.status(), StatusCode::OK);
                let response_text = response.text().await.expect("response text");
                let body: Value = serde_json::from_str(&response_text).expect("json body");
                assert_eq!(body["identity"]["api_takeover"], true);
                server.abort();
                let _ = server.await;
                return;
            }
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
    server.abort();
    let _ = server.await;
    panic!("Rust primary server did not respond: {last_error:?}");
}

#[test]
fn config_rejects_invalid_upstream_body_limit_and_env_integer() {
    assert_eq!(
        HttpShellConfig::new("ftp://127.0.0.1", Duration::from_secs(1), 1).unwrap_err(),
        HttpShellConfigError::InvalidUpstream
    );
    assert_eq!(
        HttpShellConfig::new("http://127.0.0.1:5000", Duration::from_secs(1), 0).unwrap_err(),
        HttpShellConfigError::InvalidBodyLimit
    );

    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_HTTP_TIMEOUT_MS" => Some("not-a-number".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_HTTP_TIMEOUT_MS")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS" => Some("0".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS" => Some("366".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS" => Some("0".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES" => Some("1441".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH" => Some("0".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidInteger("BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION" => Some("maybe".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidBoolean("BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION")
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE" => Some("takeover".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidImportRouteMode
    );
    assert_eq!(
        HttpShellConfig::from_env_with(|name| match name {
            "BILL_ANALYSER_PUBLIC_BASE_URL" => Some("api.example.test".to_string()),
            _ => None,
        })
        .unwrap_err(),
        HttpShellConfigError::InvalidUpstream
    );
}

#[test]
fn upstream_url_preserves_path_and_query() {
    let url =
        build_upstream_url("http://127.0.0.1:5000/", "/api/bills?x=1&y=2").expect("url builds");

    assert_eq!(url, "http://127.0.0.1:5000/api/bills?x=1&y=2");
}

#[test]
fn request_header_filter_removes_hop_by_hop_host_and_content_length() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("example.test"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("close"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("12"));
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer token"),
    );
    headers.insert(header::COOKIE, HeaderValue::from_static("session=abc"));
    headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("rid-client"));

    let filtered = filter_proxy_request_headers(&headers);

    assert!(!filtered.contains_key(header::HOST));
    assert!(!filtered.contains_key(header::CONNECTION));
    assert!(!filtered.contains_key(header::CONTENT_LENGTH));
    assert!(!filtered.contains_key(REQUEST_ID_HEADER));
    assert_eq!(
        filtered.get(header::AUTHORIZATION),
        Some(&HeaderValue::from_static("Bearer token"))
    );
    assert_eq!(
        filtered.get(header::COOKIE),
        Some(&HeaderValue::from_static("session=abc"))
    );
}

#[test]
fn response_header_filter_preserves_set_cookie_and_drops_hop_by_hop() {
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, HeaderValue::from_static("upstream=ok"));
    headers.insert(
        header::TRANSFER_ENCODING,
        HeaderValue::from_static("chunked"),
    );

    let filtered = bill_analyser_http::filter_proxy_response_headers(&headers);

    assert_eq!(
        filtered.get(header::SET_COOKIE),
        Some(&HeaderValue::from_static("upstream=ok"))
    );
    assert!(!filtered.contains_key(header::TRANSFER_ENCODING));
}

#[tokio::test]
async fn rust_owned_routes_take_precedence_over_proxy() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runtime")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    let body = read_json(response).await;
    assert_eq!(body["crate_name"], "bill-analyser-http");
    assert_eq!(body["business_migration"], "none");
    assert_eq!(body["api_takeover"], true);
}

#[tokio::test]
async fn import_db_runtime_proxies_only_manifest_python_owned_routes() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router_with_mode(
        upstream.url(),
        Duration::from_secs(5),
        ImportRouteMode::ImportDbRuntime,
    );

    let proxied_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/statistics/exchange-rates?base=CNY")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(proxied_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(proxied_response).await["path"],
        "/api/statistics/exchange-rates"
    );

    let login_preflight_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/auth/login")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(login_preflight_response.status(), StatusCode::OK);
    let login_preflight_body = read_json(login_preflight_response).await;
    assert_eq!(login_preflight_body["method"], "OPTIONS");
    assert_eq!(login_preflight_body["path"], "/api/auth/login");

    for (method, path) in [
        (Method::GET, "/api/accounts/"),
        (Method::GET, "/api/accounts/123"),
        (Method::OPTIONS, "/api/auth/register"),
        (Method::GET, "/api/categories/virtual_food"),
        (Method::PUT, "/api/categories/virtual_food"),
        (Method::DELETE, "/api/categories/virtual_food"),
        (Method::GET, "/api/categories/tree"),
        (Method::GET, "/api/settings/bundle/export"),
        (Method::GET, "/api/llm/config"),
        (Method::POST, "/api/matching/candidates/session/12/accept"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            read_json(response).await["path"],
            path.split('?').next().unwrap_or(path),
            "{path}"
        );
    }

    let unknown_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/not-in-migration-manifest")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(unknown_response.status(), StatusCode::NOT_FOUND);
    let unknown_body = read_json(unknown_response).await;
    assert_eq!(
        unknown_body["error"]["code"],
        "route_not_manifest_python_proxied"
    );

    let wrong_method_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/bills/category/quick-add-keyword")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(
        wrong_method_response.status(),
        StatusCode::METHOD_NOT_ALLOWED
    );

    let overmatched_wildcard_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/pictureship/unused")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(
        overmatched_wildcard_response.status(),
        StatusCode::NOT_FOUND
    );

    for (method, path) in [
        (Method::POST, "/api/bills/pictures/unused/extra"),
        (Method::POST, "/api/bills/category"),
        (Method::POST, "/api/bills/category/not-real"),
        (Method::GET, "/api/accounts/display-orders"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn health_route_reports_proxy_only_runtime_boundary() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    let body = read_json(response).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(
        body["identity"]["runtime_boundary"],
        "rust-http-shell:proxy-only"
    );
    assert_eq!(body["identity"]["business_migration"], "none");
    assert_eq!(body["identity"]["api_takeover"], true);
}

#[tokio::test]
async fn proxy_preserves_method_query_headers_cookies_and_json_body() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse?stage=parse")
                .header(header::AUTHORIZATION, "Bearer token")
                .header(header::COOKIE, "session=abc")
                .header(REQUEST_ID_HEADER, "rid-from-client")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"amount":123}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(REQUEST_ID_HEADER),
        Some(&HeaderValue::from_static("rid-from-client"))
    );
    assert_eq!(
        response.headers().get(header::SET_COOKIE),
        Some(&HeaderValue::from_static("upstream=ok"))
    );

    let body = read_json(response).await;
    assert_eq!(body["method"], "POST");
    assert_eq!(body["path"], "/api/bills/import/v2/parse");
    assert_eq!(body["query"], "stage=parse");
    assert_eq!(body["authorization"], "Bearer token");
    assert_eq!(body["cookie"], "session=abc");
    assert_eq!(body["request_id"], "rid-from-client");
    assert_eq!(body["content_type"], "application/json");
    assert_eq!(body["body"], r#"{"amount":123}"#);
}

#[tokio::test]
async fn proxy_preserves_non_json_body_and_content_type() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/upload")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("plain body"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    let body = read_json(response).await;
    assert_eq!(body["method"], "PUT");
    assert_eq!(body["content_type"], "text/plain");
    assert_eq!(body["body"], "plain body");
}

#[tokio::test]
async fn proxy_forwards_non_success_status_and_body_unchanged() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/upstream-status/418")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
    let body = read_string(response).await;
    assert_eq!(body, "status:418");
}

#[tokio::test]
async fn proxy_generates_request_id_when_missing() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_secs(5));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/no-request-id")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert!(body["request_id"]
        .as_str()
        .is_some_and(|value| value.starts_with("rust-http-")));
}

#[tokio::test]
async fn proxy_wraps_upstream_down_as_infrastructure_error() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    let app = test_router(format!("http://{addr}"), Duration::from_millis(200));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/down")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert!(body["error"]["code"]
        .as_str()
        .is_some_and(|code| code.starts_with("proxy_upstream_")));
}

#[tokio::test]
async fn proxy_wraps_timeout_as_infrastructure_error() {
    let upstream = spawn_fake_upstream().await;
    let app = test_router(upstream.url(), Duration::from_millis(10));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/slow")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "proxy_upstream_timeout");
}

#[tokio::test]
async fn proxy_wraps_body_limit_overflow_as_infrastructure_error() {
    let upstream = spawn_fake_upstream().await;
    let config = HttpShellConfig::new(upstream.url(), Duration::from_secs(5), 4).expect("config");
    let state = ProxyState::new(config).expect("proxy state");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/too-large")
                .body(Body::from("larger than four bytes"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "proxy_body_read_failed");
}

fn test_router(upstream: String, timeout: Duration) -> Router {
    let config = HttpShellConfig::new(upstream, timeout, 1024 * 1024).expect("config");
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn test_router_with_mode(
    upstream: String,
    timeout: Duration,
    import_route_mode: ImportRouteMode,
) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        upstream,
        timeout,
        1024 * 1024,
        import_route_mode,
    )
    .expect("config");
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

struct FakeUpstream {
    addr: SocketAddr,
}

impl FakeUpstream {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

async fn spawn_fake_upstream() -> FakeUpstream {
    let app = Router::new()
        .route("/slow", get(slow_handler))
        .route("/upstream-status/:code", any(status_handler))
        .fallback(any(echo_handler));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("fake upstream serves");
    });

    FakeUpstream { addr }
}

async fn slow_handler() -> impl IntoResponse {
    tokio::time::sleep(Duration::from_millis(100)).await;
    "slow"
}

async fn status_handler(axum::extract::Path(code): axum::extract::Path<u16>) -> impl IntoResponse {
    (
        StatusCode::from_u16(code).expect("test status code"),
        format!("status:{code}"),
    )
}

async fn echo_handler(request: Request<Body>) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.expect("body bytes");
    let body_text = String::from_utf8_lossy(&body).to_string();
    let response = json!({
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query().unwrap_or(""),
        "authorization": header_value(&parts.headers, header::AUTHORIZATION.as_str()),
        "cookie": header_value(&parts.headers, header::COOKIE.as_str()),
        "request_id": header_value(&parts.headers, REQUEST_ID_HEADER),
        "content_type": header_value(&parts.headers, header::CONTENT_TYPE.as_str()),
        "body": body_text,
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, HeaderValue::from_static("upstream=ok"));

    (headers, response.to_string())
}

fn header_value(headers: &HeaderMap, name: impl AsRef<str>) -> String {
    headers
        .get(name.as_ref())
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

async fn read_string(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}
