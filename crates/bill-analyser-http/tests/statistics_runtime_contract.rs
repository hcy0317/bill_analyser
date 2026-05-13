use std::{error::Error, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState, STATISTICS_PROXIED_ROUTE_PATTERNS,
    STATISTICS_ROUTE_PATTERNS,
};
use chrono::{Local, TimeZone};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "statistics-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn statistics_read_runtime_serves_owned_routes_and_reads_db() -> Result<(), Box<dyn Error>> {
    for route in [
        ("GET", "/api/statistics/category-statistics"),
        ("GET", "/api/statistics/category-statistics/trends"),
        ("GET", "/api/statistics/asset-trends"),
        ("GET", "/api/statistics/category-pie"),
        ("GET", "/api/statistics/top-merchants"),
        ("GET", "/api/statistics/amounts"),
    ] {
        assert!(STATISTICS_ROUTE_PATTERNS.iter().any(|item| item == &route));
    }
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);
    let (start_time, end_time) = march_2026_timestamps();

    let category_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!(
                "/api/statistics/category-statistics?startTime={start_time}&endTime={end_time}&keyword=coffee"
            ),
            Body::empty(),
        ))
        .await?;
    assert_eq!(category_response.status(), StatusCode::OK);
    let category_body = read_json(category_response).await;
    assert_eq!(category_body["success"], true);
    assert_eq!(category_body["result"]["startTime"], start_time);
    assert_eq!(category_body["result"]["endTime"], end_time);
    let category_items = category_body["result"]["items"]
        .as_array()
        .expect("category items");
    assert_eq!(category_items.len(), 1);
    assert_eq!(category_items[0]["categoryId"], "1");
    assert_eq!(category_items[0]["accountId"], "10");
    assert_eq!(category_items[0]["amount"], -1234);

    let trend_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-statistics/trends?startYearMonth=202603&endYearMonth=202604",
            Body::empty(),
        ))
        .await?;
    assert_eq!(trend_response.status(), StatusCode::OK);
    let trend_body = read_json(trend_response).await;
    assert_eq!(trend_body["success"], true);
    assert_eq!(
        trend_body["result"]
            .as_array()
            .expect("trend buckets")
            .len(),
        2
    );
    assert_eq!(trend_body["result"][0]["month"], 3);

    let asset_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/asset-trends?startTime=0&endTime=0",
            Body::empty(),
        ))
        .await?;
    assert_eq!(asset_response.status(), StatusCode::OK);
    let asset_body = read_json(asset_response).await;
    assert_eq!(asset_body["success"], true);
    assert!(!asset_body["result"]
        .as_array()
        .expect("asset days")
        .is_empty());

    let pie_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-pie?type=%E6%94%AF%E5%87%BA&start_date=2026-03-01&end_date=2026-03-31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(pie_response.status(), StatusCode::OK);
    let pie_body = read_json(pie_response).await;
    assert_eq!(pie_body["success"], true);
    assert_eq!(pie_body["data"][0]["name"], "餐饮");
    assert_eq!(pie_body["data"][0]["value"], 12.34);

    let merchants_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/top-merchants?limit=2&start_date=2026-03-01&end_date=2026-03-31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(merchants_response.status(), StatusCode::OK);
    let merchants_body = read_json(merchants_response).await;
    assert_eq!(merchants_body["success"], true);
    assert_eq!(
        merchants_body["data"].as_array().expect("merchants").len(),
        2
    );
    assert_eq!(merchants_body["data"][0]["name"], "ACME");
    assert_eq!(merchants_body["runtime"], Value::Null);

    let amounts_response = app
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/statistics/amounts?query=thisMonth_{start_time}_{end_time}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(amounts_response.status(), StatusCode::OK);
    let amounts_body = read_json(amounts_response).await;
    assert_eq!(amounts_body["success"], true);
    assert_eq!(
        amounts_body["result"]["thisMonth"]["amounts"][0]["incomeAmount"],
        10000
    );
    assert_eq!(
        amounts_body["result"]["thisMonth"]["amounts"][0]["expenseAmount"],
        1234
    );

    Ok(())
}

#[tokio::test]
async fn statistics_runtime_covers_error_edges_auth_and_proxy_boundaries(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new_with_upstream(spawn_fake_upstream().await.url())?;
    let app = runtime_router(&fixture);
    let (start_time, end_time) = march_2026_timestamps();
    let (wide_start_time, wide_end_time) = wide_timestamps();

    for (path, status) in [
        (
            format!(
                "/api/statistics/category-statistics?startTime={end_time}&endTime={start_time}"
            ),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/statistics/category-statistics/trends?startYearMonth=202604&endYearMonth=202603"
                .to_string(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/statistics/asset-trends?startTime=0&endTime=bad".to_string(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/statistics/amounts".to_string(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/statistics/category-statistics/trends?startYearMonth=bad&endYearMonth=202603"
                .to_string(),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!(
                "/api/statistics/asset-trends?startTime={wide_start_time}&endTime={wide_end_time}"
            ),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/statistics/top-merchants?limit=bad".to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(Method::GET, &path, Body::empty()))
            .await?;
        assert_eq!(response.status(), status, "{path}");
    }

    let default_month_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-statistics",
            Body::empty(),
        ))
        .await?;
    assert_eq!(default_month_response.status(), StatusCode::OK);
    let default_month_body = read_json(default_month_response).await;
    assert_eq!(default_month_body["success"], true);
    assert!(
        default_month_body["result"]["startTime"]
            .as_i64()
            .expect("default start timestamp")
            > 0
    );

    let all_category_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-statistics?startTime=0&endTime=0",
            Body::empty(),
        ))
        .await?;
    assert_eq!(all_category_response.status(), StatusCode::OK);
    let all_category_body = read_json(all_category_response).await;
    assert_eq!(all_category_body["result"]["startTime"], 0);
    assert!(!all_category_body["result"]["items"]
        .as_array()
        .expect("all category items")
        .is_empty());

    let default_trend_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-statistics/trends",
            Body::empty(),
        ))
        .await?;
    assert_eq!(default_trend_response.status(), StatusCode::OK);
    assert_eq!(read_json(default_trend_response).await["success"], true);

    let all_trend_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-statistics/trends?startYearMonth=0&endYearMonth=0",
            Body::empty(),
        ))
        .await?;
    assert_eq!(all_trend_response.status(), StatusCode::OK);
    assert!(!read_json(all_trend_response).await["result"]
        .as_array()
        .expect("all trend buckets")
        .is_empty());

    let mixed_amounts_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/statistics/amounts?query=bad|thisMonth_{start_time}_{end_time}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(mixed_amounts_response.status(), StatusCode::OK);
    let mixed_amounts_body = read_json(mixed_amounts_response).await;
    assert_eq!(
        mixed_amounts_body["result"]["thisMonth"]["amounts"][0]["expenseAmount"],
        1234
    );
    assert_eq!(mixed_amounts_body["result"]["bad"], Value::Null);

    let unauthenticated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/statistics/category-pie")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    let missing_db_state = ProxyState::new(
        HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            Duration::from_secs(1),
            1024 * 1024,
            ImportRouteMode::ImportDbRuntime,
        )?
        .with_trusted_user_header_secret(TEST_AUTH_SECRET),
    )?;
    let missing_db_response = build_router(missing_db_state)
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-pie",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        missing_db_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let invalid_db_path_response = build_router(ProxyState::new(
        HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            Duration::from_secs(1),
            1024 * 1024,
            ImportRouteMode::ImportDbRuntime,
        )?
        .with_sqlite_db_path(
            fixture
                ._temp_dir
                .path()
                .join("missing-parent")
                .join("statistics.db")
                .display()
                .to_string(),
        )
        .with_trusted_user_header_secret(TEST_AUTH_SECRET),
    )?)
    .oneshot(authed_request(
        Method::GET,
        "/api/statistics/category-pie",
        Body::empty(),
    ))
    .await?;
    assert_eq!(
        invalid_db_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let empty_fixture = RuntimeFixture::new_empty()?;
    let empty_app = runtime_router(&empty_fixture);
    let empty_trend_response = empty_app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-statistics/trends?startYearMonth=0&endYearMonth=0",
            Body::empty(),
        ))
        .await?;
    assert_eq!(empty_trend_response.status(), StatusCode::OK);
    assert_eq!(read_json(empty_trend_response).await["result"], json!([]));
    let empty_asset_response = empty_app
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/asset-trends?startTime=0&endTime=0",
            Body::empty(),
        ))
        .await?;
    assert_eq!(empty_asset_response.status(), StatusCode::OK);
    assert_eq!(read_json(empty_asset_response).await["result"], json!([]));

    for route in [
        ("GET", "/api/statistics/exchange-rates"),
        ("PUT", "/api/statistics/exchange-rates/custom"),
        ("DELETE", "/api/statistics/exchange-rates/custom/{currency}"),
    ] {
        assert!(STATISTICS_ROUTE_PATTERNS.iter().any(|item| item == &route));
    }
    let proxied_route = ("GET", "/api/statistics/overview");
    assert!(STATISTICS_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|item| item == &proxied_route));
    assert!(!STATISTICS_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|item| item == &("GET", "/api/statistics/exchange-rates")));
    assert!(!STATISTICS_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|item| item == &("PUT", "/api/statistics/exchange-rates/custom")));

    let invalid_provider_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/exchange-rates?provider=unknown_provider",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_provider_response.status(), StatusCode::BAD_REQUEST);
    let invalid_provider_body = read_json(invalid_provider_response).await;
    assert_eq!(invalid_provider_body["success"], false);
    assert_eq!(
        invalid_provider_body["error"],
        "Unsupported exchange rate provider: unknown_provider"
    );

    let exchange_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/exchange-rates",
            Body::empty(),
        ))
        .await?;
    assert_eq!(exchange_response.status(), StatusCode::OK);
    let exchange_body = read_json(exchange_response).await;
    assert_eq!(exchange_body["success"], true);
    assert_eq!(exchange_body["result"]["providerKey"], "user_custom");
    assert_eq!(exchange_body["result"]["baseCurrency"], "CNY");
    let exchange_rates = exchange_body["result"]["exchangeRates"]
        .as_array()
        .expect("exchange rates");
    assert!(exchange_rates
        .iter()
        .any(|rate| rate["currency"] == "CNY" && rate["rate"] == "1.0"));
    assert!(exchange_rates
        .iter()
        .any(|rate| rate["currency"] == "USD" && rate["rate"] == "7.12"));

    let upsert_response = app
        .clone()
        .oneshot(authed_request(
            Method::PUT,
            "/api/statistics/exchange-rates/custom",
            Body::from(json!({"currency": "eur", "rate": "8.5"}).to_string()),
        ))
        .await?;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upsert_body = read_json(upsert_response).await;
    assert_eq!(upsert_body["success"], true);
    assert_eq!(upsert_body["result"]["currency"], "EUR");
    assert_eq!(upsert_body["result"]["rate"], "8.5");
    assert!(upsert_body["result"]["updateTime"].as_i64().is_some());

    let invalid_upsert_response = app
        .clone()
        .oneshot(authed_request(
            Method::PUT,
            "/api/statistics/exchange-rates/custom",
            Body::from(json!({"currency": "EUR", "rate": 0}).to_string()),
        ))
        .await?;
    assert_eq!(invalid_upsert_response.status(), StatusCode::BAD_REQUEST);
    let invalid_upsert_body = read_json(invalid_upsert_response).await;
    assert_eq!(invalid_upsert_body["error"], "Invalid request");
    assert_eq!(
        invalid_upsert_body["message"],
        "rate must be greater than 0"
    );

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/statistics/exchange-rates/custom/EUR",
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);

    let proxied_response = app
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/overview?period=month",
            Body::empty(),
        ))
        .await?;
    assert_eq!(proxied_response.status(), StatusCode::OK);
    let proxied_body = read_json(proxied_response).await;
    assert_eq!(proxied_body["runtime"], "python-sidecar");
    assert_eq!(proxied_body["path"], "/api/statistics/overview");

    Ok(())
}

#[tokio::test]
async fn statistics_runtime_handles_legacy_minimal_statistics_schema() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new_minimal()?;
    let app = runtime_router(&fixture);
    let (start_time, end_time) = march_2026_timestamps();

    let category_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!(
                "/api/statistics/category-statistics?startTime={start_time}&endTime={end_time}"
            ),
            Body::empty(),
        ))
        .await?;
    assert_eq!(category_response.status(), StatusCode::OK);
    let category_body = read_json(category_response).await;
    assert_eq!(category_body["success"], true);
    assert_eq!(category_body["result"]["items"][0]["accountId"], "0");

    let pie_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/statistics/category-pie?type=%E6%94%AF%E5%87%BA&start_date=2026-03-01&end_date=2026-03-31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(pie_response.status(), StatusCode::OK);
    let pie_body = read_json(pie_response).await;
    assert_eq!(pie_body["data"][0]["name"], "");
    assert_eq!(pie_body["data"][0]["value"], 5.0);

    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
    upstream: String,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        Self::new_with_upstream("http://127.0.0.1:9".to_string())
    }

    fn new_with_upstream(upstream: String) -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("statistics-http.db");
        init_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            upstream,
        })
    }

    fn new_empty() -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("statistics-empty-http.db");
        init_empty_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            upstream: "http://127.0.0.1:9".to_string(),
        })
    }

    fn new_minimal() -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("statistics-minimal-http.db");
        init_minimal_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            upstream: "http://127.0.0.1:9".to_string(),
        })
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        fixture.upstream.clone(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn init_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            default_currency TEXT DEFAULT 'CNY'
        );
        CREATE TABLE accounts(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0,
            currency TEXT,
            icon TEXT,
            hidden INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            payment_method TEXT DEFAULT '',
            main_category TEXT,
            sub_category TEXT,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0
        );
        CREATE TABLE user_exchange_rates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            from_currency TEXT NOT NULL,
            to_currency TEXT NOT NULL,
            rate REAL NOT NULL,
            source TEXT NOT NULL,
            effective_date TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, from_currency, to_currency, effective_date)
        );
        ",
    )?;
    connection.execute(
        "INSERT INTO users(id, username, default_currency) VALUES (42, 'owner', 'CNY')",
        [],
    )?;
    connection.execute(
        "INSERT INTO accounts(id, user_id, name, type, balance, initial_balance, currency, icon, hidden, created_at, updated_at)
         VALUES (10, 42, 'cash', 'cash', 87.66, 100.0, 'CNY', 'wallet', 0, 'now', 'now'),
                (11, 42, 'bank', 'debit', 100.0, 0.0, 'CNY', 'banknote', 0, 'now', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, created_at)
         VALUES (1, 42, 1, '餐饮', '午餐', 'now'),
                (2, 42, 2, '工资', '', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO bills(user_id, date, type, amount, counterparty, description, payment_method, main_category, sub_category, source_account_id)
         VALUES (42, '2026-03-15 12:00:00', '支出', -12.34, 'Cafe', 'coffee lunch', 'cash', '餐饮', '午餐', 10),
                (42, '2026-03-20 09:00:00', '收入', 100.0, 'ACME', 'salary', 'bank', '工资', '', 11),
                (42, '2026-04-01 09:00:00', '支出', -9.0, 'Cafe', 'coffee april', 'cash', '餐饮', '午餐', 10),
                (77, '2026-03-16 09:00:00', '支出', -999.0, 'Other', 'other user', 'cash', '餐饮', '午餐', 10)",
        [],
    )?;
    connection.execute(
        "INSERT INTO user_exchange_rates(user_id, from_currency, to_currency, rate, source, effective_date, created_at, updated_at)
         VALUES (42, 'CNY', 'USD', 7.12, 'manual', '2026-03-01T12:00:00+00:00', 'now', 'now')",
        [],
    )?;
    Ok(())
}

fn init_empty_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE accounts(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0
        );
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn init_minimal_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE accounts(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL
        );
        INSERT INTO accounts(id, user_id, name) VALUES (20, 42, 'legacy');
        INSERT INTO bills(user_id, date, type, amount)
        VALUES (42, '2026-03-10 08:00:00', '支出', -5.0);
        ",
    )?;
    Ok(())
}

fn march_2026_timestamps() -> (i64, i64) {
    let start = Local
        .with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
        .single()
        .expect("start timestamp");
    let end = Local
        .with_ymd_and_hms(2026, 3, 31, 23, 59, 59)
        .single()
        .expect("end timestamp");
    (start.timestamp(), end.timestamp())
}

fn wide_timestamps() -> (i64, i64) {
    let start = Local
        .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
        .single()
        .expect("wide start timestamp");
    let end = Local
        .with_ymd_and_hms(2026, 12, 31, 23, 59, 59)
        .single()
        .expect("wide end timestamp");
    (start.timestamp(), end.timestamp())
}

fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", TEST_USER_ID)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
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
    let app = Router::new().fallback(any(echo_handler));
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

async fn echo_handler(request: Request<Body>) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.expect("body bytes");
    axum::Json(json!({
        "runtime": "python-sidecar",
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query().unwrap_or(""),
        "body": String::from_utf8_lossy(&body).to_string(),
    }))
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}
