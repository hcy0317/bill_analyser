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
    #[serde(rename = "src/backend/core/runtime_governance/mod.rs::contract_oracle")]
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
                "src/backend/core/runtime_governance/mod.rs::contract_oracle"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RouteContractDetails {
    pub(super) handler: RouteHandlerId,
    pub(super) deletion_blockers: &'static [&'static str],
    pub(super) blocked_status: RuntimeBlockedStatus,
    pub(super) unsupported_behavior: &'static str,
    pub(super) decision_required: DecisionRequired,
    pub(super) decision_owner: &'static str,
    pub(super) transition_evidence: &'static [&'static str],
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
