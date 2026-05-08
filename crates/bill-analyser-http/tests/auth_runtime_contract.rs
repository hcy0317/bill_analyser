use std::{error::Error, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::{connect_info::ConnectInfo, Request},
    http::{HeaderValue, Method, StatusCode},
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use bcrypt::hash;
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState, AUTH_PROXIED_ROUTE_PATTERNS,
    AUTH_TOKEN_ROUTE_PATTERNS,
};
use chrono::{Duration as ChronoDuration, Local};
use ring::hmac;
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "auth-token-route-secret";
const TEST_PASSWORD: &str = "correct-password";

#[tokio::test]
async fn auth_token_runtime_lists_and_revokes_user_scoped_sessions() -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/tokens")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("DELETE", "/api/tokens/{token_id}")));
    assert!(AUTH_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/tokens/refresh")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/refresh")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let tokens = list_body["result"].as_array().expect("token list");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0]["tokenId"], "3");
    assert_eq!(tokens[0]["tokenType"], 5);
    assert_eq!(tokens[1]["tokenId"], "2");
    assert_eq!(tokens[1]["tokenType"], 8);
    assert_eq!(tokens[2]["tokenId"], "1");
    assert_eq!(tokens[2]["tokenType"], 0);
    assert_eq!(tokens[2]["deviceName"], "Windows 10 (Chrome)");
    assert_eq!(tokens[2]["isCurrent"], true);
    assert_eq!(tokens[2]["isCurrentToken"], true);
    let empty_timestamp_token = token_by_id(tokens, "6");
    assert_eq!(empty_timestamp_token["lastActivityAt"], "");
    assert_eq!(empty_timestamp_token["lastSeen"], 0);
    assert!(!session_is_active(fixture.db_path(), 5)?);

    let revoke_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/2",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(revoke_response.status(), StatusCode::OK);
    assert_eq!(read_json(revoke_response).await["result"], true);
    assert!(!session_is_active(fixture.db_path(), 2)?);

    let not_found_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/999",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(not_found_response.status(), StatusCode::NOT_FOUND);
    let not_found_body = read_json(not_found_response).await;
    assert_eq!(not_found_body["error"], "Not Found");
    assert_eq!(not_found_body["message"], "Token not found");

    let revoke_others_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(revoke_others_response.status(), StatusCode::OK);
    let revoke_others_body = read_json(revoke_others_response).await;
    assert_eq!(revoke_others_body["result"], true);
    assert_eq!(revoke_others_body["revokedCount"], 2);
    assert!(session_is_active(fixture.db_path(), 1)?);
    assert!(!session_is_active(fixture.db_path(), 3)?);
    assert!(session_is_active(fixture.db_path(), 4)?);
    assert!(!session_is_active(fixture.db_path(), 6)?);

    Ok(())
}

#[tokio::test]
async fn auth_token_runtime_preserves_flask_error_shapes() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let invalid_id_response = app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/not-an-int",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_id_response.status(), StatusCode::BAD_REQUEST);
    let invalid_id_body = read_json(invalid_id_response).await;
    assert_eq!(invalid_id_body["success"], false);
    assert_eq!(invalid_id_body["error"], "Invalid request");
    assert_eq!(
        invalid_id_body["message"],
        "tokenId must be a valid integer"
    );

    let trusted_delete_all_response = app
        .clone()
        .oneshot(trusted_request(
            Method::DELETE,
            "/api/tokens",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        trusted_delete_all_response.status(),
        StatusCode::UNAUTHORIZED
    );
    let trusted_delete_all_body = read_json(trusted_delete_all_response).await;
    assert_eq!(trusted_delete_all_body["error"], "Unauthorized");
    assert_eq!(
        trusted_delete_all_body["message"],
        "Current bearer session is required"
    );

    Ok(())
}

#[tokio::test]
async fn auth_token_runtime_generates_personal_and_refresh_tokens() -> Result<(), Box<dyn Error>> {
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/api")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/mcp")));
    assert!(!AUTH_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/api")));
    assert!(AUTH_TOKEN_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tokens/refresh")));

    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let api_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/api",
            &token,
            TEST_PASSWORD,
            3600,
        ))
        .await?;
    assert_eq!(api_response.status(), StatusCode::OK);
    let api_body = read_json(api_response).await;
    assert_eq!(api_body["success"], true);
    let api_result = api_body["result"].as_object().expect("api result");
    let api_token = api_result["token"].as_str().expect("api token");
    assert_eq!(api_result["apiBaseUrl"], "https://api.example.test/api");
    let api_payload = jwt_payload(api_token);
    assert_eq!(api_payload["type"], "access");
    assert_eq!(api_payload["token_kind"], "api");
    assert_eq!(api_payload["user_id"], 42);
    assert_eq!(api_payload["username"], "alice");
    assert_eq!(
        api_payload["exp"].as_i64().expect("api exp")
            - api_payload["iat"].as_i64().expect("api iat"),
        3600
    );
    assert_token_session(
        fixture.db_path(),
        api_token,
        "Bill Analyser API Token",
        "198.51.100.10",
    )?;
    assert_auth_log(
        fixture.db_path(),
        "api_token_generate_success",
        true,
        "Bill Analyser API Token",
    )?;
    let api_token_list_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            api_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(api_token_list_response.status(), StatusCode::OK);

    let mcp_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/mcp",
            &token,
            TEST_PASSWORD,
            0,
        ))
        .await?;
    assert_eq!(mcp_response.status(), StatusCode::OK);
    let mcp_body = read_json(mcp_response).await;
    let mcp_result = mcp_body["result"].as_object().expect("mcp result");
    let mcp_token = mcp_result["token"].as_str().expect("mcp token");
    assert_eq!(mcp_result["mcpUrl"], "https://api.example.test/mcp");
    assert_eq!(jwt_payload(mcp_token)["token_kind"], "mcp");
    assert_token_session(
        fixture.db_path(),
        mcp_token,
        "Bill Analyser MCP Token",
        "198.51.100.10",
    )?;

    let peer_ip_response = app
        .clone()
        .oneshot(personal_token_request_with_peer(
            "/api/tokens/api",
            &token,
            TEST_PASSWORD,
            120,
            "203.0.113.9:4300".parse()?,
        ))
        .await?;
    assert_eq!(peer_ip_response.status(), StatusCode::OK);
    let peer_ip_body = read_json(peer_ip_response).await;
    let peer_ip_token = peer_ip_body["result"]["token"]
        .as_str()
        .expect("peer ip token");
    assert_token_session(
        fixture.db_path(),
        peer_ip_token,
        "Bill Analyser API Token",
        "203.0.113.9",
    )?;

    let invalid_password_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/api",
            &token,
            "wrong-password",
            3600,
        ))
        .await?;
    assert_eq!(invalid_password_response.status(), StatusCode::UNAUTHORIZED);
    let invalid_password_body = read_json(invalid_password_response).await;
    assert_eq!(invalid_password_body["error"], "Invalid credentials");
    assert_eq!(
        invalid_password_body["message"],
        "Current password is incorrect"
    );
    assert_auth_log(
        fixture.db_path(),
        "api_token_generate_failed",
        false,
        "Mozilla/5.0 (Rust contract)",
    )?;
    for octet in 10..14 {
        let mut retry_request =
            personal_token_request("/api/tokens/api", &token, "wrong-password", 3600);
        retry_request.headers_mut().insert(
            "x-forwarded-for",
            HeaderValue::from_str(&format!("192.0.2.{octet}")).expect("xff value"),
        );
        let retry_response = app.clone().oneshot(retry_request).await?;
        assert_eq!(retry_response.status(), StatusCode::UNAUTHORIZED);
    }
    let throttled_response = app
        .clone()
        .oneshot(personal_token_request(
            "/api/tokens/api",
            &token,
            "wrong-password",
            3600,
        ))
        .await?;
    assert_eq!(throttled_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        read_json(throttled_response).await["message"],
        "Too many failed token password attempts, please try again later"
    );

    let missing_password_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/tokens/api",
            &token,
            Body::from(json!({"expiresInSeconds": 3600}).to_string()),
        ))
        .await?;
    assert_eq!(missing_password_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_password_response).await["message"],
        "Current password is required"
    );

    let invalid_expires_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/tokens/api",
            &token,
            Body::from(
                json!({"password": TEST_PASSWORD, "expiresInSeconds": {"bad": true}}).to_string(),
            ),
        ))
        .await?;
    assert_eq!(invalid_expires_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_expires_response).await["message"],
        "expiresInSeconds must be a valid integer"
    );

    let trusted_generate_response = app
        .clone()
        .oneshot(trusted_request(
            Method::POST,
            "/api/tokens/api",
            Body::from(json!({"password": TEST_PASSWORD}).to_string()),
        ))
        .await?;
    assert_eq!(trusted_generate_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(trusted_generate_response).await["message"],
        "Current bearer session is required"
    );

    let refresh_token = test_refresh_token(42, "alice", TEST_AUTH_SECRET, ChronoDuration::days(1));
    seed_refresh_session(fixture.db_path(), &refresh_token)?;
    let refresh_response = app
        .clone()
        .oneshot(refresh_token_request(&refresh_token))
        .await?;
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = read_json(refresh_response).await;
    assert_eq!(refresh_body["success"], true);
    let refresh_result = refresh_body["result"].as_object().expect("refresh result");
    let new_access = refresh_result["token"].as_str().expect("new access");
    let new_refresh = refresh_result["refreshToken"]
        .as_str()
        .expect("new refresh");
    assert_eq!(refresh_result["newToken"], new_access);
    let new_access_payload = jwt_payload(new_access);
    let new_refresh_payload = jwt_payload(new_refresh);
    assert_eq!(new_access_payload["type"], "access");
    assert_eq!(new_access_payload.get("token_kind"), None);
    assert_eq!(new_refresh_payload["type"], "refresh");
    assert_eq!(new_refresh_payload["user_id"], 42);
    assert_eq!(refresh_result["user"]["username"], "alice");
    assert_eq!(refresh_result["user"]["nickname"], "Alice A.");
    assert_eq!(
        refresh_result["applicationCloudSettings"][0]["settingKey"],
        "showAmountInHomePage"
    );
    assert_session_token_pair(
        fixture.db_path(),
        new_access,
        new_refresh,
        "Mozilla/5.0 (Refresh contract)",
        "198.51.100.11",
    )?;
    assert_refresh_session_consumed(fixture.db_path(), 1)?;

    let replay_response = app
        .clone()
        .oneshot(refresh_token_request(&refresh_token))
        .await?;
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(replay_response).await["message"],
        "Invalid refresh token"
    );

    let access_expired_refresh =
        test_refresh_token(42, "alice", TEST_AUTH_SECRET, ChronoDuration::days(1));
    seed_access_expired_refresh_session(fixture.db_path(), &access_expired_refresh)?;
    let cleanup_trigger_response = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            new_access,
            Body::empty(),
        ))
        .await?;
    assert_eq!(cleanup_trigger_response.status(), StatusCode::OK);
    let access_expired_response = app
        .clone()
        .oneshot(refresh_token_request(&access_expired_refresh))
        .await?;
    assert_eq!(access_expired_response.status(), StatusCode::OK);

    let missing_refresh_response = app
        .clone()
        .oneshot(bearer_request(
            Method::POST,
            "/api/tokens/refresh",
            &token,
            Body::from(json!({}).to_string()),
        ))
        .await?;
    assert_eq!(missing_refresh_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_refresh_response).await["message"],
        "Refresh token is required"
    );

    let access_as_refresh_response = app.clone().oneshot(refresh_token_request(&token)).await?;
    assert_eq!(access_as_refresh_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(access_as_refresh_response).await["message"],
        "Not a refresh token"
    );

    let expired_refresh =
        test_refresh_token(42, "alice", TEST_AUTH_SECRET, -ChronoDuration::hours(1));
    let expired_response = app
        .clone()
        .oneshot(refresh_token_request(&expired_refresh))
        .await?;
    assert_eq!(expired_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(read_json(expired_response).await["error"], "Token expired");

    let missing_session_refresh =
        test_refresh_token(42, "alice", TEST_AUTH_SECRET, ChronoDuration::hours(1));
    let missing_session_response = app
        .clone()
        .oneshot(refresh_token_request(&missing_session_refresh))
        .await?;
    assert_eq!(missing_session_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_session_response).await["message"],
        "Invalid refresh token"
    );

    Ok(())
}

#[tokio::test]
async fn auth_token_runtime_covers_configuration_and_db_error_edges() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new()?;
    let token = test_access_token(42, TEST_AUTH_SECRET);
    seed_auth_db(fixture.db_path(), &token)?;
    let app = runtime_router(&fixture);

    let missing_auth_response = app
        .clone()
        .oneshot(Request::builder().uri("/api/tokens").body(Body::empty())?)
        .await?;
    assert_eq!(missing_auth_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(missing_auth_response).await["message"],
        "Missing authorization header"
    );

    let no_db_app = runtime_router_without_sqlite_path();
    let no_db_response = no_db_app
        .clone()
        .oneshot(trusted_request(Method::GET, "/api/tokens", Body::empty()))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["message"],
        "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let invalid_path = fixture
        .db_path()
        .parent()
        .expect("fixture parent")
        .join("missing-parent")
        .join("auth.db");
    let invalid_path_app = runtime_router_with_db_path(&invalid_path, true);
    let invalid_path_response = invalid_path_app
        .oneshot(trusted_request(Method::GET, "/api/tokens", Body::empty()))
        .await?;
    assert_eq!(
        invalid_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let no_jwt_secret_app = runtime_router_with_db_path(fixture.db_path(), false);
    let no_jwt_secret_response = no_jwt_secret_app
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_jwt_secret_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read_json(no_jwt_secret_response).await["error"],
        "Service Unavailable"
    );

    let missing_sessions_fixture = RuntimeFixture::new()?;
    Connection::open(missing_sessions_fixture.db_path())?.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, email TEXT NOT NULL UNIQUE, is_active INTEGER NOT NULL DEFAULT 1);",
    )?;
    let missing_sessions_app =
        runtime_router_with_db_path(missing_sessions_fixture.db_path(), true);
    let missing_sessions_response = missing_sessions_app
        .oneshot(trusted_request(Method::GET, "/api/tokens", Body::empty()))
        .await?;
    assert_eq!(
        missing_sessions_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(missing_sessions_response).await["message"],
        "Rust auth token runtime DB error"
    );

    let bad_list_fixture = RuntimeFixture::new()?;
    seed_auth_db_missing_list_columns(bad_list_fixture.db_path(), &token)?;
    let bad_list_app = runtime_router_with_db_path(bad_list_fixture.db_path(), true);
    let bad_list_response = bad_list_app
        .oneshot(bearer_request(
            Method::GET,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        bad_list_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let revoke_error_fixture = RuntimeFixture::new()?;
    seed_auth_db(revoke_error_fixture.db_path(), &token)?;
    install_revoke_failure_trigger(revoke_error_fixture.db_path())?;
    let revoke_error_app = runtime_router_with_db_path(revoke_error_fixture.db_path(), true);

    let revoke_other_error_response = revoke_error_app
        .clone()
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        revoke_other_error_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let revoke_one_error_response = revoke_error_app
        .oneshot(bearer_request(
            Method::DELETE,
            "/api/tokens/2",
            &token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        revoke_one_error_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("auth-runtime.db");
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
        })
    }

    fn db_path(&self) -> &Path {
        &self.db_path
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    runtime_router_with_db_path(fixture.db_path(), true)
}

fn runtime_router_without_sqlite_path() -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:59999",
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_trusted_user_header_secret(TEST_AUTH_SECRET)
    .with_auth_jwt_secret(TEST_AUTH_SECRET)
    .with_public_base_url("https://api.example.test");
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn runtime_router_with_db_path(path: &Path, include_jwt_secret: bool) -> Router {
    let mut config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:59999",
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET)
    .with_public_base_url("https://api.example.test");
    if include_jwt_secret {
        config = config.with_auth_jwt_secret(TEST_AUTH_SECRET);
    }
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn seed_auth_db(path: &Path, token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            nickname TEXT,
            avatar TEXT,
            default_account_id INTEGER,
            transaction_edit_scope INTEGER DEFAULT 0,
            language TEXT DEFAULT 'zh_Hans',
            default_currency TEXT DEFAULT 'CNY',
            first_day_of_week INTEGER DEFAULT 1,
            fiscal_year_start INTEGER DEFAULT 1,
            calendar_display_type INTEGER DEFAULT 0,
            date_display_type INTEGER DEFAULT 0,
            long_date_format INTEGER DEFAULT 0,
            short_date_format INTEGER DEFAULT 0,
            long_time_format INTEGER DEFAULT 0,
            short_time_format INTEGER DEFAULT 0,
            fiscal_year_format INTEGER DEFAULT 0,
            currency_display_type INTEGER DEFAULT 0,
            numeral_system INTEGER DEFAULT 0,
            decimal_separator INTEGER DEFAULT 0,
            digit_grouping_symbol INTEGER DEFAULT 0,
            digit_grouping INTEGER DEFAULT 0,
            coordinate_display_type INTEGER DEFAULT 0,
            expense_amount_color INTEGER DEFAULT 0,
            income_amount_color INTEGER DEFAULT 0,
            cash_account_id INTEGER,
            cash_transfer_category_id INTEGER,
            import_learning_enabled INTEGER DEFAULT 1,
            investment_platform_keywords TEXT,
            investment_product_keywords TEXT,
            investment_exclude_keywords TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            email_verified INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '2026-01-01T00:00:00',
            updated_at TEXT NOT NULL DEFAULT '2026-01-01T00:00:00'
        );
        CREATE TABLE sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token_hash TEXT NOT NULL,
            refresh_token_hash TEXT,
            expires_at TEXT NOT NULL,
            refresh_expires_at TEXT,
            user_agent TEXT,
            ip_address TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            last_activity_at TEXT,
            created_at TEXT
        );
        CREATE TABLE auth_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER,
            username TEXT,
            event_type TEXT NOT NULL,
            ip_address TEXT,
            user_agent TEXT,
            success INTEGER NOT NULL,
            error_message TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE user_application_cloud_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, setting_key)
        );
        CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
        "#,
    )?;
    let password_hash = hash(TEST_PASSWORD, 4)?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, nickname, is_active, email_verified) VALUES (42, 'alice', 'alice@example.test', ?1, 'Alice A.', 1, 1)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, password_hash, is_active) VALUES (77, 'bob', 'bob@example.test', ?1, 1)",
        [&password_hash],
    )?;

    let active_expires_at = (Local::now().naive_local() + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let current_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    let rows = [
        (
            1,
            42,
            current_hash.as_str(),
            active_expires_at.as_str(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36",
            "127.0.0.1",
            1,
            "2026-01-01T00:00:00",
            "2026-01-01T00:00:00",
        ),
        (
            2,
            42,
            "api-hash",
            active_expires_at.as_str(),
            "Bill Analyser API Token",
            "127.0.0.2",
            1,
            "2026-01-02T00:00:00",
            "2026-01-02T00:00:00",
        ),
        (
            3,
            42,
            "mcp-hash",
            active_expires_at.as_str(),
            "Bill Analyser MCP Token",
            "127.0.0.3",
            1,
            "2026-01-03T00:00:00",
            "2026-01-03T00:00:00",
        ),
        (
            4,
            77,
            "other-user-hash",
            active_expires_at.as_str(),
            "Other user",
            "127.0.0.4",
            1,
            "2026-01-04T00:00:00",
            "2026-01-04T00:00:00",
        ),
        (
            5,
            42,
            "expired-hash",
            "2020-01-01T00:00:00",
            "Expired",
            "127.0.0.5",
            1,
            "2020-01-01T00:00:00",
            "2020-01-01T00:00:00",
        ),
        (
            6,
            42,
            "empty-date-hash",
            active_expires_at.as_str(),
            "",
            "",
            1,
            "",
            "",
        ),
    ];
    for row in rows {
        connection.execute(
            r#"
            INSERT INTO sessions (
                id, user_id, token_hash, expires_at, refresh_expires_at,
                user_agent, ip_address, is_active, last_activity_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)
            "#,
            row,
        )?;
    }
    Ok(())
}

fn seed_auth_db_missing_list_columns(path: &Path, token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            is_active INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token_hash TEXT NOT NULL,
            refresh_token_hash TEXT,
            expires_at TEXT NOT NULL,
            refresh_expires_at TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            last_activity_at TEXT,
            created_at TEXT
        );
        CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
        INSERT INTO users(id, username, email, is_active) VALUES (42, 'alice', 'alice@example.test', 1);
        "#,
    )?;
    let active_expires_at = (Local::now().naive_local() + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        r#"
        INSERT INTO sessions (
            id, user_id, token_hash, expires_at, refresh_expires_at,
            is_active, last_activity_at, created_at
        ) VALUES (1, 42, ?1, ?2, NULL, 1, '2026-01-01T00:00:00', '2026-01-01T00:00:00')
        "#,
        (
            format!("{:x}", Sha256::digest(token.as_bytes())),
            active_expires_at,
        ),
    )?;
    Ok(())
}

fn seed_refresh_session(path: &Path, refresh_token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let refresh_hash = format!("{:x}", Sha256::digest(refresh_token.as_bytes()));
    let refresh_expires_at = (Local::now().naive_local() + ChronoDuration::hours(2))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        "UPDATE sessions SET refresh_token_hash = ?1, refresh_expires_at = ?2 WHERE id = 1",
        (&refresh_hash, &refresh_expires_at),
    )?;
    connection.execute(
        r#"
        INSERT INTO user_application_cloud_settings (
            user_id, setting_key, setting_value, created_at, updated_at
        ) VALUES (42, 'showAmountInHomePage', 'true', '2026-01-01T00:00:00', '2026-01-01T00:00:00')
        "#,
        [],
    )?;
    Ok(())
}

fn seed_access_expired_refresh_session(
    path: &Path,
    refresh_token: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let refresh_hash = format!("{:x}", Sha256::digest(refresh_token.as_bytes()));
    let refresh_expires_at = (Local::now().naive_local() + ChronoDuration::hours(2))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        r#"
        INSERT INTO sessions (
            id, user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at
        ) VALUES (
            88, 42, 'access-expired-refresh-backed', ?1,
            '2020-01-01T00:00:00', ?2,
            'Refresh backed', '127.0.0.88', 1, '2020-01-01T00:00:00', '2020-01-01T00:00:00'
        )
        "#,
        (&refresh_hash, &refresh_expires_at),
    )?;
    Ok(())
}

fn install_revoke_failure_trigger(path: &Path) -> Result<(), Box<dyn Error>> {
    Connection::open(path)?.execute_batch(
        r#"
        CREATE TRIGGER fail_session_revoke
        BEFORE UPDATE OF is_active ON sessions
        WHEN NEW.is_active = 0
        BEGIN
            SELECT RAISE(ABORT, 'forced revoke failure');
        END;
        "#,
    )?;
    Ok(())
}

fn session_is_active(path: &Path, session_id: i64) -> Result<bool, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let active = connection.query_row(
        "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND is_active = 1",
        [session_id],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(active > 0)
}

fn assert_token_session(
    path: &Path,
    token: &str,
    expected_user_agent: &str,
    expected_ip_address: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    let (user_agent, ip_address, refresh_hash, is_active): (String, String, Option<String>, i64) =
        connection.query_row(
            r#"
            SELECT user_agent, ip_address, refresh_token_hash, is_active
            FROM sessions
            WHERE token_hash = ?1
            "#,
            [token_hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    assert_eq!(user_agent, expected_user_agent);
    assert_eq!(ip_address, expected_ip_address);
    assert_eq!(refresh_hash, None);
    assert_eq!(is_active, 1);
    Ok(())
}

fn assert_session_token_pair(
    path: &Path,
    access_token: &str,
    refresh_token: &str,
    expected_user_agent: &str,
    expected_ip_address: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let token_hash = format!("{:x}", Sha256::digest(access_token.as_bytes()));
    let expected_refresh_hash = format!("{:x}", Sha256::digest(refresh_token.as_bytes()));
    let (user_agent, ip_address, refresh_hash, is_active): (String, String, String, i64) =
        connection.query_row(
            r#"
            SELECT user_agent, ip_address, refresh_token_hash, is_active
            FROM sessions
            WHERE token_hash = ?1
            "#,
            [token_hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    assert_eq!(user_agent, expected_user_agent);
    assert_eq!(ip_address, expected_ip_address);
    assert_eq!(refresh_hash, expected_refresh_hash);
    assert_eq!(is_active, 1);
    Ok(())
}

fn assert_refresh_session_consumed(path: &Path, session_id: i64) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (refresh_hash, is_active): (Option<String>, i64) = connection.query_row(
        "SELECT refresh_token_hash, is_active FROM sessions WHERE id = ?1",
        [session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(refresh_hash, None);
    assert_eq!(is_active, 0);
    Ok(())
}

fn assert_auth_log(
    path: &Path,
    expected_event: &str,
    expected_success: bool,
    expected_user_agent: &str,
) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (success, user_agent): (i64, String) = connection.query_row(
        r#"
        SELECT success, user_agent
        FROM auth_logs
        WHERE event_type = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
        [expected_event],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(success == 1, expected_success);
    assert_eq!(user_agent, expected_user_agent);
    Ok(())
}

fn token_by_id<'a>(tokens: &'a [Value], token_id: &str) -> &'a Value {
    tokens
        .iter()
        .find(|token| token["tokenId"] == token_id)
        .expect("token id exists")
}

fn personal_token_request(
    uri: &str,
    token: &str,
    password: &str,
    expires_in_seconds: i64,
) -> Request<Body> {
    let mut request = personal_token_request_with_peer(
        uri,
        token,
        password,
        expires_in_seconds,
        "198.51.100.10:4300".parse().expect("test peer addr"),
    );
    request
        .headers_mut()
        .insert("x-forwarded-proto", HeaderValue::from_static("https"));
    request.headers_mut().insert(
        "x-forwarded-host",
        HeaderValue::from_static("attacker.example.test"),
    );
    request.headers_mut().insert(
        "x-forwarded-for",
        HeaderValue::from_static("192.0.2.99, 192.0.2.10"),
    );
    request
}

fn personal_token_request_with_peer(
    uri: &str,
    token: &str,
    password: &str,
    expires_in_seconds: i64,
    peer_addr: SocketAddr,
) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("host", "api.example.test")
        .header("user-agent", "Mozilla/5.0 (Rust contract)")
        .body(Body::from(
            json!({
                "password": password,
                "expiresInSeconds": expires_in_seconds,
            })
            .to_string(),
        ))
        .expect("personal token peer request builds");
    request.extensions_mut().insert(ConnectInfo(peer_addr));
    request
}

fn refresh_token_request(refresh_token: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/tokens/refresh")
        .header("content-type", "application/json")
        .header("user-agent", "Mozilla/5.0 (Refresh contract)")
        .body(Body::from(
            json!({ "refreshToken": refresh_token }).to_string(),
        ))
        .expect("refresh token request builds");
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.11:4300"
            .parse::<SocketAddr>()
            .expect("test peer addr"),
    ));
    request
}

fn bearer_request(method: Method, uri: &str, token: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(body)
        .expect("request builds")
}

fn trusted_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", "42")
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

fn jwt_payload(token: &str) -> Value {
    let payload = token.split('.').nth(1).expect("jwt payload segment exists");
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("payload decodes");
    serde_json::from_slice(&bytes).expect("payload json")
}

fn test_access_token(user_id: i64, secret: &str) -> String {
    test_jwt_token(
        user_id,
        &format!("user-{user_id}"),
        "access",
        secret,
        ChronoDuration::hours(1),
    )
}

fn test_refresh_token(
    user_id: i64,
    username: &str,
    secret: &str,
    expires_in: ChronoDuration,
) -> String {
    test_jwt_token(user_id, username, "refresh", secret, expires_in)
}

fn test_jwt_token(
    user_id: i64,
    username: &str,
    token_type: &str,
    secret: &str,
    expires_in: ChronoDuration,
) -> String {
    let now = Local::now();
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "username": username,
        "type": token_type,
        "iat": now.timestamp(),
        "exp": (now + expires_in).timestamp(),
        "nonce": "auth-token-route-test"
    });
    let encoded_header =
        general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header json"));
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("payload json"));
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    format!("{signing_input}.{encoded_signature}")
}
