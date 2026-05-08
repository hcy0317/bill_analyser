use std::{error::Error, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{Method, StatusCode},
    routing::any,
    Router,
};
use base64::{engine::general_purpose, Engine as _};
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
use tokio::net::TcpListener;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "auth-token-route-secret";

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
async fn auth_token_runtime_keeps_generation_and_refresh_routes_on_python_proxy(
) -> Result<(), Box<dyn Error>> {
    let upstream = spawn_fake_upstream().await?;
    let app = proxy_runtime_router(&upstream.url());

    for path in ["/api/tokens/api", "/api/tokens/mcp", "/api/tokens/refresh"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer still-python-owned")
                    .body(Body::from(json!({"route": path}).to_string()))?,
            )
            .await?;

        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = read_json(response).await;
        assert_eq!(body["method"], "POST", "{path}");
        assert_eq!(body["path"], path, "{path}");
        assert_eq!(body["authorization"], "Bearer still-python-owned");
        assert!(body["body"].as_str().unwrap_or_default().contains(path));
    }

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
    .with_auth_jwt_secret(TEST_AUTH_SECRET);
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn proxy_runtime_router(upstream: &str) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        upstream,
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config");
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
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
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
            is_active INTEGER NOT NULL DEFAULT 1
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
        CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
        "#,
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, is_active) VALUES (42, 'alice', 'alice@example.test', 1)",
        [],
    )?;
    connection.execute(
        "INSERT INTO users(id, username, email, is_active) VALUES (77, 'bob', 'bob@example.test', 1)",
        [],
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

fn token_by_id<'a>(tokens: &'a [Value], token_id: &str) -> &'a Value {
    tokens
        .iter()
        .find(|token| token["tokenId"] == token_id)
        .expect("token id exists")
}

struct FakeUpstream {
    addr: SocketAddr,
}

impl FakeUpstream {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

async fn spawn_fake_upstream() -> Result<FakeUpstream, Box<dyn Error>> {
    let app = Router::new().fallback(any(echo_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("fake upstream serves");
    });

    Ok(FakeUpstream { addr })
}

async fn echo_handler(request: Request<Body>) -> impl axum::response::IntoResponse {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.expect("body bytes");
    json!({
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "authorization": parts.headers.get("authorization").and_then(|value| value.to_str().ok()).unwrap_or(""),
        "body": String::from_utf8_lossy(&body).to_string(),
    })
    .to_string()
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

fn test_access_token(user_id: i64, secret: &str) -> String {
    let now = Local::now();
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "username": format!("user-{user_id}"),
        "type": "access",
        "iat": now.timestamp(),
        "exp": (now + ChronoDuration::hours(1)).timestamp(),
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
