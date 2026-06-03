//! Runtime governance contracts for the current Rust HTTP backend.
//!
//! These contracts encode the current runtime state
//! machine, route/domain manifests, and the evidence required to keep removed
//! paths absent.

// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    RustImplemented,
    RustOwnedVerified,
    Retired,
    ContractOnly,
    Planned,
}

impl RuntimeState {
    pub fn is_rust_runtime_state(self) -> bool {
        matches!(
            self,
            Self::RustImplemented | Self::RustOwnedVerified | Self::Retired
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeBlockedStatus {
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
    #[serde(rename = "src/backend/http/router.rs::build_router")]
    RouterBuildRouter,
    #[serde(rename = "src/backend/http/bill_routes/mod.rs::bills_crud_runtime")]
    BillsCrudRuntime,
    #[serde(rename = "src/backend/http/import_routes/mod.rs::import_db_runtime")]
    ImportDbRuntime,
    #[serde(rename = "src/backend/http/import_routes/mod.rs::llm_learning_runtime_boundary")]
    LlmLearningRuntimeBoundary,
    #[serde(rename = "src/backend/http/import_routes/mod.rs::ocr_runtime_boundary")]
    OcrRuntimeBoundary,
    #[serde(rename = "src/backend/http/budget_routes.rs::budgets_crud_runtime")]
    BudgetsCrudRuntime,
    #[serde(rename = "src/backend/http/budget_routes.rs::budgets_analysis_runtime")]
    BudgetsAnalysisRuntime,
    #[serde(rename = "src/backend/http/budget_routes.rs::budgets_history_runtime")]
    BudgetsHistoryRuntime,
    #[serde(rename = "src/backend/http/budget_routes.rs::budgets_import_runtime")]
    BudgetsImportRuntime,
    #[serde(rename = "src/backend/http/statistics_routes/mod.rs::statistics_read_runtime")]
    StatisticsReadRuntime,
    #[serde(rename = "src/backend/http/statistics_routes/mod.rs::statistics_analyzer_runtime")]
    StatisticsAnalyzerRuntime,
    #[serde(rename = "src/backend/http/statistics_routes/mod.rs::statistics_exchange_runtime")]
    StatisticsExchangeRuntime,
    #[serde(rename = "src/backend/http/auth_routes/mod.rs::auth_token_runtime")]
    AuthTokenRuntime,
    #[serde(rename = "src/backend/http/taxonomy_routes/mod.rs::taxonomy_runtime")]
    TaxonomyRuntime,
    #[serde(
        rename = "src/backend/http/matching_routes.rs::matching_recurring_calendar_networth_runtime"
    )]
    MatchingRecurringCalendarNetworthRuntime,
    #[serde(rename = "src/backend/http/backup_routes/mod.rs::backup_ops_runtime")]
    BackupOpsRuntime,
    #[serde(rename = "src/backend/core/runtime_governance.rs::contract_oracle")]
    DatabaseFacadeContractOracle,
}

impl RouteHandlerId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RouterBuildRouter => "src/backend/http/router.rs::build_router",
            Self::BillsCrudRuntime => "src/backend/http/bill_routes/mod.rs::bills_crud_runtime",
            Self::ImportDbRuntime => "src/backend/http/import_routes/mod.rs::import_db_runtime",
            Self::LlmLearningRuntimeBoundary => {
                "src/backend/http/import_routes/mod.rs::llm_learning_runtime_boundary"
            }
            Self::OcrRuntimeBoundary => {
                "src/backend/http/import_routes/mod.rs::ocr_runtime_boundary"
            }
            Self::BudgetsCrudRuntime => "src/backend/http/budget_routes.rs::budgets_crud_runtime",
            Self::BudgetsAnalysisRuntime => {
                "src/backend/http/budget_routes.rs::budgets_analysis_runtime"
            }
            Self::BudgetsHistoryRuntime => {
                "src/backend/http/budget_routes.rs::budgets_history_runtime"
            }
            Self::BudgetsImportRuntime => {
                "src/backend/http/budget_routes.rs::budgets_import_runtime"
            }
            Self::StatisticsReadRuntime => {
                "src/backend/http/statistics_routes/mod.rs::statistics_read_runtime"
            }
            Self::StatisticsAnalyzerRuntime => {
                "src/backend/http/statistics_routes/mod.rs::statistics_analyzer_runtime"
            }
            Self::StatisticsExchangeRuntime => {
                "src/backend/http/statistics_routes/mod.rs::statistics_exchange_runtime"
            }
            Self::AuthTokenRuntime => "src/backend/http/auth_routes/mod.rs::auth_token_runtime",
            Self::TaxonomyRuntime => "src/backend/http/taxonomy_routes/mod.rs::taxonomy_runtime",
            Self::MatchingRecurringCalendarNetworthRuntime => {
                "src/backend/http/matching_routes.rs::matching_recurring_calendar_networth_runtime"
            }
            Self::BackupOpsRuntime => "src/backend/http/backup_routes/mod.rs::backup_ops_runtime",
            Self::DatabaseFacadeContractOracle => {
                "src/backend/core/runtime_governance.rs::contract_oracle"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RouteContractDetails {
    handler: RouteHandlerId,
    deletion_blockers: &'static [&'static str],
    blocked_status: RuntimeBlockedStatus,
    unsupported_behavior: &'static str,
    decision_required: DecisionRequired,
    decision_owner: &'static str,
    transition_evidence: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseEnvelopeFamily {
    RustHttpShell,
    RuntimeInfrastructureError,
    CurrentSuccessData,
    CurrentSuccessResult,
    RawPassthrough,
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
    pub state: RuntimeState,
    pub envelope: ResponseEnvelopeFamily,
    pub deletion_blocked_until_all_import_gates: bool,
    pub notes: &'static str,
}

impl EndpointOwnership {
    pub fn is_import_deletion_blocked(&self) -> bool {
        self.deletion_blocked_until_all_import_gates
    }

    pub fn has_external_runtime_owner(&self) -> bool {
        false
    }

    pub fn is_rust_owned_verified(&self) -> bool {
        self.state == RuntimeState::RustOwnedVerified
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainGovernancePolicy {
    pub domain: &'static str,
    pub retired_source_files: &'static [&'static str],
    pub rust_owner_files: &'static [&'static str],
    pub tests_verified: &'static [&'static str],
    pub fixtures: &'static [&'static str],
    pub db_invariant_ids: &'static [&'static str],
    pub coverage_evidence: &'static str,
    pub deletion_blockers: &'static [&'static str],
    pub blocked_status: RuntimeBlockedStatus,
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
    pub state: RuntimeState,
    pub handler: RouteHandlerId,
    pub deletion_blockers: &'static [&'static str],
    pub blocked_status: RuntimeBlockedStatus,
    pub unsupported_behavior: &'static str,
    pub decision_required: DecisionRequired,
    pub decision_owner: &'static str,
    pub transition_evidence: &'static [&'static str],
    pub envelope: ResponseEnvelopeFamily,
    pub notes: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GovernanceManifestSnapshot {
    pub runtime_state_machine: &'static [RuntimeState],
    pub manifest_states: &'static [RuntimeState],
    pub coverage_evidence_contract: &'static str,
    pub domains: &'static [DomainGovernancePolicy],
    pub routes: Vec<ExpandedRouteManifestEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseEnvelopePolicy {
    pub family: ResponseEnvelopeFamily,
    pub route_contexts: &'static [RuntimeState],
    pub success_shape: &'static str,
    pub error_shape: &'static str,
    pub runtime_may_wrap: bool,
}

impl ResponseEnvelopePolicy {
    pub fn applies_to_owner(&self, state: RuntimeState) -> bool {
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
    RetiredSourcePrimary,
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

macro_rules! retired_route {
    ($method:literal, $pattern:literal, $domain:literal, $envelope:expr, $notes:literal) => {
        EndpointOwnership {
            method: $method,
            pattern: $pattern,
            domain: $domain,
            state: RuntimeState::Retired,
            envelope: $envelope,
            deletion_blocked_until_all_import_gates: false,
            notes: $notes,
        }
    };
}

const OWNERSHIP_MATRIX: &[EndpointOwnership] = &[
    EndpointOwnership {
        method: "GET",
        pattern: "/api/health",
        domain: "api-runtime-shell",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::RustHttpShell,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust HTTP shell health route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/runtime",
        domain: "api-runtime-shell",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::RustHttpShell,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust HTTP shell metadata route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns core list route; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns trailing-slash list route; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns core create route with cents-to-yuan adapter semantics; the removed bills/crud_create_update.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns trailing-slash create route; the removed bills/crud_create_update.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/by-month",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns month list route; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/get",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns query get route; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST get route; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST update route; the removed bills/crud_create_update.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST delete route; the removed bills/crud_create_update.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/modify",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns modify route; the removed bills/crud_prepare.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/delete",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns delete route; the removed bills/crud_prepare.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/batch",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch create route with one PostgreSQL transaction; the removed bills/crud_create_update.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/batch/update",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch update route; the removed bills/category_actions.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/batch/delete",
        domain: "bills-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch delete route; the removed bills/category_actions.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/export",
        domain: "bills-crud-adjacent",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::RawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills export runtime owns CSV/XLSX file responses, current export filenames, BOM CSV output, empty-result errors, and formula-like text cell escaping; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/pictures",
        domain: "bills-crud-adjacent",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills picture runtime owns multipart transaction picture upload and data URL response semantics; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/pictures/unused",
        domain: "bills-crud-adjacent",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills picture runtime owns unused transaction picture cleanup; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/reconciliation_statements",
        domain: "bills-crud-adjacent",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills reconciliation runtime owns account statements, balance trace, filters, and current API envelopes; the removed bills/reconciliation.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/{bill_id}/recurring-candidates",
        domain: "bills-recurring",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills recurring runtime owns formal bill recurring candidate generation; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/{bill_id}/recurring-match",
        domain: "bills-recurring",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills recurring runtime owns recurring template binding and next-date recalculation; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/{bill_id}/recurring-match",
        domain: "bills-recurring",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills recurring runtime owns recurring template unbinding and next-date recalculation; the removed bills/crud_query.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/category/quick-add-keyword",
        domain: "bills-category-actions",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills category-actions runtime appends category keywords with the current API quick-add envelope; the removed bills/category_actions.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/category/refresh",
        domain: "bills-category-actions",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills category-actions runtime refreshes bill categories through canonical category_rules matching; the removed bills/category_actions.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns core list route; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns trailing-slash list route; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns core create route with yuan-style budget amount semantics; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns trailing-slash create route; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST get route; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST update route; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST delete route; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/export",
        domain: "budgets-crud",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns JSON export for budget rows; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/execution",
        domain: "budgets-analysis",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_execution_runtime owns budget execution aggregation; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/forecast",
        domain: "budgets-analysis",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_forecast_runtime owns historical bill aggregation, budget-map projection, forecast summary, and period-progress shape; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/history",
        domain: "budgets-history",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_history_runtime owns persisted snapshot lookup, exact-period preference, category enrichment, and on-demand fallback; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/history/snapshot",
        domain: "budgets-history",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_history_runtime owns budget_history replacement writes with canonical filter_summary and user scope; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/import",
        domain: "budgets-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_import_runtime owns current API array validation, per-item error accounting, and user-scoped upsert-by-name writes; the removed budgets route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-statistics",
        domain: "statistics-read",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns DB-backed category/account cents aggregation with timestamp range and keyword filtering; the removed statistics read route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-statistics/trends",
        domain: "statistics-read",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns monthly category/account trend buckets for bounded and all-mode ranges; the removed statistics read route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/asset-trends",
        domain: "statistics-read",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns DB-backed daily asset trend balances with 365-day bounded-range guard; the removed statistics read route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-pie",
        domain: "statistics-read",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns category pie aggregation for bill type/date filters; the removed statistics read route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/top-merchants",
        domain: "statistics-read",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns top merchant aggregation for date filters; the removed statistics read route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/amounts",
        domain: "statistics-read",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns transaction amount period aggregation with CNY cents response semantics; the removed statistics read route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/overview",
        domain: "statistics-analyzer",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_analyzer_runtime owns Analyzer overview report projection; the removed Analyzer route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/trends",
        domain: "statistics-analyzer",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_analyzer_runtime owns Analyzer monthly/yearly trend projection; the removed Analyzer route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/comparison",
        domain: "statistics-analyzer",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_analyzer_runtime owns Analyzer category comparison projection; the removed Analyzer route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category",
        domain: "statistics-analyzer",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_analyzer_runtime owns Analyzer category drilldown projection; the removed Analyzer route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/trend",
        domain: "statistics-analyzer",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_analyzer_runtime owns Analyzer trend alias projection; the removed Analyzer route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/exchange-rates",
        domain: "statistics-exchange",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics exchange runtime owns provider selection, provider fallback, user custom-rate precedence, and current API result envelope; the removed exchange route shell is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/statistics/exchange-rates/custom",
        domain: "statistics-exchange",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics exchange runtime validates and persists authenticated user custom exchange rates; the removed exchange route shell is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/statistics/exchange-rates/custom/{currency}",
        domain: "statistics-exchange",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics exchange runtime deletes authenticated user custom exchange rates for the current default base currency; the removed exchange route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/parse",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/parse_generic",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/dedup",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/confirm",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/session/{session_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/v2/session/{session_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview/{session_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview/{session_id}/index",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview/{session_id}/selection",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview/{session_id}/update",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/reclassify/{session_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-candidates",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/learning/{session_id}/suggestions",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this preview-bypass route; it applies supplied preview updates and returns an explicit empty suggestion list without provider/model execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/learning/{session_id}/suggestions",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this preview-bypass route; it validates the session and returns an explicit empty suggestion list without provider/model execution.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/learning/{session_id}/promote",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this DB-write decision route and promotes stored session suggestions into import learning rules.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/preview",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/confirm",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/batch",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/parse_import",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/upload",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/parsers",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/reclassify",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/configs",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs/match",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs/suggest",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/configs/{config_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/learning-rules",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/learning-rules/{rule_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/learning-rules/{rule_id}",
        domain: "bills-import",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route; the removed bills import route package is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns live LLM preview recommendation provider execution, applies yellow preview suggestions, and writes memory events without fallback routing.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend/accept",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; provider generation routes are Rust-owned.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend/reject",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; provider generation routes are Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/memory",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; provider generation routes are Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/config",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns effective LLM config reads, combining process-local runtime overrides with active saved configs; provider generation routes are Rust-owned.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/config",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns process-local LLM runtime config updates without persisting temporary provider secrets; provider generation routes are Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/configs",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns saved LLM config listing with secret redaction and user scoping.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/configs",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns saved LLM config creation and active-config runtime override clearing.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/llm/configs/{config_id}",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns saved LLM config updates, preserving redacted API keys when clients send the placeholder.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/llm/configs/{config_id}",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns saved LLM config deletion with user scoping.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/configs/{config_id}/activate",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns saved LLM config activation and process-local runtime override clearing.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/candidates",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns DB-backed LLM candidate listing and count filters; provider generation routes are Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/candidates/{candidate_id}",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns single LLM candidate reads with user scoping.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/candidates/{candidate_id}/accept",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns provider-bypassed LLM candidate accept, including category-rule materialization for rule candidates.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/candidates/{candidate_id}/reject",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns provider-bypassed LLM candidate rejection.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/analyze-transactions",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns live LLM persisted transaction analysis and import-session rule induction candidate generation.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/rule-synthesis",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns live LLM rule-synthesis provider execution and persists reviewable rule_synthesis candidates.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/learning/suggestions",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns global Learning Center suggestion listing, persistence, and deterministic decisions; provider-backed LLM generation is Rust-owned.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/generate",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime mines deterministic global Learning Center suggestions from import learning corpus samples; provider-backed LLM generation is Rust-owned.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/{suggestion_id}/accept",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns global Learning Center suggestion accept and rule materialization.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/batch-accept",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns global Learning Center batch suggestion accept and rule materialization.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/{suggestion_id}/reject",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns global Learning Center suggestion rejection.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/learning/rules",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns the global Learning Center rule store list contract.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/learning/rules/{rule_id}/toggle",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns the global Learning Center rule enable/disable contract.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/learning/rules/{rule_id}",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns the global Learning Center rule update contract.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/learning/rules/{rule_id}",
        domain: "ai-learning-llm",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns the global Learning Center rule delete contract.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/ml/receipt-recognition",
        domain: "ai-ocr",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns receipt OCR recognition with disabled/cloud-stub typed errors and external tesseract provider execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/ml/receipt-recognition/config",
        domain: "ai-ocr",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns OCR config and receipt recognition.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/ml/receipt-recognition/config",
        domain: "ai-ocr",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust import_db_runtime owns OCR config and receipt recognition.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/enable/request",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime generates current API TOTP setup secrets and QR-code PNG data URLs for authenticated users.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/enable/confirm",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime validates TOTP setup passcodes, atomically enables 2FA with recovery codes plus a new session, and records 2fa_enabled audit details.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/disable",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime accepts the current password, operation password fallback, or step-up action tokens; atomically disables 2FA, clears recovery codes, and records 2fa_disabled audit details.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/recovery/regenerate",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime accepts the current password, operation password fallback, or step-up action tokens; replaces recovery codes and records 2fa_recovery_regenerated audit details.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/2fa/status",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime owns the authenticated 2FA status read route and the companion 2FA write management routes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/verify",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime verifies pending_2fa action tokens plus TOTP passcodes, creates the access/refresh session, and logs login_2fa_success.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/2fa/recovery/verify",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime verifies pending_2fa action tokens plus one-time recovery codes, creates the access/refresh session, logs login_2fa_recovery_success, and records a best-effort 2fa_recovery_code_used audit log when audit_logs exists.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/security/step-up/verify",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime verifies the current password, configured operation password fallback, or 2FA TOTP passcode and issues a one-hour step_up action token for sensitive operations.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/data/export.{file_type}",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::RawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime exports authenticated user bills as CSV/TSV with the current export filename, BOM, account-name, tag-name, and filter semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/data/clear/transactions",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime clears current-user bills and bill side effects after current password, configured operation password fallback, or step-up action-token verification, then records user_data audit metadata.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/data/clear/all",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust auth runtime clears current-user business data after current password, configured operation password fallback, or step-up action-token verification, preserving current count keys and user_data audit metadata.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/accounts/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime lists authenticated user accounts, builds the current parent-child response hierarchy, and returns frontend cents fields; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/accounts/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime creates accounts and subaccounts with frontend cents to Postgres balance_cents conversion; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/accounts/{account_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime deletes authenticated user accounts and their direct subaccounts without fallback routing; the old accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/accounts/{account_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime reads account detail with direct subaccounts and frontend account DTO formatting; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/accounts/{account_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime updates account fields and direct subaccount sets using the authenticated user scope; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/accounts/{account_id}/transactions/clear",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime verifies the current password or operation password fallback, deletes authenticated-user account-linked bills/transfers and side effects, syncs balances, and records account audit metadata; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/accounts/{account_id}/transactions/move",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime verifies the current password or operation password fallback, moves authenticated-user account-linked bills/transfers, syncs balances, and records account audit metadata; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/accounts/display-orders",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime persists authenticated user account display ordering; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/accounts/sync-balances",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy accounts runtime recalculates authenticated user account balances from bill source/destination account links and returns the current API discrepancy report; the removed accounts route package has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/backup/",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime lists local backup files, validates zip or Fernet-encrypted archives, merges backup_records metadata, and sorts by created_at without fallback routing.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/backup/cleanup",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime applies record-first backup retention cleanup, deletes sidecar metadata, marks missing/deleted records, and writes backup_cleanup audit rows.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/backup/create",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime creates local data/ zip archives, optionally emits Fernet-encrypted .zip.enc backups, upserts backup_records, and writes backup_created audit rows.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/backup/delete/{filename}",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime safely resolves backup filenames, deletes the file and metadata sidecar, marks backup_records deleted, and writes backup_deleted audit rows.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/backup/download/{filename}",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::RawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime safely resolves local backup files and streams attachment bytes while recording backup_downloaded audit rows.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/backup/jobs",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime lists authenticated user backup job schedules from PostgreSQL backup_jobs.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/backup/jobs",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime validates and saves authenticated user backup job schedules and writes backup_job_saved audit rows.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/backup/sync",
        domain: "backup-ops",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust backup ops runtime creates a local backup, validates redacted cloud sync config, and uploads to OSS, S3, COS, Azure Blob, or WebDAV without sidecar SDK execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/calendar/events",
        domain: "matching-recurring-calendar-networth",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust matching recurring calendar networth runtime owns calendar event aggregation and recurring projections; the removed calendar.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime lists authenticated user categories as the current type-keyed tree without requiring a trailing slash.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/categories",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime creates parent and child categories from the frontend no-trailing-slash endpoint.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime lists authenticated user categories as the current type-keyed tree.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/categories/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime creates parent and child categories with current API parentId and duplicate responses.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/categories/{category_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime deletes user-scoped categories, including parent category cascades by main category.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/{category_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime reads category details with virtual parent fallback for child categories.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/categories/{category_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime updates category fields and preserves parent rename conflict behavior.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/all",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime returns the raw user-scoped category rows for current consumers.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/categories/all",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime preserves the current batch update success response.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/categories/batch",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime creates preset parent/subcategory trees idempotently for the authenticated user.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/export",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime exports category JSON without generated ids or created_at values.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/flat",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime returns the current flat category DTO list.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/categories/import",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime imports category JSON by upserting main/subcategory rows under the current user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/categories/move",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime persists category display-order updates.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/rules",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category rules runtime returns the current user's current CategoryEngine rule DTO shape or the persisted config cache.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/categories/rules",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category rules runtime persists the rules payload into app_settings as a user-scoped config cache and returns the current update message.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/statistics",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime reads user-scoped bill category aggregates and returns the current tree-shaped category statistics response.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/categories/tree",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime serves the category tree alias.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/categories/update-all",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy categories runtime recategorizes authenticated user bills from canonical category_rules and preserves the current force flag response.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/category-rules/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime lists authenticated user canonical category rules with optional category_id and enabled_only filters; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/category-rules/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime creates canonical user-scoped rules and returns the created rule payload; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/category-rules/{rule_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime deletes canonical user-scoped rules and preserves Rule not found responses; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/category-rules/{rule_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime updates canonical user-scoped rules and returns the refreshed rule payload; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/category-rules/{rule_id}/test",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime tests a stored user-scoped rule expression against request text without mutating rule state; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/category-rules/defaults",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime creates missing built-in default daily categories and canonical rules for the authenticated user; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/category-rules/reorder",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy category-rules runtime reorders canonical user-scoped rules by request order; the removed category_rules.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/account-rules/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy account-rules runtime lists authenticated user account recognition rules with optional account_id and enabled_only filters.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/account-rules/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy account-rules runtime creates user-scoped account recognition rules and returns the created rule payload.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/account-rules/{rule_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy account-rules runtime deletes user-scoped account recognition rules and preserves Rule not found responses.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/account-rules/{rule_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy account-rules runtime updates user-scoped account recognition rules and returns the refreshed rule payload.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/account-rules/{rule_id}/test",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy account-rules runtime tests a stored account rule against explicit parser/counterparty/payment-method/description context without mutating rule state.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/account-rules/reorder",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy account-rules runtime reorders canonical user-scoped rules after validating the requested ids are unique and owned by the current user.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/rules/overview",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy rule-center runtime aggregates user-scoped import learning rules, enabled category rule count, and recurring rule summaries without mutating rule state; the removed rule-center overview route shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/insights/anomalies",
        domain: "statistics-analyzer",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_analyzer_runtime owns insights anomaly detection for current-user bills; the removed insights route shell is deleted.",
    },
    retired_route!(
        "GET",
        "/api/matching/bills/{bill_id}/candidates",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns formal bill candidate reads; the removed matching route shell is deleted."
    ),
    retired_route!(
        "GET",
        "/api/matching/bills/{bill_id}/feedback",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns formal bill feedback reads; the removed matching route shell is deleted."
    ),
    retired_route!(
        "GET",
        "/api/matching/candidates",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns unified session and formal bill candidate reads; the removed matching route shell is deleted."
    ),
    retired_route!(
        "POST",
        "/api/matching/candidates/{*candidate_id}/accept",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns candidate accept actions across formal bills and import-preview matching families; the removed matching route shell is deleted."
    ),
    retired_route!(
        "POST",
        "/api/matching/candidates/{*candidate_id}/clear",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns candidate clear actions for supported import-preview matching families; the removed matching route shell is deleted."
    ),
    retired_route!(
        "POST",
        "/api/matching/candidates/{*candidate_id}/reject",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns candidate reject actions across formal bills and import-preview matching families; the removed matching route shell is deleted."
    ),
    retired_route!(
        "GET",
        "/api/matching/investment-settings",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns the retired investment-settings endpoint and returns the deterministic 410 contract; the removed matching route shell is deleted."
    ),
    retired_route!(
        "PUT",
        "/api/matching/investment-settings",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns the retired investment-settings endpoint and returns the deterministic 410 contract; the removed matching route shell is deleted."
    ),
    retired_route!(
        "POST",
        "/api/matching/manual-pair",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns manual transfer and investment pair creation for formal bills; the removed matching route shell is deleted."
    ),
    retired_route!(
        "GET",
        "/api/matching/pairs",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns manual pair listing for formal bills; the removed matching route shell is deleted."
    ),
    retired_route!(
        "DELETE",
        "/api/matching/pairs/{pair_id}",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns manual pair deletion for formal bills; the removed matching route shell is deleted."
    ),
    retired_route!(
        "POST",
        "/api/matching/reconcile-history",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns explicit formal bill matching history reconciliation; the removed matching route shell is deleted."
    ),
    retired_route!(
        "GET",
        "/api/matching/reconciliation-candidates",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns persisted import-to-formal reconciliation candidate reads; the removed matching route shell is deleted."
    ),
    retired_route!(
        "GET",
        "/api/matching/sessions/{session_id}/candidates",
        "matching-recurring-calendar-networth",
        ResponseEnvelopeFamily::CurrentSuccessData,
        "Rust matching runtime owns import session candidate reads; the removed matching route shell is deleted."
    ),
    EndpointOwnership {
        method: "GET",
        pattern: "/api/networth/snapshot",
        domain: "matching-recurring-calendar-networth",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust matching recurring calendar networth runtime owns net-worth snapshot aggregation; the removed networth.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/recurring/suggestions",
        domain: "matching-recurring-calendar-networth",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust matching recurring calendar networth runtime owns recurring suggestion listing; the removed recurring.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/recurring/suggestions/{suggestion_id}/accept",
        domain: "matching-recurring-calendar-networth",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust matching recurring calendar networth runtime owns recurring suggestion accept and recurring rule creation; the removed recurring.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/recurring/suggestions/{suggestion_id}/reject",
        domain: "matching-recurring-calendar-networth",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust matching recurring calendar networth runtime owns recurring suggestion rejection; the removed recurring.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/recurring/suggestions/detect",
        domain: "matching-recurring-calendar-networth",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust matching recurring calendar networth runtime owns recurring pattern detection and suggestion persistence; the removed recurring.py route shell is deleted.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/settings/bundle/export",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::RawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle export runtime returns the current user's unified JSON settings bundle with redacted LLM secrets; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/settings/bundle/import",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle import runtime applies one transaction of current-user upserts across accounts, categories, tags, templates, rules, LLM config skeletons, and OCR config; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/settings/bundle/import/preview",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle preview runtime runs the same cross-section upsert flow inside a rollback-only transaction; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/settings/bundle/sections/{section_key}/export",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::RawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle export runtime returns one non-sensitive section and preserves the current password-required response for sensitive sections; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/settings/bundle/sections/{section_key}/export",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::RawPassthrough,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle export runtime validates the current password before exporting sensitive LLM/OCR sections; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/settings/bundle/sections/{section_key}/import",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle section import wraps the requested section only before running current-user upsert semantics; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/settings/bundle/sections/{section_key}/import/preview",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust taxonomy settings-bundle section preview wraps the requested section only and rolls back all writes; the removed settings_bundle.py route shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/tags/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns current-user tag list route and emits frontend displayOrder/hidden DTO fields; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tags/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns current-user tag creation with current API name validation; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tags/{tag_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns current-user tag deletion and preserves Tag not found for cross-user IDs; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/tags/{tag_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns current-user tag detail lookup; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/tags/{tag_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns tag content and visibility updates for the authenticated user; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tags/batch",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns user-scoped tag batch creation with duplicate skip/409 semantics; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/tags/display-orders",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy runtime owns user-scoped tag display-order updates; the removed tags.py shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/templates/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy templates runtime lists authenticated user bill and recurring templates with templateType filtering; the removed templates.py shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/templates/",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy templates runtime creates authenticated user bill or recurring templates and returns the current API template DTO; the removed templates.py shell has been removed.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/templates/{template_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy templates runtime deletes authenticated user templates with templateType scoping; the removed templates.py shell has been removed.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/templates/{template_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy templates runtime reads authenticated user template details with templateType filtering; the removed templates.py shell has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/templates/{template_id}",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy templates runtime updates authenticated user bill or recurring template fields; the removed templates.py shell has been removed.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/templates/display-orders",
        domain: "taxonomy-rules-settings",
        state: RuntimeState::Retired,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust taxonomy templates runtime persists authenticated user template display ordering; the removed templates.py shell has been removed.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/login",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies username/email password login, handles lockout/inactive/2FA-pending branches, writes sessions and auth logs, and returns current API tokens, profile, and application cloud settings.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/register",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime validates registration input/password policy, creates the user, default categories/rules/accounts, and auth log transactionally, and returns the current API registration result.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/logout",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime invalidates the bearer session by token hash, records logout auth logs for active sessions, and keeps current API idempotent success.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/email/verify",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime validates verify_email action tokens, marks email_verified, optionally issues a new session token, writes email_verified auth logs, and returns result.user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/email/resend-verification",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime verifies email/password credentials, issues mock-success verification tokens, records verification_email_resend_requested metadata, and returns result=true.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/password/forgot",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime honors the forget-password feature flag, returns success for unknown emails, and records mock-success reset token metadata for existing users.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/password/reset",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth account-recovery runtime validates reset_password action tokens, password policy, and email/user match before updating the password hash and writing password_reset_completed auth logs.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/auth/oauth2/authorize",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime owns the OAuth2 callback authorize disabled-safe/not-implemented response contract; the current workspace build has no live provider exchange implementation or fallback remainder for this route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tokens",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime revokes all other token sessions while preserving current bearer session semantics.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/tokens",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime lists active token sessions with current API success/result envelope.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/tokens/{token_id}",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime revokes one token session by id with user-scope validation.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/api",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies the current password and issues API personal access tokens with session and auth-log writes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/mcp",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies the current password and issues MCP personal access tokens with session and auth-log writes.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/tokens/refresh",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime verifies refresh token JWT/session state and issues rotated access/refresh sessions with profile/cloud settings response.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime returns the authenticated user's current API profile payload.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/profile",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime updates whitelisted user profile/display/investment-keyword fields, validates scoped account/category references, resets email verification on email changes, and returns result.user.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/avatar",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime accepts validated PNG/JPEG/GIF/WebP multipart avatar uploads, stores a data URL, and returns the updated profile.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/profile/avatar",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime clears the avatar field and returns the updated profile.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/email/resend-verification",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime rate-limits and records the mock-success verification email resend auth log, then returns result=true.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime lists application cloud settings and preserves the empty false response.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime validates supported application cloud setting keys/types and upserts full or partial updates.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/profile/cloud-settings",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime deletes all authenticated-user cloud settings and returns result=true.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/profile/external-auths",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime lists user-scoped external-auth bindings, preserves createdAt millisecond projection, and appends the configured OAuth2 unlinked placeholder.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/profile/external-auths/unlink",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth profile runtime verifies the current password before deleting a user-scoped external-auth binding and writing external_auth_unlinked audit metadata.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/system/version",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth runtime serves the unauthenticated system version metadata route with the current payload shape.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/data/statistics",
        domain: "auth-security-user-data",
        state: RuntimeState::RustOwnedVerified,
        envelope: ResponseEnvelopeFamily::CurrentSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes:
            "Rust auth user-data runtime returns authenticated user-scoped bill/account/category/tag/template counts with the current API success/result envelope.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://import-v2-envelope-oracle",
        domain: "bills-import",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins import envelope families for Rust-owned import_db_runtime routes.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://postgres-import-writer-policy",
        domain: "database-facade",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins PostgreSQL writer invariants for import_db_runtime.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://postgres-foundational-schema-policy",
        domain: "database-schema",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust DB crate owns PostgreSQL foundational schema initialization and user-scoped constraints.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://postgres-repository-policy",
        domain: "database-repositories",
        state: RuntimeState::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust DB crate records the active PostgreSQL repository surfaces for import staging, bills, budgets, statistics, app settings, and taxonomy.",
    },
];

const COVERAGE_EVIDENCE_CONTRACT: &str = "workspace.lcov";
const EMPTY_STRINGS: &[&str] = &[];
const POSTGRES_DB_INVARIANT_IDS: &[&str] = &[
    "postgres_authority",
    "referential_integrity",
    "rollback_on_error",
    "positive_user_scope",
    "amount_units",
    "time_normalization",
];
const IMPORT_DB_INVARIANT_IDS: &[&str] = &[
    "postgres_authority",
    "referential_integrity",
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
const DB_SCHEMA_EVIDENCE: &[&str] = &["db_smoke", "schema_contract"];
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
const AUTH_TOKEN_DELETION_BLOCKERS: &[&str] = EMPTY_STRINGS;
const TAXONOMY_DELETION_BLOCKERS: &[&str] = EMPTY_STRINGS;

const DOMAIN_GOVERNANCE_POLICIES: &[DomainGovernancePolicy] = &[
    DomainGovernancePolicy {
        domain: "api-runtime-shell",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/router.rs",
            "src/backend/http/server.rs",
            "src/backend/http/runtime.rs",
        ],
        tests_verified: &[
            "tests/backend/http/import_runtime_contract.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: RUNTIME_METADATA_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-import",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/import_routes/mod.rs",
            "src/backend/db/import_staging.rs",
            "src/backend/core/import_pipeline.rs",
            "src/backend/parsers/lib.rs",
        ],
        tests_verified: &[
            "tests/backend/http/import_runtime_contract.rs",
            "tests/backend/db/import_staging.rs",
            "tests/backend/parsers/parser_contracts.rs",
        ],
        fixtures: &["tests/backend/parsers/fixtures/parser_golden_contracts.json"],
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "The removed bills import route package has been removed; current core import services remain only as preserved sidecar/support code for matching, provider, and global learning ownership.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "ai-learning-llm",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/import_routes/mod.rs",
            "src/backend/core/ai_ocr_llm/mod.rs",
            "src/backend/core/ai_ocr_llm/llm_config.rs",
            "src/backend/core/ai_ocr_llm/llm_prompts.rs",
            "src/backend/core/ai_ocr_llm/llm_provider.rs",
            "src/backend/core/ai_ocr_llm/llm_responses.rs",
            "src/backend/core/import_learning.rs",
        ],
        tests_verified: &[
            "tests/backend/core/ai_ocr_llm_contracts.rs",
            "tests/backend/core/import_learning_contracts.rs",
            "tests/backend/http/import_runtime_contract.rs",
            "tests/new_ui/test_llm_import_session_analysis_api.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Live provider-backed preview recommendation generation, transaction analysis, and rule synthesis remain external-provider-owned; global Learning Center suggestions and rules are Rust-owned.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: PROVIDER_ROUTE_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "ai-ocr",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/import_routes/mod.rs",
            "src/backend/core/ai_ocr_llm/mod.rs",
            "src/backend/core/ai_ocr_llm/ocr_config.rs",
            "src/backend/core/ai_ocr_llm/ocr_parser.rs",
            "src/backend/db/app_settings.rs",
        ],
        tests_verified: &[
            "tests/backend/core/ai_ocr_llm_contracts.rs",
            "tests/backend/db/app_settings.rs",
            "tests/backend/http/import_runtime_contract.rs",
            "tests/test_ocr_service.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Rust owns receipt OCR config and recognition. The tesseract provider uses an external tesseract binary through stdin/stdout and returns provider_unconfigured when the binary or cloud provider is unavailable.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "bills-crud",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/bill_routes/",
            "src/backend/db/bills.rs",
            "src/backend/core/adapters/transaction.rs",
        ],
        tests_verified: &[
            "tests/backend/http/bills_runtime_contract.rs",
            "tests/backend/db/bills_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FRONTEND_DB_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-crud-adjacent",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/bill_routes/",
            "src/backend/core/adapters/transaction.rs",
            "src/backend/core/matching.rs",
            "src/backend/core/ops.rs",
        ],
        tests_verified: &[
            "tests/backend/http/bills_runtime_contract.rs",
            "tests/backend/core/transaction_adapter_contracts.rs",
            "tests/backend/core/matching_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-recurring",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/bill_routes/",
            "src/backend/db/bills.rs",
        ],
        tests_verified: &[
            "tests/backend/http/bills_runtime_contract.rs",
            "tests/backend/core/runtime_governance_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "bills-category-actions",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/bill_routes/",
            "src/backend/core/category_rules/mod.rs",
            "src/backend/core/matching.rs",
        ],
        tests_verified: &[
            "tests/backend/http/bills_runtime_contract.rs",
            "src/backend/core/category_rules/mod.rs",
            "tests/domains/categories/unit/test_category_rule_rust_bridge.py",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-crud",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/budget_routes.rs",
            "src/backend/db/budgets.rs",
            "src/backend/core/budgets.rs",
        ],
        tests_verified: &[
            "tests/backend/http/budget_runtime_contract.rs",
            "tests/backend/db/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "The removed budgets route package has been removed; Rust budgets_crud_runtime remains the runtime owner.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FRONTEND_DB_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-analysis",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/budget_routes.rs",
            "src/backend/db/budgets.rs",
            "src/backend/core/budgets.rs",
        ],
        tests_verified: &[
            "tests/backend/http/budget_runtime_contract.rs",
            "tests/backend/db/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "The removed budgets route package has been removed; Rust budgets_analysis_runtime remains the runtime owner.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-history",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/budget_routes.rs",
            "src/backend/db/budgets.rs",
            "src/backend/core/budgets.rs",
        ],
        tests_verified: &[
            "tests/backend/http/budget_runtime_contract.rs",
            "tests/backend/db/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "The removed budgets route package has been removed; Rust budgets_history_runtime remains the runtime owner.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "budgets-import",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/budget_routes.rs",
            "src/backend/db/budgets.rs",
            "src/backend/core/budgets.rs",
        ],
        tests_verified: &[
            "tests/backend/http/budget_runtime_contract.rs",
            "tests/backend/db/budgets_runtime.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "The removed budgets route package has been removed; Rust budgets_import_runtime remains the runtime owner.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "statistics-read",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/statistics_routes/mod.rs",
            "src/backend/db/statistics.rs",
            "src/backend/core/statistics.rs",
        ],
        tests_verified: &[
            "tests/backend/http/statistics_runtime_contract.rs",
            "tests/backend/core/statistics_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "The removed statistics read route shell has been removed; Analyzer overview/trends/comparison/category/trend remain governed by statistics-analyzer.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "statistics-analyzer",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/statistics_routes/mod.rs",
            "src/backend/db/statistics.rs",
            "src/backend/core/statistics.rs",
        ],
        tests_verified: &[
            "tests/backend/http/statistics_runtime_contract.rs",
            "tests/backend/core/statistics_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "The removed statistics Analyzer and insights route shells have been removed; Analyzer report data, chart-plan contract, and insights anomaly detection are Rust-owned. Net worth remains governed by matching-recurring-calendar-networth.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "statistics-exchange",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/statistics_routes/mod.rs",
            "src/backend/core/statistics.rs",
            "src/backend/db/statistics.rs",
        ],
        tests_verified: &[
            "tests/backend/http/statistics_runtime_contract.rs",
            "tests/backend/core/statistics_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior: "The removed statistics exchange route shell has been removed.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: FULL_ROUTE_EVIDENCE_NO_FIXTURE,
    },
    DomainGovernancePolicy {
        domain: "auth-security-user-data",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/auth_routes/mod.rs",
            "src/backend/db/auth.rs",
            "src/backend/db/auth_registration.rs",
            "src/backend/db/user_data.rs",
            "src/backend/core/auth/mod.rs",        ],
        tests_verified: &[
            "tests/backend/http/auth_runtime_contract.rs",
            "tests/backend/core/auth_security_contracts.rs",
            "tests/backend/core/ops_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Login, registration, token session list/revoke, API/MCP personal token generation, refresh token exchange, logout, account recovery email verification/resend/password forgot/reset, OAuth2 callback authorize disabled-safe/not-implemented response, profile, avatar, profile cloud settings, profile external-auth list/unlink, profile verification-email resend, system version, user-data statistics, user-data CSV/TSV export, destructive user-data clear, authenticated 2FA status, TOTP login verification, recovery-code login verification, 2FA write management, and step-up verification routes are Rust-owned; OAuth2 provider exchange is explicitly disabled-safe/not-implemented in the current workspace build.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_RUNTIME_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "taxonomy-rules-settings",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/taxonomy_routes/mod.rs",            "src/backend/core/adapters/category.rs",
            "src/backend/db/taxonomy",
        ],
        tests_verified: &["tests/backend/http/taxonomy_runtime_contract.rs"],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: TAXONOMY_DELETION_BLOCKERS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Account CRUD/display-order/balance sync/transaction move-clear, account-rule list/create/update/delete/reorder/test, tag CRUD/display-order/batch-create, category master-data/statistics/update-all, category-rule list/create/update/delete/reorder/defaults/test, rule overview, templates, and settings bundle import/preview/export routes are Rust-owned; all removed taxonomy route shells have been removed.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "matching-recurring-calendar-networth",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/matching_routes.rs",
            "src/backend/db/matching.rs",
            "src/backend/db/recurring.rs",
            "src/backend/db/statistics.rs",            "src/backend/core/matching.rs",
            "src/backend/core/statistics.rs",
        ],
        tests_verified: &[
            "tests/backend/core/matching_contracts.rs",
            "tests/backend/http/matching_runtime_contract.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Formal matching candidates, feedback, manual pairs, candidate actions, reconcile history, retired investment settings, recurring suggestions, calendar, and net worth runtime routes are Rust-owned; the removed matching, recurring, calendar, and networth route shells have been removed.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "backup-ops",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/http/backup_routes/mod.rs",
            "src/backend/http/backup_sync.rs",
            "src/backend/core/ops.rs",
            "src/backend/db/backup.rs",
        ],
        tests_verified: &[
            "tests/backend/core/ops_contracts.rs",
            "tests/backend/db/backup_runtime.rs",
            "tests/backend/http/backup_runtime_contract.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: EMPTY_STRINGS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Backup file list/create/download/delete/cleanup, jobs, and cloud sync provider runtime routes are Rust-owned; restore and history recovery routes are intentionally absent.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "database-schema",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/db/postgres.rs",
            "src/backend/db/postgres/migrations",
            "src/backend/db/runtime.rs",
        ],
        tests_verified: &[
            "tests/backend/core/runtime_governance_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Rust now owns PostgreSQL foundational schema initialization and core user-scoped constraints; non-Postgres schema/migration/recovery paths are absent.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_SCHEMA_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "database-repositories",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/db/bills.rs",
            "src/backend/db/budgets.rs",
            "src/backend/db/import_staging.rs",
            "src/backend/db/auth.rs",
            "src/backend/db/statistics.rs",
            "src/backend/db/app_settings.rs",
            "src/backend/db/taxonomy",
        ],
        tests_verified: &["tests/backend/db/vector_outbox.rs"],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: POSTGRES_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "Repository implementations are PostgreSQL-only across Rust-owned domains; non-Postgres repository, recovery, and migration facades are absent.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: DB_REPOSITORY_EVIDENCE,
    },
    DomainGovernancePolicy {
        domain: "database-facade",
        retired_source_files: EMPTY_STRINGS,
        rust_owner_files: &[
            "src/backend/core/runtime_governance.rs",
            "src/backend/db/runtime.rs",
            "src/backend/db/postgres.rs",
        ],
        tests_verified: &[
            "tests/backend/core/runtime_governance_contracts.rs",
        ],
        fixtures: EMPTY_STRINGS,
        db_invariant_ids: IMPORT_DB_INVARIANT_IDS,
        coverage_evidence: COVERAGE_EVIDENCE_CONTRACT,
        deletion_blockers: EMPTY_STRINGS,
        blocked_status: RuntimeBlockedStatus::None,
        unsupported_behavior:
            "The database-facade contract is governance-only and does not, by itself, operate on removed runtime entry points.",
        decision_required: DecisionRequired::None,
        decision_owner: "none",
        transition_evidence: ROUTE_MATRIX_ONLY_EVIDENCE,
    },
];

const RUST_ENVELOPE_CONTEXT: &[RuntimeState] = &[
    RuntimeState::RustImplemented,
    RuntimeState::RustOwnedVerified,
    RuntimeState::Retired,
];
const CONTRACT_ENVELOPE_CONTEXT: &[RuntimeState] = &[RuntimeState::ContractOnly];

const ENVELOPE_POLICIES: &[ResponseEnvelopePolicy] = &[
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RustHttpShell,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "typed Rust health/runtime JSON",
        error_shape: "typed Rust shell error when applicable",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RuntimeInfrastructureError,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "none",
        error_shape: "ApiResponse success=false error.code/message",
        runtime_may_wrap: true,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::CurrentSuccessData,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data emitted by current Rust routes",
        error_shape: "success=false error/code/message in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::CurrentSuccessResult,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result/message emitted by current Rust routes",
        error_shape: "success=false error/code/message in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::RawPassthrough,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "raw Rust file/download bodies and headers",
        error_shape: "current API file/download error response",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::BillsCrud,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result with frontend transaction DTO or page wrapper",
        error_shape: "success=false error in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::BudgetsCrud,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape:
            "success=true result/message with budget row/list/export/execution/forecast payload",
        error_shape: "success=false error in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::StatisticsRead,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result/data with DB-backed statistics aggregation payload",
        error_shape: "success=false error/message in current API shape",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportV2Stage,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data with import stage payload",
        error_shape: "success=false error/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportPreviewAction,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data with preview projection",
        error_shape: "success=false error plus expectedState when stale",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data responseMode=preview-item",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::LearningRoute,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data/result for learning routes",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::LlmPreview,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true data/result for LLM preview/memory",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::OcrMl,
        route_contexts: RUST_ENVELOPE_CONTEXT,
        success_shape: "success=true result for Rust OCR config and receipt recognition routes",
        error_shape: "success=false code/error_code/message",
        runtime_may_wrap: false,
    },
    ResponseEnvelopePolicy {
        family: ResponseEnvelopeFamily::ContractOracle,
        route_contexts: CONTRACT_ENVELOPE_CONTEXT,
        success_shape: "compile-time contract data only",
        error_shape: "none",
        runtime_may_wrap: false,
    },
];

const IMPORT_DB_WRITE_INVARIANTS: [DbWriteInvariant; 8] = [
    DbWriteInvariant {
        key: "postgres_authority",
        description: "Business writes must target PostgreSQL authority.",
    },
    DbWriteInvariant {
        key: "referential_integrity",
        description: "PostgreSQL schema must enforce relational integrity for current-user data.",
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
        key: "postgres_authority",
        description: "Business writes must target PostgreSQL authority.",
    },
    DbWriteInvariant {
        key: "referential_integrity",
        description: "PostgreSQL schema must enforce relational integrity for current-user data.",
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
                "Account CRUD/display-order/balance sync/transaction move-clear, account-rule list/create/update/delete/reorder/test, tag CRUD/display-order/batch-create, category master-data/statistics/update-all, category-rule list/create/update/delete/reorder/defaults/test, rule overview, templates, settings bundle import/preview/export, and settings encryption status routes are Rust-owned; all removed taxonomy route shells have been removed.",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("taxonomy-rules-settings", RuntimeState::Retired) => RouteContractDetails {
            handler: RouteHandlerId::TaxonomyRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("bills-crud", RuntimeState::Retired) => RouteContractDetails {
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
        ("bills-crud-adjacent", RuntimeState::Retired) => RouteContractDetails {
            handler: RouteHandlerId::BillsCrudRuntime,
            deletion_blockers: EMPTY_STRINGS,
            blocked_status: RuntimeBlockedStatus::None,
            unsupported_behavior: "",
            decision_required: DecisionRequired::None,
            decision_owner: "none",
            transition_evidence: DB_RUNTIME_EVIDENCE,
        },
        ("bills-recurring", RuntimeState::Retired) => RouteContractDetails {
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
