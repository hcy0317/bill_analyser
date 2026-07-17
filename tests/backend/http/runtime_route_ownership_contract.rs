use std::{fs, path::PathBuf, process::Command, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use bill_analyser_core::{rust_http_shell_ownership_matrix, RuntimeState};
use bill_analyser_http::{
    build_router, validate_live_route_ownership, HttpAppState, HttpShellConfig, LiveRouteRecord,
    RouteOwnershipMismatch, FORBIDDEN_LEGACY_VERIFIED_ROUTES,
};
use serde::Deserialize;
use serde_json::Value;
use tower::ServiceExt;

const NEGATIVE_FIXTURE_ENV: &str = "BILL_ANALYSER_ROUTE_OWNERSHIP_NEGATIVE";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratedLiveRouteGraph {
    generated_from: Vec<String>,
    routes: Vec<LiveRouteRecord>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root resolves from http crate")
}

fn load_live_route_graph() -> GeneratedLiveRouteGraph {
    let path = repo_root().join("src/backend/http/live_route_graph.generated.json");
    let raw = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "read source-derived live route graph {}: {error}",
            path.display()
        )
    });
    serde_json::from_str(&raw).expect("source-derived live route graph parses")
}

fn assert_source_derived_graph_is_current() {
    let output = Command::new("node")
        .arg("scripts/check-runtime-route-ownership.mjs")
        .current_dir(repo_root())
        .output()
        .expect("Node.js must be available for the source-derived route graph gate");
    assert!(
        output.status.success(),
        "source-derived route graph checker failed (status={}):\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn live_ownership() -> Vec<LiveRouteRecord> {
    rust_http_shell_ownership_matrix()
        .iter()
        .filter(|route| {
            matches!(
                route.state,
                RuntimeState::RustOwnedVerified | RuntimeState::Retired
            )
        })
        .map(|route| LiveRouteRecord::new(route.method, route.pattern))
        .collect()
}

#[test]
fn source_derived_live_graph_matches_live_ownership_bidirectionally() {
    assert_source_derived_graph_is_current();
    let graph = load_live_route_graph();
    assert!(
        graph
            .generated_from
            .iter()
            .any(|path| path == "src/backend/http/router.rs"),
        "graph must be derived from the root assembled router"
    );
    let mut live = graph.routes;
    let mut owned = live_ownership();

    match std::env::var(NEGATIVE_FIXTURE_ENV).as_deref() {
        Ok("missing") => owned.push(LiveRouteRecord::new(
            "GET",
            "/api/__fixture_missing_from_live__",
        )),
        Ok("extra") => owned.retain(|route| {
            !(route.method == "GET" && route.pattern == "/api/bills/import/configs")
        }),
        Ok("legacy") => owned.push(LiveRouteRecord::new(
            "PUT",
            "/api/bills/import/learning-rules/{rule_id}",
        )),
        Ok(mode) => panic!("unsupported {NEGATIVE_FIXTURE_ENV}={mode}"),
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => panic!("read {NEGATIVE_FIXTURE_ENV}: {error}"),
    }

    live.sort();
    owned.sort();
    validate_live_route_ownership(&live, &owned, FORBIDDEN_LEGACY_VERIFIED_ROUTES)
        .unwrap_or_else(|mismatch| panic!("{mismatch}"));
}

#[test]
fn checker_rejects_missing_extra_and_legacy_fixtures() {
    let live = vec![
        LiveRouteRecord::new("GET", "/api/bills/import/configs"),
        LiveRouteRecord::new("PUT", "/api/learning/rules/{rule_id}"),
    ];
    let owned = live.clone();

    let mut missing = owned.clone();
    missing.push(LiveRouteRecord::new("GET", "/api/missing"));
    assert!(matches!(
        validate_live_route_ownership(&live, &missing, FORBIDDEN_LEGACY_VERIFIED_ROUTES),
        Err(RouteOwnershipMismatch { missing, .. }) if missing == [LiveRouteRecord::new("GET", "/api/missing")]
    ));

    let extra = vec![live[0].clone()];
    assert!(matches!(
        validate_live_route_ownership(&live, &extra, FORBIDDEN_LEGACY_VERIFIED_ROUTES),
        Err(RouteOwnershipMismatch { extra, .. }) if extra == [LiveRouteRecord::new("PUT", "/api/learning/rules/{rule_id}")]
    ));

    let mut legacy = owned;
    legacy.push(LiveRouteRecord::new(
        "PUT",
        "/api/bills/import/learning-rules/{rule_id}",
    ));
    assert!(matches!(
        validate_live_route_ownership(&live, &legacy, FORBIDDEN_LEGACY_VERIFIED_ROUTES),
        Err(RouteOwnershipMismatch { legacy, .. }) if legacy == [LiveRouteRecord::new("PUT", "/api/bills/import/learning-rules/{rule_id}")]
    ));

    let mismatch = validate_live_route_ownership(&live, &legacy, FORBIDDEN_LEGACY_VERIFIED_ROUTES)
        .expect_err("legacy ownership must remain a printable contract failure");
    assert_eq!(
        mismatch.to_string(),
        format!(
            "runtime route ownership mismatch: missing={:?}; extra=[]; legacy={:?}",
            [LiveRouteRecord::new(
                "PUT",
                "/api/bills/import/learning-rules/{rule_id}"
            )],
            [LiveRouteRecord::new(
                "PUT",
                "/api/bills/import/learning-rules/{rule_id}"
            )]
        )
    );
}

#[tokio::test]
async fn assembled_root_router_exposes_w2_routes_and_rejects_legacy_learning_put() {
    let state = HttpAppState::new(
        HttpShellConfig::new("", Duration::from_millis(100), 1024 * 1024)
            .expect("test HTTP config"),
    )
    .expect("test HTTP state");
    let app = build_router(state);

    for (method, path) in [
        (Method::GET, "/api/bills/import/configs?file_format=csv"),
        (Method::POST, "/api/bills/import/configs"),
        (Method::POST, "/api/bills/import/configs/match"),
        (Method::POST, "/api/bills/import/configs/suggest"),
        (Method::DELETE, "/api/bills/import/configs/1"),
        (Method::PUT, "/api/learning/rules/1"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("route contract request"),
            )
            .await
            .expect("route contract response");
        assert_ne!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert_ne!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{path}");
    }

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/bills/import/learning-rules/1")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("legacy route request"),
        )
        .await
        .expect("legacy route response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("legacy response body"),
    )
    .expect("legacy response JSON");
    assert_eq!(body["error"]["code"], "route_not_found");
}
