// 中文导读：导入 HTTP runtime 的结构性合同测试，用源码顺序锁定 stage2 编排。
// 维护重点：这些测试不替代行为测试；它们防止拆分 stage_handlers.rs 时把账户规则提前到语义投影之前。
// 不变式：stage2 必须先完成类型/分类、recurring、learning 投影，再最后运行 account_rules 并持久化 baseline。

use std::{
    fs,
    path::{Path, PathBuf},
};

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

#[test]
fn stage2_account_rules_run_after_semantic_projection_and_before_baseline() {
    let handlers = source("src/backend/http/import_routes/stage_handlers.rs");
    let stage2_loop = section_between(
        &handlers,
        "async fn apply_import_intelligence_chain",
        "fn apply_account_rule_match_after_semantic_projection",
    );

    let transfer_demotion = position(stage2_loop, "demote_unauthorized_transfer_preview");
    let category_projection = position(stage2_loop, "apply_transfer_category_rule_match");
    let recurring_projection = position(stage2_loop, "best_recurring_candidate_for_draft");
    let learning_projection = position(stage2_loop, "apply_learning_rule_match");
    let account_pre_snapshot = position(stage2_loop, "let before_account_rule =");
    let account_rule_pass = position(
        stage2_loop,
        "apply_account_rule_match_after_semantic_projection(draft, &account_rules, &accounts);",
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
fn ocr_llm_vision_runtime_validates_url_and_caps_payload_before_network_post() {
    let source = source("src/backend/http/import_routes/multipart_and_ocr.rs");
    let input_parser = section_between(
        &source,
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

    let network_provider = section_between(
        &source,
        "async fn run_network_llm_ocr",
        "fn llm_not_found_response",
    );
    let validate_base_url = position(network_provider, "validate_llm_vision_base_url(base_url)");
    let encode_payload = position(
        network_provider,
        "let encoded = general_purpose::STANDARD.encode(&image_bytes);",
    );
    let clamp_tokens = position(
        network_provider,
        "normalize_ocr_llm_max_tokens(&config.parameters)",
    );
    let post_request = position(network_provider, ".post(&url)");

    assert!(validate_base_url < encode_payload);
    assert!(encode_payload < post_request);
    assert!(clamp_tokens < post_request);
}
