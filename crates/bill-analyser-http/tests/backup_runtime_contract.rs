use std::{error::Error, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use bill_analyser_http::{
    build_router, is_manifest_python_proxied_route, HttpShellConfig, ImportRouteMode, ProxyState,
    BACKUP_OPS_PROXIED_ROUTE_PATTERNS, BACKUP_OPS_ROUTE_PATTERNS,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_TRUST_SECRET: &str = "backup-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn backup_jobs_runtime_serves_list_validation_create_and_update() -> Result<(), Box<dyn Error>>
{
    assert!(BACKUP_OPS_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/backup/jobs")));
    assert!(BACKUP_OPS_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/backup/jobs")));
    assert!(BACKUP_OPS_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/backup/restore/verify")));
    assert!(!is_manifest_python_proxied_route("GET", "/api/backup/jobs"));
    assert!(!is_manifest_python_proxied_route(
        "POST",
        "/api/backup/jobs"
    ));
    assert!(is_manifest_python_proxied_route(
        "POST",
        "/api/backup/restore/verify"
    ));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let empty_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/backup/jobs",
            Body::empty(),
        ))
        .await?;
    assert_eq!(empty_response.status(), StatusCode::OK);
    let empty_body = read_json(empty_response).await;
    assert_eq!(empty_body["success"], true);
    assert_eq!(empty_body["data"].as_array().expect("jobs").len(), 0);

    let invalid_response = app
        .clone()
        .oneshot(json_request(Method::POST, "/api/backup/jobs", json!({})))
        .await?;
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    let invalid_body = read_json(invalid_response).await;
    assert_eq!(invalid_body["success"], false);
    assert_eq!(invalid_body["error"], "job_type is required");
    assert_eq!(
        audit_count(&fixture.db_path, "backup_job_saved", "failed")?,
        1
    );

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/backup/jobs",
            json!({
                "job_type": " manual ",
                "retention_count": null,
                "enabled": null
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["data"]["id"], 1);
    assert_eq!(create_body["data"]["job_type"], "manual");
    assert_eq!(create_body["data"]["retention_days"], 30);
    assert_eq!(create_body["data"]["retention_count"], 10);
    assert_eq!(create_body["data"]["enabled"], true);
    assert_eq!(
        audit_count(&fixture.db_path, "backup_job_saved", "success")?,
        1
    );
    let first_audit = latest_audit(&fixture.db_path, "backup_job_saved", "success")?;
    assert_eq!(first_audit.ip_address.as_deref(), Some("203.0.113.42"));
    assert_eq!(first_audit.details["user_id"], 42);

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/backup/jobs",
            json!({
                "id": "1",
                "job_type": "daily",
                "schedule_expr": " 0 3 * * * ",
                "retention_days": "45",
                "retention_count": "6",
                "enabled": false,
                "last_status": 200
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["data"]["id"], 1);
    assert_eq!(update_body["data"]["schedule_expr"], "0 3 * * *");
    assert_eq!(update_body["data"]["retention_days"], 45);
    assert_eq!(update_body["data"]["retention_count"], 6);
    assert_eq!(update_body["data"]["enabled"], false);
    assert_eq!(update_body["data"]["last_status"], "200");

    let duplicate_type_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/backup/jobs",
            json!({
                "job_type": "daily",
                "schedule_expr": "0 4 * * *",
                "retention_days": 90,
                "retention_count": 8,
            }),
        ))
        .await?;
    assert_eq!(duplicate_type_response.status(), StatusCode::OK);
    let duplicate_type_body = read_json(duplicate_type_response).await;
    assert_eq!(duplicate_type_body["data"]["id"], 1);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/backup/jobs",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    let jobs = list_body["data"].as_array().expect("jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["job_type"], "daily");
    assert_eq!(jobs[0]["retention_days"], 90);
    assert_eq!(jobs[0]["retention_count"], 8);
    assert_eq!(jobs[0]["enabled"], true);
    assert_eq!(jobs[0]["last_status"], Value::Null);
    assert_eq!(
        audit_count(&fixture.db_path, "backup_job_saved", "success")?,
        3
    );

    let other_user_list = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/backup/jobs",
            Body::empty(),
            "77",
        ))
        .await?;
    assert_eq!(other_user_list.status(), StatusCode::OK);
    let other_user_list_body = read_json(other_user_list).await;
    assert_eq!(
        other_user_list_body["data"].as_array().expect("jobs").len(),
        0
    );

    let foreign_update_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/backup/jobs",
            json!({"id": 1, "job_type": "daily"}),
            "77",
        ))
        .await?;
    assert_eq!(foreign_update_response.status(), StatusCode::NOT_FOUND);
    let foreign_update_body = read_json(foreign_update_response).await;
    assert_eq!(foreign_update_body["success"], false);
    assert_eq!(foreign_update_body["error"], "backup job not found");
    let failed_audit = latest_audit(&fixture.db_path, "backup_job_saved", "failed")?;
    assert_eq!(failed_audit.details["user_id"], 77);

    for invalid_payload in [
        json!({"job_type": "daily", "retention_days": "bad"}),
        json!({"job_type": "daily", "retention_count": "bad"}),
        json!({"job_type": "daily", "retention_count": -1}),
        json!({"job_type": "daily", "id": 0}),
    ] {
        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/backup/jobs",
                invalid_payload,
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    Ok(())
}

#[tokio::test]
async fn backup_jobs_runtime_requires_auth_and_sqlite_path() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/backup/jobs")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:5001",
        Duration::from_secs(1),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )?
    .with_trusted_user_header_secret(TEST_TRUST_SECRET);
    let state = ProxyState::new(config)?;
    let missing_db_app = build_router(state);
    let missing_db = missing_db_app
        .oneshot(authed_request(
            Method::GET,
            "/api/backup/jobs",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_db.status(), StatusCode::SERVICE_UNAVAILABLE);
    let missing_db_body = read_json(missing_db).await;
    assert!(missing_db_body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("BILL_ANALYSER_SQLITE_DB_PATH"));

    Ok(())
}

struct RuntimeFixture {
    _temp: TempDir,
    db_path: std::path::PathBuf,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let temp = TempDir::new()?;
        let db_path = temp.path().join("backup-runtime.db");
        Ok(Self {
            _temp: temp,
            db_path,
        })
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:5001",
        Duration::from_secs(1),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.to_string_lossy().to_string())
    .with_trusted_user_header_secret(TEST_TRUST_SECRET);
    build_router(ProxyState::new(config).expect("state"))
}

fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    authed_request_for_user(method, uri, body, TEST_USER_ID)
}

fn authed_request_for_user(method: Method, uri: &str, body: Body, user_id: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-bill-analyser-trusted-user-secret", TEST_TRUST_SECRET)
        .header("x-user-id", user_id)
        .header("user-agent", "backup-runtime-contract")
        .header("x-forwarded-for", "203.0.113.42, 10.0.0.1")
        .body(body)
        .expect("request")
}

fn json_request(method: Method, uri: &str, payload: Value) -> Request<Body> {
    json_request_for_user(method, uri, payload, TEST_USER_ID)
}

fn json_request_for_user(
    method: Method,
    uri: &str,
    payload: Value,
    user_id: &str,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-bill-analyser-trusted-user-secret", TEST_TRUST_SECRET)
        .header("x-user-id", user_id)
        .header("user-agent", "backup-runtime-contract")
        .header("x-forwarded-for", "203.0.113.42, 10.0.0.1")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("request")
}

async fn read_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    serde_json::from_slice(&body).expect("json")
}

fn audit_count(path: &Path, operation_type: &str, status: &str) -> Result<i64, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM audit_logs WHERE operation_type = ?1 AND status = ?2",
        [operation_type, status],
        |row| row.get(0),
    )?)
}

struct AuditRow {
    details: Value,
    ip_address: Option<String>,
}

fn latest_audit(
    path: &Path,
    operation_type: &str,
    status: &str,
) -> Result<AuditRow, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    let (details, ip_address): (Option<String>, Option<String>) = connection.query_row(
        r#"
        SELECT details, ip_address
        FROM audit_logs
        WHERE operation_type = ?1
          AND status = ?2
        ORDER BY id DESC
        LIMIT 1
        "#,
        [operation_type, status],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(AuditRow {
        details: details
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or(Value::Null),
        ip_address,
    })
}
