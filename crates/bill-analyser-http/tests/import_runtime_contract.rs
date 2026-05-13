use std::{error::Error, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    routing::{get, post, put},
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_import_session, get_import_session, get_parser_templates_by_session,
    get_preview_by_session, init_import_staging_schema, insert_preview_bills_batch,
    update_import_session_status, ImportPreviewDraft, ImportPreviewRow, ImportSessionDraft,
    ImportSessionStatusUpdate, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState, IMPORT_SKELETON_ROUTE_PATTERNS,
};
use chrono::{Duration as ChronoDuration, Local};
use ring::hmac;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "rust-import-test-secret";

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
        "rust-http-shell:import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-recurring-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-export-runtime+auth-login-register-token-account-recovery-profile-cloud-external-auth-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime"
    );
    assert_eq!(
        runtime_body["business_migration"],
        "import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime-partial+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-export-runtime+auth-login-register-token-session-personal-refresh-logout-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime"
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
async fn import_db_runtime_intercepts_all_deletion_blocked_first_phase_routes_without_proxy(
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
            "{method} {pattern} fell through to the Python proxy"
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
    assert_eq!(preview_body["data"]["preview"].as_array().unwrap().len(), 1);
    assert_eq!(
        preview_body["data"]["preview"][0]["preview_description"],
        "first preview row"
    );

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
async fn import_db_runtime_proxies_receipt_ocr_recognition_to_python_sidecar(
) -> Result<(), Box<dyn Error>> {
    let (upstream, server) = receipt_ocr_upstream().await?;
    let fixture = RuntimeFixture::new_with_upstream(upstream)?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ml/receipt-recognition")
                .header("content-type", "application/octet-stream")
                .body(Body::from("receipt-image"))
                .expect("request builds"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["runtime"], "python-sidecar");
    server.abort();
    let _ = server.await;
    Ok(())
}

#[tokio::test]
async fn import_db_runtime_proxies_provider_generation_routes_to_python_sidecar(
) -> Result<(), Box<dyn Error>> {
    let (upstream, server) = provider_generation_upstream().await?;
    let fixture = RuntimeFixture::new_with_upstream(upstream)?;
    let app = runtime_router(&fixture);

    for (uri, route) in [
        ("/api/llm/preview-recommend", "llm-preview-recommend"),
        ("/api/llm/analyze-transactions", "llm-analyze-transactions"),
        ("/api/llm/rule-synthesis", "llm-rule-synthesis"),
        (
            "/api/learning/suggestions/generate",
            "learning-suggestions-generate",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("request builds"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["runtime"], "python-sidecar");
        assert_eq!(body["route"], route);
    }

    server.abort();
    let _ = server.await;
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
                        "session_id": session_id,
                        "include_preview": false
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
    let selected_preview = preview.first().expect("preview rows").id;

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
    let session = get_import_session(db_runtime.connection(), &session_id, user_id(42))?
        .expect("session remains");
    assert_eq!(session.total_confirmed, 1);
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
    assert_eq!(candidates_body["data"]["provider_bypassed"], true);

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
async fn import_db_runtime_proxies_global_learning_center_routes_to_python_sidecar(
) -> Result<(), Box<dyn Error>> {
    let (upstream, server) = provider_generation_upstream().await?;
    let fixture = RuntimeFixture::new_with_upstream(upstream)?;
    let app = runtime_router(&fixture);

    for (method, uri, route) in [
        (
            Method::GET,
            "/api/learning/suggestions?status=pending",
            "learning-suggestions-list",
        ),
        (
            Method::POST,
            "/api/learning/suggestions/11/accept",
            "learning-suggestion-accept",
        ),
        (
            Method::POST,
            "/api/learning/suggestions/12/reject",
            "learning-suggestion-reject",
        ),
        (
            Method::POST,
            "/api/learning/suggestions/batch-accept",
            "learning-suggestions-batch-accept",
        ),
        (
            Method::GET,
            "/api/learning/rules?limit=5",
            "learning-rules-list",
        ),
        (
            Method::PUT,
            "/api/learning/rules/7/toggle",
            "learning-rule-toggle",
        ),
        (Method::PUT, "/api/learning/rules/7", "learning-rule-update"),
        (
            Method::DELETE,
            "/api/learning/rules/7",
            "learning-rule-delete",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("request builds"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["runtime"], "python-sidecar");
        assert_eq!(body["route"], route);
    }

    server.abort();
    let _ = server.await;
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
    let state = ProxyState::new(config).expect("proxy state");
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
    let state = ProxyState::new(config).expect("proxy state");
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

async fn receipt_ocr_upstream() -> Result<(String, tokio::task::JoinHandle<()>), Box<dyn Error>> {
    let app = Router::new().route(
        "/api/ml/receipt-recognition",
        post(|| async {
            (
                StatusCode::OK,
                axum::Json(json!({"success": true, "runtime": "python-sidecar"})),
            )
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("receipt OCR upstream");
    });
    Ok((format!("http://{addr}"), handle))
}

async fn provider_generation_upstream(
) -> Result<(String, tokio::task::JoinHandle<()>), Box<dyn Error>> {
    let app = Router::new()
        .route(
            "/api/llm/preview-recommend",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "llm-preview-recommend",
                    })),
                )
            }),
        )
        .route(
            "/api/llm/analyze-transactions",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "llm-analyze-transactions",
                    })),
                )
            }),
        )
        .route(
            "/api/llm/rule-synthesis",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "llm-rule-synthesis",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/suggestions/generate",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-suggestions-generate",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/suggestions",
            get(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-suggestions-list",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/suggestions/:suggestion_id/accept",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-suggestion-accept",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/suggestions/batch-accept",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-suggestions-batch-accept",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/suggestions/:suggestion_id/reject",
            post(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-suggestion-reject",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/rules",
            get(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-rules-list",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/rules/:rule_id/toggle",
            put(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-rule-toggle",
                    })),
                )
            }),
        )
        .route(
            "/api/learning/rules/:rule_id",
            put(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-rule-update",
                    })),
                )
            })
            .delete(|| async {
                (
                    StatusCode::OK,
                    axum::Json(json!({
                        "success": true,
                        "runtime": "python-sidecar",
                        "route": "learning-rule-delete",
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

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}
