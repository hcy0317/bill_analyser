use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode, Uri},
    routing::any,
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_http::{
    build_router, is_manifest_python_proxied_route, HttpShellConfig, ImportRouteMode, ProxyState,
    BACKUP_OPS_PROXIED_ROUTE_PATTERNS, BACKUP_OPS_ROUTE_PATTERNS,
};
use chrono::{Duration as ChronoDuration, Local};
use ring::hmac;
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tower::ServiceExt;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const TEST_TRUST_SECRET: &str = "backup-route-secret";
const TEST_AUTH_SECRET: &str = "backup-auth-secret";
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
    assert!(BACKUP_OPS_PROXIED_ROUTE_PATTERNS.is_empty());
    assert!(!is_manifest_python_proxied_route("GET", "/api/backup/jobs"));
    assert!(!is_manifest_python_proxied_route(
        "POST",
        "/api/backup/jobs"
    ));
    assert!(!is_manifest_python_proxied_route(
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

#[tokio::test]
async fn backup_file_runtime_creates_lists_downloads_deletes_and_cleans_up(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::create_dir_all(fixture.data_dir.join("nested"))?;
    fs::write(
        fixture.data_dir.join("nested").join("records.json"),
        b"records",
    )?;
    let app = runtime_router(&fixture);

    for route in [
        ("GET", "/api/backup/"),
        ("POST", "/api/backup/create"),
        ("POST", "/api/backup/restore/verify"),
        ("GET", "/api/backup/download/{filename}"),
        ("DELETE", "/api/backup/delete/{filename}"),
        ("POST", "/api/backup/cleanup"),
    ] {
        assert!(bill_analyser_http::BACKUP_OPS_ROUTE_PATTERNS
            .iter()
            .any(|item| item == &route));
    }
    assert!(!is_manifest_python_proxied_route("GET", "/api/backup/"));

    let create_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/backup/create",
            Body::empty(),
        ))
        .await?;
    let create_status = create_response.status();
    let create_body = read_json(create_response).await;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "create backup response body: {create_body}"
    );
    let filename = create_body["data"]["filename"]
        .as_str()
        .expect("created filename")
        .to_string();
    assert!(filename.starts_with("backup_"));
    assert!(filename.ends_with(".zip"));
    assert_eq!(create_body["data"]["valid_zip"], true);
    assert_eq!(create_body["data"]["ready_to_restore"], true);
    assert_eq!(create_body["data"]["encrypted"], false);
    assert!(fixture.backup_dir.join(&filename).exists());
    assert_eq!(
        audit_count(&fixture.db_path, "backup_created", "success")?,
        1
    );

    let list_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/backup/", Body::empty()))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["data"].as_array().expect("backups").len(), 1);
    assert_eq!(list_body["data"][0]["filename"], filename);
    assert_eq!(list_body["data"][0]["recordStatus"], "created");
    assert_eq!(list_body["data"][0]["metadata_checksum_matched"], true);

    let verify_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/backup/restore/verify",
            json!({"filename": filename}),
        ))
        .await?;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_body = read_json(verify_response).await;
    assert_eq!(verify_body["success"], true);
    assert_eq!(verify_body["data"]["filename"], filename);

    let download_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/backup/download/{filename}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(download_response.status(), StatusCode::OK);
    let content_disposition = download_response
        .headers()
        .get("content-disposition")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_disposition.contains(&filename));
    let downloaded = to_bytes(download_response.into_body(), 1024 * 1024).await?;
    assert!(!downloaded.is_empty());

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/backup/delete/{filename}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert!(!fixture.backup_dir.join(&filename).exists());
    assert_eq!(
        audit_count(&fixture.db_path, "backup_deleted", "success")?,
        1
    );

    let create_again_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/backup/create",
            Body::empty(),
        ))
        .await?;
    assert_eq!(create_again_response.status(), StatusCode::OK);
    let create_again_body = read_json(create_again_response).await;
    let second_filename = create_again_body["data"]["filename"]
        .as_str()
        .expect("second filename")
        .to_string();
    assert_ne!(second_filename, filename);

    let invalid_json_cleanup = app
        .clone()
        .oneshot(raw_json_request(Method::POST, "/api/backup/cleanup", "{"))
        .await?;
    assert_eq!(invalid_json_cleanup.status(), StatusCode::BAD_REQUEST);
    assert!(fixture.backup_dir.join(&second_filename).exists());

    let cleanup_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/backup/cleanup",
            json!({"keep_count": 0}),
        ))
        .await?;
    assert_eq!(cleanup_response.status(), StatusCode::OK);
    let cleanup_body = read_json(cleanup_response).await;
    assert_eq!(cleanup_body["data"]["deleted_count"], 1);
    assert_eq!(cleanup_body["data"]["kept_count"], 0);
    assert_eq!(
        audit_count(&fixture.db_path, "backup_cleanup", "success")?,
        1
    );

    let invalid_cleanup = app
        .oneshot(json_request(
            Method::POST,
            "/api/backup/cleanup",
            json!({"keep_count": -1}),
        ))
        .await?;
    assert_eq!(invalid_cleanup.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn backup_sync_runtime_rejects_invalid_config_before_creating_backup(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::write(fixture.data_dir.join("records.json"), b"records")?;
    let app = runtime_router(&fixture);

    assert!(BACKUP_OPS_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/backup/sync")));
    assert!(!is_manifest_python_proxied_route(
        "POST",
        "/api/backup/sync"
    ));

    for payload in [
        json!({}),
        json!({"provider": "unknown", "endpoint": "http://127.0.0.1:9"}),
        json!({
            "provider": "s3",
            "endpoint": "http://127.0.0.1:9",
            "bucket": "bucket",
            "access_key": "ak",
            "secret_key": "sk",
            "prefix": "../escape",
        }),
        json!({"provider": "webdav", "endpoint": "ftp://example.test"}),
        json!({"provider": "webdav", "endpoint": "http://user:pass@127.0.0.1/"}),
        json!({
            "provider": "azure",
            "endpoint": "http://127.0.0.1:9",
            "bucket": "container",
            "access_key": "account",
            "secret_key": "not-base64",
        }),
        json!({
            "provider": "s3",
            "endpoint": "http://127.0.0.1:9",
            "access_key": "ak",
            "secret_key": "sk",
        }),
    ] {
        let response = app
            .clone()
            .oneshot(json_request(Method::POST, "/api/backup/sync", payload))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = read_json(response).await;
        assert_eq!(body["success"], false);
    }

    assert!(fixture.backup_dir.read_dir()?.next().is_none());
    assert_eq!(
        audit_count(&fixture.db_path, "backup_cloud_synced", "failed")?,
        7
    );

    Ok(())
}

#[tokio::test]
async fn backup_sync_runtime_records_provider_failure_with_safe_payload(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::write(fixture.data_dir.join("records.json"), b"records")?;
    let server = FakeCloudServer::start_with_status(StatusCode::INTERNAL_SERVER_ERROR).await?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/api/backup/sync",
            json!({
                "provider": "s3",
                "endpoint": server.endpoint(),
                "bucket": "bucket",
                "access_key": "ak",
                "secret_key": "sk",
                "prefix": "prefix",
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["data"]["provider"], "s3");
    assert_eq!(body["data"]["safe_config"]["secret_key"], "********");
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("s3 upload failed with status 500"));
    assert_eq!(
        audit_count(&fixture.db_path, "backup_created", "success")?,
        1
    );
    assert_eq!(
        audit_count(&fixture.db_path, "backup_cloud_synced", "failed")?,
        1
    );
    assert!(server
        .requests()
        .iter()
        .any(|request| request.method == "PUT"));

    Ok(())
}

#[tokio::test]
async fn backup_sync_runtime_reports_payload_and_prepare_errors() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let invalid_payload = app
        .clone()
        .oneshot(raw_json_request(Method::POST, "/api/backup/sync", "[]"))
        .await?;
    assert_eq!(invalid_payload.status(), StatusCode::BAD_REQUEST);
    let invalid_body = read_json(invalid_payload).await;
    assert_eq!(invalid_body["success"], false);
    assert_eq!(invalid_body["error"], "JSON body must be an object");

    let source_error_fixture = RuntimeFixture::new()?;
    fs::remove_dir_all(&source_error_fixture.data_dir)?;
    fs::write(&source_error_fixture.data_dir, b"not a directory")?;
    let source_error_app = runtime_router(&source_error_fixture);
    let source_error = source_error_app
        .oneshot(json_request(
            Method::POST,
            "/api/backup/sync",
            json!({
                "provider": "webdav",
                "endpoint": "http://127.0.0.1:9",
                "prefix": "prepare/errors",
            }),
        ))
        .await?;
    assert_eq!(source_error.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let source_error_body = read_json(source_error).await;
    assert_eq!(source_error_body["success"], false);
    assert_eq!(
        audit_count(&fixture.db_path, "backup_cloud_synced", "failed")?,
        1
    );
    assert_eq!(
        audit_count(
            &source_error_fixture.db_path,
            "backup_cloud_synced",
            "failed"
        )?,
        1
    );

    Ok(())
}

#[tokio::test]
async fn backup_sync_runtime_uploads_to_webdav_and_redacts_secrets() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::write(fixture.data_dir.join("records.json"), b"records")?;
    let server = FakeCloudServer::start().await?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(json_request(
            Method::POST,
            "/api/backup/sync",
            json!({
                "config": {
                    "provider": "webdav",
                    "endpoint": server.endpoint(),
                    "access_key": "alice",
                    "secret_key": "secret",
                    "prefix": "remote/path",
                },
                "step_up_token": "trusted-header-auth-does-not-use-this-field",
            }),
        ))
        .await?;
    let status = response.status();
    let body = read_json(response).await;
    assert_eq!(status, StatusCode::OK, "sync response body: {body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["provider"], "webdav");
    assert_eq!(body["data"]["safe_config"]["access_key"], "********");
    assert_eq!(body["data"]["safe_config"]["secret_key"], "********");
    assert!(body["data"]["object_key"]
        .as_str()
        .unwrap_or_default()
        .starts_with("remote/path/backup_"));

    let requests = server.requests();
    assert!(requests
        .iter()
        .any(|request| request.method == "MKCOL" && request.path == "/remote"));
    assert!(requests
        .iter()
        .any(|request| request.method == "MKCOL" && request.path == "/remote/path"));
    let put = requests
        .iter()
        .find(|request| request.method == "PUT")
        .expect("PUT request");
    assert!(put.path.starts_with("/remote/path/backup_"));
    assert!(put.path.ends_with(".zip"));
    assert!(put.body_len > 0);
    assert_eq!(
        put.headers.get("authorization").map(String::as_str),
        Some("Basic YWxpY2U6c2VjcmV0")
    );
    assert_eq!(
        audit_count(&fixture.db_path, "backup_created", "success")?,
        1
    );
    assert_eq!(
        audit_count(&fixture.db_path, "backup_cloud_synced", "success")?,
        1
    );

    Ok(())
}

#[tokio::test]
async fn backup_sync_runtime_signs_object_storage_provider_requests() -> Result<(), Box<dyn Error>>
{
    for provider in ["oss", "s3", "cos", "azure"] {
        let fixture = RuntimeFixture::new()?;
        fs::write(
            fixture.data_dir.join("records.json"),
            format!("{provider}-records"),
        )?;
        let server = FakeCloudServer::start().await?;
        let app = runtime_router(&fixture);
        let secret_key = if provider == "azure" {
            general_purpose::STANDARD.encode("azure-secret")
        } else {
            "sk".to_string()
        };
        let access_key = if provider == "azure" { "account" } else { "ak" };
        let bucket = if provider == "azure" {
            "container"
        } else {
            "bucket"
        };
        let response = app
            .oneshot(json_request(
                Method::POST,
                "/api/backup/sync",
                json!({
                    "provider": provider,
                    "endpoint": server.endpoint(),
                    "bucket": bucket,
                    "access_key": access_key,
                    "secret_key": secret_key,
                    "prefix": "prefix",
                    "region": "ap-shanghai",
                }),
            ))
            .await?;
        let status = response.status();
        let body = read_json(response).await;
        assert_eq!(status, StatusCode::OK, "{provider} body: {body}");
        assert_eq!(body["data"]["provider"], provider);
        assert_eq!(body["data"]["safe_config"]["secret_key"], "********");

        let requests = server.requests();
        let put = requests
            .iter()
            .find(|request| request.method == "PUT")
            .unwrap_or_else(|| panic!("{provider} PUT request"));
        assert!(
            put.path.contains("/prefix/backup_"),
            "{provider}: {}",
            put.path
        );
        assert!(put.body_len > 0);
        let authorization = put
            .headers
            .get("authorization")
            .map(String::as_str)
            .unwrap_or_default();
        match provider {
            "oss" => assert!(authorization.starts_with("OSS ak:"), "{authorization}"),
            "s3" => assert!(
                authorization.starts_with("AWS4-HMAC-SHA256 Credential=ak/"),
                "{authorization}"
            ),
            "cos" => assert!(
                authorization.starts_with("q-sign-algorithm=sha1&q-ak=ak"),
                "{authorization}"
            ),
            "azure" => {
                assert!(
                    authorization.starts_with("SharedKey account:"),
                    "{authorization}"
                );
                assert_eq!(
                    put.headers.get("x-ms-blob-type").map(String::as_str),
                    Some("BlockBlob")
                );
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

#[tokio::test]
async fn backup_file_runtime_requires_step_up_for_bearer_sessions() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::write(fixture.data_dir.join("records.json"), b"records")?;
    let access_token = test_jwt(42, "access", ChronoDuration::hours(1));
    let step_up_token = test_jwt(42, "step_up", ChronoDuration::hours(1));
    seed_bearer_session(&fixture.db_path, 42, &access_token)?;
    let app = runtime_router_with_auth_secret(&fixture);

    let missing_step_up = app
        .clone()
        .oneshot(bearer_request(
            Method::GET,
            "/api/backup/",
            &access_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_step_up.status(), StatusCode::UNAUTHORIZED);
    let missing_body = read_json(missing_step_up).await;
    assert_eq!(
        missing_body["error"],
        "step-up token is required for backup file operation"
    );

    let listed = app
        .clone()
        .oneshot(bearer_request_with_step_up(
            Method::GET,
            "/api/backup/",
            &access_token,
            &step_up_token,
            Body::empty(),
        ))
        .await?;
    assert_eq!(listed.status(), StatusCode::OK);

    let foreign_step_up = test_jwt(77, "step_up", ChronoDuration::hours(1));
    let rejected = app
        .oneshot(bearer_request_with_step_up(
            Method::GET,
            "/api/backup/",
            &access_token,
            &foreign_step_up,
            Body::empty(),
        ))
        .await?;
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn backup_file_runtime_snapshots_sqlite_db_under_data_dir() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new_with_db_in_data()?;
    seed_source_marker_db(&fixture.db_path, "original")?;
    fs::write(fixture.data_dir.join("note.txt"), b"original-note")?;
    let app = runtime_router(&fixture);

    let create_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/backup/create",
            Body::empty(),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = read_json(create_response).await;
    let filename = create_body["data"]["filename"]
        .as_str()
        .expect("created filename")
        .to_string();
    assert_backup_zip_contains_sqlite_marker(&fixture.backup_dir.join(&filename), "original")?;

    seed_source_marker_db(&fixture.db_path, "mutated")?;
    fs::write(fixture.data_dir.join("note.txt"), b"mutated-note")?;
    let restore_response = app
        .oneshot(authed_request(
            Method::POST,
            &format!("/api/backup/restore/{filename}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(restore_response.status(), StatusCode::OK);
    assert_eq!(source_marker(&fixture.db_path)?, "original");
    assert_eq!(
        fs::read(fixture.data_dir.join("note.txt"))?,
        b"original-note"
    );

    Ok(())
}

#[tokio::test]
async fn backup_file_runtime_restores_plain_zip_and_rejects_unsafe_archive(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::write(fixture.data_dir.join("old.txt"), b"old")?;
    let app = runtime_router(&fixture);
    let backup_path = fixture.backup_dir.join("backup_20260515_120000.zip");
    write_zip_entries(
        &backup_path,
        &[("data/restored.txt", b"restored".as_slice())],
    )?;

    let restore_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/backup/restore/backup_20260515_120000.zip",
            Body::empty(),
        ))
        .await?;
    assert_eq!(restore_response.status(), StatusCode::OK);
    let restore_body = read_json(restore_response).await;
    assert_eq!(restore_body["success"], true);
    assert_eq!(
        fs::read(fixture.data_dir.join("restored.txt"))?,
        b"restored"
    );
    assert!(!fixture.data_dir.join("old.txt").exists());
    assert!(fixture.backup_dir.read_dir()?.any(|entry| entry
        .expect("entry")
        .file_name()
        .to_string_lossy()
        .starts_with("before_restore_")));
    assert_eq!(
        audit_count(&fixture.db_path, "backup_restored", "success")?,
        1
    );

    let unsafe_path = fixture.backup_dir.join("backup_20260515_130000.zip");
    write_zip_entries(
        &unsafe_path,
        &[
            ("data/restored.txt", b"restored".as_slice()),
            ("../escape.txt", b"escape".as_slice()),
        ],
    )?;
    let unsafe_verify = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/backup/restore/verify",
            json!({"filename": "backup_20260515_130000.zip"}),
        ))
        .await?;
    assert_eq!(unsafe_verify.status(), StatusCode::BAD_REQUEST);
    let unsafe_body = read_json(unsafe_verify).await;
    assert_eq!(unsafe_body["success"], false);
    assert!(unsafe_body["data"]["error"]
        .as_str()
        .unwrap_or_default()
        .contains("不安全路径"));

    Ok(())
}

#[tokio::test]
async fn backup_file_runtime_handles_encrypted_create_verify_and_restore(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    fs::write(fixture.data_dir.join("secret.txt"), b"original")?;
    let app = runtime_router_with_backup_key(&fixture, Some("local-backup-secret"));

    let create_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/backup/create",
            Body::empty(),
        ))
        .await?;
    let create_status = create_response.status();
    let create_body = read_json(create_response).await;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "encrypted create response body: {create_body}"
    );
    let filename = create_body["data"]["filename"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(filename.ends_with(".zip.enc"));
    assert_eq!(create_body["data"]["encrypted"], true);
    assert_eq!(create_body["data"]["ready_to_restore"], true);

    fs::write(fixture.data_dir.join("secret.txt"), b"mutated")?;
    let restore_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            &format!("/api/backup/restore/{filename}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(restore_response.status(), StatusCode::OK);
    assert_eq!(fs::read(fixture.data_dir.join("secret.txt"))?, b"original");

    let app_without_key = runtime_router(&fixture);
    let verify_without_key = app_without_key
        .oneshot(json_request(
            Method::POST,
            "/api/backup/restore/verify",
            json!({"filename": filename}),
        ))
        .await?;
    assert_eq!(verify_without_key.status(), StatusCode::BAD_REQUEST);
    let verify_without_key_body = read_json(verify_without_key).await;
    assert_eq!(
        verify_without_key_body["data"]["error"],
        "backup encryption key is not configured"
    );

    Ok(())
}

struct RuntimeFixture {
    _temp: TempDir,
    db_path: std::path::PathBuf,
    data_dir: std::path::PathBuf,
    backup_dir: std::path::PathBuf,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("backup-runtime-fixtures");
        fs::create_dir_all(&fixture_root)?;
        let temp = TempDir::new_in(fixture_root)?;
        let db_path = temp.path().join("backup-runtime.db");
        let data_dir = temp.path().join("data");
        let backup_dir = temp.path().join("backup");
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&backup_dir)?;
        Ok(Self {
            _temp: temp,
            db_path,
            data_dir,
            backup_dir,
        })
    }

    fn new_with_db_in_data() -> Result<Self, Box<dyn Error>> {
        let mut fixture = Self::new()?;
        fixture.db_path = fixture.data_dir.join("bills.db");
        Ok(fixture)
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    runtime_router_with_backup_key(fixture, None)
}

fn runtime_router_with_backup_key(fixture: &RuntimeFixture, backup_key: Option<&str>) -> Router {
    runtime_router_with_options(fixture, backup_key, None)
}

fn runtime_router_with_auth_secret(fixture: &RuntimeFixture) -> Router {
    runtime_router_with_options(fixture, None, Some(TEST_AUTH_SECRET))
}

fn runtime_router_with_options(
    fixture: &RuntimeFixture,
    backup_key: Option<&str>,
    auth_secret: Option<&str>,
) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:5001",
        Duration::from_secs(1),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.to_string_lossy().to_string())
    .with_data_dir(fixture.data_dir.to_string_lossy().to_string())
    .with_backup_dir(fixture.backup_dir.to_string_lossy().to_string())
    .with_trusted_user_header_secret(TEST_TRUST_SECRET);
    let config = if let Some(backup_key) = backup_key {
        config.with_backup_encryption_key(backup_key)
    } else {
        config
    };
    let config = if let Some(auth_secret) = auth_secret {
        config.with_auth_jwt_secret(auth_secret)
    } else {
        config
    };
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

fn raw_json_request(method: Method, uri: &str, payload: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-bill-analyser-trusted-user-secret", TEST_TRUST_SECRET)
        .header("x-user-id", TEST_USER_ID)
        .header("user-agent", "backup-runtime-contract")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("request")
}

fn bearer_request(method: Method, uri: &str, token: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("user-agent", "backup-runtime-contract")
        .body(body)
        .expect("request")
}

fn bearer_request_with_step_up(
    method: Method,
    uri: &str,
    token: &str,
    step_up_token: &str,
    body: Body,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-bill-analyser-step-up-token", step_up_token)
        .header("user-agent", "backup-runtime-contract")
        .body(body)
        .expect("request")
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

fn write_zip_entries(path: &Path, entries: &[(&str, &[u8])]) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, content) in entries {
        zip.start_file(*name, options)?;
        zip.write_all(content)?;
    }
    zip.finish()?;
    Ok(())
}

fn test_jwt(user_id: i64, token_type: &str, exp_offset: ChronoDuration) -> String {
    let now = Local::now();
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "username": format!("user-{user_id}"),
        "type": token_type,
        "iat": now.timestamp(),
        "exp": (now + exp_offset).timestamp(),
    });
    let encoded_header =
        general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header json"));
    let encoded_payload = general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("payload json"));
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac::HMAC_SHA256, TEST_AUTH_SECRET.as_bytes());
    let signature = hmac::sign(&key, signing_input.as_bytes());
    let encoded_signature = general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref());
    format!("{signing_input}.{encoded_signature}")
}

fn seed_bearer_session(path: &Path, user_id: i64, token: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    bill_analyser_db::init_auth_security_schema(&connection)?;
    let now = Local::now().naive_local();
    let now_text = now.format("%Y-%m-%dT%H:%M:%S%.f").to_string();
    let expires_at = (now + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    connection.execute(
        r#"
        INSERT INTO users(id, username, email, password_hash, is_active, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
        "#,
        (
            user_id,
            format!("user-{user_id}"),
            format!("user-{user_id}@example.test"),
            "",
            &now_text,
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO sessions(
            user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at
        ) VALUES (?1, ?2, NULL, ?3, ?3, 'test', '127.0.0.1', 1, ?4, ?4)
        "#,
        (user_id, sha256_hex(token), expires_at, now_text),
    )?;
    Ok(())
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn seed_source_marker_db(path: &Path, marker: &str) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS source_marker(value TEXT NOT NULL);
        DELETE FROM source_marker;
        ",
    )?;
    connection.execute("INSERT INTO source_marker(value) VALUES (?1)", [marker])?;
    Ok(())
}

fn source_marker(path: &Path) -> Result<String, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    Ok(
        connection.query_row("SELECT value FROM source_marker LIMIT 1", [], |row| {
            row.get(0)
        })?,
    )
}

fn assert_backup_zip_contains_sqlite_marker(
    backup_path: &Path,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(backup_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut db_file = archive.by_name("data/bills.db")?;
    let mut db_bytes = Vec::new();
    std::io::copy(&mut db_file, &mut db_bytes)?;
    let temp = TempDir::new()?;
    let db_path = temp.path().join("snapshot.db");
    fs::write(&db_path, db_bytes)?;
    assert_eq!(source_marker(&db_path)?, expected);
    Ok(())
}

#[derive(Clone)]
struct FakeCloudServer {
    endpoint: String,
    requests: Arc<Mutex<Vec<RecordedCloudRequest>>>,
}

#[derive(Debug, Clone)]
struct RecordedCloudRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body_len: usize,
}

impl FakeCloudServer {
    async fn start() -> Result<Self, Box<dyn Error>> {
        Self::start_with_status(StatusCode::OK).await
    }

    async fn start_with_status(status: StatusCode) -> Result<Self, Box<dyn Error>> {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let app = Router::new()
            .fallback(any(record_cloud_request))
            .with_state((requests.clone(), status));
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fake cloud server");
        });
        Ok(Self { endpoint, requests })
    }

    fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    fn requests(&self) -> Vec<RecordedCloudRequest> {
        self.requests.lock().expect("requests").clone()
    }
}

async fn record_cloud_request(
    State((requests, status)): State<(Arc<Mutex<Vec<RecordedCloudRequest>>>, StatusCode)>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> StatusCode {
    let body = to_bytes(body, 1024 * 1024).await.expect("cloud body");
    let headers = headers
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|text| (key.as_str().to_string(), text.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    requests
        .lock()
        .expect("requests")
        .push(RecordedCloudRequest {
            method: method.as_str().to_string(),
            path: uri.path().to_string(),
            headers,
            body_len: body.len(),
        });
    status
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
