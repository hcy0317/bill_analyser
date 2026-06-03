mod envelopes;
mod ownership;
mod policies;
mod types;
mod writer_policies;

pub use types::*;

use envelopes::ENVELOPE_POLICIES;
use ownership::OWNERSHIP_MATRIX;
use policies::{
    AUTH_TOKEN_DELETION_BLOCKERS, COVERAGE_EVIDENCE_CONTRACT, DB_RUNTIME_EVIDENCE,
    DOMAIN_GOVERNANCE_POLICIES, EMPTY_STRINGS, FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    TAXONOMY_DELETION_BLOCKERS,
};
use types::RouteContractDetails;
use writer_policies::{CRUD_DB_WRITE_INVARIANTS, IMPORT_DB_WRITE_INVARIANTS};

const IMPORT_DELETION_GATES: [ImportDeletionGate; 5] = [
    ImportDeletionGate::RustRouteRuntime,
    ImportDeletionGate::DbWriteSemantics,
    ImportDeletionGate::FrontendImportFlow,
    ImportDeletionGate::FullCoverage,
    ImportDeletionGate::NoResidualReferences,
];

const RUNTIME_STATE_MACHINE: [RuntimeState; 3] = [
    RuntimeState::RustImplemented,
    RuntimeState::RustOwnedVerified,
    RuntimeState::Retired,
];
const MANIFEST_STATES: [RuntimeState; 5] = [
    RuntimeState::RustImplemented,
    RuntimeState::RustOwnedVerified,
    RuntimeState::Retired,
    RuntimeState::ContractOnly,
    RuntimeState::Planned,
];

pub fn rust_http_shell_ownership_matrix() -> &'static [EndpointOwnership] {
    OWNERSHIP_MATRIX.as_slice()
}

pub fn runtime_state_machine() -> &'static [RuntimeState] {
    &RUNTIME_STATE_MACHINE
}

pub fn manifest_states() -> &'static [RuntimeState] {
    &MANIFEST_STATES
}

pub fn domain_governance_policies() -> &'static [DomainGovernancePolicy] {
    DOMAIN_GOVERNANCE_POLICIES
}

pub fn find_domain_policy(domain: &str) -> Option<&'static DomainGovernancePolicy> {
    DOMAIN_GOVERNANCE_POLICIES
        .iter()
        .find(|policy| policy.domain == domain)
}

fn route_handler_for_domain(domain: &str) -> RouteHandlerId {
    match domain {
        "api-runtime-shell" => RouteHandlerId::RouterBuildRouter,
        "bills-crud" => RouteHandlerId::BillsCrudRuntime,
        "bills-crud-adjacent" => RouteHandlerId::BillsCrudRuntime,
        "bills-recurring" => RouteHandlerId::BillsCrudRuntime,
        "bills-category-actions" => RouteHandlerId::BillsCrudRuntime,
        "bills-import" => RouteHandlerId::ImportDbRuntime,
        "ai-learning-llm" => RouteHandlerId::LlmLearningRuntimeBoundary,
        "ai-ocr" => RouteHandlerId::OcrRuntimeBoundary,
        "budgets-crud" => RouteHandlerId::BudgetsCrudRuntime,
        "budgets-analysis" => RouteHandlerId::BudgetsAnalysisRuntime,
        "budgets-history" => RouteHandlerId::BudgetsHistoryRuntime,
        "budgets-import" => RouteHandlerId::BudgetsImportRuntime,
        "statistics-read" => RouteHandlerId::StatisticsReadRuntime,
        "statistics-analyzer" => RouteHandlerId::StatisticsAnalyzerRuntime,
        "statistics-exchange" => RouteHandlerId::StatisticsExchangeRuntime,
        "taxonomy-rules-settings" => RouteHandlerId::TaxonomyRuntime,
        "auth-security-user-data" => RouteHandlerId::AuthTokenRuntime,
        "matching-recurring-calendar-networth" => {
            RouteHandlerId::MatchingRecurringCalendarNetworthRuntime
        }
        "backup-ops" => RouteHandlerId::BackupOpsRuntime,
        "database-schema" | "database-repositories" | "database-facade" => {
            RouteHandlerId::DatabaseFacadeContractOracle
        }
        _ => panic!("missing handler mapping for runtime governance domain {domain}"),
    }
}

fn route_contract_details(
    route: &EndpointOwnership,
    policy: &DomainGovernancePolicy,
) -> RouteContractDetails {
    match (route.domain, route.state) {
        ("ai-learning-llm", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::LlmLearningRuntimeBoundary,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
        },
        ("ai-ocr", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::OcrRuntimeBoundary,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
        },
        ("auth-security-user-data", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::AuthTokenRuntime,
            deletion_blockers: AUTH_TOKEN_DELETION_BLOCKERS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior:
                "Login, registration, token session list/revoke, API/MCP personal token generation, refresh token exchange, logout, account recovery email verification/resend/password forgot/reset, OAuth2 callback authorize disabled-safe/not-implemented response, profile, avatar, profile cloud settings, profile external-auth list/unlink, profile verification-email resend, system version, user-data statistics, user-data CSV/TSV export, destructive user-data clear, authenticated 2FA status, TOTP login verification, recovery-code login verification, 2FA write management, and step-up verification routes are Rust-owned; OAuth2 provider exchange is explicitly disabled-safe/not-implemented in the current workspace build.",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("taxonomy-rules-settings", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::TaxonomyRuntime,
            deletion_blockers: TAXONOMY_DELETION_BLOCKERS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior:
                "Account CRUD/display-order/balance sync/transaction move-clear, account-rule list/create/update/delete/reorder/test, tag CRUD/display-order/batch-create, category master-data/statistics/update-all, category-rule list/create/update/delete/reorder/defaults/test, rule overview, templates, settings bundle import/preview/export, and all removed taxonomy route shells have been removed.",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("bills-crud", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::BillsCrudRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("bills-crud-adjacent", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::BillsCrudRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("bills-recurring", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::BillsCrudRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("matching-recurring-calendar-networth", RuntimeState::Retired) => {
            RouteContractDetails {
                handler: RouteHandlerId::MatchingRecurringCalendarNetworthRuntime,
                deletion_blockers: EMPTY_STRINGS,
                blocked_status: RuntimeBlockedStatus::None,
                unsupported_behavior: "",
                decision_required: DecisionRequired::None,
                decision_owner: "none",
                transition_evidence: DB_RUNTIME_EVIDENCE,
            }
        }
        ("backup-ops", RuntimeState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::BackupOpsRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        _ => RouteContractDetails {
            handler: route_handler_for_domain(route.domain),
            deletion_blockers: policy.deletion_blockers,
            blocked_status: policy.blocked_status,
            unsupported_behavior: policy.unsupported_behavior,
            decision_required: policy.decision_required,
            decision_owner: policy.decision_owner,
            transition_evidence: policy.transition_evidence,
        },
    }
}

pub fn expanded_route_manifest() -> Vec<ExpandedRouteManifestEntry> {
    OWNERSHIP_MATRIX
        .iter()
        .map(|route| {
            let policy = find_domain_policy(route.domain)
                .unwrap_or_else(|| panic!("missing domain governance policy for {}", route.domain));
            let route_contract = route_contract_details(route, policy);
            ExpandedRouteManifestEntry {
                domain: route.domain,
                domain_policy_ref: route.domain,
                endpoint: format!("{} {}", route.method, route.pattern),
                method: route.method,
                pattern: route.pattern,
                state: route.state,
                handler: route_contract.handler,
                deletion_blockers: route_contract.deletion_blockers,
                blocked_status: route_contract.blocked_status,
                unsupported_behavior: route_contract.unsupported_behavior,
                decision_required: route_contract.decision_required,
                decision_owner: route_contract.decision_owner,
                transition_evidence: route_contract.transition_evidence,
                envelope: route.envelope,
                notes: route.notes,
            }
        })
        .collect()
}

pub fn governance_manifest_snapshot() -> GovernanceManifestSnapshot {
    GovernanceManifestSnapshot {
        runtime_state_machine: runtime_state_machine(),
        manifest_states: manifest_states(),
        coverage_evidence_contract: COVERAGE_EVIDENCE_CONTRACT,
        domains: domain_governance_policies(),
        routes: expanded_route_manifest(),
    }
}

pub fn contract_oracle() -> &'static str {
    "contract://runtime-governance-oracle"
}

pub fn routes_by_state(state: RuntimeState) -> Vec<&'static EndpointOwnership> {
    OWNERSHIP_MATRIX
        .iter()
        .filter(|endpoint| endpoint.state == state)
        .collect()
}

pub fn endpoints_by_owner(state: RuntimeState) -> Vec<&'static EndpointOwnership> {
    routes_by_state(state)
}

pub fn find_endpoint_ownership(method: &str, pattern: &str) -> Option<&'static EndpointOwnership> {
    OWNERSHIP_MATRIX.iter().find(|endpoint| {
        endpoint.pattern == pattern && method.eq_ignore_ascii_case(endpoint.method)
    })
}

pub fn import_deletion_gates() -> &'static [ImportDeletionGate] {
    &IMPORT_DELETION_GATES
}

pub fn can_delete_retired_import_paths(evidence: ImportDeletionEvidence) -> bool {
    missing_import_deletion_gates(evidence).is_empty()
}

pub fn missing_import_deletion_gates(evidence: ImportDeletionEvidence) -> Vec<ImportDeletionGate> {
    IMPORT_DELETION_GATES
        .iter()
        .copied()
        .filter(|gate| match gate {
            ImportDeletionGate::RustRouteRuntime => !evidence.rust_route_runtime,
            ImportDeletionGate::DbWriteSemantics => !evidence.db_write_semantics,
            ImportDeletionGate::FrontendImportFlow => !evidence.frontend_import_flow,
            ImportDeletionGate::FullCoverage => !evidence.full_coverage,
            ImportDeletionGate::NoResidualReferences => !evidence.no_residual_references,
        })
        .collect()
}

pub fn import_deletion_blocked_endpoints() -> Vec<&'static EndpointOwnership> {
    OWNERSHIP_MATRIX
        .iter()
        .filter(|endpoint| endpoint.is_import_deletion_blocked())
        .collect()
}

pub fn response_envelope_policies() -> &'static [ResponseEnvelopePolicy] {
    ENVELOPE_POLICIES
}

pub fn response_envelope_policy(
    family: ResponseEnvelopeFamily,
) -> Option<&'static ResponseEnvelopePolicy> {
    ENVELOPE_POLICIES
        .iter()
        .find(|policy| policy.family == family)
}

pub fn import_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "bills-import",
        mode: DbWriterMode::RustDomainOwned,
        active_writer: "src/backend/http/import_routes/mod.rs via Rust import_db_runtime",
        rust_write_allowed: true,
        invariants: &IMPORT_DB_WRITE_INVARIANTS,
    }
}

pub fn bills_crud_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "bills-crud",
        mode: DbWriterMode::RustDomainOwned,
        active_writer:
            "src/backend/http/bill_routes/ + src/backend/db/bills.rs via Rust bills_crud_runtime",
        rust_write_allowed: true,
        invariants: &CRUD_DB_WRITE_INVARIANTS,
    }
}

pub fn budgets_crud_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "budgets-crud",
        mode: DbWriterMode::RustDomainOwned,
        active_writer: "src/backend/http/budget_routes.rs + src/backend/db/budgets.rs via Rust budgets_crud_runtime",
        rust_write_allowed: true,
        invariants: &CRUD_DB_WRITE_INVARIANTS,
    }
}

pub fn database_schema_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "database-schema",
        mode: DbWriterMode::RustDomainOwned,
        active_writer:
            "src/backend/db/schema.rs::init_foundational_schema + init_auth_security_schema",
        rust_write_allowed: true,
        invariants: &CRUD_DB_WRITE_INVARIANTS,
    }
}
