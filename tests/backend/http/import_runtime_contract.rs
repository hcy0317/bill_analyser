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
    let preparation = section_between(
        &handlers,
        "fn prepare_import_intelligence_snapshot",
        "fn finish_import_intelligence_snapshot",
    );
    let finish = section_between(
        &handlers,
        "fn finish_import_intelligence_snapshot",
        "fn apply_account_rule_match_after_semantic_projection",
    );
    let application = source("src/backend/http/import_routes/stage_handlers/stage2_application.rs");

    let transfer_demotion = position(preparation, "demote_unauthorized_transfer_preview");
    let category_projection = position(preparation, "apply_transfer_category_rule_match");
    let recurring_projection = position(preparation, "best_recurring_candidate_for_draft");
    let learning_preparation = position(preparation, "prepare_learning_rule_match");
    let prepare_call = position(&application, "prepare_import_intelligence_snapshot");
    let lifecycle_batch = position(&application, "load_import_stage2_learning_lifecycle_views");
    let finish_call = position(&application, "finish_import_intelligence_snapshot");
    let learning_projection = position(finish, "apply_prepared_learning_rule_match");
    let account_pre_snapshot = position(finish, "let before_account_rule =");
    let account_rule_pass = position(
        finish,
        "apply_account_rule_match_after_semantic_projection(",
    );
    let baseline_persist = position(finish, "persist_stage2_actionable_baseline(draft);");
    let stats_update = position(finish, "stats.account_matched += 1;");

    assert!(transfer_demotion < category_projection);
    assert!(preparation.contains("if is_transfer_protected_preview(draft)"));
    assert!(category_projection < recurring_projection);
    assert!(recurring_projection < learning_preparation);
    assert!(prepare_call < lifecycle_batch);
    assert!(lifecycle_batch < finish_call);
    assert!(learning_projection < account_pre_snapshot);
    assert!(account_pre_snapshot < account_rule_pass);
    assert!(account_rule_pass < baseline_persist);
    assert!(baseline_persist < stats_update);
    assert!(
        finish.contains("Account recognition is intentionally last in stage2"),
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
    assert_eq!(
        stage2_application
            .matches("load_import_stage2_learning_lifecycle_views(")
            .count(),
        1,
        "all matched drafts must share one lifecycle batch read"
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
    assert!(
        !stage2_chain.contains("get_import_learning_lifecycle_view"),
        "the per-draft stage2 loop must not perform lifecycle point reads"
    );
    let stage2_learning_rules =
        source("src/backend/http/import_routes/stage_handlers/stage2_learning_rules.rs");
    assert!(
        !stage2_learning_rules.contains("get_import_learning_lifecycle_view"),
        "learning projection must consume the batch snapshot"
    );
    let repository = source("src/backend/db/import_stage2.rs");
    assert!(repository.contains("pub async fn load_import_stage2_context"));
    assert!(repository.contains("WHERE user_id = $1"));
}

#[test]
fn vector_recall_projects_lifecycle_from_one_batch_repository_read() {
    let results = source("src/backend/http/import_routes/stage_vector_recall/results.rs");

    assert_eq!(
        results
            .matches("load_import_stage2_learning_lifecycle_views(")
            .count(),
        1,
        "all vector recall candidates must share one user-scoped lifecycle batch read"
    );
    assert!(
        !results.contains("get_import_learning_lifecycle_view"),
        "vector recall projection must not retain a per-candidate lifecycle point read"
    );
    assert!(
        results.contains("prepare_import_learning_vector_recall_hit"),
        "recommendation keys must be prepared before the lifecycle batch read"
    );
    assert!(
        results.contains("apply_prepared_import_learning_vector_recall_hit"),
        "prepared candidates must consume the immutable lifecycle snapshot"
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

#[test]
fn preview_row_version_is_mapped_serialized_and_enforced_by_single_update_cas() {
    let migration =
        source("src/backend/db/postgres/migrations/0001_initial_authoritative_schema.sql");
    let preview_table = section_between(
        &migration,
        "CREATE TABLE IF NOT EXISTS import_preview_rows",
        "COMMENT ON COLUMN import_preview_rows.amount_cents",
    );
    assert!(
        preview_table.contains("version BIGINT NOT NULL DEFAULT 1"),
        "preview rows need a persistent monotonic version for CAS"
    );

    let patch_helpers = source("src/backend/db/import_staging/patch_payload_helpers.rs");
    assert!(
        patch_helpers.contains("version = version + 1"),
        "preview mutations must keep advancing the stored version"
    );
    assert!(
        patch_helpers.contains("AND version ="),
        "single preview update SQL must enforce row-version CAS when a token is present"
    );

    let preview_types = source("src/backend/db/import_staging/types/preview_query.rs");
    let public_row = section_between(
        &preview_types,
        "pub struct ImportPreviewRow",
        "pub struct ImportPreviewFilterIndexRow",
    );
    assert!(public_row.contains("pub version:"));
    assert!(public_row.contains("rename = \"row_version\""));

    let row_mapping = source("src/backend/db/import_staging/row_mapping.rs");
    let preview_mapping = section_between(
        &row_mapping,
        "fn preview_from_pg_row",
        "fn import_decision_member_from_pg_row",
    );
    assert!(preview_mapping.contains("version: row.try_get(\"version\")?"));

    let mutation_handler =
        source("src/backend/http/import_routes/stage_handlers/preview_mutation_handlers.rs");
    assert!(
        mutation_handler.contains("expected_row_version_from_payload"),
        "the preview update route must parse a row CAS token"
    );
    let response_projection =
        source("src/backend/http/import_routes/preview_mutation_helpers/response_projection.rs");
    assert!(
        response_projection.contains("PREVIEW_ROW_VERSION_CONFLICT")
            && response_projection.contains("expected_row_version")
            && response_projection.contains("actual_row_version")
            && response_projection.contains("previewItem"),
        "row-version conflicts must return a typed 409 envelope with the latest row"
    );
    let preview_service = source("src/web/src/lib/services/importPreview.ts");
    assert!(preview_service.contains("expected_row_version: expectedRowVersion"));
    assert!(preview_service.contains("getImportPreviewRowVersionConflict"));
}

#[test]
fn session_version_is_exposed_and_consumed_by_current_confirm_clients() {
    let migration =
        source("src/backend/db/postgres/migrations/0001_initial_authoritative_schema.sql");
    let session_table = section_between(
        &migration,
        "CREATE TABLE IF NOT EXISTS import_sessions",
        "CREATE TABLE IF NOT EXISTS import_sources",
    );
    assert!(
        session_table.contains("version BIGINT NOT NULL DEFAULT 1"),
        "import sessions need a persistent version for confirm CAS"
    );

    let session_types = source("src/backend/db/import_staging/types/session.rs");
    let session_row = &session_types[position(&session_types, "pub struct ImportSessionRow")..];
    assert!(
        session_row.contains("pub version: i64"),
        "the repository session row must expose its stored version"
    );

    let response_types = source("src/backend/core/import_pipeline/response_types.rs");
    let session_summary =
        &response_types[position(&response_types, "pub struct ImportSessionSummary")..];
    assert!(
        session_summary.contains("pub session_version: i64"),
        "GET session must provide the authoritative confirm CAS token"
    );

    let confirm_handler =
        source("src/backend/http/import_routes/stage_handlers/confirm_handler.rs");
    assert!(confirm_handler.contains("expected_session_version"));
    assert!(confirm_handler.contains("expectedSessionVersion"));

    let service_facade = source("src/web/src/lib/services/importPreview.ts");
    assert!(service_facade.contains("importSessionServices"));
    let service = source("src/web/src/lib/services/importSession.ts");
    assert!(service.contains("getImportSession"));
    assert!(service.contains("expectedSessionVersion"));
    assert!(service.contains("payload['expected_session_version'] = expectedSessionVersion"));

    for client in [
        source("src/web/src/views/desktop/transactions/import/ImportDialog.vue"),
        source("src/web/src/views/mobile/transactions/ImportPreviewPage.vue"),
    ] {
        assert!(client.contains("services.getImportSession"));
        assert!(client.contains("expectedSessionVersion"));
    }
}

#[test]
fn weak_import_api_adapters_are_discoverable_and_cannot_grow() {
    let service = source("src/web/src/lib/services/importPreview.ts");
    let weak_record_count = service.matches("Record<string, unknown>").count();
    let any_count = service
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| *token == "any")
        .count();
    assert!(
        weak_record_count == 0,
        "migrated import service adapters must not use Record<string, unknown>"
    );
    assert!(
        any_count == 0,
        "migrated import service adapters must not use any"
    );

    let import_config_model = source("src/web/src/models/import_config.ts");
    for contract in [
        "export interface ImportConfigDto",
        "export interface ImportConfigMatchDto",
        "export interface ImportConfigSuggestion",
        "export interface ImportConfigSaveRequest",
        "export interface ImportFilePreviewData",
        "export interface ImportGenericParseData",
    ] {
        assert!(
            import_config_model.contains(contract),
            "neutral import config model must own {contract}"
        );
    }
    let import_dialog_types =
        source("src/web/src/views/desktop/transactions/import/import-dialog/types.ts");
    assert!(import_dialog_types.contains("@/models/import_config.ts"));

    let matching_model = source("src/web/src/models/bill_matching.ts");
    for contract in [
        "export interface MatchingCandidateActionResponse",
        "export interface MatchingSessionCandidatesResponse",
        "export interface MatchingSessionCandidateItem",
        "export interface ReconcileHistoryRequest",
    ] {
        assert!(
            matching_model.contains(contract),
            "neutral bill matching model must own {contract}"
        );
    }
    let check_data_types =
        source("src/web/src/views/desktop/transactions/import/checkDataTypes.ts");
    assert!(check_data_types.contains("@/models/bill_matching.ts"));
    let import_preview_model = source("src/web/src/models/import_preview.ts");
    for contract in [
        "export interface ImportPreviewExpectedState",
        "export interface ImportPreviewCandidateActionPayload",
        "export interface ImportTransferDecisionPayload",
        "export interface ImportPreviewDecisionResponse",
    ] {
        assert!(
            import_preview_model.contains(contract),
            "neutral import preview model must own {contract}"
        );
    }

    let dialog = source("src/web/src/views/desktop/transactions/import/ImportDialog.vue");
    let check_data = source(
        "src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue",
    );
    let direct_v2_count = dialog.matches("/api/bills/import/v2").count()
        + check_data.matches("/api/bills/import/v2").count();
    assert!(
        direct_v2_count > 0,
        "focused direct-adapter discovery must hit"
    );
    assert!(
        direct_v2_count <= 13,
        "direct import v2 adapters must not grow beyond the C4 baseline"
    );

    let preview_model = source("src/web/src/models/import_preview.ts");
    let preview_record = section_between(
        &preview_model,
        "export interface ImportPreviewRecord",
        "export interface ImportPreviewPageData",
    );
    assert!(preview_record.contains("row_version?: number"));

    let preview_service = source("src/web/src/lib/services/importPreview.ts");
    assert!(preview_service.contains("ApiResponsePromise<ImportPreviewPageData>"));
    assert!(preview_service.contains("previewItem?: ImportPreviewRecord"));
}
