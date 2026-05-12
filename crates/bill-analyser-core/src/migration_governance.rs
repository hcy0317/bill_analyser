//! Migration governance contracts for the Python-to-Rust backend replacement.
//!
//! These contracts are deliberately conservative: they encode the cutover state
//! machine, route/domain manifests, and the evidence required before a Python
//! path may be disabled or deleted.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationState {
    PythonProxied,
    RustImplemented,
    RustOwnedVerified,
    PythonDeleted,
    ContractOnly,
    Planned,
}

impl MigrationState {
    pub fn is_rust_runtime_state(self) -> bool {
        matches!(
            self,
            Self::RustImplemented | Self::RustOwnedVerified | Self::PythonDeleted
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MigrationBlockedStatus {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "blocked:user_choice_required")]
    BlockedUserChoiceRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionRequired {
    None,
    Port,
    Remove,
    Defer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteHandlerId {
    #[serde(rename = "crates/bill-analyser-http/src/router.rs::build_router")]
    RouterBuildRouter,
    #[serde(rename = "crates/bill-analyser-http/src/bill_routes.rs::bills_crud_runtime")]
    BillsCrudRuntime,
    #[serde(
        rename = "crates/bill-analyser-http/src/bill_routes.rs::python_proxy_passthrough_bills_adjacent"
    )]
    BillsAdjacentProxyPassthrough,
    #[serde(
        rename = "crates/bill-analyser-http/src/bill_routes.rs::python_proxy_passthrough_bills_recurring"
    )]
    BillsRecurringProxyPassthrough,
    #[serde(
        rename = "crates/bill-analyser-http/src/bill_routes.rs::python_proxy_passthrough_category_actions"
    )]
    BillsCategoryActionsProxyPassthrough,
    #[serde(rename = "crates/bill-analyser-http/src/import_routes.rs::import_db_runtime")]
    ImportDbRuntime,
    #[serde(
        rename = "crates/bill-analyser-http/src/import_routes.rs::llm_learning_runtime_boundary"
    )]
    LlmLearningRuntimeBoundary,
    #[serde(rename = "crates/bill-analyser-http/src/import_routes.rs::ocr_runtime_boundary")]
    OcrRuntimeBoundary,
    #[serde(rename = "crates/bill-analyser-http/src/budget_routes.rs::budgets_crud_runtime")]
    BudgetsCrudRuntime,
    #[serde(rename = "crates/bill-analyser-http/src/budget_routes.rs::budgets_analysis_runtime")]
    BudgetsAnalysisRuntime,
    #[serde(rename = "crates/bill-analyser-http/src/budget_routes.rs::budgets_history_runtime")]
    BudgetsHistoryRuntime,
    #[serde(rename = "crates/bill-analyser-http/src/budget_routes.rs::budgets_import_runtime")]
    BudgetsImportRuntime,
    #[serde(
        rename = "crates/bill-analyser-http/src/statistics_routes.rs::statistics_read_runtime"
    )]
    StatisticsReadRuntime,
    #[serde(
        rename = "crates/bill-analyser-http/src/statistics_routes.rs::python_proxy_passthrough_statistics_analyzer"
    )]
    StatisticsAnalyzerProxyPassthrough,
    #[serde(
        rename = "crates/bill-analyser-http/src/statistics_routes.rs::python_proxy_passthrough_statistics_exchange"
    )]
    StatisticsExchangeProxyPassthrough,
    #[serde(rename = "crates/bill-analyser-http/src/auth_routes.rs::auth_token_runtime")]
    AuthTokenRuntime,
    #[serde(rename = "crates/bill-analyser-http/src/proxy.rs::ownership_aware_proxy_handler")]
    LegacyPythonProxyPassthrough,
    #[serde(rename = "crates/bill-analyser-core/src/migration_governance.rs::contract_oracle")]
    DatabaseFacadeContractOracle,
}

impl RouteHandlerId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RouterBuildRouter => "crates/bill-analyser-http/src/router.rs::build_router",
            Self::BillsCrudRuntime => {
                "crates/bill-analyser-http/src/bill_routes.rs::bills_crud_runtime"
            }
            Self::BillsAdjacentProxyPassthrough => {
                "crates/bill-analyser-http/src/bill_routes.rs::python_proxy_passthrough_bills_adjacent"
            }
            Self::BillsRecurringProxyPassthrough => {
                "crates/bill-analyser-http/src/bill_routes.rs::python_proxy_passthrough_bills_recurring"
            }
            Self::BillsCategoryActionsProxyPassthrough => {
                "crates/bill-analyser-http/src/bill_routes.rs::python_proxy_passthrough_category_actions"
            }
            Self::ImportDbRuntime => {
                "crates/bill-analyser-http/src/import_routes.rs::import_db_runtime"
            }
            Self::LlmLearningRuntimeBoundary => {
                "crates/bill-analyser-http/src/import_routes.rs::llm_learning_runtime_boundary"
            }
            Self::OcrRuntimeBoundary => {
                "crates/bill-analyser-http/src/import_routes.rs::ocr_runtime_boundary"
            }
            Self::BudgetsCrudRuntime => {
                "crates/bill-analyser-http/src/budget_routes.rs::budgets_crud_runtime"
            }
            Self::BudgetsAnalysisRuntime => {
                "crates/bill-analyser-http/src/budget_routes.rs::budgets_analysis_runtime"
            }
            Self::BudgetsHistoryRuntime => {
                "crates/bill-analyser-http/src/budget_routes.rs::budgets_history_runtime"
            }
            Self::BudgetsImportRuntime => {
                "crates/bill-analyser-http/src/budget_routes.rs::budgets_import_runtime"
            }
            Self::StatisticsReadRuntime => {
                "crates/bill-analyser-http/src/statistics_routes.rs::statistics_read_runtime"
            }
            Self::StatisticsAnalyzerProxyPassthrough => {
                "crates/bill-analyser-http/src/statistics_routes.rs::python_proxy_passthrough_statistics_analyzer"
            }
            Self::StatisticsExchangeProxyPassthrough => {
                "crates/bill-analyser-http/src/statistics_routes.rs::python_proxy_passthrough_statistics_exchange"
            }
            Self::AuthTokenRuntime => {
                "crates/bill-analyser-http/src/auth_routes.rs::auth_token_runtime"
            }
            Self::LegacyPythonProxyPassthrough => {
                "crates/bill-analyser-http/src/proxy.rs::ownership_aware_proxy_handler"
            }
            Self::DatabaseFacadeContractOracle => {
                "crates/bill-analyser-core/src/migration_governance.rs::contract_oracle"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RouteContractDetails {
    handler: RouteHandlerId,
    deletion_blockers: &'static [&'static str],
    blocked_status: MigrationBlockedStatus,
    unsupported_behavior: &'static str,
    decision_required: DecisionRequired,
    decision_owner: &'static str,
    transition_evidence: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseEnvelopeFamily {
    RustHttpShell,
    ProxyInfrastructureError,
    FlaskSuccessData,
    FlaskSuccessResult,
    FlaskRawPassthrough,
    ImportV2Stage,
    ImportPreviewAction,
    ImportPreviewItemDecision,
    BillsCrud,
    BudgetsCrud,
    StatisticsRead,
    LearningRoute,
    LlmPreview,
    OcrMl,
    ContractOracle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointOwnership {
    pub method: &'static str,
    pub pattern: &'static str,
    pub domain: &'static str,
    pub state: MigrationState,
    pub envelope: ResponseEnvelopeFamily,
    pub deletion_blocked_until_all_import_gates: bool,
    pub notes: &'static str,
}

impl EndpointOwnership {
    pub fn is_import_deletion_blocked(&self) -> bool {
        self.deletion_blocked_until_all_import_gates
    }

    pub fn is_python_runtime_owner(&self) -> bool {
        self.state == MigrationState::PythonProxied
    }

    pub fn is_rust_owned_verified(&self) -> bool {
        self.state == MigrationState::RustOwnedVerified
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainGovernancePolicy {
    pub domain: &'static str,
    pub python_owner_files: &'static [&'static str],
    pub rust_owner_files: &'static [&'static str],
    pub tests_migrated: &'static [&'static str],
    pub fixtures: &'static [&'static str],
    pub db_invariant_ids: &'static [&'static str],
    pub coverage_evidence: &'static str,
    pub deletion_blockers: &'static [&'static str],
    pub blocked_status: MigrationBlockedStatus,
    pub unsupported_behavior: &'static str,
    pub decision_required: DecisionRequired,
    pub decision_owner: &'static str,
    pub transition_evidence: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExpandedRouteManifestEntry {
    pub domain: &'static str,
    pub domain_policy_ref: &'static str,
    pub endpoint: String,
    pub method: &'static str,
    pub pattern: &'static str,
    pub state: MigrationState,
    pub handler: RouteHandlerId,
    pub deletion_blockers: &'static [&'static str],
    pub blocked_status: MigrationBlockedStatus,
    pub unsupported_behavior: &'static str,
    pub decision_required: DecisionRequired,
    pub decision_owner: &'static str,
    pub transition_evidence: &'static [&'static str],
    pub envelope: ResponseEnvelopeFamily,
    pub notes: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GovernanceManifestSnapshot {
    pub cutover_state_machine: &'static [MigrationState],
    pub manifest_states: &'static [MigrationState],
    pub coverage_evidence_contract: &'static str,
    pub domains: &'static [DomainGovernancePolicy],
    pub routes: Vec<ExpandedRouteManifestEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseEnvelopePolicy {
    pub family: ResponseEnvelopeFamily,
    pub route_contexts: &'static [MigrationState],
    pub success_shape: &'static str,
    pub error_shape: &'static str,
    pub proxy_may_wrap: bool,
}

impl ResponseEnvelopePolicy {
    pub fn applies_to_owner(&self, state: MigrationState) -> bool {
        self.route_contexts.contains(&state)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDeletionGate {
    RustRouteRuntime,
    DbWriteSemantics,
    FrontendImportFlow,
    FullCoverage,
    NoResidualReferences,
}

impl ImportDeletionGate {
    pub fn label(self) -> &'static str {
        match self {
            Self::RustRouteRuntime => "rust_route_runtime",
            Self::DbWriteSemantics => "db_write_semantics",
            Self::FrontendImportFlow => "frontend_import_flow",
            Self::FullCoverage => "full_coverage",
            Self::NoResidualReferences => "no_residual_references",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportDeletionEvidence {
    pub rust_route_runtime: bool,
    pub db_write_semantics: bool,
    pub frontend_import_flow: bool,
    pub full_coverage: bool,
    pub no_residual_references: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DbWriterMode {
    PythonPrimary,
    RustDomainOwned,
    RustPrimary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbWriteInvariant {
    pub key: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbWriterPolicy {
    pub domain: &'static str,
    pub mode: DbWriterMode,
    pub active_writer: &'static str,
    pub rust_write_allowed: bool,
    pub invariants: &'static [DbWriteInvariant],
}

impl DbWriterPolicy {
    pub fn requires_invariant(&self, key: &str) -> bool {
        self.invariants.iter().any(|invariant| invariant.key == key)
    }
}

const IMPORT_DELETION_GATES: [ImportDeletionGate; 5] = [
    ImportDeletionGate::RustRouteRuntime,
    ImportDeletionGate::DbWriteSemantics,
    ImportDeletionGate::FrontendImportFlow,
    ImportDeletionGate::FullCoverage,
    ImportDeletionGate::NoResidualReferences,
];

const CUTOVER_STATE_MACHINE: [MigrationState; 4] = [
    MigrationState::PythonProxied,
    MigrationState::RustImplemented,
    MigrationState::RustOwnedVerified,
    MigrationState::PythonDeleted,
];
const MANIFEST_STATES: [MigrationState; 6] = [
    MigrationState::PythonProxied,
    MigrationState::RustImplemented,
    MigrationState::RustOwnedVerified,
    MigrationState::PythonDeleted,
    MigrationState::ContractOnly,
    MigrationState::Planned,
];

macro_rules! python_proxy_route {
    ($method:literal, $pattern:literal, $domain:literal) => {
        python_proxy_route!(
            $method,
            $pattern,
            $domain,
            ResponseEnvelopeFamily::FlaskSuccessResult
        )
    };
    ($method:literal, $pattern:literal, $domain:literal, $envelope:expr) => {
        EndpointOwnership {
            method: $method,
            pattern: $pattern,
            domain: $domain,
            state: MigrationState::PythonProxied,
            envelope: $envelope,
            deletion_blocked_until_all_import_gates: false,
            notes: "Live Python sidecar route remains explicitly proxied until its functional domain is ported.",
        }
    };
}

const OWNERSHIP_MATRIX: &[EndpointOwnership] = &[
    EndpointOwnership {
        method: "GET",
        pattern: "/api/health",
        domain: "api-runtime-shell",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::RustHttpShell,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust HTTP shell health route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/runtime",
        domain: "api-runtime-shell",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::RustHttpShell,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust HTTP shell metadata route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns core list route; Python CRUD deletion remains blocked by the S9e residual-reference review, not by the import skeleton registry.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns trailing-slash list route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns core create route with cents-to-yuan adapter semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns trailing-slash create route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/by-month",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns month list route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/get",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns legacy get-by-query route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST get route.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST update route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST delete route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/modify",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns legacy modify route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/delete",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns legacy delete route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/batch",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch create route with a single SQLite transaction.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/batch/update",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch update route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/batch/delete",
        domain: "bills-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch delete route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/export",
        domain: "bills-crud-adjacent",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskRawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills export runtime owns CSV/XLSX file responses, legacy filenames, BOM CSV output, empty-result errors, and formula-like text cell escaping.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/pictures",
        domain: "bills-crud-adjacent",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills picture runtime owns multipart transaction picture upload and data URL response semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/pictures/unused",
        domain: "bills-crud-adjacent",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills picture runtime owns unused transaction picture cleanup.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/reconciliation_statements",
        domain: "bills-crud-adjacent",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Reconciliation statements remain Python-proxied until reconciliation runtime migrates.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/{bill_id}/recurring-candidates",
        domain: "bills-recurring",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills recurring runtime owns formal bill recurring candidate generation.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/{bill_id}/recurring-match",
        domain: "bills-recurring",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills recurring runtime owns recurring template binding and next-date recalculation.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/{bill_id}/recurring-match",
        domain: "bills-recurring",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills recurring runtime owns recurring template unbinding and next-date recalculation.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/category/quick-add-keyword",
        domain: "bills-category-actions",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Category quick keyword helper remains Python-proxied until matching/category-rule runtime migrates.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/category/refresh",
        domain: "bills-category-actions",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Category refresh helper remains Python-proxied until matching/category-rule runtime migrates.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns core list route; execution, forecast, history, and import are Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns trailing-slash list route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns core create route with yuan-style budget amount semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns trailing-slash create route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST get route.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST update route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST delete route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/export",
        domain: "budgets-crud",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns JSON export for budget rows.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/execution",
        domain: "budgets-analysis",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_execution_runtime owns budget execution aggregation.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/forecast",
        domain: "budgets-analysis",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_forecast_runtime owns historical bill aggregation, budget-map projection, forecast summary, and period-progress shape.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/history",
        domain: "budgets-history",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_history_runtime owns persisted snapshot lookup, exact-period preference, category enrichment, and on-demand fallback.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/history/snapshot",
        domain: "budgets-history",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_history_runtime owns budget_history replacement writes with canonical filter_summary and user scope.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/import",
        domain: "budgets-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_import_runtime owns Flask-compatible array validation, per-item error accounting, and user-scoped upsert-by-name writes.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-statistics",
        domain: "statistics-read",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns DB-backed category/account cents aggregation with timestamp range and keyword filtering.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-statistics/trends",
        domain: "statistics-read",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns monthly category/account trend buckets for bounded and all-mode ranges.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/asset-trends",
        domain: "statistics-read",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns DB-backed daily asset trend balances with 365-day bounded-range guard.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-pie",
        domain: "statistics-read",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns category pie aggregation for bill type/date filters.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/top-merchants",
        domain: "statistics-read",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns top merchant aggregation for date filters.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/amounts",
        domain: "statistics-read",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns transaction amount period aggregation with CNY cents response semantics.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/overview",
        domain: "statistics-analyzer",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer overview remains Python-proxied until the full Analyzer report runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/trends",
        domain: "statistics-analyzer",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer trends remain Python-proxied until the full Analyzer trend runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/comparison",
        domain: "statistics-analyzer",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer comparison remains Python-proxied until the full Analyzer comparison runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category",
        domain: "statistics-analyzer",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer category route remains Python-proxied until the full Analyzer category runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/trend",
        domain: "statistics-analyzer",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer trend alias remains Python-proxied until the full Analyzer trend runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/exchange-rates",
        domain: "statistics-exchange",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Live exchange provider fetch and custom-rate precedence remain Python-proxied until Rust owns provider execution and custom-rate persistence.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/statistics/exchange-rates/custom",
        domain: "statistics-exchange",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Custom exchange-rate writes remain Python-proxied until Rust owns provider execution and custom-rate persistence together.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/statistics/exchange-rates/custom/{currency}",
        domain: "statistics-exchange",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Custom exchange-rate deletion remains Python-proxied until Rust owns provider execution and custom-rate persistence together.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/parse",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/parse_generic",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/dedup",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/confirm",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/session/{session_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/v2/session/{session_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview/{session_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview/{session_id}/index",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview/{session_id}/update",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/reclassify/{session_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-candidates",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/learning/{session_id}/suggestions",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this preview-bypass route; it applies supplied preview updates and returns an explicit empty suggestion list without provider/model execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/learning/{session_id}/suggestions",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this preview-bypass route; it validates the session and returns an explicit empty suggestion list without provider/model execution.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/learning/{session_id}/promote",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this DB-write decision route and promotes stored session suggestions into import learning rules.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/preview",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/confirm",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/batch",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/parse_import",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/upload",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/parsers",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/reclassify",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/configs",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs/match",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs/suggest",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/configs/{config_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/learning-rules",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/learning-rules/{rule_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/learning-rules/{rule_id}",
        domain: "bills-import",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "True LLM preview recommendation generation is proxied to Python until Rust owns provider execution; Rust keeps accept/reject/memory preview decisions.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend/accept",
        domain: "ai-learning-llm",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend/reject",
        domain: "ai-learning-llm",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/memory",
        domain: "ai-learning-llm",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/analyze-transactions",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "True LLM transaction analysis and import-session rule induction are proxied to Python until Rust owns provider execution.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/rule-synthesis",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "True rule-synthesis provider generation is proxied to Python until Rust owns provider execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/learning/suggestions",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center listing remains proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/generate",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Learning suggestion mining/generation remains proxied to Python until Rust owns the learning model loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/{suggestion_id}/accept",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center decisions remain proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/batch-accept",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center decisions remain proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/{suggestion_id}/reject",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center decisions remain proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/learning/rules",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/learning/rules/{rule_id}/toggle",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/learning/rules/{rule_id}",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/learning/rules/{rule_id}",
        domain: "ai-learning-llm",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/ml/receipt-recognition",
        domain: "ai-ocr",
        state: MigrationState::PythonProxied,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: false,
        notes: "Receipt image recognition is deliberately proxied to Python until Rust owns real OCR provider execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/ml/receipt-recognition/config",
        domain: "ai-ocr",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns OCR config; true receipt recognition remains Python-proxied.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/ml/receipt-recognition/config",
        domain: "ai-ocr",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns OCR config; true receipt recognition remains Python-proxied.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/enable/request",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime generates Flask-compatible TOTP setup secrets and QR-code PNG data URLs for authenticated users.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/enable/confirm",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime validates TOTP setup passcodes, atomically enables 2FA with recovery codes plus a new session, and records 2fa_enabled audit details.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/disable",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime accepts the current password, operation password fallback, or step-up action tokens; atomically disables 2FA, clears recovery codes, and records 2fa_disabled audit details.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/recovery/regenerate",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime accepts the current password, operation password fallback, or step-up action tokens; replaces recovery codes and records 2fa_recovery_regenerated audit details.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/2fa/status",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime owns the authenticated 2FA status read route and the companion 2FA write management routes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/verify",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime verifies pending_2fa action tokens plus TOTP passcodes, creates the access/refresh session, and logs login_2fa_success.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/recovery/verify",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime verifies pending_2fa action tokens plus one-time recovery codes, creates the access/refresh session, logs login_2fa_recovery_success, and records a best-effort 2fa_recovery_code_used audit log when audit_logs exists.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/security/step-up/verify",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime verifies the current password, configured operation password fallback, or 2FA TOTP passcode and issues a one-hour step_up action token for sensitive operations.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/data/export.{file_type}",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskRawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime exports authenticated user bills as CSV/TSV with the legacy filename, BOM, account-name, tag-name, and filter semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/data/clear/transactions",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime clears current-user bills and bill side effects after current password, configured operation password fallback, or step-up action-token verification, then records user_data audit metadata.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/data/clear/all",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime clears current-user business data after current password, configured operation password fallback, or step-up action-token verification, preserving legacy count keys and user_data audit metadata.",
    },
    python_proxy_route!("GET", "/api/accounts/", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/accounts/", "taxonomy-rules-settings"),
    python_proxy_route!(
        "DELETE",
        "/api/accounts/{account_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "GET",
        "/api/accounts/{account_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "PUT",
        "/api/accounts/{account_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/accounts/{account_id}/transactions/clear",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/accounts/{account_id}/transactions/move",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "PUT",
        "/api/accounts/display-orders",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/accounts/sync-balances",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!("GET", "/api/backup/", "backup-ops"),
    python_proxy_route!("POST", "/api/backup/cleanup", "backup-ops"),
    python_proxy_route!("POST", "/api/backup/create", "backup-ops"),
    python_proxy_route!("DELETE", "/api/backup/delete/{filename}", "backup-ops"),
    python_proxy_route!(
        "GET",
        "/api/backup/download/{filename}",
        "backup-ops",
        ResponseEnvelopeFamily::FlaskRawPassthrough
    ),
    python_proxy_route!("GET", "/api/backup/jobs", "backup-ops"),
    python_proxy_route!("POST", "/api/backup/jobs", "backup-ops"),
    python_proxy_route!("POST", "/api/backup/restore/{filename}", "backup-ops"),
    python_proxy_route!("POST", "/api/backup/restore/verify", "backup-ops"),
    python_proxy_route!(
        "GET",
        "/api/calendar/events",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!("GET", "/api/categories/", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/categories/", "taxonomy-rules-settings"),
    python_proxy_route!(
        "DELETE",
        "/api/categories/{category_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "GET",
        "/api/categories/{category_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "PUT",
        "/api/categories/{category_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!("GET", "/api/categories/all", "taxonomy-rules-settings"),
    python_proxy_route!("PUT", "/api/categories/all", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/categories/batch", "taxonomy-rules-settings"),
    python_proxy_route!("GET", "/api/categories/export", "taxonomy-rules-settings"),
    python_proxy_route!("GET", "/api/categories/flat", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/categories/import", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/categories/move", "taxonomy-rules-settings"),
    python_proxy_route!("GET", "/api/categories/rules", "taxonomy-rules-settings"),
    python_proxy_route!("PUT", "/api/categories/rules", "taxonomy-rules-settings"),
    python_proxy_route!(
        "GET",
        "/api/categories/statistics",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!("GET", "/api/categories/tree", "taxonomy-rules-settings"),
    python_proxy_route!(
        "POST",
        "/api/categories/update-all",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!("GET", "/api/category-rules/", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/category-rules/", "taxonomy-rules-settings"),
    python_proxy_route!(
        "DELETE",
        "/api/category-rules/{rule_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "PUT",
        "/api/category-rules/{rule_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/category-rules/{rule_id}/test",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/category-rules/defaults",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/category-rules/migrate",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/category-rules/reorder",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!("GET", "/api/insights/anomalies", "statistics-analyzer"),
    python_proxy_route!("GET", "/api/llm/candidates", "ai-learning-llm"),
    python_proxy_route!("GET", "/api/llm/candidates/{candidate_id}", "ai-learning-llm"),
    python_proxy_route!(
        "POST",
        "/api/llm/candidates/{candidate_id}/accept",
        "ai-learning-llm"
    ),
    python_proxy_route!(
        "POST",
        "/api/llm/candidates/{candidate_id}/reject",
        "ai-learning-llm"
    ),
    python_proxy_route!("GET", "/api/llm/config", "ai-learning-llm"),
    python_proxy_route!("POST", "/api/llm/config", "ai-learning-llm"),
    python_proxy_route!("GET", "/api/llm/configs", "ai-learning-llm"),
    python_proxy_route!("POST", "/api/llm/configs", "ai-learning-llm"),
    python_proxy_route!(
        "DELETE",
        "/api/llm/configs/{config_id}",
        "ai-learning-llm"
    ),
    python_proxy_route!("PUT", "/api/llm/configs/{config_id}", "ai-learning-llm"),
    python_proxy_route!(
        "POST",
        "/api/llm/configs/{config_id}/activate",
        "ai-learning-llm"
    ),
    python_proxy_route!("POST", "/api/llm/induce-rules", "ai-learning-llm"),
    python_proxy_route!(
        "GET",
        "/api/matching/bills/{bill_id}/candidates",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/matching/bills/{bill_id}/feedback",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/matching/candidates",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/matching/candidates/{*candidate_id}/accept",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/matching/candidates/{*candidate_id}/clear",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/matching/candidates/{*candidate_id}/reject",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/matching/investment-settings",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "PUT",
        "/api/matching/investment-settings",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/matching/manual-pair",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/matching/pairs",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "DELETE",
        "/api/matching/pairs/{pair_id}",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/matching/reconcile-history",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/matching/reconciliation-candidates",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/matching/sessions/{session_id}/candidates",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/networth/snapshot",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "GET",
        "/api/recurring/suggestions",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/recurring/suggestions/{suggestion_id}/accept",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/recurring/suggestions/{suggestion_id}/reject",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!(
        "POST",
        "/api/recurring/suggestions/detect",
        "matching-recurring-calendar-networth"
    ),
    python_proxy_route!("GET", "/api/rules/overview", "taxonomy-rules-settings"),
    python_proxy_route!(
        "GET",
        "/api/settings/bundle/export",
        "taxonomy-rules-settings",
        ResponseEnvelopeFamily::FlaskRawPassthrough
    ),
    python_proxy_route!(
        "POST",
        "/api/settings/bundle/import",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/settings/bundle/import/preview",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "GET",
        "/api/settings/bundle/sections/{section_key}/export",
        "taxonomy-rules-settings",
        ResponseEnvelopeFamily::FlaskRawPassthrough
    ),
    python_proxy_route!(
        "POST",
        "/api/settings/bundle/sections/{section_key}/export",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/settings/bundle/sections/{section_key}/import",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "POST",
        "/api/settings/bundle/sections/{section_key}/import/preview",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "GET",
        "/api/settings/encryption/status",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!("GET", "/api/tags/", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/tags/", "taxonomy-rules-settings"),
    python_proxy_route!("DELETE", "/api/tags/{tag_id}", "taxonomy-rules-settings"),
    python_proxy_route!("GET", "/api/tags/{tag_id}", "taxonomy-rules-settings"),
    python_proxy_route!("PUT", "/api/tags/{tag_id}", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/tags/batch", "taxonomy-rules-settings"),
    python_proxy_route!("PUT", "/api/tags/display-orders", "taxonomy-rules-settings"),
    python_proxy_route!("GET", "/api/templates/", "taxonomy-rules-settings"),
    python_proxy_route!("POST", "/api/templates/", "taxonomy-rules-settings"),
    python_proxy_route!(
        "DELETE",
        "/api/templates/{template_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "GET",
        "/api/templates/{template_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "PUT",
        "/api/templates/{template_id}",
        "taxonomy-rules-settings"
    ),
    python_proxy_route!(
        "PUT",
        "/api/templates/display-orders",
        "taxonomy-rules-settings"
    ),
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/login",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies username/email password login, handles lockout/inactive/2FA-pending branches, writes sessions and auth logs, and returns Flask-compatible tokens, profile, and application cloud settings.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/register",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime validates registration input/password policy, creates the user, default categories/rules/accounts, and auth log transactionally, and returns the Flask-compatible registration result.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/logout",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime invalidates the bearer session by token hash, records logout auth logs for active sessions, and keeps Flask-compatible idempotent success.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/email/verify",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime validates verify_email action tokens, marks email_verified, optionally issues a new session token, writes email_verified auth logs, and returns result.user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/email/resend-verification",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime verifies email/password credentials, issues mock-success verification tokens, records verification_email_resend_requested metadata, and returns result=true.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/password/forgot",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime honors the forget-password feature flag, returns success for unknown emails, and records mock-success reset token metadata for existing users.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/password/reset",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime validates reset_password action tokens, password policy, and email/user match before updating the password hash and writing password_reset_completed auth logs.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/oauth2/authorize",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime owns the OAuth2 callback authorize disabled-safe/not-implemented response contract; real provider exchange remains deferred.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tokens",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime revokes all other token sessions while preserving current bearer session semantics.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/tokens",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime lists active token sessions with Flask-compatible success/result envelope.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tokens/{token_id}",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime revokes one token session by id with user-scope validation.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/api",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies the current password and issues API personal access tokens with session and auth-log writes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/mcp",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies the current password and issues MCP personal access tokens with session and auth-log writes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/refresh",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies refresh token JWT/session state and issues rotated access/refresh sessions with profile/cloud settings response.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime returns the authenticated user's Flask-compatible profile payload.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/profile",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime updates whitelisted user profile/display/investment-keyword fields, validates scoped account/category references, resets email verification on email changes, and returns result.user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/avatar",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime accepts validated PNG/JPEG/GIF/WebP multipart avatar uploads, stores a data URL, and returns the updated profile.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/profile/avatar",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime clears the avatar field and returns the updated profile.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/email/resend-verification",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime rate-limits and records the mock-success verification email resend auth log, then returns result=true.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime lists application cloud settings and preserves the empty false response.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime validates supported application cloud setting keys/types and upserts full or partial updates.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime deletes all authenticated-user cloud settings and returns result=true.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile/external-auths",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime lists user-scoped external-auth bindings, preserves createdAt millisecond projection, and appends the configured OAuth2 unlinked placeholder.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/external-auths/unlink",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime verifies the current password before deleting a user-scoped external-auth binding and writing external_auth_unlinked audit metadata.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/system/version",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime serves the unauthenticated system version metadata route with the legacy payload shape.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/data/statistics",
        domain: "auth-security-user-data",
        state: MigrationState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth user-data runtime returns authenticated user-scoped bill/account/category/tag/template counts with the Flask-compatible success/result envelope.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://import-v2-envelope-oracle",
        domain: "bills-import",
        state: MigrationState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins import envelope families for Rust-owned import_db_runtime routes.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://sqlite-import-writer-policy",
        domain: "database-facade",
        state: MigrationState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins DB writer invariants for import_db_runtime takeover.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://sqlite-foundational-schema-policy",
        domain: "database-schema",
        state: MigrationState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust DB crate owns foundational schema initialization and legacy user-scoped constraint migrations without deleting the Python facade.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://sqlite-repository-policy",
        domain: "database-repositories",
        state: MigrationState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust DB crate records the active repository surfaces for import staging, bills, budgets, statistics, app settings, and taxonomy while deletion remains per business domain.",
    },
];

const COVERAGE_EVIDENCE_CONTRACT: &str = "workspace.lcov";
const EMPTY_STRINGS: &[&str] = &[];
const SQLITE_DB_INVARIANT_IDS: &[&str] = &[
    "wal_mode",
    "foreign_keys",
    "rollback_on_error",
    "positive_user_scope",
    "amount_units",
    "time_normalization",
];
const IMPORT_DB_INVARIANT_IDS: &[&str] = &[
    "wal_mode",
    "foreign_keys",
    "single_writer",
    "transactional_staging",
    "rollback_on_error",
    "positive_user_scope",
    "amount_units",
    "time_normalization",
];
const ROUTE_MATRIX_ONLY_EVIDENCE: &[&str] = &["route_matrix"];
const RUNTIME_METADATA_EVIDENCE: &[&str] = &["route_matrix", "runtime_metadata"];
const DB_RUNTIME_EVIDENCE: &[&str] = &["route_matrix", "db_smoke"];
const DB_SCHEMA_EVIDENCE: &[&str] = &["db_smoke", "schema_migration_contract"];
const DB_REPOSITORY_EVIDENCE: &[&str] = &["db_smoke", "repository_contract"];
const FRONTEND_DB_EVIDENCE: &[&str] = &["route_matrix", "db_smoke", "frontend_contract"];
const FULL_ROUTE_EVIDENCE: &[&str] = &[
    "route_matrix",
    "golden_fixture",
    "db_smoke",
    "frontend_contract",
];
const FULL_ROUTE_EVIDENCE_NO_FIXTURE: &[&str] = &["route_matrix", "db_smoke", "frontend_contract"];
const PROVIDER_ROUTE_EVIDENCE: &[&str] = &["route_matrix", "provider_parity"];
const AUTH_TOKEN_DELETION_BLOCKERS: &[&str] = &[
    "login_registration_parity",
    "profile_user_data_parity",
    "2fa_oauth_parity",
];
const IMPORT_DELETION_BLOCKERS: &[&str] = &[
    "rust_route_runtime",
    "db_write_semantics",
    "frontend_import_flow",
    "full_coverage",
    "no_residual_references",
];

const DOMAIN_GOVERNANCE_POLICIES: &[DomainGovernancePolicy] = &[
    DomainGovernancePolicy {
        domain: "api-runtime-shell",
        python_owner_files: &[
            "src/bill_analyser/api/app.py",
            "src/bill_analyser/api/routes/request_context_helpers.py",
            "src/bill_analyser/utils/config.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/router.rs",
            "crates/bill-analyser-http/src/server.rs",
            "crates/bill-analyser-http/src/runtime.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/proxy_contract.rs",
            "tests/domains/runtime/unit/test_rust_runtime_workspace.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["proxy_fallback"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Python sidecar startup plus catch-all fallback proxy remain part of the runtime shell until terminal cutover.",
        decision_required: DecisionRequired::Defer,
        decision_owner: "migration-program",
        transition_evidence: RUNTIME_METADATA_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-import",
        python_owner_files: &[
            "src/bill_analyser/api/routes/bills/v2_pipeline.py",
            "src/bill_analyser/api/routes/bills/v2_preview_actions.py",
            "src/bill_analyser/api/routes/bills/import_learning.py",
            "src/bill_analyser/core/bills/service_parts/import_v2_pipeline.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/import_routes.rs",
            "crates/bill-analyser-db/src/import_staging.rs",
            "crates/bill-analyser-core/src/import_pipeline.rs",
            "crates/bill-analyser-core/src/parsers.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/import_runtime_contract.rs",
            "crates/bill-analyser-db/tests/import_staging.rs",
            "tests/domains/import_flow/unit/test_bills_route_branches.py",
        ],
        fixtures: &["crates/bill-analyser-core/tests/fixtures/parser_golden_contracts.json"],
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: IMPORT_DELETION_BLOCKERS,
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "ai-learning-llm",
        python_owner_files: &[
            "src/bill_analyser/api/routes/llm/preview.py",
            "src/bill_analyser/api/routes/llm/analysis.py",
            "src/bill_analyser/api/routes/learning.py",
            "src/bill_analyser/core/ai/llm/provider.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/import_routes.rs",
            "crates/bill-analyser-core/src/ai_ocr_llm.rs",
            "crates/bill-analyser-core/src/import_learning.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-core/tests/ai_ocr_llm_contracts.rs",
            "crates/bill-analyser-core/tests/import_learning_contracts.rs",
            "tests/new_ui/test_llm_import_session_analysis_api.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["provider_execution_parity", "learning_center_rule_loop"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Live provider-backed preview recommendation generation, transaction analysis, rule synthesis, and the global Learning Center suggestion/rule loop remain Python-owned.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: PROVIDER_ROUTE_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "ai-ocr",
        python_owner_files: &[
            "src/bill_analyser/api/routes/receipt_ocr.py",
            "src/bill_analyser/core/ai/ocr/provider.py",
            "src/bill_analyser/core/ai/ocr/payment_screenshot_parser.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/import_routes.rs",
            "crates/bill-analyser-core/src/ai_ocr_llm.rs",
            "crates/bill-analyser-db/src/app_settings.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-core/tests/ai_ocr_llm_contracts.rs",
            "crates/bill-analyser-db/tests/app_settings.rs",
            "tests/test_ocr_service.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["provider_execution_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Receipt image recognition provider execution remains Python-owned even though OCR config persistence is Rust-owned.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: PROVIDER_ROUTE_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-crud",
        python_owner_files: &[
            "src/bill_analyser/api/routes/bills/crud_query.py",
            "src/bill_analyser/api/routes/bills/crud_create_update.py",
            "src/bill_analyser/api/routes/bills/crud_prepare.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/bill_routes.rs",
            "crates/bill-analyser-db/src/bills.rs",
            "crates/bill-analyser-core/src/adapters/transaction.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/bills_runtime_contract.rs",
            "crates/bill-analyser-db/tests/bills_runtime.rs",
            "tests/domains/import_flow/unit/test_bills_route_branches.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["no_residual_references"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FRONTEND_DB_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-crud-adjacent",
        python_owner_files: &[
            "src/bill_analyser/api/routes/bills/reconciliation.py",
            "src/bill_analyser/api/routes/bills/category_actions.py",
            "src/bill_analyser/api/routes/bills/support.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/bill_routes.rs",
            "crates/bill-analyser-core/src/matching.rs",
            "crates/bill-analyser-core/src/ops.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/bills_runtime_contract.rs",
            "crates/bill-analyser-core/tests/matching_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["reconciliation_contract"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Reconciliation statement generation remains Python-proxied.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-recurring",
        python_owner_files: &[
            "src/bill_analyser/api/routes/bills/crud_query.py",
            "src/bill_analyser/core/recurring_detection.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/bill_routes.rs",
            "crates/bill-analyser-db/src/bills.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/bills_runtime_contract.rs",
            "crates/bill-analyser-core/tests/migration_governance_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-category-actions",
        python_owner_files: &[
            "src/bill_analyser/api/routes/bills/category_actions.py",
            "src/bill_analyser/core/category_engine/matcher.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/bill_routes.rs",
            "crates/bill-analyser-core/src/category_rules/mod.rs",
            "crates/bill-analyser-core/src/matching.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/bills_runtime_contract.rs",
            "tests/domains/categories/unit/test_category_rule_rust_bridge.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["category_action_helper_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "Bill category action helper routes remain Python-proxied.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: FRONTEND_DB_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-crud",
        python_owner_files: &[
            "src/bill_analyser/api/routes/budgets/crud.py",
            "src/bill_analyser/api/routes/budgets/support.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/budget_routes.rs",
            "crates/bill-analyser-db/src/budgets.rs",
            "crates/bill-analyser-core/src/budgets.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/budget_runtime_contract.rs",
            "crates/bill-analyser-db/tests/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["no_residual_references"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FRONTEND_DB_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-analysis",
        python_owner_files: &[
            "src/bill_analyser/api/routes/budgets/execution.py",
            "src/bill_analyser/api/routes/budgets/support.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/budget_routes.rs",
            "crates/bill-analyser-db/src/budgets.rs",
            "crates/bill-analyser-core/src/budgets.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/budget_runtime_contract.rs",
            "crates/bill-analyser-db/tests/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["no_residual_references"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-history",
        python_owner_files: &[
            "src/bill_analyser/api/routes/budgets/history.py",
            "src/bill_analyser/api/routes/budgets/support.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/budget_routes.rs",
            "crates/bill-analyser-db/src/budgets.rs",
            "crates/bill-analyser-core/src/budgets.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/budget_runtime_contract.rs",
            "crates/bill-analyser-db/tests/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["no_residual_references"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-import",
        python_owner_files: &[
            "src/bill_analyser/api/routes/budgets/io.py",
            "src/bill_analyser/api/routes/budgets/support.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/budget_routes.rs",
            "crates/bill-analyser-db/src/budgets.rs",
            "crates/bill-analyser-core/src/budgets.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/budget_runtime_contract.rs",
            "crates/bill-analyser-db/tests/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["no_residual_references"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "statistics-read",
        python_owner_files: &[
            "src/bill_analyser/api/routes/statistics/basic.py",
            "src/bill_analyser/api/routes/statistics/asset_trends.py",
            "src/bill_analyser/core/analyzer.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/statistics_routes.rs",
            "crates/bill-analyser-db/src/statistics.rs",
            "crates/bill-analyser-core/src/statistics.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/statistics_runtime_contract.rs",
            "crates/bill-analyser-core/tests/statistics_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["analyzer_runtime_followup"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "statistics-analyzer",
        python_owner_files: &[
            "src/bill_analyser/api/routes/insights.py",
            "src/bill_analyser/api/routes/networth.py",
            "src/bill_analyser/api/routes/statistics/trend_analysis.py",
            "src/bill_analyser/api/routes/statistics/category_analysis.py",
            "src/bill_analyser/core/analyzer.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/statistics_routes.rs",
            "crates/bill-analyser-core/src/statistics.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/statistics_runtime_contract.rs",
            "crates/bill-analyser-core/tests/statistics_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["overview_parity", "trend_projection_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Overview, trends, comparison, category, and trend analyzer endpoints remain Python-proxied.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "statistics-exchange",
        python_owner_files: &[
            "src/bill_analyser/api/routes/statistics/exchange_rates.py",
            "src/bill_analyser/core/exchange_rate_providers/manager.py",
            "src/bill_analyser/core/exchange_rate_providers/global_providers.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/statistics_routes.rs",
            "crates/bill-analyser-core/src/statistics.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/statistics_runtime_contract.rs",
            "crates/bill-analyser-core/tests/statistics_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["exchange_provider_parity", "custom_rate_write_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Live exchange-rate provider resolution and custom-rate write/delete routes remain Python-proxied.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: PROVIDER_ROUTE_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "auth-security-user-data",
        python_owner_files: &[
            "src/bill_analyser/api/routes/auth",
            "src/bill_analyser/core/security",
            "src/bill_analyser/core/user_data.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/auth_routes.rs",
            "crates/bill-analyser-db/src/auth.rs",
            "crates/bill-analyser-db/src/auth_registration.rs",
            "crates/bill-analyser-db/src/user_data.rs",
            "crates/bill-analyser-core/src/auth/mod.rs",
            "crates/bill-analyser-http/src/proxy.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-http/tests/auth_runtime_contract.rs",
            "crates/bill-analyser-core/tests/auth_security_contracts.rs",
            "crates/bill-analyser-core/tests/ops_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["auth_session_parity", "profile_user_data_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Login, registration, token session list/revoke, API/MCP personal token generation, refresh token exchange, logout, account recovery email verification/resend/password forgot/reset, OAuth2 callback authorize disabled-safe/not-implemented response, profile, avatar, profile cloud settings, profile external-auth list/unlink, profile verification-email resend, system version, user-data statistics, user-data CSV/TSV export, destructive user-data clear, authenticated 2FA status, TOTP login verification, recovery-code login verification, 2FA write management, and step-up verification routes are Rust-owned; real OAuth provider exchange remains Python-proxied until a later P3 cutover.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "taxonomy-rules-settings",
        python_owner_files: &[
            "src/bill_analyser/api/routes/accounts",
            "src/bill_analyser/api/routes/categories",
            "src/bill_analyser/api/routes/category_rules.py",
            "src/bill_analyser/api/routes/tags.py",
            "src/bill_analyser/api/routes/templates.py",
            "src/bill_analyser/api/routes/settings_bundle.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/proxy.rs",
            "crates/bill-analyser-core/src/adapters/category.rs",
            "crates/bill-analyser-db/src/taxonomy",
        ],
        tests_migrated: &["crates/bill-analyser-db/tests/taxonomy_bridge_cli.rs"],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["taxonomy_crud_parity", "settings_bundle_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Accounts, categories, tags, templates, category rules, and settings bundle routes remain Python-proxied until the P4 domain cutover ports runtime handlers.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "matching-recurring-calendar-networth",
        python_owner_files: &[
            "src/bill_analyser/api/routes/matching",
            "src/bill_analyser/api/routes/recurring.py",
            "src/bill_analyser/api/routes/calendar.py",
            "src/bill_analyser/api/routes/networth.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/proxy.rs",
            "crates/bill-analyser-core/src/matching.rs",
        ],
        tests_migrated: &["crates/bill-analyser-core/tests/matching_contracts.rs"],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["matching_runtime_parity", "recurring_calendar_networth_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Formal matching, recurring suggestions, calendar, and net worth runtime routes remain Python-proxied until the P8 domain cutover ports handlers.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "backup-ops",
        python_owner_files: &[
            "src/bill_analyser/api/routes/backup",
            "src/bill_analyser/core/backup",
            "src/bill_analyser/core/ops",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-http/src/proxy.rs",
            "crates/bill-analyser-core/src/ops.rs",
        ],
        tests_migrated: &["crates/bill-analyser-core/tests/ops_contracts.rs"],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["backup_restore_parity", "ops_runtime_config_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Backup, restore, cleanup, jobs, and operational runtime routes remain Python-proxied until the P12 domain cutover ports handlers.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "database-schema",
        python_owner_files: &[
            "src/bill_analyser/core/database/runtime.py",
            "src/bill_analyser/core/database/schema/__init__.py",
            "src/bill_analyser/core/database/schema/core/business.py",
            "src/bill_analyser/core/database/schema/core/indexes.py",
            "src/bill_analyser/core/database/schema/core/migrations.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-db/src/connection.rs",
            "crates/bill-analyser-db/src/schema.rs",
            "crates/bill-analyser-db/src/transaction.rs",
            "crates/bill-analyser-db/src/user_scope.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-db/tests/sqlite_runtime.rs",
            "tests/domains/db/integration/test_db_schema_core_paths.py",
            "tests/domains/db/unit/test_db_runtime_helpers.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["domain_runtime_takeover", "encryption_schema_parity"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Rust now owns foundational schema initialization and core user-scoped legacy migrations; SQLCipher/encryption and domain-specific table deletion remain deferred to their own cutover phases.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: DB_SCHEMA_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "database-repositories",
        python_owner_files: &[
            "src/bill_analyser/core/db.py",
            "src/bill_analyser/core/database/bills",
            "src/bill_analyser/core/database/budgets",
            "src/bill_analyser/core/database/imports",
            "src/bill_analyser/core/database/accounts",
            "src/bill_analyser/core/database/categories",
            "src/bill_analyser/core/database/tags",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-db/src/bills.rs",
            "crates/bill-analyser-db/src/budgets.rs",
            "crates/bill-analyser-db/src/import_staging.rs",
            "crates/bill-analyser-db/src/auth.rs",
            "crates/bill-analyser-db/src/statistics.rs",
            "crates/bill-analyser-db/src/app_settings.rs",
            "crates/bill-analyser-db/src/taxonomy",
        ],
        tests_migrated: &[
            "crates/bill-analyser-db/tests/bills_runtime.rs",
            "crates/bill-analyser-db/tests/budgets_runtime.rs",
            "crates/bill-analyser-db/tests/import_staging.rs",
            "crates/bill-analyser-db/tests/auth_two_factor_recovery.rs",
            "crates/bill-analyser-db/tests/app_settings.rs",
            "crates/bill-analyser-db/tests/taxonomy_bridge_cli.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: SQLITE_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["business_domain_route_takeover"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "Repository implementations exist for several Rust-owned or bridge-backed domains, including 2FA recovery-code DB primitives; Python facade deletion remains blocked until each business domain owns its routes and tests.",
        decision_required: DecisionRequired::Port,
        decision_owner: "migration-program",
        transition_evidence: DB_REPOSITORY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "database-facade",
        python_owner_files: &[
            "src/bill_analyser/core/db.py",
            "src/bill_analyser/core/database/runtime.py",
        ],
        rust_owner_files: &[
            "crates/bill-analyser-core/src/migration_governance.rs",
            "crates/bill-analyser-db/src/connection.rs",
            "crates/bill-analyser-db/src/schema.rs",
        ],
        tests_migrated: &[
            "crates/bill-analyser-db/tests/sqlite_runtime.rs",
            "crates/bill-analyser-core/tests/migration_governance_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: &["domain_runtime_takeover"],
        blocked_status: MigrationBlockedStatus::None,
        unsupported_behavior:
            "The database-facade contract is governance-only and does not, by itself, delete Python runtime entry points.",
        decision_required: DecisionRequired::Defer,
        decision_owner: "migration-program",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
];

const RUST_ENVELOPE_CONTEXT: &[MigrationState] = &[
    MigrationState::RustImplemented,
    MigrationState::RustOwnedVerified,
    MigrationState::PythonDeleted,
];
const PYTHON_PROXIED_ENVELOPE_CONTEXT: &[MigrationState] = &[MigrationState::PythonProxied];
const CONTRACT_ENVELOPE_CONTEXT: &[MigrationState] = &[MigrationState::ContractOnly];
const RUST_OR_PYTHON_PROXIED_ENVELOPE_CONTEXT: &[MigrationState] = &[
    MigrationState::RustImplemented,
    MigrationState::RustOwnedVerified,
    MigrationState::PythonDeleted,
    MigrationState::PythonProxied,
];

const ENVELOPE_POLICIES: &[ResponseEnvelopePolicy] = &[
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RustHttpShell,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "typed Rust health/runtime JSON",
        error_shape: "typed Rust shell error when applicable",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ProxyInfrastructureError,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "none",
        error_shape: "ApiResponse success=false error.code/message",
        proxy_may_wrap: true,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::FlaskSuccessData,
        route_contexts: RUST_OR_PYTHON_PROXIED_ENVELOPE_CONTEXT,
        success_shape: "success=true data emitted by Rust-compatible import routes or proxied Flask provider routes",
        error_shape: "success=false error/code/message in Flask-compatible shape",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::FlaskSuccessResult,
        route_contexts: RUST_OR_PYTHON_PROXIED_ENVELOPE_CONTEXT,
        success_shape: "success=true result/message emitted by Rust-compatible token routes or proxied Flask routes",
        error_shape: "success=false error/code/message in Flask-compatible shape",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::FlaskRawPassthrough,
        route_contexts: PYTHON_PROXIED_ENVELOPE_CONTEXT,
        success_shape: "raw Flask passthrough response, including send_file/download bodies and headers",
        error_shape: "raw Flask error response for passthrough routes",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::BillsCrud,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result with frontend transaction DTO or page wrapper",
        error_shape: "success=false error in Flask-compatible shape",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::BudgetsCrud,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result/message with budget row/list/export/execution/forecast payload",
        error_shape: "success=false error in Flask-compatible shape",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::StatisticsRead,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result/data with DB-backed statistics aggregation payload",
        error_shape: "success=false error/message in Flask-compatible shape",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportV2Stage,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data with import stage payload",
        error_shape: "success=false error/error_code/message",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportPreviewAction,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data with preview projection",
        error_shape: "success=false error plus expectedState when stale",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data responseMode=preview-item",
        error_shape: "success=false code/error_code/message",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::LearningRoute,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data/result for learning routes",
        error_shape: "success=false code/error_code/message",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::LlmPreview,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data/result for LLM preview/memory",
        error_shape: "success=false code/error_code/message",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::OcrMl,
        route_contexts: RUST_OR_PYTHON_PROXIED_ENVELOPE_CONTEXT,
        success_shape: "success=true data for Rust OCR config routes; receipt recognition remains Python-proxied",
        error_shape: "success=false code/error_code/message",
        proxy_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ContractOracle,
        route_contexts: CONTRACT_ENVELOPE_CONTEXT,
        success_shape: "compile-time contract data only",
        error_shape: "none",
        proxy_may_wrap: false,
    },
];

const IMPORT_DB_WRITE_INVARIANTS: [DbWriteInvariant; 8] = [
    DbWriteInvariant {
        key: "wal_mode",
        description: "Rust SQLite connections must use WAL mode before write takeover.",
    },
    DbWriteInvariant {
        key: "foreign_keys",
        description: "Rust SQLite connections must enable foreign key enforcement.",
    },
    DbWriteInvariant {
        key: "single_writer",
        description: "A business domain may have only one active DB writer during takeover.",
    },
    DbWriteInvariant {
        key: "transactional_staging",
        description: "Import session, preview, and confirmation mutations must be transactional.",
    },
    DbWriteInvariant {
        key: "rollback_on_error",
        description: "Failed import writes must roll back partial staging and bill mutations.",
    },
    DbWriteInvariant {
        key: "positive_user_scope",
        description: "All import writes must bind a positive authenticated user id.",
    },
    DbWriteInvariant {
        key: "amount_units",
        description: "Amount boundaries must state yuan or cents explicitly.",
    },
    DbWriteInvariant {
        key: "time_normalization",
        description: "Date/time writes must normalize local bill time explicitly.",
    },
];

const CRUD_DB_WRITE_INVARIANTS: [DbWriteInvariant; 6] = [
    DbWriteInvariant {
        key: "wal_mode",
        description: "Rust SQLite connections must use WAL mode before write takeover.",
    },
    DbWriteInvariant {
        key: "foreign_keys",
        description: "Rust SQLite connections must enable foreign key enforcement.",
    },
    DbWriteInvariant {
        key: "rollback_on_error",
        description: "Failed runtime writes must roll back partial bill or budget mutations.",
    },
    DbWriteInvariant {
        key: "positive_user_scope",
        description: "All runtime writes must bind a positive authenticated user id.",
    },
    DbWriteInvariant {
        key: "amount_units",
        description: "Amount boundaries must state yuan or cents explicitly.",
    },
    DbWriteInvariant {
        key: "time_normalization",
        description: "Date/time writes must normalize local bill time explicitly.",
    },
];

pub fn rust_http_shell_ownership_matrix() -> &'static [EndpointOwnership] {
    OWNERSHIP_MATRIX
}

pub fn migration_state_machine() -> &'static [MigrationState] {
    &CUTOVER_STATE_MACHINE
}

pub fn manifest_states() -> &'static [MigrationState] {
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
        "bills-crud-adjacent" => RouteHandlerId::BillsAdjacentProxyPassthrough,
        "bills-recurring" => RouteHandlerId::BillsCrudRuntime,
        "bills-category-actions" => RouteHandlerId::BillsCategoryActionsProxyPassthrough,
        "bills-import" => RouteHandlerId::ImportDbRuntime,
        "ai-learning-llm" => RouteHandlerId::LlmLearningRuntimeBoundary,
        "ai-ocr" => RouteHandlerId::OcrRuntimeBoundary,
        "budgets-crud" => RouteHandlerId::BudgetsCrudRuntime,
        "budgets-analysis" => RouteHandlerId::BudgetsAnalysisRuntime,
        "budgets-history" => RouteHandlerId::BudgetsHistoryRuntime,
        "budgets-import" => RouteHandlerId::BudgetsImportRuntime,
        "statistics-read" => RouteHandlerId::StatisticsReadRuntime,
        "statistics-analyzer" => RouteHandlerId::StatisticsAnalyzerProxyPassthrough,
        "statistics-exchange" => RouteHandlerId::StatisticsExchangeProxyPassthrough,
        "auth-security-user-data"
        | "taxonomy-rules-settings"
        | "matching-recurring-calendar-networth"
        | "backup-ops" => RouteHandlerId::LegacyPythonProxyPassthrough,
        "database-schema" | "database-repositories" | "database-facade" => {
            RouteHandlerId::DatabaseFacadeContractOracle
        }
        _ => panic!("missing handler mapping for migration governance domain {domain}"),
    }
}

fn route_contract_details(
    route: &EndpointOwnership,
    policy: &DomainGovernancePolicy,
) -> RouteContractDetails {
    match (route.domain, route.state) {
        ("ai-learning-llm", MigrationState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::LlmLearningRuntimeBoundary,
            deletion_blockers: IMPORT_DELETION_BLOCKERS,
            blocked_status: MigrationBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
        },
        ("ai-learning-llm", MigrationState::PythonProxied) => RouteContractDetails {
            handler: RouteHandlerId::LlmLearningRuntimeBoundary,
            deletion_blockers: &["provider_execution_parity", "learning_center_rule_loop"],
            blocked_status: MigrationBlockedStatus::None,
            unsupported_behavior:
                "Live provider-backed preview recommendation generation, transaction analysis, rule synthesis, and the global Learning Center suggestion/rule loop remain Python-owned.",
            decision_required: DecisionRequired::Port,
            decision_owner: "migration-program",
            transition_evidence: PROVIDER_ROUTE_EVIDENCE,
        },
        ("ai-ocr", MigrationState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::OcrRuntimeBoundary,
            deletion_blockers: IMPORT_DELETION_BLOCKERS,
            blocked_status: MigrationBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
        },
        ("ai-ocr", MigrationState::PythonProxied) => RouteContractDetails {
            handler: RouteHandlerId::OcrRuntimeBoundary,
            deletion_blockers: &["provider_execution_parity"],
            blocked_status: MigrationBlockedStatus::None,
            unsupported_behavior:
                "Receipt image recognition provider execution remains Python-owned even though OCR config persistence is Rust-owned.",
            decision_required: DecisionRequired::Port,
            decision_owner: "migration-program",
            transition_evidence: PROVIDER_ROUTE_EVIDENCE,
        },
        ("auth-security-user-data", MigrationState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::AuthTokenRuntime,
            deletion_blockers: AUTH_TOKEN_DELETION_BLOCKERS,
            blocked_status: MigrationBlockedStatus::None,
            unsupported_behavior:
                "Login, registration, token session list/revoke, API/MCP personal token generation, refresh token exchange, logout, account recovery email verification/resend/password forgot/reset, OAuth2 callback authorize disabled-safe/not-implemented response, profile, avatar, profile cloud settings, profile external-auth list/unlink, profile verification-email resend, system version, user-data statistics, user-data CSV/TSV export, destructive user-data clear, authenticated 2FA status, TOTP login verification, recovery-code login verification, 2FA write management, and step-up verification routes are Rust-owned; real OAuth provider exchange remains Python-owned.",
            decision_required: DecisionRequired::Port,
            decision_owner: "migration-program",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("bills-crud-adjacent", MigrationState::RustOwnedVerified) => RouteContractDetails {
            handler: RouteHandlerId::BillsCrudRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: MigrationBlockedStatus::None,
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
        cutover_state_machine: migration_state_machine(),
        manifest_states: manifest_states(),
        coverage_evidence_contract: COVERAGE_EVIDENCE_CONTRACT,
        domains: domain_governance_policies(),
        routes: expanded_route_manifest(),
    }
}

pub fn contract_oracle() -> &'static str {
    "contract://migration-governance-oracle"
}

pub fn routes_by_state(state: MigrationState) -> Vec<&'static EndpointOwnership> {
    OWNERSHIP_MATRIX
        .iter()
        .filter(|endpoint| endpoint.state == state)
        .collect()
}

pub fn endpoints_by_owner(state: MigrationState) -> Vec<&'static EndpointOwnership> {
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

pub fn can_delete_python_import_paths(evidence: ImportDeletionEvidence) -> bool {
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
        active_writer: "crates/bill-analyser-http/src/import_routes.rs via Rust import_db_runtime",
        rust_write_allowed: true,
        invariants: &IMPORT_DB_WRITE_INVARIANTS,
    }
}

pub fn bills_crud_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "bills-crud",
        mode: DbWriterMode::RustDomainOwned,
        active_writer: "crates/bill-analyser-http/src/bill_routes.rs + crates/bill-analyser-db/src/bills.rs via Rust bills_crud_runtime",
        rust_write_allowed: true,
        invariants: &CRUD_DB_WRITE_INVARIANTS,
    }
}

pub fn budgets_crud_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "budgets-crud",
        mode: DbWriterMode::RustDomainOwned,
        active_writer: "crates/bill-analyser-http/src/budget_routes.rs + crates/bill-analyser-db/src/budgets.rs via Rust budgets_crud_runtime",
        rust_write_allowed: true,
        invariants: &CRUD_DB_WRITE_INVARIANTS,
    }
}

pub fn database_schema_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "database-schema",
        mode: DbWriterMode::RustDomainOwned,
        active_writer: "crates/bill-analyser-db/src/schema.rs::init_foundational_schema + init_auth_security_schema",
        rust_write_allowed: true,
        invariants: &CRUD_DB_WRITE_INVARIANTS,
    }
}
