use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use bill_analyser_http::{bill_runtime_router, HttpAppState, HttpShellConfig};
use serde_json::{json, Value};
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root resolves from http crate")
}

fn rust_source_tree(root: &Path) -> String {
    let mut pending = vec![root.to_path_buf()];
    let mut source = String::new();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            let mut entries = fs::read_dir(&path)
                .unwrap_or_else(|error| panic!("read source directory {}: {error}", path.display()))
                .map(|entry| entry.expect("source directory entry").path())
                .collect::<Vec<_>>();
            entries.sort();
            pending.extend(entries.into_iter().rev());
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            source
                .push_str(&fs::read_to_string(&path).unwrap_or_else(|error| {
                    panic!("read source file {}: {error}", path.display())
                }));
            source.push('\n');
        }
    }
    source
}

fn bill_router() -> axum::Router {
    let state = HttpAppState::new(
        HttpShellConfig::new("", Duration::from_millis(100), 1024).expect("test HTTP config"),
    )
    .expect("test HTTP state");
    bill_runtime_router().with_state(state)
}

async fn post_json(path: &str, body: &'static str) -> (StatusCode, Value) {
    let response = bill_router()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("bill route request"),
        )
        .await
        .expect("bill route response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    let value = serde_json::from_slice(&body).expect("bill route JSON response");
    (status, value)
}

#[tokio::test]
async fn malformed_json_is_a_client_error_for_bill_body_helpers() {
    for path in [
        "/api/bills/category/quick-add-rule",
        "/api/bills/category/refresh",
    ] {
        let (status, body) = post_json(path, "{").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(
            body,
            json!({"success": false, "error": "Invalid JSON body"}),
            "{path} must not expose serde parser details"
        );
    }
}

#[tokio::test]
async fn valid_json_preserves_existing_bill_route_domain_errors() {
    let (status, body) = post_json("/api/bills/category/quick-add-rule", "{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({
            "success": false,
            "error": "Request body is required"
        })
    );

    let (status, body) = post_json("/api/bills/category/refresh", "{}").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["success"], false);
}

#[test]
fn bill_route_transport_response_owner_is_http_private() {
    let root = repo_root();
    let core = rust_source_tree(&root.join("src/backend/core"));
    let db = rust_source_tree(&root.join("src/backend/db"));
    let http = rust_source_tree(&root.join("src/backend/http/bill_routes"));
    let forbidden_core_transport = [
        "RouteResponseContract",
        "simple_route_error_response",
        "success_result_body",
        "error_result_body",
        "batch_create_success_route_response",
        "batch_create_prepare_error_route_response",
        "batch_create_persist_error_route_response",
        "delete_bill_success_payload",
        "batch_delete_success_payload",
        "reconciliation_success_response",
        "missing_reconciliation_parameters_response",
        "invalid_reconciliation_account_id_response",
        "reconciliation_account_not_found_response",
        "reconciliation_internal_error_response",
        "transaction_picture_upload_success_payload",
        "transaction_picture_upload_success_response",
        "missing_transaction_picture_file_response",
        "invalid_transaction_picture_file_response",
        "unsupported_transaction_picture_type_response",
        "missing_unused_transaction_picture_id_response",
        "remove_unused_transaction_picture_success_payload",
        "remove_unused_transaction_picture_success_response",
        "transaction_picture_internal_error_response",
    ];

    for symbol in forbidden_core_transport {
        assert!(
            !core.contains(symbol),
            "core must not own bill REST transport symbol {symbol}"
        );
        assert!(
            !db.contains(symbol),
            "DB must not own bill REST transport symbol {symbol}"
        );
    }
    assert!(
        !http.contains("route_contract_response"),
        "bill HTTP must project responses directly instead of retaining a second carrier adapter"
    );
    assert!(http.contains("fn success_result"));
    assert!(http.contains("fn error_response"));
    assert!(
        http.contains("BillInternalErrorKind"),
        "bill HTTP must own a closed internal-failure projection"
    );
    for forbidden_echo in [
        "picture_internal_error_response(error.to_string())",
        "reconciliation_internal_error_response(error.to_string())",
        "error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())",
    ] {
        assert!(
            !http.contains(forbidden_echo),
            "bill HTTP must not echo an internal Display value through {forbidden_echo}"
        );
    }
}
