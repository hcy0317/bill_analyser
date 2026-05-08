use bill_analyser_core::{
    bills_crud_db_writer_policy, budgets_crud_db_writer_policy, can_delete_python_import_paths,
    domain_governance_policies, endpoints_by_owner, expanded_route_manifest, find_domain_policy,
    find_endpoint_ownership, governance_manifest_snapshot, import_db_writer_policy,
    import_deletion_blocked_endpoints, import_deletion_gates, manifest_states,
    migration_state_machine, missing_import_deletion_gates, response_envelope_policies,
    response_envelope_policy, rust_http_shell_ownership_matrix, DbWriterMode, DecisionRequired,
    ImportDeletionEvidence, ImportDeletionGate, MigrationBlockedStatus, MigrationState,
    ResponseEnvelopeFamily, RouteHandlerId,
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

    let receipt_recognition = find_endpoint_ownership("POST", "/api/ml/receipt-recognition")
        .expect("receipt OCR recognition route is governed");
    assert_eq!(receipt_recognition.state, MigrationState::PythonProxied);
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
        assert_eq!(endpoint.state, MigrationState::PythonProxied);
        assert!(endpoint.is_python_runtime_owner());
        assert!(!endpoint.is_import_deletion_blocked());
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
            "contract://sqlite-import-writer-policy"
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
}

#[test]
fn domain_db_invariant_ids_match_public_writer_policy_helpers() {
    for (domain, policy) in [
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
