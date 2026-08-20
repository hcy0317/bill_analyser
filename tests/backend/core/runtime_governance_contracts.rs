use bill_analyser_core::{
    bills_crud_db_writer_policy, budgets_crud_db_writer_policy, can_delete_retired_import_paths,
    database_schema_db_writer_policy, domain_governance_policies, endpoints_by_owner,
    expanded_route_manifest, find_domain_policy, find_endpoint_ownership,
    governance_manifest_snapshot, import_db_writer_policy, import_deletion_blocked_endpoints,
    import_deletion_gates, manifest_states, missing_import_deletion_gates,
    response_envelope_policies, response_envelope_policy, runtime_state_machine,
    rust_http_shell_ownership_matrix, DbWriterMode, DecisionRequired, ImportDeletionEvidence,
    ImportDeletionGate, ResponseEnvelopeFamily, RouteHandlerId, RuntimeBlockedStatus, RuntimeState,
};
use serde::Deserialize;
use std::{collections::HashSet, fs, path::Path};

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct FrontendRouteOwnershipManifest {
    generated_from: String,
    routes: Vec<FrontendRouteOwnership>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct FrontendRouteOwnership {
    method: String,
    pattern: String,
    domain: String,
    state: RuntimeState,
}

#[test]
fn gitea_ci_path_filters_cover_backend_contract_tests() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let workflow = fs::read_to_string(repo_root.join(".gitea/workflows/ci.yml"))
        .expect("Gitea CI workflow is readable");

    assert_eq!(
        workflow.matches("- 'tests/backend/**'").count(),
        2,
        "backend contract tests must trigger CI for both push and pull_request"
    );
    assert_eq!(
        workflow.matches("- 'tests/web/**'").count(),
        2,
        "frontend contract tests must trigger CI for both push and pull_request"
    );
}

#[test]
fn gitea_ci_provisions_isolated_postgres_without_manual_docker_commands() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let workflow = fs::read_to_string(repo_root.join(".gitea/workflows/ci.yml"))
        .expect("Gitea CI workflow is readable");
    let backend_start = workflow
        .find("  backend-ci:")
        .expect("Gitea CI keeps the backend-ci job");
    let frontend_start = workflow
        .find("  frontend-ci:")
        .expect("Gitea CI keeps the frontend-ci job after backend-ci");
    let backend_workflow = &workflow[backend_start..frontend_start];

    for required in [
        "services:",
        "postgres:",
        "image: postgres:16",
        "BILL_ANALYSER_TEST_POSTGRES_URL:",
        "@postgres:5432/bill_analyser_test",
        "--health-cmd",
    ] {
        assert!(
            backend_workflow.contains(required),
            "Gitea backend CI must provision its isolated PostgreSQL test dependency: {required}"
        );
    }

    for forbidden in [
        "ports:",
        "/dev/tcp",
        "docker ",
        "docker-compose",
        "privileged:",
    ] {
        assert!(
            !backend_workflow.contains(forbidden),
            "Gitea CI must use the managed service network instead of privileged or port-bound infrastructure: {forbidden}"
        );
    }
}

#[test]
fn account_transaction_mutations_keep_sql_and_transactions_out_of_http_handlers() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let handler_facade = repo_root.join("src/backend/http/taxonomy_routes/account_handlers.rs");
    let handler_dir = repo_root.join("src/backend/http/taxonomy_routes/account_handlers");
    let audit_helpers =
        repo_root.join("src/backend/http/taxonomy_routes/audit_and_rules_helpers.rs");
    let mut handler_sources =
        vec![fs::read_to_string(handler_facade).expect("account handler facade is readable")];
    for entry in fs::read_dir(handler_dir).expect("account handler directory is readable") {
        let path = entry.expect("account handler entry is readable").path();
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            handler_sources.push(
                fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display())),
            );
        }
    }
    let handler_source = handler_sources.join("\n");
    for forbidden in [
        "sqlx::",
        ".begin().await",
        "UPDATE bills",
        "DELETE FROM bill_tags",
    ] {
        assert!(
            !handler_source.contains(forbidden),
            "account HTTP handlers must delegate PostgreSQL ownership to the DB repository: {forbidden}"
        );
    }

    let audit_helper_source =
        fs::read_to_string(audit_helpers).expect("account audit helper is readable");
    for forbidden in [
        "sqlx::",
        "SELECT password_hash FROM users",
        "SELECT value FROM settings",
        "INSERT INTO business_audit_events",
    ] {
        assert!(
            !audit_helper_source.contains(forbidden),
            "account security and audit SQL must be owned by DB repositories: {forbidden}"
        );
    }

    let repository = fs::read_to_string(
        repo_root.join("src/backend/db/taxonomy/postgres_reads/account_transactions.rs"),
    )
    .expect("account transaction repository is readable");
    for required in [
        "pub async fn move_all_postgres_account_transactions",
        "pub async fn clear_postgres_account_transactions",
        "pub async fn create_postgres_account_audit_event",
    ] {
        assert!(
            repository.contains(required),
            "DB repository keeps the concrete account transaction mutation entry: {required}"
        );
    }
}

#[test]
fn governance_tests_verified_paths_exist() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");

    for policy in domain_governance_policies() {
        let mut seen = HashSet::new();
        for &path in policy.tests_verified {
            assert!(
                path.starts_with("tests/"),
                "domain {} references non-test tests_verified path {}",
                policy.domain,
                path
            );
            assert!(
                seen.insert(path),
                "domain {} duplicates tests_verified path {}",
                policy.domain,
                path
            );
            assert!(
                repo_root.join(path).exists(),
                "domain {} references missing tests_verified path {}",
                policy.domain,
                path
            );
        }
    }
}

#[test]
fn rust_owned_verified_runtime_routes_include_current_rest_runtime() {
    let rust_owned: Vec<_> = endpoints_by_owner(RuntimeState::RustOwnedVerified)
        .into_iter()
        .filter(|endpoint| endpoint.pattern.starts_with("/api/"))
        .map(|endpoint| (endpoint.method, endpoint.pattern))
        .collect();

    for (method, pattern) in [
        ("GET", "/api/health"),
        ("GET", "/api/runtime"),
        ("GET", "/api/health/live"),
        ("GET", "/api/health/ready"),
        ("GET", "/api/bills"),
        ("POST", "/api/bills/batch"),
        ("POST", "/api/bills/import/v2/parse"),
        ("GET", "/api/budgets/"),
        ("POST", "/api/budgets/import"),
        ("GET", "/api/statistics/category-statistics"),
        ("GET", "/api/statistics/exchange-rates"),
        ("POST", "/api/auth/login"),
        ("POST", "/api/data/clear/all"),
        ("GET", "/api/accounts"),
        ("GET", "/api/accounts/"),
        ("POST", "/api/accounts/sync-balances"),
        ("GET", "/api/category-rules/"),
        ("POST", "/api/category-rules/reorder"),
        ("GET", "/api/account-rules/"),
        ("GET", "/api/settings/bundle/export"),
        ("GET", "/api/tags"),
        ("GET", "/api/tags/"),
        ("GET", "/api/templates"),
        ("GET", "/api/templates/"),
        ("GET", "/api/matching/candidates"),
        ("GET", "/api/recurring/suggestions"),
        ("GET", "/api/calendar/events"),
        ("GET", "/api/networth/snapshot"),
        ("POST", "/api/llm/preview-recommend"),
        ("POST", "/api/ml/receipt-recognition"),
        ("GET", "/api/learning/rules"),
    ] {
        assert!(
            rust_owned.contains(&(method, pattern)),
            "current runtime route must be RustOwnedVerified: {method} {pattern}"
        );
    }

    let retired_routes: Vec<_> = endpoints_by_owner(RuntimeState::Retired)
        .into_iter()
        .map(|endpoint| (endpoint.method, endpoint.pattern))
        .collect();
    assert_eq!(
        retired_routes,
        vec![
            ("GET", "/api/matching/investment-settings"),
            ("PUT", "/api/matching/investment-settings"),
        ]
    );

    let matrix = rust_http_shell_ownership_matrix();
    assert!(matrix
        .iter()
        .any(|endpoint| endpoint.pattern == "/api/runtime"
            && endpoint.envelope == ResponseEnvelopeFamily::RustHttpShell));
    assert!(matrix
        .iter()
        .any(|endpoint| endpoint.pattern == "/api/bills"
            && endpoint.envelope == ResponseEnvelopeFamily::BillsCrud));
    assert!(matrix
        .iter()
        .any(|endpoint| endpoint.pattern == "/api/budgets/"
            && endpoint.envelope == ResponseEnvelopeFamily::BudgetsCrud));
    assert!(matrix
        .iter()
        .any(|endpoint| endpoint.pattern == "/api/statistics/amounts"
            && endpoint.envelope == ResponseEnvelopeFamily::StatisticsRead));
    assert!(matrix.iter().any(
        |endpoint| endpoint.pattern == "/api/statistics/exchange-rates"
            && endpoint.envelope == ResponseEnvelopeFamily::CurrentSuccessResult
    ));
}
#[test]
fn bills_import_and_provider_routes_are_rust_owned() {
    let blocked_routes = import_deletion_blocked_endpoints();
    assert!(blocked_routes.is_empty());

    for (method, pattern) in [
        ("POST", "/api/bills/import/v2/parse"),
        ("PUT", "/api/bills/import/v2/preview/{session_id}/selection"),
        ("PUT", "/api/bills/import/v2/preview/{session_id}/update"),
        (
            "POST",
            "/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
        ),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing endpoint ownership for {method} {pattern}"));
        assert_eq!(endpoint.state, RuntimeState::RustOwnedVerified);
        assert!(!endpoint.has_external_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }

    for (method, pattern) in [
        ("POST", "/api/llm/preview-recommend/accept"),
        ("GET", "/api/ml/receipt-recognition/config"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing adjacent Rust endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, RuntimeState::RustOwnedVerified);
        assert!(!endpoint.has_external_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }

    for (method, pattern) in [
        ("GET", "/api/bills"),
        ("GET", "/api/bills/"),
        ("POST", "/api/bills"),
        ("POST", "/api/bills/"),
        ("GET", "/api/bills/by-month"),
        ("GET", "/api/bills/get"),
        ("GET", "/api/bills/{bill_id}"),
        ("PUT", "/api/bills/{bill_id}"),
        ("DELETE", "/api/bills/{bill_id}"),
        ("POST", "/api/bills/batch"),
        ("PUT", "/api/bills/batch/update"),
        ("DELETE", "/api/bills/batch/delete"),
        ("GET", "/api/bills/export"),
        ("POST", "/api/bills/pictures"),
        ("POST", "/api/bills/pictures/unused"),
        ("GET", "/api/bills/{bill_id}/recurring-candidates"),
        ("PUT", "/api/bills/{bill_id}/recurring-match"),
        ("DELETE", "/api/bills/{bill_id}/recurring-match"),
        ("POST", "/api/bills/category/quick-add-rule"),
        ("POST", "/api/bills/category/refresh"),
        ("GET", "/api/bills/reconciliation_statements"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing Rust-owned bills endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, RuntimeState::RustOwnedVerified);
        assert!(!endpoint.has_external_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
        assert!(endpoint.notes.contains(".py route shell"));
    }

    let receipt_recognition = find_endpoint_ownership("POST", "/api/ml/receipt-recognition")
        .expect("receipt OCR recognition route is governed");
    assert_eq!(receipt_recognition.state, RuntimeState::RustOwnedVerified);
    assert!(!receipt_recognition.has_external_runtime_owner());
    assert!(!receipt_recognition.is_import_deletion_blocked());

    for (method, pattern) in [
        ("POST", "/api/llm/preview-recommend"),
        ("POST", "/api/llm/analyze-transactions"),
        ("POST", "/api/llm/rule-synthesis"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing provider-owned endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, RuntimeState::RustOwnedVerified);
        assert!(!endpoint.has_external_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }

    for (method, pattern) in [
        ("GET", "/api/llm/config"),
        ("POST", "/api/llm/config"),
        ("GET", "/api/llm/configs"),
        ("POST", "/api/llm/configs"),
        ("PUT", "/api/llm/configs/{config_id}"),
        ("DELETE", "/api/llm/configs/{config_id}"),
        ("POST", "/api/llm/configs/{config_id}/activate"),
        ("GET", "/api/llm/candidates"),
        ("GET", "/api/llm/candidates/{candidate_id}"),
        ("POST", "/api/llm/candidates/{candidate_id}/accept"),
        ("POST", "/api/llm/candidates/{candidate_id}/reject"),
        ("GET", "/api/learning/suggestions"),
        ("POST", "/api/learning/suggestions/generate"),
        ("POST", "/api/learning/suggestions/{suggestion_id}/accept"),
        ("POST", "/api/learning/suggestions/batch-accept"),
        ("POST", "/api/learning/suggestions/{suggestion_id}/reject"),
        ("GET", "/api/learning/rules"),
        ("PUT", "/api/learning/rules/{rule_id}/toggle"),
        ("PUT", "/api/learning/rules/{rule_id}"),
        ("DELETE", "/api/learning/rules/{rule_id}"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing Rust-owned learning endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, RuntimeState::RustOwnedVerified);
        assert!(!endpoint.has_external_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }

    for (method, pattern) in [
        ("GET", "/api/statistics/overview"),
        ("GET", "/api/statistics/trends"),
        ("GET", "/api/statistics/comparison"),
        ("GET", "/api/statistics/category"),
        ("GET", "/api/statistics/trend"),
        ("GET", "/api/insights/anomalies"),
        ("GET", "/api/statistics/exchange-rates"),
        ("PUT", "/api/statistics/exchange-rates/custom"),
        ("DELETE", "/api/statistics/exchange-rates/custom/{currency}"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing Rust-owned exchange endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, RuntimeState::RustOwnedVerified);
        assert!(!endpoint.has_external_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
        assert!(endpoint.notes.contains("route shell is deleted"));
    }
}

#[test]
fn removed_llm_induce_rules_route_is_not_manifested_in_current_runtime() {
    assert!(find_endpoint_ownership("POST", "/api/llm/induce-rules").is_none());
}

#[test]
fn backup_ops_routes_are_rust_owned_in_current_file_runtime() {
    let manifest = expanded_route_manifest();
    for endpoint in [
        "GET /api/backup/",
        "POST /api/backup/create",
        "GET /api/backup/download/{filename}",
        "DELETE /api/backup/delete/{filename}",
        "POST /api/backup/cleanup",
        "GET /api/backup/jobs",
        "POST /api/backup/jobs",
        "POST /api/backup/sync",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("backup ops route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::BackupOpsRuntime);
        if endpoint == "GET /api/backup/download/{filename}" {
            assert_eq!(entry.envelope, ResponseEnvelopeFamily::RawPassthrough);
        } else {
            assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessData);
        }
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }
}
#[test]
fn taxonomy_master_data_routes_are_rust_owned_with_no_p4_config_remainder() {
    let manifest = expanded_route_manifest();
    for endpoint in [
        "GET /api/accounts",
        "POST /api/accounts",
        "GET /api/accounts/",
        "POST /api/accounts/",
        "GET /api/accounts/{account_id}",
        "PUT /api/accounts/{account_id}",
        "DELETE /api/accounts/{account_id}",
        "PUT /api/accounts/display-orders",
        "POST /api/accounts/sync-balances",
        "POST /api/accounts/{account_id}/transactions/clear",
        "POST /api/accounts/{account_id}/transactions/move",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy account route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessResult);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    for endpoint in [
        "GET /api/tags",
        "POST /api/tags",
        "GET /api/tags/",
        "POST /api/tags/",
        "GET /api/tags/{tag_id}",
        "PUT /api/tags/{tag_id}",
        "DELETE /api/tags/{tag_id}",
        "POST /api/tags/batch",
        "PUT /api/tags/display-orders",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy tag route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessResult);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    for endpoint in [
        "GET /api/categories",
        "POST /api/categories",
        "GET /api/categories/",
        "POST /api/categories/",
        "GET /api/categories/{category_id}",
        "PUT /api/categories/{category_id}",
        "DELETE /api/categories/{category_id}",
        "GET /api/categories/tree",
        "GET /api/categories/flat",
        "GET /api/categories/all",
        "PUT /api/categories/all",
        "POST /api/categories/batch",
        "POST /api/categories/move",
        "GET /api/categories/export",
        "POST /api/categories/import",
        "GET /api/categories/statistics",
        "POST /api/categories/update-all",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy category route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessResult);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    for endpoint in [
        "GET /api/templates",
        "POST /api/templates",
        "GET /api/templates/",
        "POST /api/templates/",
        "GET /api/templates/{template_id}",
        "PUT /api/templates/{template_id}",
        "DELETE /api/templates/{template_id}",
        "PUT /api/templates/display-orders",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy template route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessResult);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    for endpoint in [
        "GET /api/settings/bundle/export",
        "POST /api/settings/bundle/import",
        "POST /api/settings/bundle/import/preview",
        "GET /api/settings/bundle/sections/{section_key}/export",
        "POST /api/settings/bundle/sections/{section_key}/export",
        "POST /api/settings/bundle/sections/{section_key}/import",
        "POST /api/settings/bundle/sections/{section_key}/import/preview",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy settings route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert!(matches!(
            entry.envelope,
            ResponseEnvelopeFamily::RawPassthrough
                | ResponseEnvelopeFamily::CurrentSuccessData
                | ResponseEnvelopeFamily::CurrentSuccessResult
        ));
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    let rules_overview = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/rules/overview")
        .expect("taxonomy rules overview route is present");
    assert_eq!(rules_overview.state, RuntimeState::RustOwnedVerified);
    assert_eq!(rules_overview.handler, RouteHandlerId::TaxonomyRuntime);
    assert_eq!(
        rules_overview.envelope,
        ResponseEnvelopeFamily::CurrentSuccessData
    );
    assert!(rules_overview.deletion_blockers.is_empty());
    assert_eq!(rules_overview.decision_required, DecisionRequired::None);

    for endpoint in [
        "GET /api/category-rules/",
        "POST /api/category-rules/",
        "PUT /api/category-rules/{rule_id}",
        "DELETE /api/category-rules/{rule_id}",
        "POST /api/category-rules/{rule_id}/test",
        "POST /api/category-rules/defaults",
        "POST /api/category-rules/reorder",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy category-rules route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessData);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    for endpoint in [
        "GET /api/account-rules/",
        "POST /api/account-rules/",
        "PUT /api/account-rules/{rule_id}",
        "DELETE /api/account-rules/{rule_id}",
        "POST /api/account-rules/{rule_id}/test",
        "POST /api/account-rules/reorder",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy account-rules route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::CurrentSuccessData);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }
}

#[test]
fn contract_only_surfaces_do_not_claim_runtime_business_ownership() {
    let contract_patterns: Vec<_> = endpoints_by_owner(RuntimeState::ContractOnly)
        .into_iter()
        .map(|endpoint| endpoint.pattern)
        .collect();

    assert_eq!(
        contract_patterns,
        vec![
            "contract://import-v2-envelope-oracle",
            "contract://postgres-import-writer-policy",
            "contract://postgres-foundational-schema-policy",
            "contract://postgres-repository-policy"
        ]
    );

    let retired: Vec<_> = endpoints_by_owner(RuntimeState::Retired)
        .into_iter()
        .map(|endpoint| (endpoint.method, endpoint.pattern))
        .collect();
    assert_eq!(
        retired,
        vec![
            ("GET", "/api/matching/investment-settings"),
            ("PUT", "/api/matching/investment-settings"),
        ]
    );
    assert!(endpoints_by_owner(RuntimeState::Planned).is_empty());
    assert!(endpoints_by_owner(RuntimeState::RustImplemented).is_empty());
}

#[test]
fn import_deletion_gate_requires_runtime_db_frontend_coverage_and_reference_evidence() {
    let gate_labels: Vec<_> = import_deletion_gates()
        .iter()
        .map(|gate| gate.label())
        .collect();

    assert_eq!(
        gate_labels,
        vec![
            "rust_route_runtime",
            "db_write_semantics",
            "frontend_import_flow",
            "full_coverage",
            "no_residual_references",
        ]
    );
    assert_eq!(
        missing_import_deletion_gates(ImportDeletionEvidence::default()),
        vec![
            ImportDeletionGate::RustRouteRuntime,
            ImportDeletionGate::DbWriteSemantics,
            ImportDeletionGate::FrontendImportFlow,
            ImportDeletionGate::FullCoverage,
            ImportDeletionGate::NoResidualReferences,
        ]
    );
    assert!(!can_delete_retired_import_paths(ImportDeletionEvidence {
        rust_route_runtime: true,
        db_write_semantics: true,
        frontend_import_flow: true,
        full_coverage: true,
        no_residual_references: false,
    }));
    assert!(can_delete_retired_import_paths(ImportDeletionEvidence {
        rust_route_runtime: true,
        db_write_semantics: true,
        frontend_import_flow: true,
        full_coverage: true,
        no_residual_references: true,
    }));
    assert!(import_deletion_blocked_endpoints().is_empty());
}

#[test]
fn envelope_oracle_wraps_only_runtime_infrastructure_failures() {
    let families: Vec<_> = response_envelope_policies()
        .iter()
        .map(|policy| policy.family)
        .collect();

    assert_eq!(
        families,
        vec![
            ResponseEnvelopeFamily::RustHttpShell,
            ResponseEnvelopeFamily::RuntimeInfrastructureError,
            ResponseEnvelopeFamily::CurrentSuccessData,
            ResponseEnvelopeFamily::CurrentSuccessResult,
            ResponseEnvelopeFamily::RawPassthrough,
            ResponseEnvelopeFamily::BillsCrud,
            ResponseEnvelopeFamily::BudgetsCrud,
            ResponseEnvelopeFamily::StatisticsRead,
            ResponseEnvelopeFamily::ImportV2Stage,
            ResponseEnvelopeFamily::ImportPreviewAction,
            ResponseEnvelopeFamily::ImportPreviewItemDecision,
            ResponseEnvelopeFamily::LearningRoute,
            ResponseEnvelopeFamily::LlmPreview,
            ResponseEnvelopeFamily::OcrMl,
            ResponseEnvelopeFamily::ContractOracle,
        ]
    );
    let runtime_error =
        response_envelope_policy(ResponseEnvelopeFamily::RuntimeInfrastructureError)
            .expect("runtime error policy exists");
    assert!(runtime_error.runtime_may_wrap);
    assert!(runtime_error.applies_to_owner(RuntimeState::RustImplemented));
    assert!(runtime_error.applies_to_owner(RuntimeState::RustOwnedVerified));

    for family in [
        ResponseEnvelopeFamily::ImportV2Stage,
        ResponseEnvelopeFamily::ImportPreviewAction,
        ResponseEnvelopeFamily::ImportPreviewItemDecision,
        ResponseEnvelopeFamily::BillsCrud,
        ResponseEnvelopeFamily::BudgetsCrud,
        ResponseEnvelopeFamily::StatisticsRead,
        ResponseEnvelopeFamily::LearningRoute,
        ResponseEnvelopeFamily::LlmPreview,
        ResponseEnvelopeFamily::OcrMl,
    ] {
        let policy = response_envelope_policy(family).expect("business envelope policy exists");
        assert!(policy.applies_to_owner(RuntimeState::RustImplemented));
        assert!(policy.applies_to_owner(RuntimeState::RustOwnedVerified));
        assert!(!policy.runtime_may_wrap);
    }

    let shared_success = response_envelope_policy(ResponseEnvelopeFamily::CurrentSuccessData)
        .expect("shared envelope policy exists");
    assert!(shared_success.applies_to_owner(RuntimeState::RustImplemented));
    assert!(shared_success.applies_to_owner(RuntimeState::RustOwnedVerified));

    let raw_passthrough = response_envelope_policy(ResponseEnvelopeFamily::RawPassthrough)
        .expect("raw passthrough envelope policy exists");
    assert!(raw_passthrough.applies_to_owner(RuntimeState::RustOwnedVerified));

    let success_result = response_envelope_policy(ResponseEnvelopeFamily::CurrentSuccessResult)
        .expect("success/result envelope policy exists");
    assert!(success_result.applies_to_owner(RuntimeState::RustOwnedVerified));
}

#[test]
fn db_writer_policies_mark_rust_owned_runtime_domains_and_pin_invariants() {
    let import_policy = import_db_writer_policy();
    let bills_policy = bills_crud_db_writer_policy();
    let budgets_policy = budgets_crud_db_writer_policy();

    for policy in [import_policy, bills_policy, budgets_policy] {
        assert_eq!(policy.mode, DbWriterMode::RustDomainOwned);
        assert!(policy.rust_write_allowed);
    }

    for key in [
        "postgres_authority",
        "referential_integrity",
        "rollback_on_error",
        "positive_user_scope",
        "amount_units",
        "time_normalization",
    ] {
        assert!(
            import_policy.requires_invariant(key),
            "missing import invariant {key}"
        );
        assert!(
            bills_policy.requires_invariant(key),
            "missing bills invariant {key}"
        );
        assert!(
            budgets_policy.requires_invariant(key),
            "missing budgets invariant {key}"
        );
    }

    for key in ["single_writer", "transactional_staging"] {
        assert!(
            import_policy.requires_invariant(key),
            "missing import-only invariant {key}"
        );
        assert!(!bills_policy.requires_invariant(key));
        assert!(!budgets_policy.requires_invariant(key));
    }

    assert_eq!(import_policy.domain, "bills-import");
    assert!(import_policy.active_writer.contains("import_routes/mod.rs"));
    assert_eq!(bills_policy.domain, "bills-crud");
    assert!(bills_policy.active_writer.contains("bill_routes/"));
    assert_eq!(budgets_policy.domain, "budgets-crud");
    assert!(budgets_policy.active_writer.contains("budget_routes.rs"));
}

#[test]
fn p0_state_machine_and_manifest_schema_are_machine_checkable() {
    assert_eq!(
        runtime_state_machine(),
        &[
            RuntimeState::RustImplemented,
            RuntimeState::RustOwnedVerified,
            RuntimeState::Retired,
        ]
    );

    assert!(runtime_state_machine()[2].is_rust_runtime_state());
    assert!(!RuntimeState::Planned.is_rust_runtime_state());

    let manifest = expanded_route_manifest();
    let import_runtime = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/bills/import/v2/parse")
        .expect("expanded manifest keeps import route");
    assert_eq!(import_runtime.state, RuntimeState::RustOwnedVerified);
    assert_eq!(import_runtime.domain_policy_ref, "bills-import");
    assert_eq!(import_runtime.handler, RouteHandlerId::ImportDbRuntime);
    assert!(import_runtime
        .transition_evidence
        .contains(&"frontend_contract"));
    assert!(manifest
        .iter()
        .all(|entry| entry.handler.as_str().contains("::")));

    let import_policy = find_domain_policy(import_runtime.domain_policy_ref)
        .expect("route manifest links back to domain policy");
    assert_eq!(import_policy.coverage_evidence, "workspace.lcov");
    assert!(import_policy
        .db_invariant_ids
        .contains(&"transactional_staging"));

    let ocr_config = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/ml/receipt-recognition/config")
        .expect("ocr config route is present");
    assert_eq!(ocr_config.decision_required, DecisionRequired::None);
    assert!(ocr_config.deletion_blockers.is_empty());

    let ocr_provider = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/ml/receipt-recognition")
        .expect("ocr provider route is present");
    assert_eq!(ocr_provider.decision_required, DecisionRequired::None);
    assert!(ocr_provider.deletion_blockers.is_empty());

    let settings_export = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/settings/bundle/export")
        .expect("settings bundle export route is present");
    assert_eq!(
        settings_export.envelope,
        ResponseEnvelopeFamily::RawPassthrough
    );
    assert_eq!(settings_export.state, RuntimeState::RustOwnedVerified);
    assert_eq!(settings_export.handler, RouteHandlerId::TaxonomyRuntime);
    assert!(settings_export.deletion_blockers.is_empty());

    let settings_import = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/settings/bundle/import")
        .expect("settings bundle import route is present");
    assert_eq!(settings_import.state, RuntimeState::RustOwnedVerified);
    assert_eq!(
        settings_import.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );
    assert_eq!(settings_import.handler, RouteHandlerId::TaxonomyRuntime);
    assert!(settings_import.deletion_blockers.is_empty());

    let bills_export = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/bills/export")
        .expect("bills export route is present");
    assert_eq!(
        bills_export.envelope,
        ResponseEnvelopeFamily::RawPassthrough
    );
    assert_eq!(bills_export.state, RuntimeState::RustOwnedVerified);
    assert_eq!(bills_export.handler, RouteHandlerId::BillsCrudRuntime);
    assert!(bills_export.deletion_blockers.is_empty());

    for endpoint in [
        "GET /api/bills",
        "GET /api/bills/",
        "POST /api/bills",
        "POST /api/bills/",
        "GET /api/bills/by-month",
        "GET /api/bills/get",
        "GET /api/bills/{bill_id}",
        "PUT /api/bills/{bill_id}",
        "DELETE /api/bills/{bill_id}",
        "POST /api/bills/batch",
        "GET /api/bills/export",
        "POST /api/bills/pictures",
        "POST /api/bills/pictures/unused",
        "GET /api/bills/{bill_id}/recurring-candidates",
        "PUT /api/bills/{bill_id}/recurring-match",
        "DELETE /api/bills/{bill_id}/recurring-match",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("removed bills route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::BillsCrudRuntime);
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    for endpoint in [
        "GET /api/budgets",
        "GET /api/budgets/",
        "POST /api/budgets",
        "POST /api/budgets/",
        "GET /api/budgets/{budget_id}",
        "PUT /api/budgets/{budget_id}",
        "DELETE /api/budgets/{budget_id}",
        "GET /api/budgets/export",
        "GET /api/budgets/execution",
        "GET /api/budgets/forecast",
        "GET /api/budgets/history",
        "POST /api/budgets/history/snapshot",
        "POST /api/budgets/import",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("budget route is present: {endpoint}"));
        assert_eq!(entry.state, RuntimeState::RustOwnedVerified);
        assert!(matches!(
            entry.handler,
            RouteHandlerId::BudgetsCrudRuntime
                | RouteHandlerId::BudgetsAnalysisRuntime
                | RouteHandlerId::BudgetsHistoryRuntime
                | RouteHandlerId::BudgetsImportRuntime
        ));
        assert!(entry.deletion_blockers.is_empty());
        assert_eq!(entry.decision_required, DecisionRequired::None);
    }

    let recurring_match = manifest
        .iter()
        .find(|entry| entry.endpoint == "DELETE /api/bills/{bill_id}/recurring-match")
        .expect("recurring match delete route is present");
    assert_eq!(recurring_match.state, RuntimeState::RustOwnedVerified);
    assert_eq!(recurring_match.handler, RouteHandlerId::BillsCrudRuntime);
    assert_eq!(recurring_match.decision_required, DecisionRequired::None);
    assert!(recurring_match.deletion_blockers.is_empty());

    let auth_tokens = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/tokens")
        .expect("auth token route is present");
    assert_eq!(auth_tokens.state, RuntimeState::RustOwnedVerified);
    assert_eq!(auth_tokens.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(auth_tokens.decision_required, DecisionRequired::None);
    assert!(auth_tokens.deletion_blockers.is_empty());

    let auth_profile = manifest
        .iter()
        .find(|entry| entry.endpoint == "PUT /api/profile")
        .expect("auth profile route is present");
    assert_eq!(auth_profile.state, RuntimeState::RustOwnedVerified);
    assert_eq!(auth_profile.handler, RouteHandlerId::AuthTokenRuntime);

    let auth_cloud_settings = manifest
        .iter()
        .find(|entry| entry.endpoint == "PUT /api/profile/cloud-settings")
        .expect("auth cloud settings route is present");
    assert_eq!(auth_cloud_settings.state, RuntimeState::RustOwnedVerified);
    assert_eq!(
        auth_cloud_settings.handler,
        RouteHandlerId::AuthTokenRuntime
    );

    let auth_refresh = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/tokens/refresh")
        .expect("auth refresh route is present");
    assert_eq!(auth_refresh.state, RuntimeState::RustOwnedVerified);
    assert_eq!(auth_refresh.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_refresh.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );

    let auth_login = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/login")
        .expect("auth login route is present");
    assert_eq!(auth_login.state, RuntimeState::RustOwnedVerified);
    assert_eq!(auth_login.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_login.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );

    let auth_register = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/register")
        .expect("auth register route is present");
    assert_eq!(auth_register.state, RuntimeState::RustOwnedVerified);
    assert_eq!(auth_register.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_register.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );

    let auth_logout = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/logout")
        .expect("auth logout route is present");
    assert_eq!(auth_logout.state, RuntimeState::RustOwnedVerified);
    assert_eq!(auth_logout.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_logout.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );

    for endpoint in [
        "POST /api/auth/email/verify",
        "POST /api/auth/email/resend-verification",
        "POST /api/auth/password/forgot",
        "POST /api/auth/password/reset",
    ] {
        let auth_account_recovery = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("auth account recovery route is present: {endpoint}"));
        assert_eq!(auth_account_recovery.state, RuntimeState::RustOwnedVerified);
        assert_eq!(
            auth_account_recovery.handler,
            RouteHandlerId::AuthTokenRuntime
        );
        assert_eq!(
            auth_account_recovery.envelope,
            ResponseEnvelopeFamily::CurrentSuccessResult
        );
    }

    let auth_oauth2_authorize = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/oauth2/authorize")
        .expect("auth OAuth2 authorize route is present");
    assert_eq!(auth_oauth2_authorize.state, RuntimeState::RustOwnedVerified);
    assert_eq!(
        auth_oauth2_authorize.handler,
        RouteHandlerId::AuthTokenRuntime
    );
    assert_eq!(
        auth_oauth2_authorize.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );
    assert!(auth_oauth2_authorize
        .unsupported_behavior
        .contains("OAuth2 provider exchange is explicitly disabled-safe/not-implemented"));

    let auth_user_data_statistics = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/data/statistics")
        .expect("auth user-data statistics route is present");
    assert_eq!(
        auth_user_data_statistics.state,
        RuntimeState::RustOwnedVerified
    );
    assert_eq!(
        auth_user_data_statistics.handler,
        RouteHandlerId::AuthTokenRuntime
    );
    assert_eq!(
        auth_user_data_statistics.envelope,
        ResponseEnvelopeFamily::CurrentSuccessResult
    );
    assert!(!auth_user_data_statistics
        .unsupported_behavior
        .contains("data export"));

    let auth_user_data_export = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/data/export.{file_type}")
        .expect("auth user-data export route is present");
    assert_eq!(auth_user_data_export.state, RuntimeState::RustOwnedVerified);
    assert_eq!(
        auth_user_data_export.handler,
        RouteHandlerId::AuthTokenRuntime
    );
    assert_eq!(
        auth_user_data_export.envelope,
        ResponseEnvelopeFamily::RawPassthrough
    );

    for endpoint in [
        "POST /api/data/clear/transactions",
        "POST /api/data/clear/all",
        "GET /api/2fa/status",
        "POST /api/2fa/verify",
        "POST /api/2fa/enable/request",
        "POST /api/2fa/enable/confirm",
        "POST /api/2fa/disable",
        "POST /api/2fa/recovery/regenerate",
        "POST /api/2fa/recovery/verify",
        "POST /api/security/step-up/verify",
    ] {
        let auth_two_factor = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("auth 2FA route is present: {endpoint}"));
        assert_eq!(auth_two_factor.state, RuntimeState::RustOwnedVerified);
        assert_eq!(auth_two_factor.handler, RouteHandlerId::AuthTokenRuntime);
        assert_eq!(
            auth_two_factor.envelope,
            ResponseEnvelopeFamily::CurrentSuccessResult
        );
    }
}

#[test]
fn domain_policies_record_current_rust_runtime_state() {
    let domains = domain_governance_policies();
    assert!(!domains.is_empty());

    let ai_learning = find_domain_policy("ai-learning-llm").expect("ai domain policy exists");
    assert_eq!(ai_learning.decision_required, DecisionRequired::None);
    assert_eq!(ai_learning.blocked_status, RuntimeBlockedStatus::None);
    assert!(ai_learning.deletion_blockers.is_empty());
    assert!(ai_learning.unsupported_behavior.contains("rule synthesis"));

    let database_facade =
        find_domain_policy("database-facade").expect("database facade policy exists");
    assert_eq!(database_facade.decision_required, DecisionRequired::None);
    assert!(database_facade
        .transition_evidence
        .contains(&"route_matrix"));

    let database_schema =
        find_domain_policy("database-schema").expect("database schema policy exists");
    assert_eq!(database_schema.decision_required, DecisionRequired::None);
    assert!(database_schema
        .rust_owner_files
        .contains(&"src/backend/db/postgres.rs"));
    assert!(database_schema
        .tests_verified
        .contains(&"tests/backend/core/runtime_governance_contracts.rs"));
    assert!(database_schema.deletion_blockers.is_empty());
    assert!(database_schema
        .transition_evidence
        .contains(&"schema_contract"));

    let database_repositories =
        find_domain_policy("database-repositories").expect("database repositories policy exists");
    assert!(database_repositories.deletion_blockers.is_empty());
    assert!(database_repositories
        .transition_evidence
        .contains(&"repository_contract"));
    assert!(database_repositories
        .rust_owner_files
        .contains(&"src/backend/db/auth.rs"));
    assert!(database_repositories
        .tests_verified
        .contains(&"tests/backend/db/vector_outbox.rs"));
    assert!(database_repositories
        .unsupported_behavior
        .contains("PostgreSQL-only"));

    let taxonomy = find_domain_policy("taxonomy-rules-settings").expect("taxonomy policy exists");
    assert_eq!(taxonomy.decision_required, DecisionRequired::None);
    assert!(taxonomy.retired_source_files.is_empty());
    assert!(taxonomy.deletion_blockers.is_empty());
    assert!(taxonomy
        .unsupported_behavior
        .contains("all removed taxonomy route shells have been removed"));

    for domain in [
        "budgets-crud",
        "budgets-analysis",
        "budgets-history",
        "budgets-import",
        "statistics-read",
        "statistics-analyzer",
        "statistics-exchange",
    ] {
        let policy = find_domain_policy(domain).expect("Rust-owned deleted domain policy exists");
        assert_eq!(policy.decision_required, DecisionRequired::None);
        assert!(policy.retired_source_files.is_empty(), "{domain}");
        assert!(policy.deletion_blockers.is_empty(), "{domain}");
        assert!(!policy.unsupported_behavior.trim().is_empty(), "{domain}");
    }

    let database_schema_writer = database_schema_db_writer_policy();
    assert!(database_schema_writer
        .active_writer
        .contains("init_foundational_schema"));
    assert!(database_schema_writer
        .active_writer
        .contains("init_auth_security_schema"));
}

#[test]
fn rust_http_server_runs_postgres_migrations_before_listening() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let server_source =
        fs::read_to_string(repo_root.join("src/backend/http/bin/bill_http_server.rs"))
            .expect("bill_http_server source");

    let state_init = server_source
        .find("let state = HttpAppState::new(config)?;")
        .expect("state init");
    let migration_gate = server_source
        .find("run_startup_postgres_migrations_if_configured(&state, postgres_configured).await?;")
        .expect("startup migration gate");
    let prepare_before_bind = server_source
        .find("let state = prepare_http_state(config).await?;")
        .expect("prepare before bind");
    let bind = server_source
        .find("TcpListener::bind(bind_addr).await?")
        .expect("bind listener");

    assert!(
        state_init < migration_gate && prepare_before_bind < bind,
        "PostgreSQL migrations must complete after state init and before HTTP listen"
    );
    assert!(server_source.contains("open_postgres_repository_runtime(\"startup migrations\")"));
    assert!(server_source.contains("run_postgres_migrations(runtime.pool()).await?;"));
}

#[test]
fn one_click_launcher_repairs_missing_compose_host_port_bindings() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let launcher_source =
        fs::read_to_string(repo_root.join("一键启动.ps1")).expect("one-click launcher source");

    let compose_up = launcher_source
        .find("compose -f $composeFile up -d @composeServices")
        .expect("normal compose startup");
    let repair_function = launcher_source
        .find("function Repair-ComposeServiceHostPorts")
        .expect("missing-port repair helper");
    let repair_calls = launcher_source
        .rfind("Repair-ComposeServiceHostPorts")
        .expect("missing-port repair calls");

    assert!(launcher_source.contains("up -d --force-recreate $ServiceName"));
    assert!(launcher_source.contains("ExpectedPorts @{ 5432 = $postgresPort }"));
    assert!(launcher_source
        .contains("ExpectedPorts @{ 8080 = $weaviatePort; 50051 = $weaviateGrpcPort }"));
    assert!(repair_function < compose_up && compose_up < repair_calls);
}

#[test]
fn one_click_startup_is_supervised_and_exposes_managed_commands() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let launcher_source =
        fs::read_to_string(repo_root.join("一键启动.ps1")).expect("one-click launcher source");
    let dev_source =
        fs::read_to_string(repo_root.join("scripts/dev.ps1")).expect("managed dev command source");
    let process_manifest_source =
        fs::read_to_string(repo_root.join("scripts/process-manifest.ps1"))
            .expect("process manifest helper source");
    let process_manifest_contract =
        fs::read_to_string(repo_root.join("tests/scripts/process_manifest_contract.ps1"))
            .expect("process manifest contract source");
    let live_gate = fs::read_to_string(repo_root.join("scripts/verify_one_click_start.ps1"))
        .expect("one-click live gate source");
    let batch_source =
        fs::read_to_string(repo_root.join("一键启动.bat")).expect("one-click batch source");

    assert!(launcher_source.contains("[switch]$Headless"));
    assert!(launcher_source.contains("[string]$ProcessManifestPath"));
    assert!(launcher_source.contains("Start-StartupChildProcess"));
    assert!(launcher_source.contains("$ChildProcess.Process.HasExited"));
    assert!(launcher_source.contains("Write-StartupLogTail"));
    assert!(launcher_source.contains("listener_processes"));
    assert!(launcher_source.contains("Stop-StartupOwnedProcesses"));

    for command in ["start", "status", "logs", "stop", "check"] {
        assert!(
            dev_source.contains(&format!("\"{command}\"")),
            "managed dev command must expose {command}"
        );
    }
    assert!(dev_source.contains("ProcessManifestPath"));
    assert!(dev_source.contains("RequiredServices"));
    assert!(dev_source.contains("$requiredPorts"));
    assert!(process_manifest_source.contains("Test-ProcessIdentity"));
    assert!(process_manifest_source.contains("process_start_time"));
    assert!(process_manifest_contract.contains("ConvertTo-Json | ConvertFrom-Json"));
    assert!(batch_source.contains("-Headless"));
    assert!(batch_source.contains("DEV_MANIFEST"));

    assert!(live_gate.contains("-Headless"));
    assert!(live_gate.contains("-ProcessManifestPath"));
    assert!(live_gate.contains("Assert-BackendHealth"));
    assert!(live_gate.contains("Assert-FrontendReady"));
}

#[test]
fn one_click_launchers_find_installed_pwsh_without_explorer_path() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let batch_source =
        fs::read_to_string(repo_root.join("一键启动.bat")).expect("one-click batch source");
    let shell_helper = fs::read_to_string(repo_root.join("scripts/powershell-runtime.ps1"))
        .expect("PowerShell runtime helper source");

    assert!(batch_source.contains(r#"%ProgramFiles%\PowerShell\7\pwsh.exe"#));
    assert!(batch_source.contains(r#"if exist "%PWSH_EXE%""#));
    assert!(batch_source.contains("where pwsh"));
    assert!(batch_source.contains(r#""%PWSH_EXE%" -NoProfile"#));

    assert!(shell_helper.contains("function Get-BillAnalyserPowerShell7Path"));
    assert!(shell_helper
        .contains("[System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName"));
    assert!(shell_helper.contains(r#"$env:ProgramFiles"#));
    assert!(shell_helper.contains("Get-Command pwsh"));

    for entrypoint in [
        "一键启动.ps1",
        "scripts/dev.ps1",
        "scripts/verify_one_click_start.ps1",
    ] {
        let source = fs::read_to_string(repo_root.join(entrypoint))
            .unwrap_or_else(|error| panic!("{entrypoint} source: {error}"));
        assert!(source.contains("powershell-runtime.ps1"));
        assert!(source.contains("Get-BillAnalyserPowerShell7Path"));
    }
}

#[test]
fn one_click_stop_batch_uses_manifest_owned_shutdown() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let stop_batch =
        fs::read_to_string(repo_root.join("一键结束.bat")).expect("one-click stop batch source");

    assert!(stop_batch.contains(r#"%ProgramFiles%\PowerShell\7\pwsh.exe"#));
    assert!(stop_batch.contains("where pwsh"));
    assert!(stop_batch.contains(r#"scripts\dev.ps1"#));
    assert!(stop_batch.contains(r#""%DEV_SCRIPT%" stop"#));
    assert!(stop_batch.contains("pause"));
    assert!(!stop_batch.contains("taskkill"));
    assert!(!stop_batch.contains("docker compose"));
}

#[test]
fn domain_db_invariant_ids_match_public_writer_policy_helpers() {
    for (domain, policy) in [
        ("database-schema", database_schema_db_writer_policy()),
        ("bills-import", import_db_writer_policy()),
        ("bills-crud", bills_crud_db_writer_policy()),
        ("budgets-crud", budgets_crud_db_writer_policy()),
    ] {
        let domain_policy = find_domain_policy(domain).expect("domain policy exists");
        let policy_keys: Vec<_> = policy.invariants.iter().map(|item| item.key).collect();
        assert_eq!(domain_policy.db_invariant_ids, policy_keys.as_slice());
    }
}

#[test]
fn governance_snapshot_joins_route_and_domain_manifests() {
    let snapshot = governance_manifest_snapshot();
    assert_eq!(snapshot.coverage_evidence_contract, "workspace.lcov");
    assert_eq!(snapshot.runtime_state_machine, runtime_state_machine());
    assert_eq!(snapshot.manifest_states, manifest_states());
    assert!(snapshot
        .manifest_states
        .contains(&RuntimeState::ContractOnly));
    assert_eq!(
        snapshot.routes.len(),
        rust_http_shell_ownership_matrix().len()
    );
    assert_eq!(snapshot.domains.len(), domain_governance_policies().len());
}

#[test]
fn generated_frontend_route_ownership_manifest_matches_governance_snapshot() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should resolve from core crate manifest dir");
    let manifest_path =
        repo_root.join("src/web/src/contracts/rustRouteOwnership.manifest.generated.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
    let manifest: FrontendRouteOwnershipManifest = serde_json::from_str(&manifest_text)
        .expect("frontend route ownership manifest should parse");

    assert_eq!(
        manifest.generated_from,
        "bill_runtime_manifest::governance_manifest_snapshot.routes"
    );

    let expected: Vec<_> = governance_manifest_snapshot()
        .routes
        .iter()
        .map(|route| FrontendRouteOwnership {
            method: route.method.to_string(),
            pattern: route.pattern.to_string(),
            domain: route.domain.to_string(),
            state: route.state,
        })
        .collect();
    assert_eq!(manifest.routes, expected);
}

#[test]
fn category_rule_selection_has_one_core_owner_and_two_production_adapters() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let core_selection =
        fs::read_to_string(repo_root.join("src/backend/core/category_rules/selection.rs"))
            .expect("category rule selection owner is readable");
    let bill_adapter = fs::read_to_string(
        repo_root.join("src/backend/http/bill_routes/category_action_handlers.rs"),
    )
    .expect("bill category adapter is readable");
    let stage2_adapter = fs::read_to_string(
        repo_root.join("src/backend/http/import_routes/stage_handlers/stage2_category_rules.rs"),
    )
    .expect("Stage 2 category adapter is readable");
    let stage2_types = fs::read_to_string(
        repo_root.join("src/backend/http/import_routes/stage_handlers/stage2_types.rs"),
    )
    .expect("Stage 2 types are readable");

    assert!(core_selection.contains("pub fn select_category_rule_candidate"));
    assert_eq!(
        bill_adapter
            .matches("select_category_rule_candidate")
            .count(),
        1
    );
    assert_eq!(
        stage2_adapter
            .matches("select_category_rule_candidate")
            .count(),
        1
    );

    for removed in [
        "CategoryRuleRuntimeRecord",
        "rules.sort_by_key(|rule|",
        "match_rule_expression(&combined_text",
    ] {
        assert!(
            !bill_adapter.contains(removed),
            "bill adapter restored {removed}"
        );
    }
    for removed in [
        "apply_category_rule_match_candidate_refs",
        "match_compiled_rule_lowercase_text",
        "ImportIntelligenceRule",
    ] {
        assert!(
            !stage2_adapter.contains(removed),
            "Stage 2 adapter restored {removed}"
        );
    }
    assert!(!stage2_types.contains("ImportIntelligenceRuleSet"));
    assert!(!stage2_types.contains("struct ImportIntelligenceRule"));
}
