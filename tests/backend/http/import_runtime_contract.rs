use std::{env, error::Error, ffi::OsString, fs, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    routing::post,
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{build_composite_match_features, composite_hash_from_features, UserId};
use bill_analyser_db::{
    create_import_session, get_import_decision_groups_by_session,
    get_import_history_materializations_by_session, get_import_session,
    get_import_sources_by_session, get_import_standard_rows_by_session,
    get_parser_templates_by_session, get_preview_by_session, init_import_staging_schema,
    insert_parser_templates_batch, insert_preview_bills_batch, update_import_session_status,
    ImportParserTemplateDraft, ImportPreviewDraft, ImportPreviewRow, ImportSessionDraft,
    ImportSessionStatusUpdate, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use bill_analyser_http::{
    build_router, HttpAppState, HttpShellConfig, ImportRouteMode, IMPORT_SKELETON_ROUTE_PATTERNS,
};
use chrono::{Duration as ChronoDuration, Local};
use ring::hmac;
use rusqlite::params;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "rust-import-test-secret";
static OCR_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LLM_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvVarRestore {
    name: &'static str,
    value: Option<OsString>,
}

impl EnvVarRestore {
    fn capture(name: &'static str) -> Self {
        Self {
            name,
            value: env::var_os(name),
        }
    }
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        if let Some(value) = &self.value {
            env::set_var(self.name, value);
        } else {
            env::remove_var(self.name);
        }
    }
}

#[tokio::test]
async fn import_db_runtime_reports_primary_http_import_runtime() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let runtime = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/runtime")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    let runtime_body = read_json(runtime).await;
    assert_eq!(
        runtime_body["runtime_boundary"],
        "rust-http:rust-only-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-recurring-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+matching-recurring-calendar-networth-runtime+statistics-read-runtime+statistics-analyzer-runtime+statistics-exchange-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-bundle-runtime+ai-learning-center-runtime+ai-llm-config-candidates-runtime+ai-llm-provider-generation-runtime+ai-ocr-recognition-runtime+auth-login-register-token-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime+backup-ops-sync-runtime"
    );
    assert_eq!(
        runtime_body["business_migration"],
        "import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+matching-recurring-calendar-networth-runtime+statistics-read-runtime+statistics-analyzer-runtime+statistics-exchange-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-bundle-runtime+ai-learning-center-runtime+ai-llm-config-candidates-runtime+ai-llm-provider-generation-runtime+ai-ocr-recognition-runtime+auth-login-register-token-session-personal-refresh-logout-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime+backup-ops-sync-runtime"
    );
    assert_eq!(runtime_body["api_takeover"], true);

    let health = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    let health_body = read_json(health).await;
    assert_eq!(
        health_body["details"]["import_route_mode"],
        "import_db_runtime"
    );
    assert_eq!(health_body["details"]["sqlite_db_path_configured"], "true");
    assert_eq!(health_body["identity"]["api_takeover"], true);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_intercepts_all_deletion_blocked_first_phase_routes_without_fallback(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    for (method, pattern) in IMPORT_SKELETON_ROUTE_PATTERNS {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::from_bytes(method.as_bytes()).expect("method parses"))
                    .uri(sample_path(pattern))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("request builds"),
            )
            .await
            .expect("response");

        assert_ne!(
            response.status(),
            StatusCode::BAD_GATEWAY,
            "{method} {pattern} fell through to legacy fallback routing"
        );
    }
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_reads_and_clears_import_session_preview_rows(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let app = runtime_router(&fixture);

    let session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(session.status(), StatusCode::OK);
    let session_body = read_json(session).await;
    assert_eq!(session_body["success"], true);
    assert_eq!(session_body["data"]["session_id"], "session-a");
    assert_eq!(session_body["data"]["status"], "preview");
    assert_eq!(session_body["data"]["parsed_count"], 2);
    assert_eq!(session_body["data"]["preview_count"], 2);

    let wrong_user = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "77")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(wrong_user.status(), StatusCode::NOT_FOUND);

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/preview/session-a?page=1&pageSize=1")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_body = read_json(preview).await;
    assert_eq!(preview_body["success"], true);
    assert_eq!(preview_body["data"]["total"], 2);
    assert_eq!(preview_body["data"]["page"], 1);
    assert_eq!(preview_body["data"]["page_size"], 1);
    assert_eq!(preview_body["data"]["query"]["page"], 1);
    assert_eq!(preview_body["data"]["query"]["page_size"], 1);
    assert_eq!(preview_body["data"]["metadata"]["counts"]["total"], 2);
    assert_eq!(preview_body["data"]["metadata"]["counts"]["selected"], 0);
    assert!(preview_body["data"]["metadata"]["facets"]["tags"].is_array());
    assert_eq!(preview_body["data"]["preview"].as_array().unwrap().len(), 1);
    assert_eq!(
        preview_body["data"]["preview"][0]["preview_description"],
        "first preview row"
    );
    let preview_matching = &preview_body["data"]["preview"][0]["matching"];
    for section in [
        "transfer",
        "investment",
        "learning",
        "llm",
        "recurring",
        "dedup",
        "parser",
        "annotation",
        "reconciliation",
    ] {
        assert!(
            preview_matching.get(section).is_some(),
            "missing matching section {section}"
        );
    }
    assert_eq!(preview_matching["transfer"]["candidate_type"], "");
    assert_eq!(preview_matching["parser"]["id"], "wechat");
    assert_eq!(preview_matching["parser"]["parser_id"], "wechat");

    let preview_by_ids = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/preview/session-a?preview_ids=2,1&page=1&page_size=2")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(preview_by_ids.status(), StatusCode::OK);
    let preview_by_ids_body = read_json(preview_by_ids).await;
    assert_eq!(preview_by_ids_body["data"]["total"], 2);
    let preview_by_ids_rows = preview_by_ids_body["data"]["preview"]
        .as_array()
        .expect("preview ids rows");
    assert_eq!(preview_by_ids_rows[0]["id"], 2);
    assert_eq!(preview_by_ids_rows[1]["id"], 1);

    let preview_sorted = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/preview/session-a?sort_by=sourceAmount&sort_direction=asc&page=1&page_size=1")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(preview_sorted.status(), StatusCode::OK);
    let preview_sorted_body = read_json(preview_sorted).await;
    assert_eq!(preview_sorted_body["data"]["total"], 2);
    assert_eq!(preview_sorted_body["data"]["preview"][0]["id"], 2);

    let invalid_sort = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/preview/session-a?sort_by=unknown")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(invalid_sort.status(), StatusCode::BAD_REQUEST);

    let cancel = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(cancel.status(), StatusCode::OK);
    let cancel_body = read_json(cancel).await;
    assert_eq!(cancel_body["success"], true);
    assert_eq!(cancel_body["message"], "Session cleared");

    let runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(runtime.connection())?;
    let rows = get_preview_by_session(runtime.connection(), "session-a", user_id(42), false)?;
    assert!(rows.is_empty());
    assert!(get_import_session(runtime.connection(), "session-a", user_id(42))?.is_none());
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_rejects_untrusted_user_identity_headers() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let app = runtime_router(&fixture);

    let missing_secret = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "42")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(missing_secret.status(), StatusCode::UNAUTHORIZED);

    let wrong_secret = app
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", "wrong")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(wrong_secret.status(), StatusCode::UNAUTHORIZED);

    let invalid_user = runtime_router(&fixture)
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "not-a-number")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(invalid_user.status(), StatusCode::UNAUTHORIZED);

    let zero_user = runtime_router(&fixture)
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "0")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(zero_user.status(), StatusCode::UNAUTHORIZED);

    let alternate_user_header = runtime_router(&fixture)
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-bill-analyser-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(alternate_user_header.status(), StatusCode::OK);

    let unconfigured_secret = runtime_router_without_auth_secret(&fixture)
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("x-user-id", "42")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(
        unconfigured_secret.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_accepts_frontend_bearer_token_sessions() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let token = test_access_token(42, TEST_AUTH_SECRET, true);
    seed_auth_session(fixture.db_path(), 42, &token, true, true)?;
    let app = runtime_router(&fixture);

    let session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(session.status(), StatusCode::OK);
    let session_body = read_json(session).await;
    assert_eq!(session_body["data"]["session_id"], "session-a");

    let expired_token = test_access_token(42, TEST_AUTH_SECRET, false);
    let expired = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("authorization", format!("Bearer {expired_token}"))
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);

    let tampered = app
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/session/session-a")
                .header("authorization", format!("Bearer {token}x"))
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(tampered.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_persists_ocr_config_and_rejects_unknown_provider(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let default_config = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/ml/receipt-recognition/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(default_config.status(), StatusCode::OK);
    let default_body = read_json(default_config).await;
    assert_eq!(default_body["success"], true);
    assert_eq!(default_body["result"]["provider"], "disabled");
    assert_eq!(default_body["result"]["configured"], false);

    let stored = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "tesseract",
                        "lang": "eng",
                        "image": "must-not-be-persisted"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(stored.status(), StatusCode::OK);
    let stored_body = read_json(stored).await;
    assert_eq!(stored_body["result"]["provider"], "tesseract");
    assert_eq!(stored_body["result"]["lang"], "eng");
    assert_eq!(stored_body["result"]["configured"], true);

    let rejected = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"provider":"unknown"}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let rejected_body = read_json(rejected).await;
    assert_eq!(rejected_body["success"], false);
    assert_eq!(rejected_body["message"], "Unknown OCR provider");

    let reloaded = app
        .oneshot(
            Request::builder()
                .uri("/api/ml/receipt-recognition/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    let reloaded_body = read_json(reloaded).await;
    assert_eq!(reloaded_body["result"]["provider"], "tesseract");
    assert_eq!(reloaded_body["result"]["lang"], "eng");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_disabled_error(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "application/octet-stream")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["errorCode"], "provider_unconfigured");
    assert_eq!(body["message"], "ocr provider not configured");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_cancelled_multipart(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);
    let boundary = "ocr-cancel-boundary";
    let body = multipart_body_bytes(
        boundary,
        &[("cancelled", "true")],
        &[(
            "image",
            "receipt.png",
            "image/png",
            b"receipt-image".as_slice(),
        )],
    );

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header("x-user-id", "43")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::from_u16(499)?);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["errorCode"], "cancelled");
    assert_eq!(body["message"], "request cancelled by client");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_tesseract_payload(
) -> Result<(), Box<dyn Error>> {
    let _env_guard = OCR_ENV_LOCK.lock().await;
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);
    let fixture_text = fixture._temp_dir.path().join("ocr-text.txt");
    fs::write(
        &fixture_text,
        "支付宝\n商品: 拿铁咖啡\n付款金额 12.34\n2025-01-02 10:30",
    )?;
    let script = write_fake_tesseract_script(fixture._temp_dir.path())?;
    env::set_var("BILL_ANALYSER_RUST_OCR_FIXTURE_TEXT", &fixture_text);
    configure_fake_tesseract_command(&script);
    env::set_var("BILL_ANALYSER_RUST_OCR_TIMEOUT_MS", "1000");

    let put_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("content-type", "application/json")
                .header("x-user-id", "44")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(
                    json!({"provider": "tesseract", "lang": "eng"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(put_config.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "44")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("fake-image-bytes"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    clear_fake_tesseract_env();
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["amount"], 12.34);
    assert_eq!(body["result"]["trade_time"], "2025-01-02 10:30");
    assert_eq!(body["result"]["description"], "拿铁咖啡");
    assert_eq!(body["result"]["payment_platform"], "alipay");
    assert_eq!(body["result"]["provenance"]["provider"], "tesseract");
    assert_eq!(body["result"]["provenance"]["model"], "tesseract");
    assert_eq!(
        body["result"]["raw_provider_response"]["engine"],
        "tesseract"
    );
    assert_eq!(
        body["result"]["draft"]["auto_fill"]["amount"]["unit"],
        "yuan"
    );
    assert_eq!(
        body["result"]["draft"]["auto_fill"]["type"]["value"],
        "expense"
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_local_json_payload(
) -> Result<(), Box<dyn Error>> {
    let _env_guard = OCR_ENV_LOCK.lock().await;
    clear_fake_ocr_env();
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_import_intelligence_tables(&runtime)?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0
        );
        INSERT INTO tags(id, user_id, name, display_order, hidden)
        VALUES (77, 42, '咖啡', 0, 0);
        ",
    )?;
    let app = runtime_router(&fixture);
    let fixture_json = fixture._temp_dir.path().join("ocr-lines.json");
    fs::write(
        &fixture_json,
        json!({
            "model": "fake-paddleocr",
            "lines": [
                {"text": "支付宝", "score": 0.96, "box": [[0, 0], [10, 0], [10, 10], [0, 10]]},
                {"text": "付款方式 支付宝余额", "score": 0.95},
                {"text": "商品: 瑞幸咖啡 拿铁", "score": 0.94},
                {"text": "付款金额 12.34", "score": 0.98},
                {"text": "2025-01-02 10:30", "score": 0.93}
            ]
        })
        .to_string(),
    )?;
    let script = write_fake_local_json_ocr_script(fixture._temp_dir.path())?;
    env::set_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_FIXTURE", &fixture_json);
    configure_fake_local_json_ocr_command(&script);
    env::set_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_TIMEOUT_MS", "1000");

    let put_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("content-type", "application/json")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(
                    json!({"provider": "local_json_ocr", "lang": "eng"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(put_config.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("fake-image-bytes"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    clear_fake_ocr_env();
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["provenance"]["provider"], "local_json_ocr");
    assert_eq!(body["result"]["provenance"]["model"], "fake-paddleocr");
    assert_eq!(
        body["result"]["draft"]["auto_fill"]["category_id"]["value"],
        "900"
    );
    assert_eq!(
        body["result"]["draft"]["auto_fill"]["source_account_id"]["value"],
        "1001"
    );
    assert_eq!(
        body["result"]["draft"]["auto_fill"]["tag_ids"]["value"],
        json!(["77"])
    );

    fs::write(&fixture_json, "not json")?;
    env::set_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_FIXTURE", &fixture_json);
    configure_fake_local_json_ocr_command(&script);
    env::set_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_TIMEOUT_MS", "1000");
    let malformed = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "51")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("fake-image-bytes"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    clear_fake_ocr_env();
    assert_eq!(malformed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let malformed_body = read_json(malformed).await;
    assert_eq!(malformed_body["errorCode"], "parse_error");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_auth_and_input_edges(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let default_mime = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("x-user-id", "45")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(default_mime.status(), StatusCode::NOT_IMPLEMENTED);
    let default_body = read_json(default_mime).await;
    assert_eq!(default_body["errorCode"], "provider_unconfigured");

    let malformed_multipart = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "multipart/form-data")
                .header("x-user-id", "45")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("not-a-valid-multipart-body"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(malformed_multipart.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_provider_error_edges(
) -> Result<(), Box<dyn Error>> {
    let _env_guard = OCR_ENV_LOCK.lock().await;
    clear_fake_tesseract_env();
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let cloud_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("content-type", "application/json")
                .header("x-user-id", "46")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(
                    json!({"provider": "cloud_stub", "lang": "chi_sim"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(cloud_config.status(), StatusCode::OK);

    let cloud_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "46")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(cloud_response.status(), StatusCode::NOT_IMPLEMENTED);
    let cloud_body = read_json(cloud_response).await;
    assert_eq!(cloud_body["errorCode"], "provider_unconfigured");
    assert_eq!(cloud_body["message"], "cloud_ocr_not_configured");

    let tesseract_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("content-type", "application/json")
                .header("x-user-id", "47")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(
                    json!({"provider": "tesseract", "lang": "eng"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(tesseract_config.status(), StatusCode::OK);

    env::set_var(
        "BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND",
        "bill-analyser-missing-tesseract-command",
    );
    env::remove_var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND_ARGS");
    for index in 0..11 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/ml/receipt-recognition")
                    .header("content-type", "image/png")
                    .header("x-user-id", "47")
                    .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                    .body(Body::from("receipt-image"))
                    .expect("request builds"),
            )
            .await
            .expect("response");
        let body = read_json(response).await;
        if index < 10 {
            assert_eq!(body["errorCode"], "provider_unconfigured");
            assert!(body["message"]
                .as_str()
                .expect("message")
                .contains("tesseract provider unavailable"));
        } else {
            assert_eq!(body["errorCode"], "rate_limited");
            assert_eq!(body["message"], "ocr per-user rate limit exceeded");
        }
    }
    clear_fake_tesseract_env();
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_receipt_ocr_recognition_tesseract_process_edges(
) -> Result<(), Box<dyn Error>> {
    let _env_guard = OCR_ENV_LOCK.lock().await;
    clear_fake_tesseract_env();
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let put_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ml/receipt-recognition/config")
                .header("content-type", "application/json")
                .header("x-user-id", "48")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(
                    json!({"provider": "tesseract", "lang": "eng"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(put_config.status(), StatusCode::OK);

    let failing_script = write_failing_tesseract_script(fixture._temp_dir.path())?;
    configure_fake_tesseract_command(&failing_script);
    env::set_var("BILL_ANALYSER_RUST_OCR_TIMEOUT_MS", "1000");
    let failed = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "48")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(failed.status(), StatusCode::NOT_IMPLEMENTED);
    let failed_body = read_json(failed).await;
    assert_eq!(failed_body["errorCode"], "provider_unconfigured");
    assert!(failed_body["message"]
        .as_str()
        .expect("message")
        .contains("tesseract provider unavailable"));

    let silent_failing_script = write_silent_failing_tesseract_script(fixture._temp_dir.path())?;
    configure_fake_tesseract_command(&silent_failing_script);
    let silent_failed = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "50")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(silent_failed.status(), StatusCode::NOT_IMPLEMENTED);
    let silent_failed_body = read_json(silent_failed).await;
    assert_eq!(
        silent_failed_body["message"],
        "tesseract provider unavailable"
    );

    let slow_script = write_slow_tesseract_script(fixture._temp_dir.path())?;
    configure_fake_tesseract_command(&slow_script);
    env::set_var("BILL_ANALYSER_RUST_OCR_TIMEOUT_MS", "1");
    let timed_out = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "image/png")
                .header("x-user-id", "49")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(timed_out.status(), StatusCode::GATEWAY_TIMEOUT);
    let timed_out_body = read_json(timed_out).await;
    assert_eq!(timed_out_body["errorCode"], "timeout");
    assert_eq!(timed_out_body["message"], "ocr provider timeout");

    clear_fake_tesseract_env();
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_llm_provider_generation_routes() -> Result<(), Box<dyn Error>> {
    let _env_guard = LLM_ENV_LOCK.lock().await;
    let _allowlist_restore = EnvVarRestore::capture("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");
    let (provider_base_url, server) = llm_openai_provider_upstream().await?;
    env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", &provider_base_url);
    let fixture = RuntimeFixture::new_with_upstream(unavailable_upstream().await)?;
    seed_import_session(fixture.db_path(), "session-a")?;
    seed_llm_provider_generation_data(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let config = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/config",
        Some(
            json!({
                "enabled": true,
                "provider": "openai_compatible",
                "provider_config": {
                    "api_key": "test-key",
                    "base_url": format!("{provider_base_url}/v1"),
                    "model": "fake-model"
                }
            })
            .to_string(),
        ),
        42,
    )
    .await;
    assert_eq!(config.status(), StatusCode::OK);

    let invalid_analyze_ids = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/analyze-transactions",
        Some(json!({"bill_ids": "1"}).to_string()),
        42,
    )
    .await;
    assert_eq!(invalid_analyze_ids.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_analyze_ids).await["code"],
        "INVALID_REQUEST"
    );

    let empty_analyze_selection = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/analyze-transactions",
        Some(json!({"bill_ids": []}).to_string()),
        42,
    )
    .await;
    assert_eq!(empty_analyze_selection.status(), StatusCode::OK);
    assert_eq!(
        read_json(empty_analyze_selection).await["data"]["candidates_created"],
        0
    );
    let oversized_preview_ids = (1..=21).collect::<Vec<_>>();
    let preview_ids_too_large_with_update = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/preview-recommend",
        Some(
            json!({
                "session_id": "session-a",
                "preview_ids": oversized_preview_ids,
                "preview_updates": [{
                    "id": 1,
                    "preview_main_category": "不应保存",
                    "preview_sub_category": "不应保存"
                }]
            })
            .to_string(),
        ),
        42,
    )
    .await;
    assert_eq!(
        preview_ids_too_large_with_update.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        read_json(preview_ids_too_large_with_update).await["code"],
        "PREVIEW_SELECTION_TOO_LARGE"
    );
    assert_eq!(
        preview_rows(fixture.db_path(), "session-a")?[0].preview_main_category,
        "餐饮"
    );

    let oversized_preview_updates = (0..21)
        .map(|_| {
            json!({
                "id": 1,
                "preview_main_category": "不应保存",
                "preview_sub_category": "不应保存"
            })
        })
        .collect::<Vec<_>>();
    let too_many_preview_updates = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/preview-recommend",
        Some(
            json!({
                "session_id": "session-a",
                "preview_updates": oversized_preview_updates
            })
            .to_string(),
        ),
        42,
    )
    .await;
    assert_eq!(
        too_many_preview_updates.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        read_json(too_many_preview_updates).await["code"],
        "PREVIEW_SELECTION_TOO_LARGE"
    );
    assert_eq!(
        preview_rows(fixture.db_path(), "session-a")?[0].preview_main_category,
        "餐饮"
    );

    let preview = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/preview-recommend",
        Some(json!({"session_id": "session-a", "preview_ids": [1]}).to_string()),
        42,
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_body = read_json(preview).await;
    assert_eq!(preview_body["success"], true);
    assert_eq!(preview_body["data"]["session_id"], "session-a");
    assert_eq!(preview_body["data"]["count"], 1);
    assert_eq!(preview_body["data"]["suggestions"][0]["preview_id"], 1);
    assert_eq!(preview_body["runtime"], Value::Null);

    let analyze = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/analyze-transactions",
        Some(json!({}).to_string()),
        42,
    )
    .await;
    assert_eq!(analyze.status(), StatusCode::OK);
    let analyze_body = read_json(analyze).await;
    assert_eq!(analyze_body["success"], true);
    assert_eq!(analyze_body["data"]["mode"], "persisted_uncategorized");
    assert_eq!(analyze_body["data"]["candidates_created"], 1);
    assert_eq!(
        analyze_body["data"]["candidates"][0]["type"],
        "classification"
    );

    let session_analyze = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/analyze-transactions",
        Some(
            json!({
                "session_id": "session-a",
                "preview_updates": [{
                    "id": 2,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "午餐",
                    "preview_description": "canteen lunch"
                }]
            })
            .to_string(),
        ),
        42,
    )
    .await;
    assert_eq!(session_analyze.status(), StatusCode::OK);
    let session_analyze_body = read_json(session_analyze).await;
    assert_eq!(session_analyze_body["success"], true);
    assert_eq!(session_analyze_body["data"]["mode"], "import_session");
    assert_eq!(session_analyze_body["data"]["session_id"], "session-a");
    assert_eq!(session_analyze_body["data"]["candidates_created"], 1);
    assert_eq!(
        session_analyze_body["data"]["candidates"][0]["type"],
        "rule_induction"
    );

    let synthesis = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/rule-synthesis",
        Some(json!({"limit": 1}).to_string()),
        42,
    )
    .await;
    assert_eq!(synthesis.status(), StatusCode::OK);
    let synthesis_body = read_json(synthesis).await;
    assert_eq!(synthesis_body["success"], true);
    assert_eq!(synthesis_body["data"]["candidates_created"], 1);
    assert_eq!(
        synthesis_body["data"]["candidates"][0]["type"],
        "rule_synthesis"
    );

    let claude_config = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/config",
        Some(
            json!({
                "enabled": true,
                "provider": "anthropic",
                "provider_config": {
                    "api_key": "anthropic-test-key",
                    "base_url": provider_base_url.clone(),
                    "model": "claude-test"
                },
                "advanced_settings": {
                    "system_prompt": "domain system"
                }
            })
            .to_string(),
        ),
        42,
    )
    .await;
    assert_eq!(claude_config.status(), StatusCode::OK);
    let claude_analyze = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/analyze-transactions",
        Some(json!({"bill_ids": [1]}).to_string()),
        42,
    )
    .await;
    assert_eq!(claude_analyze.status(), StatusCode::OK);
    assert_eq!(
        read_json(claude_analyze).await["data"]["candidates"][0]["llm_provider"],
        "claude"
    );

    let ollama_config = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/config",
        Some(
            json!({
                "enabled": true,
                "provider": "ollama",
                "provider_config": {
                    "base_url": provider_base_url.clone(),
                    "model": "llama-test"
                }
            })
            .to_string(),
        ),
        42,
    )
    .await;
    assert_eq!(ollama_config.status(), StatusCode::OK);
    let ollama_preview = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/preview-recommend",
        Some(json!({"session_id": "session-a", "preview_ids": [1]}).to_string()),
        42,
    )
    .await;
    assert_eq!(ollama_preview.status(), StatusCode::OK);
    assert_eq!(read_json(ollama_preview).await["data"]["count"], 1);

    server.abort();
    let _ = server.await;
    Ok(())
}

#[tokio::test]
async fn llm_rule_synthesis_skips_provider_without_learning_evidence() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new_with_upstream(unavailable_upstream().await)?;
    seed_llm_provider_categories_only(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let synthesis = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/rule-synthesis",
        Some(json!({"limit": 1}).to_string()),
        42,
    )
    .await;
    assert_eq!(synthesis.status(), StatusCode::OK);
    let synthesis_body = read_json(synthesis).await;
    assert_eq!(synthesis_body["success"], true);
    assert_eq!(synthesis_body["data"]["mode"], "rule_synthesis");
    assert_eq!(synthesis_body["data"]["candidates_created"], 0);
    assert_eq!(synthesis_body["total"], 0);
    assert!(
        synthesis_body["data"]["knowledge_summary_pack"]["existing_categories"]
            .as_array()
            .is_some_and(|values| !values.is_empty())
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_parse_dedup_confirm_writes_import_chain() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    let app = runtime_router(&fixture);

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "parser_id": "wechat",
                        "file_count": 1,
                        "bills": [
                            {
                                "date": "2026-05-01 08:30:00",
                                "type": "支出",
                                "amount": 12.5,
                                "description": "coffee",
                                "counterparty": "cafe",
                                "payment_method": "wechat",
                                "source_account_id": "1001",
                                "main_category": "餐饮",
                                "sub_category": "咖啡"
                            },
                            {
                                "date": "2026-05-02 12:00:00",
                                "type": "支出",
                                "amount": 32.0,
                                "description": "lunch",
                                "counterparty": "canteen",
                                "payment_method": "wechat",
                                "source_account_id": "1001",
                                "main_category": "餐饮",
                                "sub_category": "午餐"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["data"]["parsed_count"], 2);
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let session = get_import_session(db_runtime.connection(), &session_id, user_id(42))?
        .expect("session persisted");
    assert_eq!(session.status, "parsed");
    assert_eq!(session.total_parsed, 2);
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), &session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0].parser_id, "wechat");
    assert!(!templates[0].parser_is_processed);

    let parse_generic = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse_generic")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "parser_id": "generic",
                        "rows": [{
                            "trade_time": "2026-05-03 19:20:00",
                            "trade_type": "收入",
                            "source_amount": 100.0,
                            "summary": "refund",
                            "merchant": "store",
                            "paymentMethod": "alipay",
                            "source_account_id": "2002",
                            "main_category": "退款",
                            "sub_category": "购物退款"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse_generic.status(), StatusCode::OK);
    let parse_generic_body = read_json(parse_generic).await;
    assert_eq!(parse_generic_body["data"]["session_id"], session_id);
    assert_eq!(parse_generic_body["data"]["parsed_count"], 1);
    let session = get_import_session(db_runtime.connection(), &session_id, user_id(42))?
        .expect("session remains");
    assert_eq!(session.total_parsed, 3);

    let dedup = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup.status(), StatusCode::OK);
    let dedup_body = read_json(dedup).await;
    assert_eq!(dedup_body["data"]["session_id"], session_id);
    assert_eq!(dedup_body["data"]["preview_included"], false);
    assert_eq!(dedup_body["data"]["total"], 3);
    assert_eq!(dedup_body["data"]["after_dedup"], 3);
    assert_eq!(dedup_body["data"]["preview"].as_array().unwrap().len(), 0);

    let preview = get_preview_by_session(db_runtime.connection(), &session_id, user_id(42), false)?;
    assert_eq!(preview.len(), 3);

    let dedup_replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup_replay.status(), StatusCode::OK);
    let dedup_replay_body = read_json(dedup_replay).await;
    assert_eq!(dedup_replay_body["data"]["total"], 3);
    assert_eq!(dedup_replay_body["data"]["after_dedup"], 3);
    assert_eq!(
        dedup_replay_body["data"]["preview"]
            .as_array()
            .expect("replayed preview rows")
            .len(),
        3
    );
    assert_eq!(
        dedup_replay_body["data"]["match_stats"]["idempotent_replay"],
        true
    );
    let replay_session = get_import_session(db_runtime.connection(), &session_id, user_id(42))?
        .expect("session remains preview");
    assert_eq!(replay_session.status, "preview");
    assert_eq!(replay_session.total_parsed, 3);
    assert_eq!(replay_session.total_preview, 3);
    let replay_preview =
        get_preview_by_session(db_runtime.connection(), &session_id, user_id(42), false)?;
    assert_eq!(replay_preview.len(), 3);

    let selected_preview = preview.first().expect("preview rows").id;

    let preview_index = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/bills/import/v2/preview/{session_id}/index"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(preview_index.status(), StatusCode::OK);
    let preview_index_body = read_json(preview_index).await;
    assert_eq!(preview_index_body["data"]["total"], 3);

    let preview_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/api/bills/import/v2/preview/{session_id}?page=1&page_size=2"
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(preview_page.status(), StatusCode::OK);
    let preview_page_body = read_json(preview_page).await;
    assert_eq!(preview_page_body["data"]["total"], 3);
    assert_eq!(
        preview_page_body["data"]["preview"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let confirm = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "preview_updates": [{
                            "id": selected_preview,
                            "type": 3,
                            "isSelected": true,
                            "mainCategory": "餐饮",
                            "subCategory": "咖啡"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm.status(), StatusCode::OK);
    let confirm_body = read_json(confirm).await;
    assert_eq!(confirm_body["success"], true);
    assert_eq!(confirm_body["data"]["imported_count"], 1);
    assert_eq!(confirm_body["data"]["skipped_count"], 0);

    let bill_count: i64 = db_runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(bill_count, 1);
    let (bill_type, bill_amount): (String, f64) = db_runtime.connection().query_row(
        "SELECT type, amount FROM bills WHERE user_id = ?1",
        [42],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(bill_type, "支出");
    assert_eq!(bill_amount, -12.5);
    assert!(get_import_session(db_runtime.connection(), &session_id, user_id(42))?.is_none());
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_accepts_frontend_multipart_parse_upload() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let boundary = "rust-import-upload-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[(
            "files",
            "wechat.csv",
            "text/csv",
            "交易时间,收支类型,金额,商品,支付方式\n2026-05-04 09:00:00,支出,18.50,早餐,微信\n",
        )],
    );

    let parse = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["data"]["parsed_count"], 1);
    assert_eq!(
        parse_body["data"]["unmatched_files"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(parse_body["data"]["files"][0]["parser_id"], "wechat");
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id");

    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].parser_id, "wechat");
    assert_eq!(templates[0].parser_type, "支出");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_parallel_parse_keeps_multi_file_order_and_single_staging(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let boundary = "rust-import-parallel-upload-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[
            (
                "files",
                "wechat-a.csv",
                "text/csv",
                "交易时间,收支类型,金额,商品,支付方式\n2026-05-04 09:00:00,支出,18.50,早餐,微信\n",
            ),
            (
                "files",
                "wechat-b.csv",
                "text/csv",
                "交易时间,收支类型,金额,商品,支付方式\n2026-05-05 12:30:00,支出,32.00,午餐,微信\n",
            ),
        ],
    );

    let parse = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["data"]["parsed_count"], 2);
    assert_eq!(
        parse_body["data"]["unmatched_files"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let files = parse_body["data"]["files"].as_array().expect("files");
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["filename"], "wechat-a.csv");
    assert_eq!(files[1]["filename"], "wechat-b.csv");
    assert!(files
        .iter()
        .all(|file| file["parser_id"].as_str() == Some("wechat")));
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id");

    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let session =
        get_import_session(db_runtime.connection(), session_id, user_id(42))?.expect("session");
    assert_eq!(session.file_count, 2);
    assert_eq!(session.total_parsed, 2);
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 2);
    assert!(templates
        .iter()
        .all(|template| template.parser_id == "wechat"));
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_parallel_parse_preserves_per_file_parser_identity(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let boundary = "rust-import-mixed-parser-upload-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[
            (
                "files",
                "alipay.csv",
                "text/csv",
                "交易时间,交易分类,交易对方,对方账号,商品说明,收/支,金额,收/付款方式,交易状态,交易订单号,商家订单号,备注\n2026-05-04 09:00:00,餐饮,支付宝咖啡店,,拿铁,支出,21.00,支付宝余额,交易成功,ali-1,,\n",
            ),
            (
                "files",
                "wechat.csv",
                "text/csv",
                "交易时间,收支类型,金额,商品,支付方式\n2026-05-05 12:30:00,支出,32.00,午餐,微信\n",
            ),
        ],
    );

    let parse = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    let files = parse_body["data"]["files"].as_array().expect("files");
    assert_eq!(files[0]["parser_id"], "alipay");
    assert_eq!(files[1]["parser_id"], "wechat");
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id");

    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0].parser_id, "alipay");
    assert_eq!(templates[1].parser_id, "wechat");
    assert!(templates[0]
        .parser_tags
        .iter()
        .any(|tag| tag == "parser:alipay"));
    assert!(!templates[0]
        .parser_tags
        .iter()
        .any(|tag| tag == "parser:wechat"));
    assert!(templates[1]
        .parser_tags
        .iter()
        .any(|tag| tag == "parser:wechat"));
    let sources = get_import_sources_by_session(db_runtime.connection(), session_id, user_id(42))?;
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].source_index, 0);
    assert_eq!(sources[0].parser_id, "alipay");
    assert_eq!(sources[0].parser_signal, "matched");
    assert_eq!(
        sources[0].metadata["parser_decision"]["selected_parser_id"],
        "alipay"
    );
    assert_eq!(sources[1].source_index, 1);
    assert_eq!(sources[1].parser_id, "wechat");

    let standard_rows =
        get_import_standard_rows_by_session(db_runtime.connection(), session_id, user_id(42))?;
    assert_eq!(standard_rows.len(), 2);
    assert_eq!(standard_rows[0].source_index, 0);
    assert_eq!(standard_rows[0].source_row_index, 0);
    assert_eq!(standard_rows[0].parser_id, "alipay");
    assert_eq!(standard_rows[0].amount_cents, -2100);
    assert_eq!(standard_rows[0].direction, "expense");
    assert_eq!(standard_rows[0].parser_payload["parser_id"], "alipay");
    assert_eq!(standard_rows[1].source_index, 1);
    assert_eq!(standard_rows[1].parser_id, "wechat");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_parser_conflict_is_unmatched_with_detection_evidence(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let boundary = "rust-import-parser-conflict-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[(
            "files",
            "ambiguous-bank.csv",
            "text/csv",
            "交易日期,交易金额,对手信息,对方户名,对方账号\n2026-05-04,12.34,张三,张三,6222000000000000\n",
        )],
    );

    let parse = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["data"]["parsed_count"], 0);
    let unmatched = parse_body["data"]["unmatched_files"]
        .as_array()
        .expect("unmatched");
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0]["parser_id"], "rust-import");
    assert_eq!(
        unmatched[0]["parser_decision"]["status"], "conflict",
        "{parse_body:?}"
    );
    assert_eq!(
        unmatched[0]["parser_decision"]["conflict_group"],
        json!(["icbc", "abc"])
    );
    assert!(unmatched[0]["reason"]
        .as_str()
        .unwrap()
        .contains("Multiple dedicated Rust parsers matched"));
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_stage2_restores_import_intelligence_chain() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new().await?;
    let mut runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_import_intelligence_tables(&runtime)?;
    let session_id = "session-stage2-intelligence";
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            ImportParserTemplateDraft {
                parser_date: "2026-05-04 09:00:00".to_string(),
                parser_amount: -21.0,
                parser_type: "支出".to_string(),
                parser_description: "拿铁".to_string(),
                parser_id: "alipay".to_string(),
                parser_tags: Some(json!(["parser:alipay", "channel:wallet"])),
                parser_counterparty: "支付宝咖啡店".to_string(),
                parser_payment_method: "支付宝余额".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "alipay".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2026-05-05 12:30:00".to_string(),
                parser_amount: -45.0,
                parser_type: "支出".to_string(),
                parser_description: "会员日采购".to_string(),
                parser_id: "wechat".to_string(),
                parser_tags: Some(json!(["parser:wechat", "channel:wallet"])),
                parser_counterparty: "学习超市".to_string(),
                parser_payment_method: "微信支付".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "wechat".to_string(),
            },
        ],
    )?;
    drop(runtime);

    let app = runtime_router(&fixture);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"session_id": session_id, "include_preview": true}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["match_stats"]["provider_bypassed"], false);
    assert_eq!(body["data"]["match_stats"]["category_matched"], 2);
    assert_eq!(body["data"]["match_stats"]["account_matched"], 2);
    assert_eq!(body["data"]["match_stats"]["learning_applied"], 1);
    let preview = body["data"]["preview"].as_array().expect("preview");
    assert_eq!(preview.len(), 2);

    let coffee = preview
        .iter()
        .find(|item| item["preview_counterparty"] == "支付宝咖啡店")
        .expect("coffee preview");
    assert_eq!(coffee["preview_main_category"], "餐饮");
    assert_eq!(coffee["preview_sub_category"], "咖啡");
    assert_eq!(coffee["preview_source_account_id"], 1001);
    assert_eq!(coffee["matching"]["parser"]["id"], "alipay");
    assert_eq!(coffee["matching"]["parser"]["parser_id"], "alipay");
    assert_eq!(
        coffee["matching"]["stage2_baseline"]["preview_main_category"],
        "餐饮"
    );
    assert_eq!(
        coffee["matching"]["stage2_baseline"]["preview_sub_category"],
        "咖啡"
    );
    assert_eq!(
        coffee["matching"]["stage2_baseline"]["preview_source_account_id"],
        1001
    );
    assert!(coffee["matching"]["transfer"].is_object());
    assert_eq!(coffee["matching"]["recurring"]["id"], 3001);
    let coffee_id = coffee["id"].as_i64().expect("coffee preview id");
    let recurring_candidates = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/bills/import/v2/preview-item/{coffee_id}/recurring-candidates"
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(recurring_candidates.status(), StatusCode::OK);
    let recurring_candidates_body = read_json(recurring_candidates).await;
    assert_eq!(
        recurring_candidates_body["data"]["provider_bypassed"],
        false
    );
    assert_eq!(recurring_candidates_body["data"]["candidate_count"], 1);
    assert_eq!(
        recurring_candidates_body["data"]["candidates"][0]["id"],
        3001
    );

    let learned = preview
        .iter()
        .find(|item| item["preview_counterparty"] == "学习超市")
        .expect("learned preview");
    assert_eq!(learned["preview_main_category"], "生活");
    assert_eq!(learned["preview_sub_category"], "超市");
    assert_eq!(learned["preview_source_account_id"], 1002);
    assert_eq!(learned["matching"]["learning"]["rule_id"], 7001);
    assert_eq!(learned["matching"]["learning"]["auto_apply"], true);
    let learning_suggestions = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/bills/import/v2/learning/{session_id}/suggestions"
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(learning_suggestions.status(), StatusCode::OK);
    let learning_suggestions_body = read_json(learning_suggestions).await;
    assert_eq!(
        learning_suggestions_body["data"]["provider_bypassed"],
        false
    );
    assert_eq!(learning_suggestions_body["data"]["count"], 1);
    assert_eq!(
        learning_suggestions_body["data"]["suggestions"][0]["rule_id"],
        7001
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_parses_dedicated_xlsx_upload_without_legacy_fallback(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let sample_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/import_samples/abc_statement_sample.xlsx");
    let sample = std::fs::read(sample_path)?;
    let boundary = "rust-import-dedicated-xlsx-boundary";
    let body = multipart_body_bytes(
        boundary,
        &[("parser_type", "auto")],
        &[(
            "files",
            "abc_statement_sample.xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            &sample,
        )],
    );

    let parse = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["data"]["parsed_count"], 2);
    assert_eq!(
        parse_body["data"]["unmatched_files"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(parse_body["data"]["files"][0]["parser_id"], "abc");
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id");

    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0].parser_id, "abc");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_stage2_learning_does_not_override_transfer_pair_type(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let mut runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_import_intelligence_tables(&runtime)?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
         VALUES (902, 42, 2, '其他收入', '原路退款', 30)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
         VALUES (904, 42, 4, '一般转账', '电子支付', 5)",
        [],
    )?;
    let features = build_composite_match_features(
        "cmbc",
        "支付宝（中国）网络技术有限公司客户备付金",
        "支付宝快捷支付",
        "网络银行",
    )
    .expect("transfer learning features");
    let composite_hash = composite_hash_from_features(&features);
    runtime.connection().execute(
        "INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, normalized_match_value, learned_type,
            learned_category_id, learned_source_account_id, learned_destination_account_id,
            enabled, parser_id, composite_match_hash, match_features_json, created_at, updated_at
         ) VALUES (7101, 42, 'composite', ?1, ?1, '收入', 902, 1002, NULL, 1, 'cmbc', ?1, ?2, '2026-05-01', '2026-05-01')",
        [composite_hash, serde_json::to_string(&features)?],
    )?;
    let session_id = "session-stage2-transfer-learning-guard";
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            ImportParserTemplateDraft {
                parser_date: "2016-09-01 12:22:16".to_string(),
                parser_amount: -6000.0,
                parser_type: "支出".to_string(),
                parser_description: "支付宝快捷支付".to_string(),
                parser_id: "cmbc".to_string(),
                parser_tags: Some(json!(["parser:cmbc", "channel:bank"])),
                parser_counterparty: "支付宝（中国）网络技术有限公司客户备付金".to_string(),
                parser_payment_method: "网络银行".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "1001".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2016-09-01 12:22:11".to_string(),
                parser_amount: 6000.0,
                parser_type: "收入".to_string(),
                parser_description: String::new(),
                parser_id: "alipay".to_string(),
                parser_tags: Some(json!(["parser:alipay", "channel:wallet"])),
                parser_counterparty: String::new(),
                parser_payment_method: String::new(),
                parser_original_type: "不计收支".to_string(),
                parser_original_category: "转账红包".to_string(),
                parser_account_id: "1002".to_string(),
            },
        ],
    )?;
    drop(runtime);

    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"session_id": session_id, "include_preview": true}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let preview = body["data"]["preview"].as_array().expect("preview rows");
    assert_eq!(preview.len(), 1);
    let transfer = &preview[0];

    assert_eq!(transfer["dedup_type"], "transfer");
    assert_eq!(transfer["preview_type"], "转账");
    assert_eq!(transfer["preview_main_category"], "一般转账");
    assert_eq!(transfer["preview_sub_category"], "电子支付");
    assert_eq!(transfer["preview_source_account_id"], 1001);
    assert_eq!(transfer["preview_destination_account_id"], 1002);
    assert_ne!(transfer["preview_main_category"], "其他收入");
    assert_ne!(transfer["preview_sub_category"], "原路退款");
    assert_ne!(transfer["matching"]["learning"]["auto_apply"], true);
    assert_eq!(transfer["matching"]["learning"]["review_status"], "");
    assert_eq!(transfer["matching"]["learning"]["reason"], "");
    assert_ne!(
        transfer["matching"]["annotation"]["type"],
        "transfer_account_direction"
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_stage2_uses_transfer_account_rules_over_fuzzy_names(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let mut runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_import_intelligence_tables(&runtime)?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
         VALUES (904, 42, 4, '一般转账', '电子支付', 5)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO accounts(id, user_id, name, aliases, hidden)
         VALUES
         (42, 42, '民生银行', '[\"网络银行\", \"民生银行\"]', 0),
         (312, 42, '支付宝', '[\"余额宝\", \"Alipay\", \"alipay\"]', 0)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO account_rules(
             id, user_id, account_id, rule_expression, enabled, priority,
             account_role_scope, transaction_type_scope, field_scope
         ) VALUES
             (8201, 42, 42, 'OR={网络银行}', 1, 1, 'source', 'transfer', '[\"expense_payment_method\"]'),
             (8202, 42, 312, 'OR={alipay}', 1, 1, 'destination', 'transfer', '[\"parser\"]')",
        [],
    )?;
    let session_id = "session-stage2-transfer-exact-account-alias";
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            ImportParserTemplateDraft {
                parser_date: "2016-09-01 12:22:16".to_string(),
                parser_amount: -6000.0,
                parser_type: "支出".to_string(),
                parser_description: "支付宝快捷支付 | 支出 | 余额宝-单次转入".to_string(),
                parser_id: "cmbc".to_string(),
                parser_tags: Some(json!(["parser:cmbc", "channel:bank"])),
                parser_counterparty: "支付宝（中国）网络技术有限公司客户备付金".to_string(),
                parser_payment_method: "网络银行".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "cmbc".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2016-09-01 12:22:11".to_string(),
                parser_amount: 6000.0,
                parser_type: "收入".to_string(),
                parser_description: "余额宝-单次转入".to_string(),
                parser_id: "alipay".to_string(),
                parser_tags: Some(json!(["parser:alipay", "channel:wallet"])),
                parser_counterparty: "天弘基金管理有限公司".to_string(),
                parser_payment_method: "中国民生银行储蓄卡(6332)".to_string(),
                parser_original_type: "不计收支".to_string(),
                parser_original_category: "转账红包".to_string(),
                parser_account_id: "alipay".to_string(),
            },
        ],
    )?;
    drop(runtime);

    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"session_id": session_id, "include_preview": true}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let preview = body["data"]["preview"].as_array().expect("preview rows");
    assert_eq!(preview.len(), 1);
    let transfer = &preview[0];

    assert_eq!(transfer["dedup_type"], "transfer");
    assert_eq!(transfer["preview_type"], "转账");
    assert_eq!(transfer["preview_main_category"], "一般转账");
    assert_eq!(transfer["preview_sub_category"], "电子支付");
    assert_eq!(transfer["preview_source_account_id"], 42);
    assert_eq!(transfer["preview_destination_account_id"], 312);
    assert_ne!(
        transfer["matching"]["annotation"]["type"],
        "transfer_account_direction"
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_stage2_allows_transfer_domain_learning_for_transfer_pair(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let mut runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_import_intelligence_tables(&runtime)?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
         VALUES (904, 42, 4, '一般转账', '电子支付', 5)",
        [],
    )?;
    let features = build_composite_match_features(
        "cmbc",
        "支付宝（中国）网络技术有限公司客户备付金",
        "支付宝快捷支付",
        "网络银行",
    )
    .expect("transfer learning features");
    let composite_hash = composite_hash_from_features(&features);
    runtime.connection().execute(
        "INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, normalized_match_value, learned_type,
            learned_category_id, learned_source_account_id, learned_destination_account_id,
            enabled, parser_id, composite_match_hash, match_features_json, created_at, updated_at
         ) VALUES (7102, 42, 'composite', ?1, ?1, '转账', 904, 1001, 1002, 1, 'cmbc', ?1, ?2, '2026-05-01', '2026-05-01')",
        [composite_hash, serde_json::to_string(&features)?],
    )?;
    let session_id = "session-stage2-transfer-domain-learning";
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            ImportParserTemplateDraft {
                parser_date: "2016-09-01 12:22:16".to_string(),
                parser_amount: -6000.0,
                parser_type: "支出".to_string(),
                parser_description: "支付宝快捷支付".to_string(),
                parser_id: "cmbc".to_string(),
                parser_tags: Some(json!(["parser:cmbc", "channel:bank"])),
                parser_counterparty: "支付宝（中国）网络技术有限公司客户备付金".to_string(),
                parser_payment_method: "网络银行".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "1001".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2016-09-01 12:22:11".to_string(),
                parser_amount: 6000.0,
                parser_type: "收入".to_string(),
                parser_description: String::new(),
                parser_id: "alipay".to_string(),
                parser_tags: Some(json!(["parser:alipay", "channel:wallet"])),
                parser_counterparty: String::new(),
                parser_payment_method: String::new(),
                parser_original_type: "不计收支".to_string(),
                parser_original_category: "转账红包".to_string(),
                parser_account_id: "1002".to_string(),
            },
        ],
    )?;
    drop(runtime);

    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"session_id": session_id, "include_preview": true}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["data"]["match_stats"]["learning_applied"], 1);
    let preview = body["data"]["preview"].as_array().expect("preview rows");
    assert_eq!(preview.len(), 1);
    let transfer = &preview[0];

    assert_eq!(transfer["dedup_type"], "transfer");
    assert_eq!(transfer["preview_type"], "转账");
    assert_eq!(transfer["preview_main_category"], "一般转账");
    assert_eq!(transfer["preview_sub_category"], "电子支付");
    assert_eq!(transfer["preview_source_account_id"], 1001);
    assert_eq!(transfer["preview_destination_account_id"], 1002);
    assert_eq!(
        transfer["matching"]["learning"]["review_status"],
        "auto_applied"
    );
    assert_eq!(transfer["matching"]["learning"]["auto_apply"], true);
    assert_eq!(transfer["matching"]["learning"]["rule_id"], 7102);
    assert_ne!(
        transfer["matching"]["annotation"]["type"],
        "transfer_account_direction"
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_stage2_learning_rejects_unknown_or_incompatible_categories(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let mut runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_import_intelligence_tables(&runtime)?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
         VALUES (903, 42, 2, '其他收入', '原路退款', 30)",
        [],
    )?;

    for (rule_id, counterparty, description, category_id) in [
        (7201, "无效分类商户", "无效分类学习", 999_999),
        (7202, "错类商户", "错类学习", 903),
    ] {
        let features =
            build_composite_match_features("wechat", counterparty, description, "微信支付")
                .expect("learning features");
        let composite_hash = composite_hash_from_features(&features);
        runtime.connection().execute(
            "INSERT INTO import_learning_rules(
                id, user_id, match_type, match_value, normalized_match_value, learned_type,
                learned_category_id, learned_source_account_id, learned_destination_account_id,
                enabled, parser_id, composite_match_hash, match_features_json, created_at, updated_at
             ) VALUES (?1, 42, 'composite', ?2, ?2, '支出', ?3, 1002, NULL, 1, 'wechat', ?2, ?4, '2026-05-01', '2026-05-01')",
            rusqlite::params![rule_id, composite_hash, category_id, serde_json::to_string(&features)?],
        )?;
    }
    let overbroad_features = build_composite_match_features(
        "cmbc",
        "网银在线（北京）科技有限公司客户备付金",
        "快捷支付退货 | 网银在线（北京）科技有限公司客户备付金 | 网络银行 | 695438343",
        "网络银行",
    )
    .expect("overbroad learning features");
    let overbroad_hash = composite_hash_from_features(&overbroad_features);
    runtime.connection().execute(
        "INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, normalized_match_value, learned_type,
            learned_category_id, learned_source_account_id, learned_destination_account_id,
            enabled, parser_id, composite_match_hash, match_features_json, created_at, updated_at
         ) VALUES (7203, 42, 'composite', ?1, ?1, '收入', 903, 1002, NULL, 1, 'cmbc', ?1, ?2, '2026-05-01', '2026-05-01')",
        [overbroad_hash, serde_json::to_string(&overbroad_features)?],
    )?;

    let session_id = "session-stage2-learning-category-guard";
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            ImportParserTemplateDraft {
                parser_date: "2026-05-06 10:00:00".to_string(),
                parser_amount: -18.0,
                parser_type: "支出".to_string(),
                parser_description: "无效分类学习".to_string(),
                parser_id: "wechat".to_string(),
                parser_tags: Some(json!(["parser:wechat", "channel:wallet"])),
                parser_counterparty: "无效分类商户".to_string(),
                parser_payment_method: "微信支付".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "wechat".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2026-05-06 11:00:00".to_string(),
                parser_amount: -19.0,
                parser_type: "支出".to_string(),
                parser_description: "错类学习".to_string(),
                parser_id: "wechat".to_string(),
                parser_tags: Some(json!(["parser:wechat", "channel:wallet"])),
                parser_counterparty: "错类商户".to_string(),
                parser_payment_method: "微信支付".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "wechat".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2016-06-18 16:27:07".to_string(),
                parser_amount: -0.78,
                parser_type: "支出".to_string(),
                parser_description: "快捷支付 | 财付通支付科技有限公司客户备付金 | 网络银行 | 支出 | 1820014210000931".to_string(),
                parser_id: "cmbc".to_string(),
                parser_tags: Some(json!(["parser:cmbc", "channel:bank"])),
                parser_counterparty: "财付通支付科技有限公司客户备付金".to_string(),
                parser_payment_method: "网络银行".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "wechat".to_string(),
            },
        ],
    )?;
    drop(runtime);

    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"session_id": session_id, "include_preview": true}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["data"]["match_stats"]["learning_applied"], 0);
    let preview = body["data"]["preview"].as_array().expect("preview rows");
    assert_eq!(preview.len(), 3);

    for item in preview
        .iter()
        .filter(|item| item["preview_counterparty"] != "财付通支付科技有限公司客户备付金")
    {
        assert_ne!(item["matching"]["learning"]["auto_apply"], true);
        let reason = item["matching"]["learning"]["reason"]
            .as_str()
            .expect("learning skip reason");
        assert!(reason.contains("category"));
        assert_ne!(item["preview_main_category"], "其他收入");
        assert_ne!(item["preview_sub_category"], "原路退款");
    }
    let overbroad = preview
        .iter()
        .find(|item| item["preview_counterparty"] == "财付通支付科技有限公司客户备付金")
        .expect("overbroad preview");
    assert_ne!(overbroad["matching"]["learning"]["auto_apply"], true);
    assert_eq!(overbroad["preview_type"], "支出");
    assert_ne!(overbroad["preview_main_category"], "其他收入");
    assert_ne!(overbroad["preview_sub_category"], "原路退款");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_previews_temp_file_and_parses_column_mapping(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let boundary = "rust-import-unmatched-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[(
            "files",
            "custom-ledger.csv",
            "text/csv",
            "when,kind,value,note\n2026-05-05 10:00:00,收款,88.20,奖金\n",
        )],
    );

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["data"]["parsed_count"], 0);
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    let temp_path = parse_body["data"]["unmatched_files"][0]["temp_path"]
        .as_str()
        .expect("temp path")
        .to_string();
    assert!(!std::path::Path::new(&temp_path).is_absolute());
    assert!(temp_path.starts_with("user-42/"));

    let preview_boundary = "rust-import-preview-boundary";
    let preview_body = multipart_body(preview_boundary, &[("temp_path", temp_path.as_str())], &[]);
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={preview_boundary}"),
                )
                .body(Body::from(preview_body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_body = read_json(preview).await;
    assert_eq!(preview_body["success"], true);
    assert_eq!(preview_body["result"]["sampleData"][0][0], "when");
    assert_eq!(preview_body["result"]["delimiter"], ",");

    let json_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "temp_path": temp_path.clone(),
                        "delimiter": ","
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(json_preview.status(), StatusCode::OK);
    let json_preview_body = read_json(json_preview).await;
    assert_eq!(
        json_preview_body["result"]["previewRows"][0][0],
        "2026-05-05 10:00:00"
    );

    let encoded_temp_path = temp_path.replace('/', "%2F");
    let urlencoded_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "temp_path={encoded_temp_path}&delimiter=%2C"
                )))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(urlencoded_preview.status(), StatusCode::OK);
    let urlencoded_preview_body = read_json(urlencoded_preview).await;
    assert_eq!(urlencoded_preview_body["result"]["headers"][0], "when");

    let upload_preview_boundary = "rust-import-upload-preview-boundary";
    let upload_preview_body = multipart_body(
        upload_preview_boundary,
        &[],
        &[(
            "file",
            "tab-ledger.tsv",
            "text/tab-separated-values",
            "时间\t类型\t金额\t说明\n2026-05-06 09:00:00\t收入\t10.00\t零钱\n",
        )],
    );
    let upload_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={upload_preview_boundary}"),
                )
                .body(Body::from(upload_preview_body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(upload_preview.status(), StatusCode::OK);
    let upload_preview_body = read_json(upload_preview).await;
    assert_eq!(upload_preview_body["result"]["delimiter"], "\t");

    let invalid_temp_path_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"temp_path": "../not-owned/ledger.csv"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(invalid_temp_path_preview.status(), StatusCode::BAD_REQUEST);

    let other_user_preview_body =
        multipart_body(preview_boundary, &[("temp_path", temp_path.as_str())], &[]);
    let other_user_preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header("x-user-id", "77")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={preview_boundary}"),
                )
                .body(Body::from(other_user_preview_body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(other_user_preview.status(), StatusCode::NOT_FOUND);

    let parse_generic = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse_generic")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "parser_id": "generic",
                        "temp_path": temp_path.clone(),
                        "column_mapping": {
                            "1": 0,
                            "3": 1,
                            "8": 2,
                            "14": 3
                        },
                        "transaction_type_mapping": {
                            "收款": 2
                        },
                        "has_header_line": true,
                        "delimiter": ","
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse_generic.status(), StatusCode::OK);
    let parse_generic_body = read_json(parse_generic).await;
    assert_eq!(parse_generic_body["data"]["session_id"], session_id);
    assert_eq!(parse_generic_body["data"]["parsed_count"], 1);

    let invalid_mapping = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse_generic")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "parser_id": "generic",
                        "temp_path": temp_path.clone(),
                        "column_mapping": {
                            "1": 0,
                            "3": 1
                        },
                        "has_header_line": "yes",
                        "delimiter": ","
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(invalid_mapping.status(), StatusCode::BAD_REQUEST);

    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let session = get_import_session(db_runtime.connection(), &session_id, user_id(42))?
        .expect("session remains");
    assert_eq!(session.total_parsed, 1);
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), &session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].parser_id, "generic");
    assert_eq!(templates[0].parser_type, "收入");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_parse_keeps_multiple_unmatched_files_in_one_session(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);
    let boundary = "rust-import-multi-unmatched-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[
            (
                "files",
                "custom-ledger-a.csv",
                "text/csv",
                "when,kind,value,note\n2026-05-05 10:00:00,收款,88.20,奖金\n",
            ),
            (
                "files",
                "custom-ledger-b.csv",
                "text/csv",
                "when,kind,value,note\n2026-05-06 10:00:00,付款,12.30,咖啡\n",
            ),
        ],
    );

    let parse = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["data"]["parsed_count"], 0);
    let unmatched_files = parse_body["data"]["unmatched_files"]
        .as_array()
        .expect("unmatched files");
    assert_eq!(unmatched_files.len(), 2);
    assert_eq!(unmatched_files[0]["original_name"], "custom-ledger-a.csv");
    assert_eq!(unmatched_files[1]["original_name"], "custom-ledger-b.csv");
    assert!(unmatched_files.iter().all(|file| file["temp_path"]
        .as_str()
        .is_some_and(|value| value.starts_with("user-42/"))));
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_updates_preview_row_and_returns_preview_item(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("seeded preview row");
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-a/update")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "id": row.id,
                        "type": "收入",
                        "amount": "99.5",
                        "destinationAmount": 9.25,
                        "mainCategory": "工资",
                        "subCategory": "基本工资",
                        "sourceAccountId": 1001,
                        "destinationAccountId": null,
                        "counterparty": "employer",
                        "paymentMethod": "bank",
                        "description": "runtime updated row",
                        "isSelected": false,
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["updated"], true);
    assert_eq!(body["data"]["previewItem"]["preview_type"], "收入");
    assert_eq!(
        body["data"]["previewItem"]["preview_description"],
        "runtime updated row"
    );
    assert_eq!(body["data"]["previewItem"]["preview_selected"], false);

    let rows = preview_rows(fixture.db_path(), "session-a")?;
    let updated = rows
        .iter()
        .find(|preview| preview.id == row.id)
        .expect("updated row remains");
    assert_eq!(updated.preview_amount, 99.5);
    assert_eq!(updated.preview_destination_amount, 9.25);
    assert_eq!(updated.preview_source_account_id, Some(1001));
    assert_eq!(updated.preview_destination_account_id, None);
    assert!(!updated.preview_selected);

    let wrong_session = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/other-session/update")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(json!({"id": row.id}).to_string()))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(wrong_session.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_applies_reclassify_preview_updates() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("seeded preview row");
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/reclassify/session-a")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                            "preview_updates": [{
                                "id": row.id,
                                "preview_type": "收入",
                                "preview_amount": 33.75,
                                "preview_source_account_id": 1001,
                                "preview_destination_account_id": null,
                                "preview_recurring_id": 2002,
                            "preview_recurring_name": "salary",
                            "preview_recurring_candidate_count": 3,
                            "preview_recurring_match_score": 0.91,
                            "preview_recurring_match_reasons": ["amount", "date"],
                            "preview_recurring_matched_date": "2026-05-03",
                            "matchingFeedback": {
                                "llm": {
                                    "review_status": "pending"
                                }
                            },
                            "selected": false,
                            "clear_transfer_decision": true
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["session_id"], "session-a");
    assert_eq!(body["data"]["total"], 2);
    assert_eq!(body["data"]["updated"], 1);
    assert_eq!(body["data"]["categorized"], 2);
    assert_eq!(body["data"]["account_matched"], 1);
    let preview = body["data"]["preview"].as_array().expect("preview array");
    let updated = preview
        .iter()
        .find(|item| item["id"] == row.id)
        .expect("updated row in response");
    assert_eq!(updated["preview_type"], "收入");
    assert_eq!(updated["preview_recurring_id"], 2002);
    assert_eq!(updated["preview_recurring_name"], "salary");
    assert_eq!(
        updated["preview_recurring_match_reasons"],
        r#"["amount","date"]"#
    );
    assert_eq!(updated["preview_selected"], false);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_clears_actionable_matching_families_only() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("seeded preview row");
    {
        let runtime = runtime_for(fixture.db_path())?;
        runtime.connection().execute(
            "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = ?2",
            params![
                json!({
                    "parser": {"parser_id": "wechat"},
                    "dedup": {"type": "remaining"},
                    "transfer": {"review_status": "pending"},
                    "learning": {"review_status": "pending"},
                    "llm": {"review_status": "pending"}
                })
                .to_string(),
                row.id
            ],
        )?;
    }
    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-a/update")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "id": row.id,
                        "clearActionableSuggestions": ["learning", "llm"],
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let matching = &body["data"]["previewItem"]["matching"];
    let persisted_feedback = &body["data"]["previewItem"]["preview_matching_feedback"];
    assert_eq!(matching["parser"]["parser_id"], "wechat");
    assert_eq!(matching["dedup"]["type"], "remaining");
    assert_eq!(matching["transfer"]["review_status"], "pending");
    assert!(persisted_feedback.get("learning").is_none());
    assert!(persisted_feedback.get("llm").is_none());
    assert_eq!(matching["learning"]["review_status"], "");
    assert_eq!(matching["llm"]["review_status"], "");

    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-a/update")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "id": row.id,
                        "clearActionableSuggestions": {
                            "transfer": true,
                            "learning": false
                        },
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let matching = &body["data"]["previewItem"]["matching"];
    assert!(body["data"]["previewItem"]["preview_matching_feedback"]
        .get("transfer")
        .is_none());
    assert_eq!(matching["parser"]["parser_id"], "wechat");
    assert_eq!(matching["dedup"]["type"], "remaining");
    assert_eq!(matching["transfer"]["review_status"], "");

    {
        let runtime = runtime_for(fixture.db_path())?;
        runtime.connection().execute(
            "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = ?2",
            params![
                json!({
                    "parser": {"parser_id": "wechat"},
                    "dedup": {"type": "remaining"},
                    "transfer": {"review_status": "pending"},
                    "learning": {"review_status": "pending"},
                    "llm": {"review_status": "pending"}
                })
                .to_string(),
                row.id
            ],
        )?;
    }
    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-a/update")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "id": row.id,
                        "clearActionableSuggestions": true,
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let persisted_feedback = &body["data"]["previewItem"]["preview_matching_feedback"];
    assert!(persisted_feedback.get("transfer").is_none());
    assert!(persisted_feedback.get("learning").is_none());
    assert!(persisted_feedback.get("llm").is_none());

    {
        let runtime = runtime_for(fixture.db_path())?;
        runtime.connection().execute(
            "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = ?2",
            params![
                json!({
                    "transfer": {"review_status": "accepted"},
                    "learning": {"review_status": "rejected", "suppressed": true},
                    "llm": {"review_status": "auto_applied"}
                })
                .to_string(),
                row.id
            ],
        )?;
    }
    let app = runtime_router(&fixture);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-a/update")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "id": row.id,
                        "clearActionableSuggestions": true,
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let persisted_feedback = &body["data"]["previewItem"]["preview_matching_feedback"];
    assert_eq!(persisted_feedback["transfer"]["review_status"], "accepted");
    assert_eq!(persisted_feedback["learning"]["review_status"], "rejected");
    assert!(persisted_feedback.get("llm").is_none());

    Ok(())
}

#[tokio::test]
async fn import_db_runtime_handles_preview_item_transfer_and_recurring_decisions(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let rows = preview_rows(fixture.db_path(), "session-a")?;
    let transfer_row = rows.first().expect("seeded transfer row");
    let recurring_row = rows.get(1).expect("seeded recurring row");
    let app = runtime_router(&fixture);

    let transfer = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "/api/bills/import/v2/preview-item/{}/transfer-decision",
                    transfer_row.id
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "decision": "accept",
                        "expectedState": {
                            "sessionId": "session-a",
                            "type": "支出",
                            "recurringTemplateId": null,
                            "sourceAccountId": null,
                            "destinationAccountId": null
                        },
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(transfer.status(), StatusCode::OK);
    let transfer_body = read_json(transfer).await;
    assert_eq!(transfer_body["success"], true);
    assert_eq!(transfer_body["data"]["decision"], "accept");
    assert_eq!(transfer_body["data"]["previewItem"]["preview_type"], "转账");
    assert_eq!(transfer_body["data"]["sessionId"], "session-a");

    let recurring = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/bills/import/v2/preview-item/{}/recurring-match",
                    recurring_row.id
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "recurringId": 2002,
                        "expectedState": {
                            "sessionId": "session-a",
                            "type": "支出",
                            "recurringTemplateId": null
                        },
                        "candidate": {
                            "id": 2002,
                            "name": "salary",
                            "matchScore": 0.87,
                            "matchReasons": "amount,date",
                            "matchedOccurrenceDate": "2026-05-01"
                        },
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(recurring.status(), StatusCode::OK);
    let recurring_body = read_json(recurring).await;
    assert_eq!(recurring_body["success"], true);
    assert_eq!(recurring_body["data"]["recurringId"], 2002);
    assert_eq!(
        recurring_body["data"]["previewItem"]["preview_recurring_name"],
        "salary"
    );

    let clear = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!(
                    "/api/bills/import/v2/preview-item/{}/recurring-match",
                    recurring_row.id
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "expectedState": {
                            "sessionId": "session-a",
                            "recurringTemplateId": 2002
                        },
                        "responseMode": "preview-item"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(clear.status(), StatusCode::OK);
    let clear_body = read_json(clear).await;
    assert_eq!(clear_body["data"]["recurringId"], Value::Null);
    assert_eq!(
        clear_body["data"]["previewItem"]["preview_recurring_id"],
        Value::Null
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_returns_empty_learning_suggestions_and_applies_preview_updates(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("seeded preview row");
    let app = runtime_router(&fixture);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/learning/session-a/suggestions")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = read_json(get_response).await;
    assert_eq!(get_body["data"]["suggestions"].as_array().unwrap().len(), 0);

    let post_response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/learning/session-a/suggestions")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "preview_updates": [{
                            "id": row.id,
                            "preview_amount": 44.25,
                            "preview_main_category": "交通",
                            "preview_sub_category": "地铁"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(post_response.status(), StatusCode::OK);
    let post_body = read_json(post_response).await;
    assert_eq!(post_body["data"]["applied_preview_updates"], 1);
    assert_eq!(post_body["data"]["count"], 0);

    let rows = preview_rows(fixture.db_path(), "session-a")?;
    let updated = rows
        .iter()
        .find(|preview| preview.id == row.id)
        .expect("updated row remains");
    assert_eq!(updated.preview_amount, 44.25);
    assert_eq!(updated.preview_main_category, "交通");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_handles_llm_review_decisions_and_memory() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("seeded preview row");
    let app = runtime_router(&fixture);

    let accept = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/llm/preview-recommend/accept")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": "session-a",
                        "preview_id": row.id,
                        "suggestion": {
                            "suggested_main_category": "餐饮",
                            "suggested_sub_category": "咖啡",
                            "suggested_source_account": "招商银行",
                            "resolved_source_account_id": 300,
                            "confidence": 0.91,
                            "reason": "merchant pattern"
                        }
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(accept.status(), StatusCode::OK);
    let accept_body = read_json(accept).await;
    assert_eq!(accept_body["success"], true);
    assert_eq!(accept_body["data"]["session_id"], "session-a");
    assert_eq!(accept_body["data"]["decision"], "accept");
    assert!(accept_body["data"]["event_id"].as_i64().unwrap_or_default() > 0);
    assert_eq!(
        accept_body["data"]["matching"]["llm"]["review_status"],
        "accepted"
    );

    let reject = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/llm/preview-recommend/reject")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "sessionId": "session-a",
                        "previewId": row.id,
                        "suggestion": {
                            "mainCategory": "交通",
                            "subCategory": "地铁",
                            "sourceAccount": "现金",
                            "destinationAccount": "储蓄卡",
                            "confidence": 0.5
                        },
                        "userCorrection": {
                            "categoryName": "餐饮",
                            "accountName": "招商银行"
                        }
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(reject.status(), StatusCode::OK);
    let reject_body = read_json(reject).await;
    assert_eq!(reject_body["data"]["decision"], "reject");
    assert_eq!(
        reject_body["data"]["matching"]["llm"]["review_status"],
        "rejected"
    );

    let memory = app
        .oneshot(
            Request::builder()
                .uri("/api/llm/memory?session_id=session-a&limit=10")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(memory.status(), StatusCode::OK);
    let memory_body = read_json(memory).await;
    assert_eq!(memory_body["success"], true);
    assert_eq!(memory_body["total"], 2);
    let decisions = memory_body["data"]
        .as_array()
        .expect("memory rows")
        .iter()
        .map(|row| row["decision"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(decisions.contains(&"accept"));
    assert!(decisions.contains(&"reject"));
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_llm_configs_and_candidates() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_llm_runtime_tables(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let default_config = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/llm/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(default_config.status(), StatusCode::OK);
    let default_body = read_json(default_config).await;
    assert_eq!(default_body["success"], true);
    assert_eq!(default_body["data"]["enabled"], false);
    assert!(default_body["data"]["available_providers"]
        .as_array()
        .expect("providers")
        .iter()
        .any(|item| item == "openai"));

    let update_runtime = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/llm/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "provider": "openai_compatible",
                        "provider_config": {
                            "model": "runtime-model",
                            "api_key": "runtime-secret"
                        },
                        "advanced_settings": {
                            "reasoning_depth": "medium",
                            "temperature": 0.4
                        }
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(update_runtime.status(), StatusCode::OK);
    let update_body = read_json(update_runtime).await;
    assert_eq!(update_body["data"]["enabled"], true);
    assert_eq!(update_body["data"]["model"], "runtime-model");
    assert!(update_body["data"].get("available_providers").is_none());

    let stored_runtime = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/llm/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(
        read_json(stored_runtime).await["data"]["model"],
        "runtime-model"
    );

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/llm/configs")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "primary",
                        "provider": "openai",
                        "model": "saved-model",
                        "api_key": "saved-secret",
                        "base_url": "https://api.example.test",
                        "advanced_settings": {"reasoning_depth": "high"},
                        "is_active": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(created.status(), StatusCode::OK);
    let created_body = read_json(created).await;
    assert_eq!(created_body["data"]["api_key"], "********");
    assert_eq!(created_body["data"]["has_api_key"], true);
    let config_id = created_body["data"]["id"].as_i64().expect("config id");

    let active_config = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/llm/config")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(
        read_json(active_config).await["data"]["model"],
        "saved-model"
    );

    let updated_config = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/llm/configs/{config_id}"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"api_key": "********", "model": "saved-model-v2"}).to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(updated_config.status(), StatusCode::OK);
    let updated_body = read_json(updated_config).await;
    assert_eq!(updated_body["data"]["model"], "saved-model-v2");
    assert_eq!(updated_body["data"]["has_api_key"], true);

    let candidates = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/llm/candidates?status=pending&type=rule_synthesis")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(candidates.status(), StatusCode::OK);
    let candidates_body = read_json(candidates).await;
    assert_eq!(candidates_body["success"], true);
    assert_eq!(candidates_body["total"], 1);
    let candidate_id = candidates_body["data"][0]["id"]
        .as_i64()
        .expect("candidate id");

    let accepted = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/llm/candidates/{candidate_id}/accept"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_body = read_json(accepted).await;
    assert_eq!(accepted_body["data"]["status"], "accepted");
    assert!(accepted_body["data"]["created_rule_id"].as_i64().is_some());

    let rejected = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/llm/candidates/2/reject")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(read_json(rejected).await["data"]["rejected"], true);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_covers_llm_config_and_candidate_edges() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_llm_runtime_tables(fixture.db_path())?;
    let app = runtime_router(&fixture);

    let empty_runtime_update =
        trusted_json_route(&app, Method::POST, "/api/llm/config", None, 42).await;
    assert_eq!(empty_runtime_update.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(empty_runtime_update).await["error"],
        "No data provided"
    );

    let invalid_runtime_update = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/config",
        Some("{not-json".to_string()),
        42,
    )
    .await;
    assert_eq!(invalid_runtime_update.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_runtime_update).await["error"],
        "Invalid JSON request"
    );

    let missing_name = trusted_json_route(&app, Method::POST, "/api/llm/configs", None, 42).await;
    assert_eq!(missing_name.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_name).await["error"], "name is required");

    let fallback_config = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/configs",
        Some(json!({"name": "fallback"}).to_string()),
        42,
    )
    .await;
    assert_eq!(fallback_config.status(), StatusCode::OK);
    let fallback_body = read_json(fallback_config).await;
    assert_eq!(fallback_body["data"]["provider"], "openai");
    assert_eq!(fallback_body["data"]["model"], "");
    assert_eq!(fallback_body["data"]["has_api_key"], false);
    let fallback_id = fallback_body["data"]["id"]
        .as_i64()
        .expect("fallback config id");

    let listed = trusted_json_route(&app, Method::GET, "/api/llm/configs", None, 42).await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = read_json(listed).await;
    assert_eq!(listed_body["data"].as_array().expect("configs").len(), 1);

    let activate_missing = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/configs/9999/activate",
        None,
        42,
    )
    .await;
    assert_eq!(activate_missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(activate_missing).await["error"],
        "config_not_found"
    );

    let activate_fallback = trusted_json_route(
        &app,
        Method::POST,
        format!("/api/llm/configs/{fallback_id}/activate"),
        None,
        42,
    )
    .await;
    assert_eq!(activate_fallback.status(), StatusCode::OK);

    let update_missing = trusted_json_route(
        &app,
        Method::PUT,
        "/api/llm/configs/9999",
        Some(json!({"api_key": "real-secret"}).to_string()),
        42,
    )
    .await;
    assert_eq!(update_missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(read_json(update_missing).await["error"], "config_not_found");

    let candidate_detail =
        trusted_json_route(&app, Method::GET, "/api/llm/candidates/1", None, 42).await;
    assert_eq!(candidate_detail.status(), StatusCode::OK);
    assert_eq!(
        read_json(candidate_detail).await["data"]["suggested_sub_category"],
        "咖啡"
    );
    let clamped_candidates = trusted_json_route(
        &app,
        Method::GET,
        "/api/llm/candidates?limit=100000&offset=-20",
        None,
        42,
    )
    .await;
    assert_eq!(clamped_candidates.status(), StatusCode::OK);
    assert_eq!(
        read_json(clamped_candidates).await["data"]
            .as_array()
            .expect("candidate list")
            .len(),
        2
    );

    let missing_candidate =
        trusted_json_route(&app, Method::GET, "/api/llm/candidates/9999", None, 42).await;
    assert_eq!(missing_candidate.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_candidate).await["error"],
        "Candidate 9999 not found"
    );

    let missing_accept = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/candidates/9999/accept",
        None,
        42,
    )
    .await;
    assert_eq!(missing_accept.status(), StatusCode::NOT_FOUND);

    let missing_reject = trusted_json_route(
        &app,
        Method::POST,
        "/api/llm/candidates/9999/reject",
        None,
        42,
    )
    .await;
    assert_eq!(missing_reject.status(), StatusCode::NOT_FOUND);

    let deleted = trusted_json_route(
        &app,
        Method::DELETE,
        format!("/api/llm/configs/{fallback_id}"),
        None,
        42,
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);

    let delete_missing =
        trusted_json_route(&app, Method::DELETE, "/api/llm/configs/9999", None, 42).await;
    assert_eq!(delete_missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(read_json(delete_missing).await["error"], "config_not_found");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_serves_preview_index_and_legacy_parser_catalog(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let app = runtime_router(&fixture);

    let index = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/preview/session-a/index")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(index.status(), StatusCode::OK);
    let index_body = read_json(index).await;
    assert_eq!(index_body["success"], true);
    assert_eq!(index_body["data"]["total"], 2);
    assert_eq!(
        index_body["data"]["items"][0]["comment"],
        "first preview row"
    );
    assert_eq!(index_body["data"]["items"][0]["type"], 3);

    let parsers = app
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/parsers")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parsers.status(), StatusCode::OK);
    let parsers_body = read_json(parsers).await;
    let parser_ids = parsers_body["result"]
        .as_array()
        .expect("parser catalog")
        .iter()
        .map(|item| item["id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(parser_ids.contains(&"auto"));
    assert!(parser_ids.contains(&"wechat"));
    assert!(parser_ids.contains(&"ccb"));
    Ok(())
}

#[tokio::test]
async fn import_dedup_materializes_historical_duplicates_into_preview() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    runtime.connection().execute(
        "
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, source_account_id,
            hash, created_at, updated_at
        ) VALUES (
            42, '2026-05-04 10:00:05', '支出', -20.00, 'coffee shop',
            'latte', 'icbc', '餐饮', '咖啡', 101, 'history-hash',
            '2026-05-04T10:00:05', '2026-05-04T10:00:05'
        )
        ",
        [],
    )?;
    let history_bill_id = runtime.connection().last_insert_rowid();
    let app = runtime_router(&fixture);

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "parser_id": "wechat",
                        "file_count": 1,
                        "bills": [{
                            "date": "2026-05-04 10:00:00",
                            "type": "支出",
                            "amount": -20.0,
                            "description": "latte",
                            "counterparty": "coffee shop",
                            "payment_method": "wechat",
                            "source_account_id": "1001",
                            "main_category": "餐饮",
                            "sub_category": "咖啡"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let session_id = read_json(parse).await["data"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let dedup = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup.status(), StatusCode::OK);
    let dedup_body = read_json(dedup).await;
    assert_eq!(dedup_body["data"]["after_dedup"], 1);
    assert_eq!(dedup_body["data"]["match_stats"]["database_candidates"], 1);
    let preview = dedup_body["data"]["preview"]
        .as_array()
        .expect("preview rows");
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0]["dedup_type"], "database_duplicate");
    assert_eq!(
        preview[0]["matching"]["reconciliation"]["planned_operation"],
        "update_history"
    );
    assert_eq!(
        preview[0]["matching"]["reconciliation"]["history_bill_id"],
        history_bill_id
    );
    assert_eq!(
        preview[0]["matching"]["annotation"]["type"],
        "history_rewrite_pending"
    );

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = read_json(replay).await;
    assert_eq!(
        replay_body["data"]["match_stats"]["idempotent_replay"],
        true
    );
    assert_eq!(
        replay_body["data"]["preview"][0]["dedup_type"],
        "database_duplicate"
    );

    let db_runtime = runtime_for(fixture.db_path())?;
    let groups =
        get_import_decision_groups_by_session(db_runtime.connection(), &session_id, user_id(42))?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].group_type, "historical_duplicate");
    assert!(groups[0]
        .members
        .iter()
        .any(|member| member.history_bill_id == Some(history_bill_id)));
    let materializations = get_import_history_materializations_by_session(
        db_runtime.connection(),
        &session_id,
        user_id(42),
    )?;
    assert_eq!(materializations.len(), 1);
    assert_eq!(materializations[0].history_bill_id, history_bill_id);

    let select_all = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/bills/import/v2/preview/{session_id}/selection"
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "selectionAction": "select_all"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(select_all.status(), StatusCode::OK);

    let confirm_without_ack = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm_without_ack.status(), StatusCode::BAD_REQUEST);
    assert!(read_json(confirm_without_ack).await["error"]
        .as_str()
        .unwrap_or_default()
        .contains("history rewrite acknowledgement"));
    let remaining_history_rows: i64 = db_runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = 42 AND id = ?1",
        [history_bill_id],
        |row| row.get(0),
    )?;
    assert_eq!(remaining_history_rows, 1);
    let preview_id = preview[0]["id"].as_i64().expect("preview id");
    let confirm_with_ack = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "history_rewrite_acknowledgement": {
                            "acknowledged": true,
                            "selected_preview_ids": [preview_id],
                            "selection_scope": {"mode": "select_all"},
                            "operations": [{
                                "preview_id": preview_id,
                                "operation_id": preview[0]["matching"]["reconciliation"]["operation_id"],
                                "planned_operation": "update_history",
                                "history_bill_id": history_bill_id,
                                "history_bill_version": preview[0]["matching"]["reconciliation"]["history_bill_version"],
                                "acknowledgement_token": preview[0]["matching"]["reconciliation"]["acknowledgement_token"]
                            }]
                        }
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm_with_ack.status(), StatusCode::OK);
    let confirm_body = read_json(confirm_with_ack).await;
    assert_eq!(confirm_body["data"]["imported_count"], 1);
    let updated_history_rows: i64 = db_runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = 42 AND id = ?1 AND import_history_id = 2",
        [history_bill_id],
        |row| row.get(0),
    )?;
    assert_eq!(updated_history_rows, 1);
    Ok(())
}

#[tokio::test]
async fn import_dedup_keeps_distinct_same_amount_history_match_as_import_preview(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    runtime.connection().execute(
        "
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, source_account_id,
            hash, created_at, updated_at
        ) VALUES (
            42, '2026-05-04 10:00:05', '支出', -20.00, 'book store',
            'magazine', 'icbc', '购物', '图书', 101, 'history-distinct-hash',
            '2026-05-04T10:00:05', '2026-05-04T10:00:05'
        )
        ",
        [],
    )?;
    let app = runtime_router(&fixture);

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "parser_id": "wechat",
                        "file_count": 1,
                        "bills": [{
                            "date": "2026-05-04 10:00:00",
                            "type": "支出",
                            "amount": -20.0,
                            "description": "latte",
                            "counterparty": "coffee shop",
                            "payment_method": "wechat",
                            "source_account_id": "1001"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let session_id = read_json(parse).await["data"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let dedup = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup.status(), StatusCode::OK);
    let dedup_body = read_json(dedup).await;
    assert_eq!(dedup_body["data"]["after_dedup"], 1);
    assert_eq!(dedup_body["data"]["match_stats"]["database_candidates"], 0);
    let preview = dedup_body["data"]["preview"]
        .as_array()
        .expect("preview rows");
    assert_eq!(preview.len(), 1);
    assert_ne!(preview[0]["dedup_type"], "database_duplicate");
    assert_eq!(preview[0]["preview_counterparty"], "coffee shop");

    let db_runtime = runtime_for(fixture.db_path())?;
    assert!(get_import_history_materializations_by_session(
        db_runtime.connection(),
        &session_id,
        user_id(42),
    )?
    .is_empty());
    Ok(())
}

#[tokio::test]
async fn import_dedup_persists_same_batch_duplicate_decision_group() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    let app = runtime_router(&fixture);

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "parser_id": "icbc",
                        "file_count": 1,
                        "bills": [
                            {
                                "date": "2026-05-04 10:00:00",
                                "type": "支出",
                                "amount": -18.60,
                                "description": "bank card",
                                "counterparty": "breakfast",
                                "payment_method": "icbc",
                                "source_account_id": "101"
                            },
                            {
                                "date": "2026-05-04 10:00:20",
                                "type": "支出",
                                "amount": -18.60,
                                "description": "wallet note",
                                "counterparty": "breakfast shop",
                                "payment_method": "wechat",
                                "source_account_id": "202"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let session_id = read_json(parse).await["data"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let dedup = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup.status(), StatusCode::OK);
    let dedup_body = read_json(dedup).await;
    assert_eq!(dedup_body["data"]["total"], 2);
    assert_eq!(dedup_body["data"]["after_dedup"], 1);
    assert_eq!(dedup_body["data"]["dedup_stats"]["duplicates"], 1);
    let preview = dedup_body["data"]["preview"]
        .as_array()
        .expect("preview rows");
    assert_eq!(preview[0]["dedup_type"], "same_batch");
    assert_eq!(preview[0]["matching"]["dedup"]["source_count"], 2);
    assert!(preview[0]["matching"]["dedup"]["source_label"]
        .as_str()
        .is_some_and(|value| value.contains("来源1")));

    let db_runtime = runtime_for(fixture.db_path())?;
    let groups =
        get_import_decision_groups_by_session(db_runtime.connection(), &session_id, user_id(42))?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].group_type, "duplicate");
    assert_eq!(groups[0].members.len(), 2);
    assert_eq!(groups[0].signal_payload["dedup_type"], "same_batch");
    Ok(())
}

#[tokio::test]
async fn import_dedup_persists_same_batch_transfer_decision_group() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let mut runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    let session_id = "session-same-batch-transfer-group";
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 2,
        },
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            ImportParserTemplateDraft {
                parser_date: "2026-05-04 10:00:00".to_string(),
                parser_amount: -125.0,
                parser_type: "支出".to_string(),
                parser_description: "outgoing wallet note".to_string(),
                parser_id: "wechat".to_string(),
                parser_tags: Some(json!(["parser:wechat"])),
                parser_counterparty: "wallet transfer".to_string(),
                parser_payment_method: "wechat balance".to_string(),
                parser_original_type: "支出".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "1001".to_string(),
            },
            ImportParserTemplateDraft {
                parser_date: "2026-05-04 10:00:12".to_string(),
                parser_amount: 125.0,
                parser_type: "收入".to_string(),
                parser_description: "incoming bank note".to_string(),
                parser_id: "icbc".to_string(),
                parser_tags: Some(json!(["parser:icbc"])),
                parser_counterparty: "bank transfer".to_string(),
                parser_payment_method: "icbc card".to_string(),
                parser_original_type: "收入".to_string(),
                parser_original_category: String::new(),
                parser_account_id: "1002".to_string(),
            },
        ],
    )?;
    drop(runtime);

    let app = runtime_router(&fixture);
    let dedup = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup.status(), StatusCode::OK);
    let dedup_body = read_json(dedup).await;
    assert_eq!(dedup_body["data"]["total"], 2);
    assert_eq!(dedup_body["data"]["after_dedup"], 1);
    assert_eq!(dedup_body["data"]["dedup_stats"]["transfer_pairs"], 1);
    let preview = dedup_body["data"]["preview"]
        .as_array()
        .expect("preview rows");
    assert_eq!(preview[0]["dedup_type"], "transfer");
    assert_eq!(preview[0]["preview_type"], "转账");
    assert_eq!(preview[0]["preview_source_account_id"], 1001);
    assert_eq!(preview[0]["preview_destination_account_id"], 1002);
    assert_eq!(
        preview[0]["matching"]["transfer"]["source_chain"][0]["description"],
        "outgoing wallet note"
    );
    assert_eq!(
        preview[0]["matching"]["transfer"]["source_chain"][1]["description"],
        "incoming bank note"
    );
    assert!(preview[0]["matching"]["transfer"]["source_label"]
        .as_str()
        .is_some_and(|value| value.contains("匹配 | 微信 | 工商银行")));

    let db_runtime = runtime_for(fixture.db_path())?;
    let groups =
        get_import_decision_groups_by_session(db_runtime.connection(), session_id, user_id(42))?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].group_type, "same_batch_transfer");
    assert_eq!(groups[0].decision_status, "matched");
    assert_eq!(groups[0].members.len(), 2);
    assert_eq!(
        groups[0].signal_payload["planned_operation"],
        "merge_transfer"
    );
    assert_eq!(
        groups[0].signal_payload["source_chain"][0]["description"],
        "outgoing wallet note"
    );
    assert!(groups[0]
        .members
        .iter()
        .any(|member| member.member_role == "outgoing"));
    assert!(groups[0]
        .members
        .iter()
        .any(|member| member.member_role == "incoming"));
    Ok(())
}

#[tokio::test]
async fn import_dedup_materializes_historical_transfers_into_preview() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    runtime.connection().execute(
        "
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, source_account_id,
            hash, created_at, updated_at
        ) VALUES (
            42, '2026-05-04 10:04:00', '收入', 300.00, 'bank incoming',
            'incoming bank note', 'icbc', '一般转账', '电子支付', 2002,
            'history-transfer-hash', '2026-05-04T10:04:00', '2026-05-04T10:04:00'
        )
        ",
        [],
    )?;
    let history_bill_id = runtime.connection().last_insert_rowid();
    let app = runtime_router(&fixture);

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "parser_id": "wechat",
                        "file_count": 1,
                        "bills": [{
                            "date": "2026-05-04 10:00:00",
                            "type": "支出",
                            "amount": -300.0,
                            "description": "outgoing wallet note",
                            "counterparty": "wallet transfer",
                            "payment_method": "wechat balance",
                            "source_account_id": "1001",
                            "main_category": "一般转账",
                            "sub_category": "电子支付"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let session_id = read_json(parse).await["data"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let dedup = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(dedup.status(), StatusCode::OK);
    let dedup_body = read_json(dedup).await;
    assert_eq!(dedup_body["data"]["after_dedup"], 1);
    assert_eq!(dedup_body["data"]["match_stats"]["database_candidates"], 1);
    let preview = dedup_body["data"]["preview"]
        .as_array()
        .expect("preview rows");
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0]["dedup_type"], "transfer_cross_batch");
    assert_eq!(preview[0]["preview_type"], "转账");
    assert_eq!(preview[0]["preview_source_account_id"], 1001);
    assert_eq!(preview[0]["preview_destination_account_id"], 2002);
    assert_eq!(
        preview[0]["matching"]["transfer"]["candidate_type"],
        "transfer_cross_batch"
    );
    assert_eq!(
        preview[0]["matching"]["transfer"]["source_chain"][0]["description"],
        "outgoing wallet note"
    );
    assert_eq!(
        preview[0]["matching"]["transfer"]["source_chain"][1]["description"],
        "incoming bank note"
    );
    assert_eq!(
        preview[0]["matching"]["reconciliation"]["planned_operation"],
        "merge_transfer_history"
    );
    assert_eq!(
        preview[0]["matching"]["reconciliation"]["history_bill_id"],
        history_bill_id
    );
    assert_eq!(
        preview[0]["matching"]["reconciliation"]["time_diff_seconds"],
        240
    );
    assert_eq!(
        preview[0]["matching"]["annotation"]["type"],
        "history_rewrite_pending"
    );

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/dedup")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "include_preview": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = read_json(replay).await;
    assert_eq!(
        replay_body["data"]["match_stats"]["idempotent_replay"],
        true
    );
    assert_eq!(
        replay_body["data"]["preview"][0]["dedup_type"],
        "transfer_cross_batch"
    );

    let db_runtime = runtime_for(fixture.db_path())?;
    let groups =
        get_import_decision_groups_by_session(db_runtime.connection(), &session_id, user_id(42))?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].group_type, "historical_transfer");
    assert_eq!(
        groups[0].signal_payload["planned_operation"],
        "merge_transfer_history"
    );
    assert!(groups[0]
        .members
        .iter()
        .any(|member| member.history_bill_id == Some(history_bill_id)));
    let materializations = get_import_history_materializations_by_session(
        db_runtime.connection(),
        &session_id,
        user_id(42),
    )?;
    assert_eq!(materializations.len(), 1);
    assert_eq!(materializations[0].history_bill_id, history_bill_id);
    assert_eq!(
        materializations[0].materialized_payload["operation"],
        "merge_transfer_history"
    );

    let select_all = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!(
                    "/api/bills/import/v2/preview/{session_id}/selection"
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "selectionAction": "select_all"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(select_all.status(), StatusCode::OK);

    let confirm = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm.status(), StatusCode::BAD_REQUEST);
    let confirm_body = read_json(confirm).await;
    assert!(confirm_body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("history rewrite acknowledgement"));
    let remaining_history_rows: i64 = db_runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = 42 AND id = ?1",
        [history_bill_id],
        |row| row.get(0),
    )?;
    assert_eq!(remaining_history_rows, 1);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_preview_query_and_confirm_preserve_server_paged_selection(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-preserve")?;
    let app = runtime_router(&fixture);
    let runtime = runtime_for(fixture.db_path())?;
    init_bills_schema(&runtime)?;
    let previews =
        get_preview_by_session(runtime.connection(), "session-preserve", user_id(42), false)?;
    assert_eq!(previews.len(), 2);
    assert!(previews.iter().all(|preview| !preview.preview_selected));

    let invalid_sort = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/v2/preview/session-preserve?page=1&page_size=1&sort_direction=sideways")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(invalid_sort.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_sort).await["error"],
        "Unsupported preview sort direction"
    );

    let select_all = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-preserve/selection")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "selectionAction": "select_all"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(select_all.status(), StatusCode::OK);
    let select_all_body = read_json(select_all).await;
    assert_eq!(select_all_body["success"], true);
    assert_eq!(select_all_body["data"]["metadata"]["counts"]["selected"], 2);

    let confirm = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": "session-preserve",
                        "preserve_unpatched_selection": true,
                        "preview_updates": [{
                            "id": previews[0].id,
                            "selected": false
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm.status(), StatusCode::OK);
    let confirm_body = read_json(confirm).await;
    assert_eq!(confirm_body["success"], true);
    assert_eq!(confirm_body["data"]["imported_count"], 1);

    let imported_descriptions = {
        let mut statement = runtime.connection().prepare(
            "SELECT description FROM bills WHERE user_id = ?1 ORDER BY date ASC, id ASC",
        )?;
        let rows = statement
            .query_map([42], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    assert_eq!(imported_descriptions, vec!["second preview row"]);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_handles_legacy_parse_upload_and_reclassify() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);
    let column_mapping = json!({
        "1": 0,
        "3": 1,
        "4": 4,
        "6": 3,
        "8": 2,
        "14": 5
    })
    .to_string();
    let transaction_type_mapping = json!({
        "支出": 3,
        "投资": 5
    })
    .to_string();
    let boundary = "rust-legacy-parse-import-boundary";
    let parse_body = multipart_body(
        boundary,
        &[
            ("fileType", "csv"),
            ("columnMapping", column_mapping.as_str()),
            ("transactionTypeMapping", transaction_type_mapping.as_str()),
            ("hasHeaderLine", "true"),
            ("delimiter", ","),
        ],
        &[(
            "file",
            "legacy-ledger.csv",
            "text/csv",
            "账单导出说明,,,,,\n统计周期,2026-05,,,,\n交易时间,交易类型,金额,账户,分类,备注\n2026-05-01 08:30:00,支出,12.34,支付宝,餐饮,早餐\n2026-05-02 10:00:00,投资,88.00,农业银行,投资理财,基金买入\n",
        )],
    );

    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/parse_import")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(parse_body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    assert_eq!(parse_body["success"], true);
    assert_eq!(parse_body["result"]["parserType"], "generic");
    assert_eq!(parse_body["result"]["totalCount"], 2);
    assert_eq!(parse_body["result"]["items"][0]["type"], 3);
    assert_eq!(parse_body["result"]["items"][0]["sourceAmount"], 1234);
    assert_eq!(
        parse_body["result"]["items"][0]["originalSourceAccountName"],
        "支付宝"
    );
    assert_eq!(parse_body["result"]["items"][1]["comment"], "基金买入");

    let upload_boundary = "rust-legacy-upload-boundary";
    let upload_body = multipart_body(
        upload_boundary,
        &[("parser_type", "auto"), ("preview_only", "true")],
        &[(
            "file",
            "wechat.csv",
            "text/csv",
            "交易时间,收支类型,金额,商品,支付方式\n2026-05-04 09:00:00,支出,18.50,早餐,微信\n",
        )],
    );
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/upload")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={upload_boundary}"),
                )
                .body(Body::from(upload_body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(upload.status(), StatusCode::OK);
    let upload_body = read_json(upload).await;
    assert_eq!(upload_body["success"], true);
    assert_eq!(upload_body["data"]["total"], 1);
    assert_eq!(upload_body["data"]["preview"][0]["sourceAmount"], 1850);

    let reclassify = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/reclassify")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "transactions": [{
                            "categoryName": "餐饮",
                            "sourceAccountId": "1001",
                            "originalSourceAccountName": "支付宝"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(reclassify.status(), StatusCode::OK);
    let reclassify_body = read_json(reclassify).await;
    assert_eq!(reclassify_body["success"], true);
    assert_eq!(reclassify_body["result"][0]["index"], 0);
    assert_eq!(reclassify_body["result"][0]["sourceAccountId"], "1001");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_persists_import_configs_and_suggests_matches(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let app = runtime_router(&fixture);

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/configs")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "支付宝 CSV",
                        "fileFormat": "csv",
                        "fieldMappings": {"1": 0, "3": 1, "8": 2, "6": 3, "14": 4},
                        "sampleHeaders": ["交易时间", "交易类型", "金额", "账户", "备注"],
                        "hasHeader": true,
                        "isDefault": true
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(save.status(), StatusCode::CREATED);
    let save_body = read_json(save).await;
    assert_eq!(save_body["success"], true);
    assert_eq!(save_body["result"]["id"], 1);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/configs?fileFormat=csv")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = read_json(list).await;
    assert_eq!(list_body["result"].as_array().expect("configs").len(), 1);
    assert_eq!(list_body["result"][0]["fieldMappings"]["8"], 2);

    let matched = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/configs/match")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "fileFormat": "csv",
                        "headers": ["交易时间", "交易类型", "金额", "账户", "备注"]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(matched.status(), StatusCode::OK);
    let matched_body = read_json(matched).await;
    assert_eq!(matched_body["result"]["id"], 1);
    assert!(
        matched_body["result"]["matchScore"]
            .as_f64()
            .unwrap_or_default()
            > 0.9
    );

    let suggested = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/configs/suggest")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "fileFormat": "csv",
                        "headers": ["交易时间", "交易类型", "金额", "账户", "备注"]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(suggested.status(), StatusCode::OK);
    let suggested_body = read_json(suggested).await;
    assert_eq!(suggested_body["result"]["fieldMappings"]["1"], 0);
    assert_eq!(suggested_body["result"]["fieldMappings"]["8"], 2);

    let description_suggestion = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/configs/suggest")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "fileFormat": "csv",
                        "headers": [
                            "说明",
                            "摘要",
                            "商品",
                            "商户",
                            "对方",
                            "remark",
                            "description",
                            "merchant",
                            "counterparty"
                        ]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(description_suggestion.status(), StatusCode::OK);
    let description_suggestion_body = read_json(description_suggestion).await;
    assert_eq!(
        description_suggestion_body["result"]["fieldMappings"]["14"],
        8
    );

    let signature_save = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/configs")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "CSV signature",
                        "fileFormat": "csv",
                        "fieldMappings": {"1": 0, "8": 1, "14": 2},
                        "headerSignature": "date|amount|merchant"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(signature_save.status(), StatusCode::CREATED);
    let signature_save_body = read_json(signature_save).await;
    assert_eq!(signature_save_body["result"]["id"], 2);
    let config_runtime = runtime_for(fixture.db_path())?;
    config_runtime.connection().execute(
        "UPDATE app_settings SET value = ?1 WHERE key = 'import_configs_user_42'",
        [json!([
            {
                "id": 1,
                "name": "支付宝 CSV",
                "fileFormat": "csv",
                "fieldMappings": {"1": 0, "3": 1, "8": 2, "6": 3, "14": 4},
                "sampleHeaders": ["交易时间", "交易类型", "金额", "账户", "备注"]
            },
            {
                "id": 2,
                "name": "CSV signature",
                "fileFormat": "csv",
                "fieldMappings": {"1": 0, "8": 1, "14": 2},
                "headerSignature": "date|amount|merchant"
            }
        ])
        .to_string()],
    )?;

    let signature_match = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/configs/match")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "fileFormat": "csv",
                        "headers": ["date", "amount", "merchant"]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(signature_match.status(), StatusCode::OK);
    let signature_match_body = read_json(signature_match).await;
    assert_eq!(signature_match_body["result"]["id"], 2);
    assert_eq!(signature_match_body["result"]["matchedHeaderCount"], 3);

    let deleted = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/bills/import/configs/1")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(deleted.status(), StatusCode::OK);
    let deleted_body = read_json(deleted).await;
    assert_eq!(deleted_body["result"], true);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_promotes_learning_rules_and_handles_crud() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let app = runtime_router(&fixture);
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("preview row");

    let promote = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/learning/session-a/promote")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "preview_updates": [{
                            "id": row.id,
                            "preview_type": "收入",
                            "annotated_category_id": 8,
                            "preview_source_account_id": 100
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(promote.status(), StatusCode::OK);
    let promote_body = read_json(promote).await;
    assert_eq!(promote_body["data"]["rules_total"], 1);
    assert_eq!(promote_body["data"]["created"], 1);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bills/import/learning-rules?page=1&pageSize=10")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = read_json(list).await;
    assert_eq!(list_body["totalCount"], 1);
    assert_eq!(list_body["result"][0]["matchType"], "composite");
    let rule_id = list_body["result"][0]["id"].as_i64().expect("rule id");

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/bills/import/learning-rules/{rule_id}"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(json!({"enabled": false}).to_string()))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(update.status(), StatusCode::OK);
    let update_body = read_json(update).await;
    assert_eq!(update_body["result"]["enabled"], false);

    let delete = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/bills/import/learning-rules/{rule_id}"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(delete.status(), StatusCode::OK);
    let delete_body = read_json(delete).await;
    assert_eq!(delete_body["result"], true);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_handles_legacy_confirm_direct() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    let app = runtime_router(&fixture);

    let confirm = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "bills": [{
                            "type": 3,
                            "sourceAmount": 1234,
                            "timeText": "2026-05-01 08:30:00",
                            "originalSourceAccountName": "支付宝",
                            "categoryName": "餐饮",
                            "comment": "早餐"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm.status(), StatusCode::OK);
    let confirm_body = read_json(confirm).await;
    assert_eq!(confirm_body["success"], true);
    assert_eq!(confirm_body["result"]["inserted"], 1);

    let bill_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(bill_count, 1);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_handles_legacy_confirm_session_batch_and_recurring_candidates(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("preview row");
    let app = runtime_router(&fixture);

    let candidates = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/bills/import/v2/preview-item/{}/recurring-candidates",
                    row.id
                ))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(candidates.status(), StatusCode::OK);
    let candidates_body = read_json(candidates).await;
    assert_eq!(candidates_body["data"]["previewId"], row.id);
    assert_eq!(candidates_body["data"]["provider_bypassed"], false);
    assert_eq!(candidates_body["data"]["candidate_count"], 0);

    let select_all = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/v2/preview/session-a/selection")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "selectionAction": "select_all"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(select_all.status(), StatusCode::OK);
    let select_all_body = read_json(select_all).await;
    assert_eq!(select_all_body["success"], true);
    assert_eq!(select_all_body["data"]["metadata"]["counts"]["selected"], 2);

    let confirm = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/confirm")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": "session-a"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(confirm.status(), StatusCode::OK);
    let confirm_body = read_json(confirm).await;
    assert_eq!(confirm_body["success"], true);
    assert_eq!(confirm_body["result"]["inserted"], 2);

    let bill_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(bill_count, 2);

    let boundary = "rust-import-batch-temp-boundary";
    let upload = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[(
            "files",
            "custom-ledger.csv",
            "text/csv",
            "when,kind,value,note\n2026-05-05 10:00:00,收款,88.20,奖金\n",
        )],
    );
    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(upload))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    let temp_path = parse_body["data"]["unmatched_files"][0]["temp_path"]
        .as_str()
        .expect("temp path");

    let batch = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/batch")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "file_path": temp_path
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(batch.status(), StatusCode::OK);
    let batch_body = read_json(batch).await;
    assert_eq!(batch_body["success"], true);
    assert_eq!(
        batch_body["result"]["runtime"],
        "rust-import-db-runtime-partial"
    );
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_global_learning_center_routes() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    seed_global_learning_corpus(&runtime)?;
    let app = runtime_router(&fixture);

    let generate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/learning/suggestions/generate")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("{}"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(generate_response.status(), StatusCode::OK);
    let generate_body = read_json(generate_response).await;
    assert_eq!(generate_body["success"], true);
    assert_eq!(generate_body["data"]["total_annotations"], 2);
    assert_eq!(generate_body["data"]["created"], 2);

    let suggestions_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/learning/suggestions?status=pending&limit=5")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(suggestions_response.status(), StatusCode::OK);
    let suggestions_body = read_json(suggestions_response).await;
    let suggestions = suggestions_body["data"]["items"]
        .as_array()
        .expect("suggestions");
    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0]["match_type"], "composite");
    let accept_id = suggestions[0]["id"].as_i64().expect("accept suggestion id");
    let reject_id = suggestions[1]["id"].as_i64().expect("reject suggestion id");

    let accept_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/learning/suggestions/{accept_id}/accept"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("{}"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(accept_response.status(), StatusCode::OK);
    let accept_body = read_json(accept_response).await;
    assert_eq!(accept_body["success"], true);
    assert_eq!(accept_body["data"]["status"], "accepted");
    let rule_id = accept_body["data"]["rule_id"].as_i64().expect("rule id");

    let reject_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/learning/suggestions/{reject_id}/reject"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("{}"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(reject_response.status(), StatusCode::OK);
    assert_eq!(read_json(reject_response).await["success"], true);

    let rules_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/learning/rules?limit=5")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(rules_response.status(), StatusCode::OK);
    let rules_body = read_json(rules_response).await;
    assert_eq!(rules_body["data"]["total"], 1);
    assert_eq!(rules_body["data"]["items"][0]["id"], rule_id);
    assert_eq!(rules_body["data"]["items"][0]["enabled"], true);

    let toggle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/learning/rules/{rule_id}/toggle"))
                .header("content-type", "application/json")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(r#"{"enabled":false}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(toggle_response.status(), StatusCode::OK);
    let toggle_body = read_json(toggle_response).await;
    assert_eq!(toggle_body["data"]["ruleId"], rule_id);
    assert_eq!(toggle_body["data"]["enabled"], false);

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/api/learning/rules/{rule_id}"))
                .header("content-type", "application/json")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(r#"{"learnedType":"支出","enabled":true}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["data"]["learned_type"], "支出");
    assert_eq!(update_body["data"]["enabled"], true);

    let empty_batch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/learning/suggestions/batch-accept")
                .header("content-type", "application/json")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(r#"{"suggestionIds":[]}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(empty_batch_response.status(), StatusCode::BAD_REQUEST);

    let invalid_batch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/learning/suggestions/batch-accept")
                .header("content-type", "application/json")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from(r#"{"suggestionIds":[999998,999999]}"#))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(invalid_batch_response.status(), StatusCode::OK);
    let invalid_batch_body = read_json(invalid_batch_response).await;
    assert_eq!(invalid_batch_body["data"]["acceptedCount"], 0);
    assert_eq!(invalid_batch_body["data"]["failedCount"], 2);

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(format!("/api/learning/rules/{rule_id}"))
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["success"], true);

    let missing_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/learning/suggestions/999999/accept")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .body(Body::from("{}"))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_accepts_string_encoded_generic_mappings() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    let runtime = runtime_for(fixture.db_path())?;
    seed_users(&runtime, &[42])?;
    let app = runtime_router(&fixture);

    let boundary = "rust-import-string-mapping-boundary";
    let body = multipart_body(
        boundary,
        &[("parser_type", "auto")],
        &[(
            "files",
            "string-mapping.csv",
            "text/csv",
            "when,kind,value,note,account\n2026-05-06 12:30:00,收款,18.50,退款,银行卡\n",
        )],
    );
    let parse = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse.status(), StatusCode::OK);
    let parse_body = read_json(parse).await;
    let session_id = parse_body["data"]["session_id"]
        .as_str()
        .expect("session id");
    let temp_path = parse_body["data"]["unmatched_files"][0]["temp_path"]
        .as_str()
        .expect("temp path");

    let parse_generic = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/parse_generic")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                        "parser_id": "generic",
                        "temp_path": temp_path,
                        "column_mapping": "{\"1\":0,\"3\":1,\"8\":2,\"14\":3,\"6\":4}",
                        "transaction_type_mapping": "{\"收款\":2}",
                        "has_header_line": "yes",
                        "delimiter": ","
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(parse_generic.status(), StatusCode::OK);
    let parse_generic_body = read_json(parse_generic).await;
    assert_eq!(parse_generic_body["data"]["parsed_count"], 1);
    let db_runtime = runtime_for(fixture.db_path())?;
    init_import_staging_schema(db_runtime.connection())?;
    let templates =
        get_parser_templates_by_session(db_runtime.connection(), session_id, user_id(42), None)?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].parser_type, "收入");
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_owns_learning_decision_llm_review_and_ocr_config_routes(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new().await?;
    seed_import_session(fixture.db_path(), "session-a")?;
    let app = runtime_router(&fixture);
    let row = preview_rows(fixture.db_path(), "session-a")?
        .into_iter()
        .next()
        .expect("preview row");

    let promote = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/v2/learning/session-a/promote")
                .header("x-user-id", "42")
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "preview_updates": [{
                            "id": row.id,
                            "preview_type": "收入",
                            "annotated_category_id": 8
                        }]
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("response");
    assert_eq!(promote.status(), StatusCode::OK);
    let promote_body = read_json(promote).await;
    assert_eq!(promote_body["success"], true);
    assert_eq!(promote_body["data"]["created"], 1);
    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
    upstream: String,
}

impl RuntimeFixture {
    async fn new() -> Result<Self, Box<dyn Error>> {
        Self::new_with_upstream(unavailable_upstream().await)
    }

    fn new_with_upstream(upstream: String) -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("import-runtime.db");
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            upstream,
        })
    }

    fn db_path(&self) -> &Path {
        &self.db_path
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        fixture.upstream.clone(),
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET)
    .with_auth_jwt_secret(TEST_AUTH_SECRET);
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn runtime_router_without_auth_secret(fixture: &RuntimeFixture) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        fixture.upstream.clone(),
        Duration::from_millis(200),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.display().to_string());
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn runtime_for(path: &Path) -> Result<SqliteRuntime, Box<dyn Error>> {
    let db_path = SqliteDbPath::temporary_file(path)?;
    Ok(SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: Duration::from_secs(1),
    })?)
}

fn seed_import_session(path: &Path, session_id: &str) -> Result<(), Box<dyn Error>> {
    let mut runtime = runtime_for(path)?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        session_id,
        user_id(42),
        &[
            preview_draft("2026-05-01", 12.5, "first preview row"),
            preview_draft("2026-05-02", 8.0, "second preview row"),
        ],
    )?;
    update_import_session_status(
        runtime.connection(),
        &ImportSessionStatusUpdate {
            session_id: session_id.to_string(),
            user_id: user_id(42),
            status: "preview".to_string(),
            total_parsed: Some(2),
            total_preview: Some(2),
            total_confirmed: None,
        },
    )?;
    Ok(())
}

fn seed_import_intelligence_tables(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            priority INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '',
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE IF NOT EXISTS category_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            category_id INTEGER NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 100,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            applied_count INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            type INTEGER DEFAULT 1,
            aliases TEXT,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS account_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            account_id INTEGER NOT NULL,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            priority INTEGER DEFAULT 100,
            account_role_scope TEXT DEFAULT 'any',
            transaction_type_scope TEXT DEFAULT 'all',
            field_scope TEXT DEFAULT '[\"counterparty\",\"payment_method\",\"description\"]'
        );
        CREATE TABLE IF NOT EXISTS import_learning_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            match_type TEXT NOT NULL,
            match_value TEXT NOT NULL,
            normalized_match_value TEXT NOT NULL,
            learned_type TEXT,
            learned_category_id INTEGER,
            learned_source_account_id INTEGER,
            learned_destination_account_id INTEGER,
            enabled INTEGER NOT NULL DEFAULT 1,
            parser_id TEXT,
            composite_match_hash TEXT,
            match_features_json TEXT,
            applied_count INTEGER NOT NULL DEFAULT 0,
            last_applied_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, match_type, normalized_match_value)
        );
        CREATE TABLE IF NOT EXISTS recurring_bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            template_id INTEGER,
            name TEXT NOT NULL,
            description TEXT,
            type TEXT NOT NULL,
            category TEXT,
            amount REAL NOT NULL,
            destination_amount REAL DEFAULT 0,
            account TEXT,
            counterparty TEXT,
            tag TEXT,
            comment TEXT,
            frequency TEXT,
            scheduled_frequency_type INTEGER DEFAULT 0,
            start_date TEXT,
            end_date TEXT,
            next_date TEXT,
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT ''
        );
        ",
    )?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
         VALUES (900, 42, 3, '餐饮', '咖啡', 10), (901, 42, 3, '生活', '超市', 20)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO category_rules(user_id, category_id, name, priority, rule_expression, regex_enabled, enabled)
         VALUES (42, 900, '咖啡规则', 10, 'OR={咖啡}', 0, 1)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO accounts(id, user_id, name, aliases)
         VALUES
         (1001, 42, '支付宝账户', '[\"alipay\",\"支付宝\",\"支付宝余额\"]'),
         (1002, 42, '微信账户', '[\"wechat\",\"微信\",\"微信支付\"]')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO account_rules(
             id, user_id, account_id, rule_expression, enabled, priority,
             account_role_scope, transaction_type_scope, field_scope
         ) VALUES
             (8001, 42, 1001, 'OR={alipay,支付宝余额}', 1, 1, 'source', 'expense', '[\"parser\",\"payment_method\"]')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO recurring_bills(
            id, user_id, name, type, amount, account, counterparty, frequency, start_date,
            next_date, enabled, created_at, updated_at
         ) VALUES (3001, 42, '咖啡月付', '支出', 2100, '1001', '', 'monthly', '2026-04-04', '2026-05-04', 1, '2026-05-01', '2026-05-01')",
        [],
    )?;

    let features = build_composite_match_features("wechat", "学习超市", "会员日采购", "微信支付")
        .expect("learning features");
    let composite_hash = composite_hash_from_features(&features);
    runtime.connection().execute(
        "INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, normalized_match_value, learned_type,
            learned_category_id, learned_source_account_id, learned_destination_account_id,
            enabled, parser_id, composite_match_hash, match_features_json, created_at, updated_at
         ) VALUES (7001, 42, 'composite', ?1, ?1, '支出', 901, 1002, NULL, 1, 'wechat', ?1, ?2, '2026-05-01', '2026-05-01')",
        [composite_hash, serde_json::to_string(&features)?],
    )?;
    Ok(())
}

fn seed_llm_runtime_tables(path: &Path) -> Result<(), Box<dyn Error>> {
    let runtime = runtime_for(path)?;
    seed_users(&runtime, &[42, 77])?;
    bill_analyser_db::init_llm_runtime_schema(runtime.connection())?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS category_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            category_id INTEGER NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 100,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS import_learning_concept_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            concept_key TEXT NOT NULL,
            concept_type TEXT NOT NULL,
            sample_count INTEGER NOT NULL DEFAULT 0,
            accepted_count INTEGER NOT NULL DEFAULT 0,
            rejected_count INTEGER NOT NULL DEFAULT 0,
            auto_applied_count INTEGER NOT NULL DEFAULT 0,
            rollback_count INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, concept_key, concept_type)
        );
        ",
    )?;
    runtime.connection().execute(
        "INSERT INTO categories(user_id, main_category, sub_category) VALUES (42, '餐饮', '咖啡')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO llm_candidates(
            id, user_id, type, source_bill_ids, suggested_main_category, suggested_sub_category,
            suggested_rule_expression, confidence, llm_provider, llm_model, llm_response_raw, status
         )
         VALUES (1, 42, 'rule_synthesis', '[1]', '餐饮', '咖啡', 'OR={咖啡}', 0.91, 'openai', 'gpt', '{}', 'pending')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO llm_candidates(
            id, user_id, type, source_bill_ids, suggested_main_category, suggested_sub_category,
            suggested_rule_expression, confidence, llm_provider, llm_model, llm_response_raw, status
         )
         VALUES (2, 42, 'classification', '[2]', '交通', '打车', '', 0.7, 'openai', 'gpt', '{}', 'pending')",
        [],
    )?;
    Ok(())
}

fn seed_llm_provider_generation_data(path: &Path) -> Result<(), Box<dyn Error>> {
    let runtime = runtime_for(path)?;
    seed_users(&runtime, &[42])?;
    bill_analyser_db::init_llm_runtime_schema(runtime.connection())?;
    init_bills_schema(&runtime)?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS category_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            category_id INTEGER NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 100,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS import_learning_concept_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            concept_key TEXT NOT NULL,
            concept_type TEXT NOT NULL,
            sample_count INTEGER NOT NULL DEFAULT 0,
            accepted_count INTEGER NOT NULL DEFAULT 0,
            rejected_count INTEGER NOT NULL DEFAULT 0,
            auto_applied_count INTEGER NOT NULL DEFAULT 0,
            rollback_count INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, concept_key, concept_type)
        );
        CREATE TABLE IF NOT EXISTS import_learning_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            match_type TEXT NOT NULL,
            match_value TEXT NOT NULL,
            normalized_match_value TEXT NOT NULL,
            learned_type TEXT,
            learned_category_id INTEGER,
            learned_source_account_id INTEGER,
            learned_destination_account_id INTEGER,
            enabled INTEGER NOT NULL DEFAULT 1,
            source_session_id TEXT,
            source_preview_id INTEGER,
            parser_id TEXT,
            composite_match_hash TEXT,
            match_features_json TEXT,
            applied_count INTEGER NOT NULL DEFAULT 0,
            last_applied_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, match_type, normalized_match_value)
        );
        ",
    )?;
    let category_id = seed_llm_provider_category_row(&runtime)?;
    runtime.connection().execute(
        "INSERT INTO accounts(user_id, name) VALUES (42, '现金')",
        [],
    )?;
    runtime.connection().execute(
        "
        INSERT INTO import_learning_rules(
            user_id, match_type, match_value, normalized_match_value, learned_type,
            learned_category_id, enabled, match_features_json, applied_count,
            created_at, updated_at
        )
        VALUES (
            42, 'merchant', 'cafe', 'cafe', '支出',
            ?1, 1, '{\"parser_id\":\"wechat\"}', 3,
            '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z'
        )
        ",
        [category_id],
    )?;
    runtime.connection().execute(
        "INSERT INTO import_learning_concept_stats(
            user_id, concept_key, concept_type, sample_count, accepted_count,
            rejected_count, auto_applied_count, rollback_count, updated_at
         )
         VALUES (42, 'rule:1', 'rule', 3, 2, 0, 1, 0, '2026-05-14T00:00:00Z')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO bills(
            user_id, date, type, amount, counterparty, description, payment_method,
            main_category, sub_category, hash, created_at, updated_at
         )
         VALUES (42, '2026-05-03', '支出', 18.5, 'cafe', 'latte', 'cash', '', '', 'llm-bill-1', '2026-05-03', '2026-05-03')",
        [],
    )?;
    Ok(())
}

fn seed_llm_provider_categories_only(path: &Path) -> Result<(), Box<dyn Error>> {
    let runtime = runtime_for(path)?;
    seed_users(&runtime, &[42])?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL
        );
        ",
    )?;
    let _ = seed_llm_provider_category_row(&runtime)?;
    Ok(())
}

fn seed_llm_provider_category_row(runtime: &SqliteRuntime) -> Result<i64, Box<dyn Error>> {
    runtime.connection().execute(
        "INSERT INTO categories(user_id, main_category, sub_category) VALUES (42, '餐饮', '咖啡')",
        [],
    )?;
    Ok(runtime.connection().last_insert_rowid())
}

fn preview_rows(path: &Path, session_id: &str) -> Result<Vec<ImportPreviewRow>, Box<dyn Error>> {
    let runtime = runtime_for(path)?;
    init_import_staging_schema(runtime.connection())?;
    Ok(get_preview_by_session(
        runtime.connection(),
        session_id,
        user_id(42),
        false,
    )?)
}

fn seed_users(runtime: &SqliteRuntime, user_ids: &[i64]) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "CREATE TABLE IF NOT EXISTS users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            email TEXT NOT NULL DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 1
        );",
    )?;
    for user_id in user_ids {
        runtime.connection().execute(
            "INSERT OR IGNORE INTO users(id, username, email, is_active) VALUES (?1, ?2, ?3, 1)",
            (
                user_id,
                format!("user-{user_id}"),
                format!("user-{user_id}@example.test"),
            ),
        )?;
    }
    Ok(())
}

fn seed_global_learning_corpus(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS import_learning_corpus_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            session_id TEXT NOT NULL,
            preview_id INTEGER NOT NULL,
            parser_id TEXT,
            counterparty TEXT,
            description TEXT,
            payment_method TEXT,
            composite_match_hash TEXT,
            match_features_json TEXT,
            annotated_type TEXT,
            annotated_category_id INTEGER,
            annotated_source_account_id INTEGER,
            annotated_destination_account_id INTEGER,
            source_snapshot_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, session_id, preview_id)
        );
        ",
    )?;
    for (preview_id, counterparty, description, category_id) in
        [(1001, "商户A", "早餐", 31), (1002, "商户B", "午餐", 32)]
    {
        let features = serde_json::json!({
            "parser_id": "wechat",
            "counterparty": counterparty,
            "description": description,
            "payment_method": "微信支付",
        });
        let composite_hash = format!("c={counterparty}|d={description}|m=微信支付|p=wechat");
        runtime.connection().execute(
            "
            INSERT INTO import_learning_corpus_samples (
                user_id, session_id, preview_id, parser_id, counterparty,
                description, payment_method, composite_match_hash, match_features_json,
                annotated_type, annotated_category_id, created_at, updated_at
            ) VALUES (42, ?1, ?2, 'wechat', ?3, ?4, '微信支付', ?5, ?6, '支出', ?7, ?8, ?8)
            ",
            (
                format!("session-{preview_id}"),
                preview_id,
                counterparty,
                description,
                composite_hash,
                features.to_string(),
                category_id,
                "2026-05-14T00:00:00Z",
            ),
        )?;
    }
    Ok(())
}

fn seed_auth_session(
    path: &Path,
    user_id: i64,
    token: &str,
    active_session: bool,
    active_user: bool,
) -> Result<(), Box<dyn Error>> {
    let runtime = runtime_for(path)?;
    seed_users(&runtime, &[user_id])?;
    runtime.connection().execute(
        "UPDATE users SET is_active = ?1 WHERE id = ?2",
        (if active_user { 1 } else { 0 }, user_id),
    )?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
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
        CREATE INDEX IF NOT EXISTS idx_sessions_token_hash ON sessions(token_hash);
        ",
    )?;
    let now = Local::now().naive_local();
    let expires_at = (now + ChronoDuration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string();
    let now_text = now.format("%Y-%m-%dT%H:%M:%S%.f").to_string();
    runtime.connection().execute(
        "
        INSERT INTO sessions (
            user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at
        ) VALUES (?1, ?2, '', ?3, ?3, 'test', '127.0.0.1', ?4, ?5, ?5)
        ",
        (
            user_id,
            format!("{:x}", Sha256::digest(token.as_bytes())),
            expires_at,
            if active_session { 1 } else { 0 },
            now_text,
        ),
    )?;
    Ok(())
}

fn test_access_token(user_id: i64, secret: &str, valid: bool) -> String {
    let now = Local::now();
    let exp = if valid {
        now + ChronoDuration::hours(1)
    } else {
        now - ChronoDuration::hours(1)
    };
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = json!({
        "user_id": user_id,
        "username": format!("user-{user_id}"),
        "type": "access",
        "iat": now.timestamp(),
        "exp": exp.timestamp(),
        "nonce": "test"
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

fn init_bills_schema(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bills (
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
            batch_id TEXT,
            hash TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash);
        ",
    )?;
    Ok(())
}

fn preview_draft(date: &str, amount: f64, description: &str) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: date.to_string(),
        preview_type: "支出".to_string(),
        preview_amount: amount,
        preview_destination_amount: 0.0,
        preview_main_category: "餐饮".to_string(),
        preview_sub_category: "午餐".to_string(),
        preview_counterparty: "canteen".to_string(),
        preview_payment_method: "card".to_string(),
        preview_description: description.to_string(),
        preview_parser_id: "wechat".to_string(),
        ..ImportPreviewDraft::default()
    }
}

fn user_id(value: u64) -> UserId {
    UserId::new(value).expect("positive test user id")
}

async fn unavailable_upstream() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    drop(listener);
    format!("http://{addr}")
}

async fn llm_openai_provider_upstream(
) -> Result<(String, tokio::task::JoinHandle<()>), Box<dyn Error>> {
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|axum::Json(payload): axum::Json<Value>| async move {
            let prompt = llm_openai_prompt(&payload);
            let content = llm_fake_content_for_prompt(&prompt);
            (
                StatusCode::OK,
                axum::Json(json!({
                    "choices": [{"message": {"content": content}}],
                    "usage": {"total_tokens": 7},
                })),
            )
        }),
    );
    let app = app
        .route(
            "/messages",
            post(|axum::Json(payload): axum::Json<Value>| async move {
                let prompt = llm_openai_prompt(&payload);
                let content = llm_fake_content_for_prompt(&prompt);
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "content": [{"type": "text", "text": content}],
                        "usage": {"input_tokens": 3, "output_tokens": 4},
                    })),
                )
            }),
        )
        .route(
            "/api/generate",
            post(|axum::Json(payload): axum::Json<Value>| async move {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let content = llm_fake_content_for_prompt(prompt);
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "response": content,
                        "done": true,
                    })),
                )
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("provider generation upstream");
    });
    Ok((format!("http://{addr}"), handle))
}

fn llm_openai_prompt(payload: &Value) -> String {
    payload
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.last())
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn llm_fake_content_for_prompt(prompt: &str) -> &'static str {
    if prompt.contains("KnowledgeSummaryPack") {
        r#"[{"rule_name":"无效分类","suggested_main_category":"不存在","suggested_sub_category":"咖啡","rule_expression":"OR={咖啡}","confidence":0.2,"reason":"invalid category"},{"rule_name":"无效语法","suggested_main_category":"餐饮","suggested_sub_category":"咖啡","rule_expression":"(OR={broken}","confidence":0.2,"reason":"invalid expression"},{"rule_name":"咖啡规则","suggested_main_category":"餐饮","suggested_sub_category":"咖啡","rule_expression":" OR={咖啡} ","confidence":0.88,"reason":"稳定咖啡证据"}]"#
    } else if prompt.contains("关键词匹配规则") || prompt.contains("已被归类") {
        r#"[{"rule_name":"午餐规则","rule_expression":"OR={canteen,lunch}","confidence":0.82,"explanation":"午餐样本稳定"}]"#
    } else if prompt.contains("preview_id=") {
        r#"[{"preview_id":1,"suggested_main_category":"餐饮","suggested_sub_category":"咖啡","suggested_source_account":"现金","suggested_destination_account":"","confidence":0.91,"reason":"商户和描述匹配咖啡"}]"#
    } else {
        r#"[{"bill_id":1,"suggested_main_category":"餐饮","suggested_sub_category":"咖啡","confidence":0.87}]"#
    }
}

fn sample_path(pattern: &str) -> String {
    pattern
        .replace("{session_id}", "sess-1")
        .replace("{preview_id}", "42")
        .replace("{config_id}", "7")
        .replace("{rule_id}", "9")
        .replace("{suggestion_id}", "11")
}

fn multipart_body(
    boundary: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &str, &str, &str)],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    for (name, filename, content_type, value) in files {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn multipart_body_bytes(
    boundary: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &str, &str, &[u8])],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    for (name, filename, content_type, value) in files {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn write_fake_tesseract_script(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    #[cfg(windows)]
    {
        let script = dir.join("fake-tesseract.cmd");
        fs::write(
            &script,
            "@echo off\r\ntype \"%BILL_ANALYSER_RUST_OCR_FIXTURE_TEXT%\"\r\n",
        )?;
        Ok(script)
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("fake-tesseract.sh");
        fs::write(
            &script,
            "#!/bin/sh\ncat \"$BILL_ANALYSER_RUST_OCR_FIXTURE_TEXT\"\n",
        )?;
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions)?;
        Ok(script)
    }
}

fn write_failing_tesseract_script(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    #[cfg(windows)]
    {
        let script = dir.join("failing-tesseract.cmd");
        fs::write(&script, "@echo off\r\necho boom 1>&2\r\nexit /b 3\r\n")?;
        Ok(script)
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("failing-tesseract.sh");
        fs::write(&script, "#!/bin/sh\necho boom >&2\nexit 3\n")?;
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions)?;
        Ok(script)
    }
}

fn write_silent_failing_tesseract_script(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    #[cfg(windows)]
    {
        let script = dir.join("silent-failing-tesseract.cmd");
        fs::write(&script, "@echo off\r\nexit /b 3\r\n")?;
        Ok(script)
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("silent-failing-tesseract.sh");
        fs::write(&script, "#!/bin/sh\nexit 3\n")?;
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions)?;
        Ok(script)
    }
}

fn write_slow_tesseract_script(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    #[cfg(windows)]
    {
        let script = dir.join("slow-tesseract.cmd");
        fs::write(&script, "@echo off\r\nping -n 3 127.0.0.1 > nul\r\n")?;
        Ok(script)
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("slow-tesseract.sh");
        fs::write(&script, "#!/bin/sh\nsleep 2\n")?;
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions)?;
        Ok(script)
    }
}

fn configure_fake_tesseract_command(script: &Path) {
    #[cfg(windows)]
    {
        env::set_var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND", "cmd");
        env::set_var(
            "BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND_ARGS",
            format!("/C;{}", script.display()),
        );
    }
    #[cfg(not(windows))]
    {
        env::set_var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND", "sh");
        env::set_var(
            "BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND_ARGS",
            script.display().to_string(),
        );
    }
}

fn write_fake_local_json_ocr_script(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    #[cfg(windows)]
    {
        let script = dir.join("fake-local-json-ocr.cmd");
        fs::write(
            &script,
            "@echo off\r\ntype \"%BILL_ANALYSER_RUST_OCR_LOCAL_JSON_FIXTURE%\"\r\n",
        )?;
        Ok(script)
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("fake-local-json-ocr.sh");
        fs::write(
            &script,
            "#!/bin/sh\ncat \"$BILL_ANALYSER_RUST_OCR_LOCAL_JSON_FIXTURE\"\n",
        )?;
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions)?;
        Ok(script)
    }
}

fn configure_fake_local_json_ocr_command(script: &Path) {
    #[cfg(windows)]
    {
        env::set_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND", "cmd");
        env::set_var(
            "BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND_ARGS",
            format!("/C;{}", script.display()),
        );
    }
    #[cfg(not(windows))]
    {
        env::set_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND", "sh");
        env::set_var(
            "BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND_ARGS",
            script.display().to_string(),
        );
    }
}

fn clear_fake_ocr_env() {
    env::remove_var("BILL_ANALYSER_RUST_OCR_FIXTURE_TEXT");
    env::remove_var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND");
    env::remove_var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND_ARGS");
    env::remove_var("BILL_ANALYSER_RUST_OCR_TIMEOUT_MS");
    env::remove_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_FIXTURE");
    env::remove_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND");
    env::remove_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND_ARGS");
    env::remove_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_INPUT_MODE");
    env::remove_var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_TIMEOUT_MS");
}

fn clear_fake_tesseract_env() {
    clear_fake_ocr_env();
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

async fn trusted_json_route(
    app: &Router,
    method: Method,
    uri: impl AsRef<str>,
    body: Option<String>,
    user_id: i64,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri.as_ref())
                .header("x-user-id", user_id.to_string())
                .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
                .header("content-type", "application/json")
                .body(body.map_or_else(Body::empty, Body::from))
                .expect("request builds"),
        )
        .await
        .expect("response")
}
