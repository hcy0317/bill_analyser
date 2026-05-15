use std::{error::Error, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{Method, StatusCode},
    Router,
};
use bill_analyser_http::{
    build_router, is_manifest_python_proxied_route, HttpShellConfig, ImportRouteMode, ProxyState,
    MATCHING_RECURRING_CALENDAR_NETWORTH_PROXIED_ROUTE_PATTERNS,
    MATCHING_RECURRING_CALENDAR_NETWORTH_ROUTE_PATTERNS,
};
use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
use rusqlite::{params, Connection};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "matching-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn recurring_calendar_networth_runtime_serves_owned_routes_and_keeps_matching_proxied(
) -> Result<(), Box<dyn Error>> {
    for route in [
        ("GET", "/api/calendar/events"),
        ("GET", "/api/networth/snapshot"),
        ("GET", "/api/recurring/suggestions"),
        ("POST", "/api/recurring/suggestions/detect"),
        ("POST", "/api/recurring/suggestions/{suggestion_id}/accept"),
        ("POST", "/api/recurring/suggestions/{suggestion_id}/reject"),
    ] {
        assert!(MATCHING_RECURRING_CALENDAR_NETWORTH_ROUTE_PATTERNS
            .iter()
            .any(|item| item == &route));
    }
    for route in [
        ("GET", "/api/matching/candidates"),
        ("POST", "/api/matching/candidates/{*candidate_id}/accept"),
        ("GET", "/api/matching/investment-settings"),
    ] {
        assert!(MATCHING_RECURRING_CALENDAR_NETWORTH_PROXIED_ROUTE_PATTERNS
            .iter()
            .any(|item| item == &route));
    }
    assert!(!is_manifest_python_proxied_route(
        "GET",
        "/api/recurring/suggestions"
    ));
    assert!(is_manifest_python_proxied_route(
        "GET",
        "/api/matching/candidates"
    ));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let calendar_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-15",
            Body::empty(),
        ))
        .await?;
    assert_eq!(calendar_response.status(), StatusCode::OK);
    let calendar_body = read_json(calendar_response).await;
    assert_eq!(calendar_body["success"], true);
    assert_eq!(calendar_body["data"]["events"][0]["date"], "2026-03-01");
    assert_eq!(calendar_body["data"]["events"][0]["expense"], 25.5);
    assert_eq!(calendar_body["data"]["events"][1]["income"], 100.0);
    assert_eq!(calendar_body["data"]["events"][2]["transferOut"], 10.0);
    assert!(calendar_body["data"]["events"]
        .as_array()
        .expect("calendar events")
        .iter()
        .any(|event| event["date"] == "2026-03-15" && event["count"] == 1));
    assert_eq!(
        calendar_body["data"]["recurringProjections"][0]["type"],
        "recurring_projection"
    );

    let missing_calendar_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/calendar/events?start_date=2026-03-01",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_calendar_response.status(), StatusCode::BAD_REQUEST);
    let missing_calendar_body = read_json(missing_calendar_response).await;
    assert_eq!(
        missing_calendar_body["message"],
        "start_date and end_date are required"
    );

    let networth_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/networth/snapshot",
            Body::empty(),
        ))
        .await?;
    assert_eq!(networth_response.status(), StatusCode::OK);
    let networth_body = read_json(networth_response).await;
    assert_eq!(networth_body["success"], true);
    assert_eq!(networth_body["data"]["totalAssets"], 1200.12);
    assert_eq!(networth_body["data"]["totalLiabilities"], 300.4);
    assert_eq!(networth_body["data"]["netWorth"], 899.72);
    assert_eq!(networth_body["data"]["accountCount"], 2);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/recurring/suggestions?status=pending&limit=25&offset=0",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    assert_eq!(list_body["data"]["total"], 2);
    assert_eq!(list_body["data"]["items"][0]["patternHash"], "pending-rent");
    assert_eq!(list_body["data"]["items"][0]["sampleBillIds"][0], 1);

    let detect_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/recurring/suggestions/detect",
            Body::empty(),
        ))
        .await?;
    assert_eq!(detect_response.status(), StatusCode::OK);
    let detect_body = read_json(detect_response).await;
    assert_eq!(detect_body["success"], true);
    assert!(
        detect_body["data"]["detected"]
            .as_i64()
            .expect("detected count")
            >= 1,
        "{detect_body}"
    );
    assert!(
        detect_body["data"]["created"]
            .as_i64()
            .expect("created count")
            >= 1,
        "{detect_body}"
    );

    let accept_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/recurring/suggestions/1/accept",
            Body::empty(),
        ))
        .await?;
    let accept_status = accept_response.status();
    let accept_body = read_json(accept_response).await;
    assert_eq!(accept_status, StatusCode::OK, "{accept_body}");
    assert_eq!(accept_body["success"], true);
    assert_eq!(accept_body["data"]["suggestion_id"], 1);
    assert_eq!(accept_body["data"]["status"], "accepted");
    assert!(accepted_recurring_rule_exists(
        &fixture.db_path,
        42,
        "Rent"
    )?);

    let reject_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/recurring/suggestions/2/reject",
            Body::empty(),
        ))
        .await?;
    assert_eq!(reject_response.status(), StatusCode::OK);
    let reject_body = read_json(reject_response).await;
    assert_eq!(reject_body["success"], true);
    assert_eq!(reject_body["data"]["status"], "rejected");

    let repeat_reject_response = app
        .oneshot(authed_request(
            Method::POST,
            "/api/recurring/suggestions/2/reject",
            Body::empty(),
        ))
        .await?;
    assert_eq!(repeat_reject_response.status(), StatusCode::NOT_FOUND);
    let repeat_reject_body = read_json(repeat_reject_response).await;
    assert_eq!(
        repeat_reject_body["message"],
        "Suggestion not found or already processed"
    );

    Ok(())
}

#[tokio::test]
async fn recurring_calendar_networth_runtime_covers_auth_validation_and_config_edges(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    for (method, uri) in [
        (
            Method::GET,
            "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-15",
        ),
        (Method::GET, "/api/networth/snapshot"),
        (Method::GET, "/api/recurring/suggestions"),
        (Method::POST, "/api/recurring/suggestions/detect"),
        (Method::POST, "/api/recurring/suggestions/1/accept"),
        (Method::POST, "/api/recurring/suggestions/1/reject"),
    ] {
        let response = app
            .clone()
            .oneshot(trusted_request_without_user(method, uri))
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            read_json(response).await["message"],
            "Missing Rust route user id"
        );
    }

    let invalid_date = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/calendar/events?start_date=2026-03-01&end_date=03/15/2026",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_date.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_date).await["message"],
        "end_date must use YYYY-MM-DD format"
    );

    let invalid_limit = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/recurring/suggestions?limit=not-a-number",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_limit.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(read_json(invalid_limit).await["success"], false);

    let invalid_offset = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/recurring/suggestions?offset=not-a-number",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_offset.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(read_json(invalid_offset).await["success"], false);

    let default_list = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/recurring/suggestions?status=",
            Body::empty(),
        ))
        .await?;
    assert_eq!(default_list.status(), StatusCode::OK);
    let default_list_body = read_json(default_list).await;
    assert_eq!(default_list_body["data"]["limit"], 200);
    assert_eq!(default_list_body["data"]["offset"], 0);

    let missing_accept = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/recurring/suggestions/999/accept",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_accept.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_accept).await["message"],
        "Suggestion not found or already processed"
    );

    let no_db_path_app = runtime_router_without_db_path();
    let no_db_path_response = no_db_path_app
        .oneshot(authed_request(
            Method::GET,
            "/api/networth/snapshot",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_db_path_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(read_json(no_db_path_response).await["error"]
        .as_str()
        .expect("error")
        .contains("BILL_ANALYSER_SQLITE_DB_PATH"));

    let missing_parent = fixture
        ._temp_dir
        .path()
        .join("missing-parent")
        .join("runtime.db");
    let bad_path_response = runtime_router_for_path(&missing_parent)
        .oneshot(authed_request(
            Method::GET,
            "/api/networth/snapshot",
            Body::empty(),
        ))
        .await?;
    assert_eq!(bad_path_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(read_json(bad_path_response).await["success"], false);

    let malformed_dir = tempfile::tempdir()?;
    let malformed_calendar_db = malformed_dir.path().join("calendar.db");
    Connection::open(&malformed_calendar_db)?
        .execute("CREATE TABLE bills(id INTEGER PRIMARY KEY)", [])?;
    let malformed_calendar = runtime_router_for_path(&malformed_calendar_db)
        .oneshot(authed_request(
            Method::GET,
            "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-15",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        malformed_calendar.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(read_json(malformed_calendar).await["success"], false);

    let malformed_networth_db = malformed_dir.path().join("networth.db");
    Connection::open(&malformed_networth_db)?.execute(
        "CREATE TABLE accounts(id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )?;
    let malformed_networth = runtime_router_for_path(&malformed_networth_db)
        .oneshot(authed_request(
            Method::GET,
            "/api/networth/snapshot",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        malformed_networth.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(read_json(malformed_networth).await["success"], false);

    let projection_error_db = malformed_dir.path().join("projection-error.db");
    init_calendar_schema_with_malformed_recurring(&projection_error_db)?;
    let projection_error_response = runtime_router_for_path(&projection_error_db)
        .oneshot(authed_request(
            Method::GET,
            "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(projection_error_response.status(), StatusCode::OK);
    let projection_error_body = read_json(projection_error_response).await;
    assert_eq!(projection_error_body["success"], true);
    assert_eq!(
        projection_error_body["data"]["events"][0]["date"],
        "2026-03-15"
    );
    assert_eq!(
        projection_error_body["data"]["recurringProjections"]
            .as_array()
            .expect("recurring projections")
            .len(),
        0
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
        let db_path = temp_dir.path().join("matching-http.db");
        init_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
        })
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    runtime_router_for_path(&fixture.db_path)
}

fn runtime_router_for_path(db_path: &Path) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:9".to_string(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(db_path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn runtime_router_without_db_path() -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:9".to_string(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
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
            username TEXT NOT NULL
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
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            main_category TEXT,
            sub_category TEXT,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_recurring INTEGER
        );
        CREATE TABLE bill_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL
        );
        CREATE TABLE recurring_bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            template_id INTEGER,
            name TEXT NOT NULL,
            description TEXT,
            type TEXT NOT NULL,
            category TEXT,
            amount REAL NOT NULL,
            account TEXT,
            counterparty TEXT,
            tag TEXT,
            comment TEXT,
            frequency TEXT NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            next_date TEXT NOT NULL,
            enabled BOOLEAN DEFAULT 1,
            auto_create BOOLEAN DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE recurring_suggestions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            pattern_hash TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            source_account_id INTEGER,
            destination_account_id TEXT,
            counterparty TEXT,
            frequency TEXT NOT NULL,
            detected_interval_days REAL,
            confidence_score REAL NOT NULL DEFAULT 0,
            sample_count INTEGER NOT NULL DEFAULT 0,
            sample_bill_ids_json TEXT,
            first_occurrence TEXT,
            last_occurrence TEXT,
            suggested_next_date TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(user_id, pattern_hash)
        );
        ",
    )?;
    connection.execute("INSERT INTO users(id, username) VALUES (42, 'owner')", [])?;
    connection.execute(
        "INSERT INTO accounts(id, user_id, name, type, balance, initial_balance, currency, icon, hidden, created_at, updated_at)
         VALUES (10, 42, 'Cash', 'cash', 1200.12, 0.0, 'CNY', 'wallet', 0, 'now', 'now'),
                (11, 42, 'Card', 'credit_card', -300.4, 0.0, 'USD', 'card', 0, 'now', 'now'),
                (12, 42, 'Hidden', 'cash', 999.0, 0.0, 'CNY', 'wallet', 1, 'now', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO bills(user_id, date, type, amount, counterparty, description, main_category, sub_category, source_account_id, destination_account_id, destination_amount)
         VALUES (42, '2026-03-01T08:30:00', 'expense', -25.5, 'Cafe', 'Breakfast', 'Food', 'Coffee', 10, 0, 0),
                (42, '2026-03-02', 'income', 100.0, 'Client', 'Invoice', 'Work', 'Consulting', 10, 0, 0),
                (42, '2026-03-03', 'transfer', 10.0, 'Savings', 'Move', 'Transfer', 'Internal', 10, 11, 10.0),
                (42, '2026-03-15T23:59:59', 'expense', -7.25, 'Late Store', 'End date timestamp', 'Food', 'Snack', 10, 0, 0),
                (77, '2026-03-01', 'expense', -999.0, 'Other', 'Other user', 'Food', '', 10, 0, 0)",
        [],
    )?;
    let monthly_dates = recent_monthly_dates(Local::now().date_naive());
    for date in &monthly_dates {
        connection.execute(
            "INSERT INTO bills(user_id, date, type, amount, counterparty, description, main_category, sub_category, source_account_id)
             VALUES (42, ?1, 'expense', -9.99, 'Gym', 'Membership', 'Health', 'Fitness', 10)",
            params![date],
        )?;
    }
    connection.execute(
        "INSERT INTO recurring_bills(user_id, name, description, type, amount, account, counterparty, frequency, start_date, next_date, enabled, auto_create, display_order)
         VALUES (42, 'Weekly Rent', 'Rent projection', 'expense', 9.99, '10', 'Landlord', 'weekly', '2026-03-01', '2026-03-01', 1, 0, 0)",
        [],
    )?;
    connection.execute(
        "INSERT INTO recurring_suggestions(user_id, pattern_hash, name, description, type, amount, source_account_id, counterparty, frequency, detected_interval_days, confidence_score, sample_count, sample_bill_ids_json, first_occurrence, last_occurrence, suggested_next_date, status, created_at, updated_at)
         VALUES (42, 'pending-rent', 'Rent', 'monthly rent', 'expense', 9.99, 10, 'Landlord', 'monthly', 30, 0.9, 3, '[1,2,3]', '2026-01-01', '2026-03-01', '2026-04-01', 'pending', 'now', 'now'),
                (42, 'pending-phone', 'Phone', 'phone bill', 'expense', 19.99, 10, 'Carrier', 'monthly', 30, 0.8, 3, '[4,5,6]', '2026-01-02', '2026-03-02', '2026-04-02', 'pending', 'now', 'now'),
                (77, 'other-user', 'Other', 'other user', 'expense', 99.0, 10, 'Other', 'monthly', 30, 1.0, 3, '[9]', '2026-01-01', '2026-03-01', '2026-04-01', 'pending', 'now', 'now')",
        [],
    )?;
    Ok(())
}

fn init_calendar_schema_with_malformed_recurring(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            main_category TEXT,
            sub_category TEXT,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0
        );
        CREATE TABLE recurring_bills(
            id INTEGER PRIMARY KEY,
            name TEXT
        );
        ",
    )?;
    connection.execute(
        "INSERT INTO bills(user_id, date, type, amount, counterparty, description, main_category, sub_category, source_account_id)
         VALUES (42, '2026-03-15T23:59:59', 'expense', -7.25, 'Late Store', 'End date timestamp', 'Food', 'Snack', 10)",
        [],
    )?;
    Ok(())
}

fn recent_monthly_dates(today: NaiveDate) -> Vec<String> {
    let start = first_day_of_month(today) - ChronoDuration::days(92);
    (0..3)
        .map(|index| start + ChronoDuration::days(30 * index))
        .map(|date| date.format("%Y-%m-%d").to_string())
        .collect()
}

fn first_day_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("valid month start")
}

fn accepted_recurring_rule_exists(
    path: &Path,
    user_id: i64,
    name: &str,
) -> Result<bool, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let count = connection.query_row(
        "SELECT COUNT(*) FROM recurring_bills WHERE user_id = ? AND name = ?",
        params![user_id, name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(count > 0)
}

fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .header("x-bill-analyser-user-id", TEST_USER_ID)
        .body(body)
        .expect("request")
}

fn trusted_request_without_user(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(Body::empty())
        .expect("request")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json")
}
