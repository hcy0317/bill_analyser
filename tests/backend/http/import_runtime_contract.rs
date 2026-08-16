// 中文导读：导入 HTTP runtime 的结构性合同测试，用源码顺序锁定 stage2 编排。
// 维护重点：这些测试不替代行为测试；它们防止拆分 stage_handlers.rs 时把账户规则提前到语义投影之前。
// 不变式：stage2 必须先完成类型/分类、recurring、learning 投影，再最后运行 account_rules 并持久化 baseline。

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use bill_analyser_http::{import_routes::import_runtime_router, HttpAppState, HttpShellConfig};
use tower::ServiceExt;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root resolves from http crate")
}

fn source(path: &str) -> String {
    let file_path = repo_root().join(path);
    let raw = fs::read_to_string(&file_path).unwrap_or_else(|err| panic!("read {path}: {err}"));
    expand_rust_includes(&raw, file_path.parent().expect("source file has parent"))
}

fn expand_rust_includes(source: &str, base_dir: &Path) -> String {
    let mut expanded = String::new();
    for line in source.lines() {
        if let Some(include_path) = rust_include_path(line) {
            let nested_path = base_dir.join(include_path);
            let nested = fs::read_to_string(&nested_path)
                .unwrap_or_else(|err| panic!("read include {}: {err}", nested_path.display()));
            let nested_base = nested_path.parent().expect("include file has parent");
            expanded.push_str(&expand_rust_includes(&nested, nested_base));
        } else {
            expanded.push_str(line);
            expanded.push('\n');
        }
    }
    expanded
}

fn rust_include_path(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let path = trimmed.strip_prefix("include!(\"")?.strip_suffix("\");")?;
    Some(path)
}

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing section start {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing section end {end}"));
    &rest[..end_index]
}

fn position(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("missing marker {needle}"))
}

#[tokio::test]
async fn assembled_import_router_exposes_import_preview_config_and_learning_update_routes() {
    let state = HttpAppState::new(
        HttpShellConfig::new("", Duration::from_secs(1), 1024 * 1024).expect("test HTTP config"),
    )
    .expect("test HTTP state");
    let app = import_runtime_router().with_state(state);

    for (method, path, body, expected_status) in [
        (
            Method::POST,
            "/api/bills/import/preview",
            "",
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::GET,
            "/api/bills/import/configs?file_format=csv",
            "",
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::POST,
            "/api/bills/import/configs",
            "{}",
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/bills/import/configs/match",
            "{}",
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/bills/import/configs/suggest",
            "{}",
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::DELETE,
            "/api/bills/import/configs/1",
            "",
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::PUT,
            "/api/learning/rules/1",
            "{}",
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::POST,
            "/api/llm/configs/1/test",
            "",
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::PUT,
            "/api/bills/import/learning-rules/1",
            "{}",
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("route contract request"),
            )
            .await
            .expect("route contract response");
        assert_eq!(response.status(), expected_status, "{path}");
    }
}

#[test]
fn stage2_account_rules_run_after_semantic_projection_and_before_baseline() {
    let handlers = source("src/backend/http/import_routes/stage_handlers.rs");
    let stage2_loop = section_between(
        &handlers,
        "fn evaluate_import_intelligence_snapshot",
        "fn apply_account_rule_match_after_semantic_projection",
    );

    let transfer_demotion = position(stage2_loop, "demote_unauthorized_transfer_preview");
    let category_projection = position(stage2_loop, "apply_transfer_category_rule_match");
    let recurring_projection = position(stage2_loop, "best_recurring_candidate_for_draft");
    let learning_projection = position(stage2_loop, "apply_learning_rule_match");
    let account_pre_snapshot = position(stage2_loop, "let before_account_rule =");
    let account_rule_pass = position(
        stage2_loop,
        "apply_account_rule_match_after_semantic_projection(draft, account_rules, accounts);",
    );
    let baseline_persist = position(stage2_loop, "persist_stage2_actionable_baseline(draft);");
    let stats_update = position(stage2_loop, "stats.account_matched += 1;");

    assert!(transfer_demotion < category_projection);
    assert!(stage2_loop.contains("if is_transfer_protected_preview(draft)"));
    assert!(category_projection < recurring_projection);
    assert!(recurring_projection < learning_projection);
    assert!(learning_projection < account_pre_snapshot);
    assert!(account_pre_snapshot < account_rule_pass);
    assert!(account_rule_pass < baseline_persist);
    assert!(baseline_persist < stats_update);
    assert!(
        stage2_loop.contains("Account recognition is intentionally last in stage2"),
        "stage2 account-rule ordering comment should survive refactors"
    );
}

#[test]
fn stage2_production_paths_share_one_application_boundary_and_http_owns_no_sql() {
    let handlers = source("src/backend/http/import_routes/stage_handlers.rs");
    assert!(
        handlers.contains("struct ImportStage2"),
        "stage2 needs one named application boundary"
    );
    assert_eq!(
        handlers.matches("ImportStage2::evaluate(").count(),
        3,
        "initial, direct reclassify, and decision-group reclassify must enter the same boundary"
    );
    assert!(
        !handlers.contains("apply_import_intelligence_chain("),
        "production callers must not retain the legacy HTTP-owned stage2 entry"
    );

    let stage2_application =
        source("src/backend/http/import_routes/stage_handlers/stage2_application.rs");
    assert_eq!(
        stage2_application
            .matches("load_import_stage2_context(")
            .count(),
        1,
        "one context snapshot is loaded for each stage2 batch"
    );
    assert!(
        stage2_application.contains("ImportStage2ContextSnapshot"),
        "the loaded context must become an immutable batch snapshot"
    );

    let stage2_chain = source("src/backend/http/import_routes/stage_handlers/stage2_chain.rs");
    assert!(
        !stage2_chain.contains("sqlx::query"),
        "stage2 evaluation must not own PostgreSQL queries"
    );
    assert!(
        !stage2_chain.contains("SELECT "),
        "stage2 evaluation must remain transport and SQL agnostic"
    );
    let repository = source("src/backend/db/import_stage2.rs");
    assert!(repository.contains("pub async fn load_import_stage2_context"));
    assert!(repository.contains("WHERE user_id = $1"));
}

#[test]
fn stage2_materialization_dispatches_decision_groups_after_preview() {
    let handlers = source("src/backend/http/import_routes/stage_handlers.rs");
    let handler = section_between(
        &handlers,
        "pub async fn import_dedup_runtime_handler",
        "#[derive(Debug, Clone, Default)]",
    );
    let materialization = &handler[position(handler, "let _preview_insert_started_at")..];

    let clear_existing = position(
        materialization,
        "clear_import_preview_materialization_state",
    );
    let insert_preview = position(materialization, "insert_preview_bills_batch");
    let insert_history = position(
        materialization,
        "insert_import_history_materializations_batch",
    );
    let dispatch_groups = position(
        materialization,
        "spawn_import_decision_group_materialization",
    );
    let update_status = position(materialization, "update_import_session_status");
    let mark_templates_processed = position(
        materialization,
        "mark_unprocessed_parser_templates_processed_for_session",
    );

    assert!(clear_existing < insert_preview);
    assert!(insert_preview < insert_history);
    assert!(insert_history < dispatch_groups);
    assert!(dispatch_groups < update_status);
    assert!(update_status < mark_templates_processed);

    let background_materialization = section_between(
        &handlers,
        "fn spawn_import_decision_group_materialization",
        "#[derive(Debug, Clone, Default)]",
    );
    let load_preview_for_groups =
        position(background_materialization, "let preview_rows_for_groups =");
    let build_groups = position(
        background_materialization,
        "build_import_match_decision_groups",
    );
    let insert_groups = position(
        background_materialization,
        "insert_import_decision_groups_batch",
    );

    assert!(load_preview_for_groups < build_groups);
    assert!(build_groups < insert_groups);
}

#[test]
fn matching_candidate_action_is_not_an_unconditional_conflict() {
    let payloads = source("src/backend/http/matching_routes/payloads.rs");
    let response = section_between(
        &payloads,
        "fn matching_candidate_action_response",
        "fn preview_action_request_from_payload",
    );

    assert!(
        !response.contains("PostgreSQL matching candidate actions require a materialized candidate"),
        "matching candidate action must not be an unconditional PostgreSQL materialization conflict"
    );
}

#[test]
fn ocr_llm_vision_runtime_validates_url_and_caps_payload_before_network_post() {
    let multipart_source = source("src/backend/http/import_routes/multipart_and_ocr.rs");
    let input_parser = section_between(
        &multipart_source,
        "fn ocr_recognition_input_from_request",
        "fn ocr_rate_limit_try_acquire",
    );
    assert!(
        input_parser.contains("validate_ocr_image_size(&image.0)?"),
        "multipart OCR image bytes are size-checked before provider execution"
    );
    assert!(
        input_parser.contains("validate_ocr_image_size(body)?"),
        "raw OCR image bytes are size-checked before provider execution"
    );

    let network_provider =
        source("src/backend/http/import_routes/multipart_and_ocr/network_llm_ocr.rs");
    let validate_base_url = position(&network_provider, "validate_llm_vision_base_url(base_url)");
    let encode_payload = position(
        &network_provider,
        "let encoded = general_purpose::STANDARD.encode(&image_bytes);",
    );
    let clamp_tokens = position(
        &network_provider,
        "normalize_ocr_llm_max_tokens(&config.parameters)",
    );
    let post_request = position(&network_provider, ".post(&url)");

    assert!(validate_base_url < encode_payload);
    assert!(encode_payload < post_request);
    assert!(clamp_tokens < post_request);
}
