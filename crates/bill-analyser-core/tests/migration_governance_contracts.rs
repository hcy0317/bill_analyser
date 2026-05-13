use bill_analyser_core::{
    bills_crud_db_writer_policy, budgets_crud_db_writer_policy, can_delete_python_import_paths,
    database_schema_db_writer_policy, domain_governance_policies, endpoints_by_owner,
    expanded_route_manifest, find_domain_policy, find_endpoint_ownership,
    governance_manifest_snapshot, import_db_writer_policy, import_deletion_blocked_endpoints,
    import_deletion_gates, manifest_states, migration_state_machine, missing_import_deletion_gates,
    response_envelope_policies, response_envelope_policy, rust_http_shell_ownership_matrix,
    DbWriterMode, DecisionRequired, ImportDeletionEvidence, ImportDeletionGate,
    MigrationBlockedStatus, MigrationState, ResponseEnvelopeFamily, RouteHandlerId,
};

#[test]
fn rust_owned_verified_runtime_routes_include_health_metadata_and_first_phase_import_runtime() {
    let rust_owned: Vec<_> = endpoints_by_owner(MigrationState::RustOwnedVerified)
        .into_iter()
        .filter(|endpoint| endpoint.pattern.starts_with("/api/"))
        .map(|endpoint| (endpoint.method, endpoint.pattern))
        .collect();

    assert!(rust_owned.contains(&("GET", "/api/health")));
    assert!(rust_owned.contains(&("GET", "/api/runtime")));
    assert!(rust_owned.contains(&("POST", "/api/bills/import/v2/parse")));
    assert!(rust_owned.contains(&("GET", "/api/bills")));
    assert!(rust_owned.contains(&("POST", "/api/bills/batch")));
    assert!(rust_owned.contains(&("PUT", "/api/bills/batch/update")));
    assert!(rust_owned.contains(&("DELETE", "/api/bills/batch/delete")));
    assert!(rust_owned.contains(&("GET", "/api/budgets/")));
    assert!(rust_owned.contains(&("POST", "/api/budgets/")));
    assert!(rust_owned.contains(&("GET", "/api/budgets/export")));
    assert!(rust_owned.contains(&("GET", "/api/budgets/execution")));
    assert!(rust_owned.contains(&("GET", "/api/budgets/forecast")));
    assert!(rust_owned.contains(&("GET", "/api/budgets/history")));
    assert!(rust_owned.contains(&("POST", "/api/budgets/history/snapshot")));
    assert!(rust_owned.contains(&("POST", "/api/budgets/import")));
    assert!(rust_owned.contains(&("GET", "/api/statistics/category-statistics")));
    assert!(rust_owned.contains(&("GET", "/api/statistics/category-statistics/trends")));
    assert!(rust_owned.contains(&("GET", "/api/statistics/asset-trends")));
    assert!(rust_owned.contains(&("GET", "/api/statistics/category-pie")));
    assert!(rust_owned.contains(&("GET", "/api/statistics/top-merchants")));
    assert!(rust_owned.contains(&("GET", "/api/statistics/amounts")));
    assert!(rust_owned.contains(&("GET", "/api/tokens")));
    assert!(rust_owned.contains(&("DELETE", "/api/tokens")));
    assert!(rust_owned.contains(&("DELETE", "/api/tokens/{token_id}")));
    assert!(rust_owned.contains(&("POST", "/api/tokens/api")));
    assert!(rust_owned.contains(&("POST", "/api/tokens/mcp")));
    assert!(rust_owned.contains(&("POST", "/api/auth/login")));
    assert!(rust_owned.contains(&("POST", "/api/auth/register")));
    assert!(rust_owned.contains(&("POST", "/api/auth/logout")));
    assert!(rust_owned.contains(&("POST", "/api/auth/email/verify")));
    assert!(rust_owned.contains(&("POST", "/api/auth/email/resend-verification")));
    assert!(rust_owned.contains(&("POST", "/api/auth/password/forgot")));
    assert!(rust_owned.contains(&("POST", "/api/auth/password/reset")));
    assert!(rust_owned.contains(&("POST", "/api/auth/oauth2/authorize")));
    assert!(rust_owned.contains(&("POST", "/api/security/step-up/verify")));
    assert!(rust_owned.contains(&("GET", "/api/data/export.{file_type}")));
    assert!(rust_owned.contains(&("POST", "/api/data/clear/transactions")));
    assert!(rust_owned.contains(&("POST", "/api/data/clear/all")));
    assert!(rust_owned.contains(&("GET", "/api/templates/")));
    assert!(rust_owned.contains(&("PUT", "/api/templates/display-orders")));
    assert!(rust_owned.contains(&("GET", "/api/category-rules/")));
    assert!(rust_owned.contains(&("POST", "/api/category-rules/{rule_id}/test")));
    assert!(rust_owned.contains(&("GET", "/api/rules/overview")));
    assert!(rust_owned.contains(&("POST", "/api/llm/preview-recommend/accept")));
    assert!(rust_owned.contains(&("GET", "/api/ml/receipt-recognition/config")));
    assert!(!rust_owned.contains(&("GET", "/api/learning/rules")));
    assert!(!rust_owned.contains(&("GET", "/api/learning/suggestions")));
    assert!(!rust_owned.contains(&("POST", "/api/llm/preview-recommend")));
    assert!(!rust_owned.contains(&("POST", "/api/llm/analyze-transactions")));
    assert!(!rust_owned.contains(&("POST", "/api/llm/rule-synthesis")));
    assert!(!rust_owned.contains(&("POST", "/api/ml/receipt-recognition")));

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
}

#[test]
fn import_and_preview_adjacent_routes_are_rust_owned_but_python_deletion_is_still_gate_blocked() {
    let blocked_routes = import_deletion_blocked_endpoints();
    assert!(!blocked_routes.is_empty());
    assert!(blocked_routes.iter().all(|endpoint| {
        endpoint.state == MigrationState::RustOwnedVerified && endpoint.is_import_deletion_blocked()
    }));

    for (method, pattern) in [
        ("POST", "/api/bills/import/v2/parse"),
        ("PUT", "/api/bills/import/v2/preview/{session_id}/update"),
        (
            "POST",
            "/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
        ),
        ("POST", "/api/llm/preview-recommend/accept"),
        ("GET", "/api/ml/receipt-recognition/config"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing endpoint ownership for {method} {pattern}"));
        assert_eq!(endpoint.state, MigrationState::RustOwnedVerified);
        assert!(!endpoint.is_python_runtime_owner());
        assert!(endpoint.is_import_deletion_blocked());
    }

    for (method, pattern) in [
        ("GET", "/api/bills/export"),
        ("POST", "/api/bills/pictures"),
        ("POST", "/api/bills/pictures/unused"),
        ("GET", "/api/bills/reconciliation_statements"),
        ("GET", "/api/bills/{bill_id}/recurring-candidates"),
        ("PUT", "/api/bills/{bill_id}/recurring-match"),
        ("DELETE", "/api/bills/{bill_id}/recurring-match"),
        ("POST", "/api/bills/category/quick-add-keyword"),
        ("POST", "/api/bills/category/refresh"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern).unwrap_or_else(|| {
            panic!("missing Rust-owned bills adjacent endpoint {method} {pattern}")
        });
        assert_eq!(endpoint.state, MigrationState::RustOwnedVerified);
        assert!(!endpoint.is_python_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }

    let receipt_recognition = find_endpoint_ownership("POST", "/api/ml/receipt-recognition")
        .expect("receipt OCR recognition route is governed");
    assert_eq!(receipt_recognition.state, MigrationState::PythonProxied);
    assert!(receipt_recognition.is_python_runtime_owner());
    assert!(!receipt_recognition.is_import_deletion_blocked());

    for (method, pattern) in [
        ("GET", "/api/statistics/overview"),
        ("GET", "/api/statistics/trends"),
        ("GET", "/api/statistics/comparison"),
        ("GET", "/api/statistics/category"),
        ("GET", "/api/statistics/trend"),
        ("GET", "/api/statistics/exchange-rates"),
        ("PUT", "/api/statistics/exchange-rates/custom"),
        ("DELETE", "/api/statistics/exchange-rates/custom/{currency}"),
        ("POST", "/api/llm/preview-recommend"),
        ("POST", "/api/llm/analyze-transactions"),
        ("POST", "/api/llm/rule-synthesis"),
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
            .unwrap_or_else(|| panic!("missing provider-owned endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, MigrationState::PythonProxied);
        assert!(endpoint.is_python_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }
}

#[test]
fn live_python_sidecar_routes_are_manifested_for_import_db_runtime_proxy() {
    for (method, pattern, domain) in [
        (
            "POST",
            "/api/accounts/{account_id}/transactions/move",
            "taxonomy-rules-settings",
        ),
        ("GET", "/api/categories/rules", "taxonomy-rules-settings"),
        (
            "POST",
            "/api/settings/bundle/import",
            "taxonomy-rules-settings",
        ),
        ("GET", "/api/llm/config", "ai-learning-llm"),
        (
            "POST",
            "/api/matching/candidates/{*candidate_id}/accept",
            "matching-recurring-calendar-networth",
        ),
        ("GET", "/api/backup/jobs", "backup-ops"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("missing live Python sidecar endpoint {method} {pattern}"));
        assert_eq!(endpoint.state, MigrationState::PythonProxied);
        assert_eq!(endpoint.domain, domain);
    }
}

#[test]
fn taxonomy_master_data_routes_are_rust_owned_while_p4_remainder_stays_proxied() {
    let manifest = expanded_route_manifest();
    for endpoint in [
        "GET /api/accounts/",
        "POST /api/accounts/",
        "GET /api/accounts/{account_id}",
        "PUT /api/accounts/{account_id}",
        "DELETE /api/accounts/{account_id}",
        "PUT /api/accounts/display-orders",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy account route is present: {endpoint}"));
        assert_eq!(entry.state, MigrationState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::FlaskSuccessResult);
        assert!(entry
            .unsupported_behavior
            .contains("settings bundle export routes are Rust-owned"));
    }

    for endpoint in [
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
        assert_eq!(entry.state, MigrationState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::FlaskSuccessResult);
        assert!(entry
            .unsupported_behavior
            .contains("tag CRUD/display-order/batch-create"));
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
        assert_eq!(entry.state, MigrationState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::FlaskSuccessResult);
        assert!(entry
            .unsupported_behavior
            .contains("category master-data/statistics/update-all, category-rule list/test, rule overview, templates, and settings bundle export routes are Rust-owned"));
        assert!(entry
            .deletion_blockers
            .contains(&"templates_category_rules_settings_parity"));
        assert!(entry
            .deletion_blockers
            .contains(&"account_transaction_operations_parity"));
    }

    for endpoint in [
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
        assert_eq!(entry.state, MigrationState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::FlaskSuccessResult);
        assert!(entry
            .unsupported_behavior
            .contains("settings bundle export routes are Rust-owned"));
    }

    for endpoint in [
        "GET /api/category-rules/",
        "POST /api/category-rules/{rule_id}/test",
        "GET /api/rules/overview",
    ] {
        let entry = manifest
            .iter()
            .find(|entry| entry.endpoint == endpoint)
            .unwrap_or_else(|| panic!("taxonomy category-rules route is present: {endpoint}"));
        assert_eq!(entry.state, MigrationState::RustOwnedVerified);
        assert_eq!(entry.handler, RouteHandlerId::TaxonomyRuntime);
        assert_eq!(entry.envelope, ResponseEnvelopeFamily::FlaskSuccessData);
        assert!(entry
            .unsupported_behavior
            .contains("category rule mutations"));
    }

    for (method, pattern) in [
        ("POST", "/api/accounts/{account_id}/transactions/clear"),
        ("POST", "/api/accounts/{account_id}/transactions/move"),
        ("POST", "/api/accounts/sync-balances"),
        ("GET", "/api/categories/rules"),
        ("PUT", "/api/categories/rules"),
        ("POST", "/api/category-rules/"),
        ("DELETE", "/api/category-rules/{rule_id}"),
        ("PUT", "/api/category-rules/{rule_id}"),
        ("POST", "/api/category-rules/defaults"),
        ("POST", "/api/category-rules/migrate"),
        ("POST", "/api/category-rules/reorder"),
    ] {
        let endpoint = find_endpoint_ownership(method, pattern)
            .unwrap_or_else(|| panic!("taxonomy proxied route is present: {method} {pattern}"));
        assert_eq!(endpoint.state, MigrationState::PythonProxied);
        assert_eq!(endpoint.domain, "taxonomy-rules-settings");
    }
}

#[test]
fn contract_only_surfaces_do_not_claim_runtime_business_ownership() {
    let contract_patterns: Vec<_> = endpoints_by_owner(MigrationState::ContractOnly)
        .into_iter()
        .map(|endpoint| endpoint.pattern)
        .collect();

    assert_eq!(
        contract_patterns,
        vec![
            "contract://import-v2-envelope-oracle",
            "contract://sqlite-import-writer-policy",
            "contract://sqlite-foundational-schema-policy",
            "contract://sqlite-repository-policy"
        ]
    );

    assert!(endpoints_by_owner(MigrationState::PythonDeleted).is_empty());
    assert!(endpoints_by_owner(MigrationState::Planned).is_empty());
    assert!(endpoints_by_owner(MigrationState::RustImplemented).is_empty());
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
    assert!(!can_delete_python_import_paths(ImportDeletionEvidence {
        rust_route_runtime: true,
        db_write_semantics: true,
        frontend_import_flow: true,
        full_coverage: true,
        no_residual_references: false,
    }));
    assert!(can_delete_python_import_paths(ImportDeletionEvidence {
        rust_route_runtime: true,
        db_write_semantics: true,
        frontend_import_flow: true,
        full_coverage: true,
        no_residual_references: true,
    }));
    assert!(import_deletion_blocked_endpoints()
        .iter()
        .all(|endpoint| endpoint.state == MigrationState::RustOwnedVerified));
}

#[test]
fn envelope_oracle_wraps_only_proxy_infrastructure_failures() {
    let families: Vec<_> = response_envelope_policies()
        .iter()
        .map(|policy| policy.family)
        .collect();

    assert_eq!(
        families,
        vec![
            ResponseEnvelopeFamily::RustHttpShell,
            ResponseEnvelopeFamily::ProxyInfrastructureError,
            ResponseEnvelopeFamily::FlaskSuccessData,
            ResponseEnvelopeFamily::FlaskSuccessResult,
            ResponseEnvelopeFamily::FlaskRawPassthrough,
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
    let proxy_error = response_envelope_policy(ResponseEnvelopeFamily::ProxyInfrastructureError)
        .expect("proxy error policy exists");
    assert!(proxy_error.proxy_may_wrap);
    assert!(proxy_error.applies_to_owner(MigrationState::RustImplemented));
    assert!(proxy_error.applies_to_owner(MigrationState::RustOwnedVerified));
    assert!(!proxy_error.applies_to_owner(MigrationState::PythonProxied));

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
        assert!(policy.applies_to_owner(MigrationState::RustImplemented));
        assert!(policy.applies_to_owner(MigrationState::RustOwnedVerified));
        assert!(!policy.proxy_may_wrap);
    }

    for family in [
        ResponseEnvelopeFamily::FlaskSuccessData,
        ResponseEnvelopeFamily::OcrMl,
    ] {
        let policy = response_envelope_policy(family).expect("shared envelope policy exists");
        assert!(policy.applies_to_owner(MigrationState::RustImplemented));
        assert!(policy.applies_to_owner(MigrationState::RustOwnedVerified));
        assert!(policy.applies_to_owner(MigrationState::PythonProxied));
    }

    let raw_passthrough = response_envelope_policy(ResponseEnvelopeFamily::FlaskRawPassthrough)
        .expect("raw passthrough envelope policy exists");
    assert!(raw_passthrough.applies_to_owner(MigrationState::PythonProxied));
    assert!(!raw_passthrough.applies_to_owner(MigrationState::RustImplemented));
    assert!(!raw_passthrough.applies_to_owner(MigrationState::RustOwnedVerified));

    let success_result = response_envelope_policy(ResponseEnvelopeFamily::FlaskSuccessResult)
        .expect("success/result envelope policy exists");
    assert!(success_result.applies_to_owner(MigrationState::PythonProxied));
    assert!(success_result.applies_to_owner(MigrationState::RustOwnedVerified));
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
        "wal_mode",
        "foreign_keys",
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
    assert!(import_policy.active_writer.contains("import_routes.rs"));
    assert_eq!(bills_policy.domain, "bills-crud");
    assert!(bills_policy.active_writer.contains("bill_routes.rs"));
    assert_eq!(budgets_policy.domain, "budgets-crud");
    assert!(budgets_policy.active_writer.contains("budget_routes.rs"));
}

#[test]
fn p0_state_machine_and_manifest_schema_are_machine_checkable() {
    assert_eq!(
        migration_state_machine(),
        &[
            MigrationState::PythonProxied,
            MigrationState::RustImplemented,
            MigrationState::RustOwnedVerified,
            MigrationState::PythonDeleted,
        ]
    );

    assert!(migration_state_machine()[2].is_rust_runtime_state());
    assert!(!MigrationState::Planned.is_rust_runtime_state());

    let manifest = expanded_route_manifest();
    let import_runtime = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/bills/import/v2/parse")
        .expect("expanded manifest keeps import route");
    assert_eq!(import_runtime.state, MigrationState::RustOwnedVerified);
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
    assert!(ocr_config.deletion_blockers.contains(&"rust_route_runtime"));
    assert!(!ocr_config
        .deletion_blockers
        .contains(&"provider_execution_parity"));

    let ocr_provider = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/ml/receipt-recognition")
        .expect("ocr provider route is present");
    assert_eq!(ocr_provider.decision_required, DecisionRequired::Port);
    assert!(ocr_provider
        .deletion_blockers
        .contains(&"provider_execution_parity"));

    let settings_export = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/settings/bundle/export")
        .expect("settings bundle export route is present");
    assert_eq!(
        settings_export.envelope,
        ResponseEnvelopeFamily::FlaskRawPassthrough
    );
    assert_eq!(settings_export.state, MigrationState::RustOwnedVerified);
    assert_eq!(settings_export.handler, RouteHandlerId::TaxonomyRuntime);

    let settings_import = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/settings/bundle/import")
        .expect("settings bundle import route is present");
    assert_eq!(settings_import.state, MigrationState::PythonProxied);

    let bills_export = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/bills/export")
        .expect("bills export route is present");
    assert_eq!(
        bills_export.envelope,
        ResponseEnvelopeFamily::FlaskRawPassthrough
    );
    assert_eq!(bills_export.state, MigrationState::RustOwnedVerified);
    assert_eq!(bills_export.handler, RouteHandlerId::BillsCrudRuntime);

    let recurring_match = manifest
        .iter()
        .find(|entry| entry.endpoint == "DELETE /api/bills/{bill_id}/recurring-match")
        .expect("recurring match delete route is present");
    assert_eq!(recurring_match.state, MigrationState::RustOwnedVerified);
    assert_eq!(recurring_match.handler, RouteHandlerId::BillsCrudRuntime);
    assert_eq!(recurring_match.decision_required, DecisionRequired::None);
    assert!(recurring_match.deletion_blockers.is_empty());

    let auth_tokens = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/tokens")
        .expect("auth token route is present");
    assert_eq!(auth_tokens.state, MigrationState::RustOwnedVerified);
    assert_eq!(auth_tokens.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(auth_tokens.decision_required, DecisionRequired::Port);
    assert!(auth_tokens
        .deletion_blockers
        .contains(&"profile_user_data_parity"));
    assert!(!auth_tokens
        .deletion_blockers
        .contains(&"token_refresh_parity"));

    let auth_profile = manifest
        .iter()
        .find(|entry| entry.endpoint == "PUT /api/profile")
        .expect("auth profile route is present");
    assert_eq!(auth_profile.state, MigrationState::RustOwnedVerified);
    assert_eq!(auth_profile.handler, RouteHandlerId::AuthTokenRuntime);

    let auth_cloud_settings = manifest
        .iter()
        .find(|entry| entry.endpoint == "PUT /api/profile/cloud-settings")
        .expect("auth cloud settings route is present");
    assert_eq!(auth_cloud_settings.state, MigrationState::RustOwnedVerified);
    assert_eq!(
        auth_cloud_settings.handler,
        RouteHandlerId::AuthTokenRuntime
    );

    let auth_refresh = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/tokens/refresh")
        .expect("auth refresh route is present");
    assert_eq!(auth_refresh.state, MigrationState::RustOwnedVerified);
    assert_eq!(auth_refresh.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_refresh.envelope,
        ResponseEnvelopeFamily::FlaskSuccessResult
    );

    let auth_login = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/login")
        .expect("auth login route is present");
    assert_eq!(auth_login.state, MigrationState::RustOwnedVerified);
    assert_eq!(auth_login.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_login.envelope,
        ResponseEnvelopeFamily::FlaskSuccessResult
    );

    let auth_register = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/register")
        .expect("auth register route is present");
    assert_eq!(auth_register.state, MigrationState::RustOwnedVerified);
    assert_eq!(auth_register.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_register.envelope,
        ResponseEnvelopeFamily::FlaskSuccessResult
    );

    let auth_logout = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/logout")
        .expect("auth logout route is present");
    assert_eq!(auth_logout.state, MigrationState::RustOwnedVerified);
    assert_eq!(auth_logout.handler, RouteHandlerId::AuthTokenRuntime);
    assert_eq!(
        auth_logout.envelope,
        ResponseEnvelopeFamily::FlaskSuccessResult
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
        assert_eq!(
            auth_account_recovery.state,
            MigrationState::RustOwnedVerified
        );
        assert_eq!(
            auth_account_recovery.handler,
            RouteHandlerId::AuthTokenRuntime
        );
        assert_eq!(
            auth_account_recovery.envelope,
            ResponseEnvelopeFamily::FlaskSuccessResult
        );
    }

    let auth_oauth2_authorize = manifest
        .iter()
        .find(|entry| entry.endpoint == "POST /api/auth/oauth2/authorize")
        .expect("auth OAuth2 authorize route is present");
    assert_eq!(
        auth_oauth2_authorize.state,
        MigrationState::RustOwnedVerified
    );
    assert_eq!(
        auth_oauth2_authorize.handler,
        RouteHandlerId::AuthTokenRuntime
    );
    assert_eq!(
        auth_oauth2_authorize.envelope,
        ResponseEnvelopeFamily::FlaskSuccessResult
    );
    assert!(auth_oauth2_authorize
        .unsupported_behavior
        .contains("real OAuth provider exchange"));

    let auth_user_data_statistics = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/data/statistics")
        .expect("auth user-data statistics route is present");
    assert_eq!(
        auth_user_data_statistics.state,
        MigrationState::RustOwnedVerified
    );
    assert_eq!(
        auth_user_data_statistics.handler,
        RouteHandlerId::AuthTokenRuntime
    );
    assert_eq!(
        auth_user_data_statistics.envelope,
        ResponseEnvelopeFamily::FlaskSuccessResult
    );
    assert!(!auth_user_data_statistics
        .unsupported_behavior
        .contains("data export"));

    let auth_user_data_export = manifest
        .iter()
        .find(|entry| entry.endpoint == "GET /api/data/export.{file_type}")
        .expect("auth user-data export route is present");
    assert_eq!(
        auth_user_data_export.state,
        MigrationState::RustOwnedVerified
    );
    assert_eq!(
        auth_user_data_export.handler,
        RouteHandlerId::AuthTokenRuntime
    );
    assert_eq!(
        auth_user_data_export.envelope,
        ResponseEnvelopeFamily::FlaskRawPassthrough
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
        assert_eq!(auth_two_factor.state, MigrationState::RustOwnedVerified);
        assert_eq!(auth_two_factor.handler, RouteHandlerId::AuthTokenRuntime);
        assert_eq!(
            auth_two_factor.envelope,
            ResponseEnvelopeFamily::FlaskSuccessResult
        );
    }
}

#[test]
fn domain_policies_record_provider_and_deletion_blockers() {
    let domains = domain_governance_policies();
    assert!(!domains.is_empty());

    let ai_learning = find_domain_policy("ai-learning-llm").expect("ai domain policy exists");
    assert_eq!(ai_learning.decision_required, DecisionRequired::Port);
    assert_eq!(ai_learning.blocked_status, MigrationBlockedStatus::None);
    assert!(ai_learning
        .deletion_blockers
        .contains(&"provider_execution_parity"));
    assert!(ai_learning.unsupported_behavior.contains("rule synthesis"));

    let database_facade =
        find_domain_policy("database-facade").expect("database facade policy exists");
    assert_eq!(database_facade.decision_required, DecisionRequired::Defer);
    assert!(database_facade
        .transition_evidence
        .contains(&"route_matrix"));

    let database_schema =
        find_domain_policy("database-schema").expect("database schema policy exists");
    assert_eq!(database_schema.decision_required, DecisionRequired::Port);
    assert!(database_schema
        .rust_owner_files
        .contains(&"crates/bill-analyser-db/src/schema.rs"));
    assert!(database_schema
        .tests_migrated
        .contains(&"crates/bill-analyser-db/tests/sqlite_runtime.rs"));
    assert!(database_schema
        .deletion_blockers
        .contains(&"encryption_schema_parity"));
    assert!(database_schema
        .transition_evidence
        .contains(&"schema_migration_contract"));

    let database_repositories =
        find_domain_policy("database-repositories").expect("database repositories policy exists");
    assert_eq!(
        database_repositories.deletion_blockers,
        &["business_domain_route_takeover"]
    );
    assert!(database_repositories
        .transition_evidence
        .contains(&"repository_contract"));
    assert!(database_repositories
        .rust_owner_files
        .contains(&"crates/bill-analyser-db/src/auth.rs"));
    assert!(database_repositories
        .tests_migrated
        .contains(&"crates/bill-analyser-db/tests/auth_two_factor_recovery.rs"));
    assert!(database_repositories
        .unsupported_behavior
        .contains("2FA recovery-code DB primitives"));

    let database_schema_writer = database_schema_db_writer_policy();
    assert!(database_schema_writer
        .active_writer
        .contains("init_foundational_schema"));
    assert!(database_schema_writer
        .active_writer
        .contains("init_auth_security_schema"));
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
    assert_eq!(snapshot.cutover_state_machine, migration_state_machine());
    assert_eq!(snapshot.manifest_states, manifest_states());
    assert!(snapshot
        .manifest_states
        .contains(&MigrationState::ContractOnly));
    assert_eq!(
        snapshot.routes.len(),
        rust_http_shell_ownership_matrix().len()
    );
    assert_eq!(snapshot.domains.len(), domain_governance_policies().len());
}
