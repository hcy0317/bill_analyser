use bill_analyser_core::{
    bills_crud_db_writer_policy, budgets_crud_db_writer_policy, can_delete_python_import_paths,
    endpoints_by_owner, find_endpoint_ownership, import_db_writer_policy,
    import_deletion_blocked_endpoints, import_deletion_gates, missing_import_deletion_gates,
    response_envelope_policies, response_envelope_policy, rust_http_shell_ownership_matrix,
    DbWriterMode, ImportDeletionEvidence, ImportDeletionGate, ResponseEnvelopeFamily, RouteOwner,
};

#[test]
fn rust_owned_runtime_routes_include_health_metadata_and_first_phase_import_runtime() {
    let rust_owned: Vec<_> = endpoints_by_owner(RouteOwner::RustOwned)
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
        endpoint.owner == RouteOwner::RustOwned && endpoint.is_import_deletion_blocked()
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
        assert_eq!(endpoint.owner, RouteOwner::RustOwned);
        assert!(!endpoint.is_python_runtime_owner());
        assert!(endpoint.is_import_deletion_blocked());
    }

    let receipt_recognition = find_endpoint_ownership("POST", "/api/ml/receipt-recognition")
        .expect("receipt OCR recognition route is governed");
    assert_eq!(receipt_recognition.owner, RouteOwner::PythonProxied);
    assert!(receipt_recognition.is_python_runtime_owner());
    assert!(!receipt_recognition.is_import_deletion_blocked());

    for (method, pattern) in [
        ("GET", "/api/bills/export"),
        ("POST", "/api/bills/pictures*"),
        ("GET", "/api/bills/reconciliation_statements"),
        ("GET", "/api/bills/{bill_id}/recurring-candidates"),
        ("PUT", "/api/bills/{bill_id}/recurring-match"),
        ("POST", "/api/bills/category/*"),
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
        assert_eq!(endpoint.owner, RouteOwner::PythonProxied);
        assert!(endpoint.is_python_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
    }
}

#[test]
fn contract_only_surfaces_do_not_claim_runtime_business_ownership() {
    let contract_patterns: Vec<_> = endpoints_by_owner(RouteOwner::ContractOnly)
        .into_iter()
        .map(|endpoint| endpoint.pattern)
        .collect();

    assert_eq!(
        contract_patterns,
        vec![
            "contract://import-v2-envelope-oracle",
            "contract://sqlite-import-writer-policy"
        ]
    );

    assert!(endpoints_by_owner(RouteOwner::Deleted).is_empty());
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
        .all(|endpoint| endpoint.owner == RouteOwner::RustOwned));
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
    assert!(proxy_error.applies_to_owner(RouteOwner::RustOwned));
    assert!(!proxy_error.applies_to_owner(RouteOwner::PythonProxied));

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
        assert!(policy.applies_to_owner(RouteOwner::RustOwned));
        assert!(!policy.proxy_may_wrap);
    }

    for family in [
        ResponseEnvelopeFamily::FlaskSuccessData,
        ResponseEnvelopeFamily::OcrMl,
    ] {
        let policy = response_envelope_policy(family).expect("shared envelope policy exists");
        assert!(policy.applies_to_owner(RouteOwner::RustOwned));
        assert!(policy.applies_to_owner(RouteOwner::PythonProxied));
    }
}

#[test]
fn db_writer_policies_mark_rust_owned_runtime_domains_and_pin_invariants() {
    for key in [
        "wal_mode",
        "foreign_keys",
        "single_writer",
        "transactional_staging",
        "rollback_on_error",
        "positive_user_scope",
        "amount_units",
        "time_normalization",
    ] {
        for policy in [
            import_db_writer_policy(),
            bills_crud_db_writer_policy(),
            budgets_crud_db_writer_policy(),
        ] {
            assert_eq!(policy.mode, DbWriterMode::RustDomainOwned);
            assert!(policy.rust_write_allowed);
            assert!(policy.requires_invariant(key), "missing invariant {key}");
        }
    }
    assert_eq!(import_db_writer_policy().domain, "bills-import");
    assert!(import_db_writer_policy()
        .active_writer
        .contains("import_routes.rs"));
    assert_eq!(bills_crud_db_writer_policy().domain, "bills-crud");
    assert!(bills_crud_db_writer_policy()
        .active_writer
        .contains("bill_routes.rs"));
    assert_eq!(budgets_crud_db_writer_policy().domain, "budgets-crud");
    assert!(budgets_crud_db_writer_policy()
        .active_writer
        .contains("budget_routes.rs"));
}
