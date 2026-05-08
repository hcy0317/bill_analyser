//! Migration ownership contracts for the Python-to-Rust backend replacement.
//!
//! These contracts are deliberately conservative: they record what Rust owns
//! now, what is still proxied to Python, and which evidence is required before
//! a Python import path may be disabled or deleted.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteOwner {
    RustOwned,
    PythonProxied,
    ContractOnly,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseEnvelopeFamily {
    RustHttpShell,
    ProxyInfrastructureError,
    FlaskSuccessData,
    FlaskSuccessResult,
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
    pub owner: RouteOwner,
    pub envelope: ResponseEnvelopeFamily,
    pub deletion_blocked_until_all_import_gates: bool,
    pub notes: &'static str,
}

impl EndpointOwnership {
    pub fn is_import_deletion_blocked(&self) -> bool {
        self.deletion_blocked_until_all_import_gates
    }

    pub fn is_python_runtime_owner(&self) -> bool {
        self.owner == RouteOwner::PythonProxied
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseEnvelopePolicy {
    pub family: ResponseEnvelopeFamily,
    pub route_contexts: &'static [RouteOwner],
    pub success_shape: &'static str,
    pub error_shape: &'static str,
    pub proxy_may_wrap: bool,
}

impl ResponseEnvelopePolicy {
    pub fn applies_to_owner(&self, owner: RouteOwner) -> bool {
        self.route_contexts.contains(&owner)
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

const OWNERSHIP_MATRIX: &[EndpointOwnership] = &[
    EndpointOwnership {
        method: "GET",
        pattern: "/api/health",
        domain: "api-runtime-shell",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::RustHttpShell,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust HTTP shell health route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/runtime",
        domain: "api-runtime-shell",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::RustHttpShell,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust HTTP shell metadata route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns core list route; Python CRUD deletion remains blocked by the S9e residual-reference review, not by the import skeleton registry.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns trailing-slash list route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns core create route with cents-to-yuan adapter semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns trailing-slash create route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/by-month",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns month list route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/get",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns legacy get-by-query route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST get route.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST update route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/{bill_id}",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns REST delete route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/modify",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns legacy modify route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/delete",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns legacy delete route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/batch",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch create route with a single SQLite transaction.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/batch/update",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch update route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/batch/delete",
        domain: "bills-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BillsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust bills_crud_runtime owns batch delete route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/export",
        domain: "bills-crud-adjacent",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Bills export remains Python-proxied until the export/media/reconciliation slice owns streaming response semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/pictures*",
        domain: "bills-crud-adjacent",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Transaction picture upload/remove remains Python-proxied.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/reconciliation_statements",
        domain: "bills-crud-adjacent",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Reconciliation statements remain Python-proxied until reconciliation runtime migrates.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/{bill_id}/recurring-candidates",
        domain: "bills-recurring",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Recurring candidate generation remains Python-proxied.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/{bill_id}/recurring-match",
        domain: "bills-recurring",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Recurring match decisions remain Python-proxied.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/category/*",
        domain: "bills-category-actions",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Category action helpers remain Python-proxied until matching/category-rule runtime migrates.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns core list route; execution, forecast, history, and import are Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns trailing-slash list route.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns core create route with yuan-style budget amount semantics.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns trailing-slash create route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST get route.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST update route.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/budgets/{budget_id}",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns REST delete route.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/export",
        domain: "budgets-crud",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_crud_runtime owns JSON export for budget rows.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/execution",
        domain: "budgets-analysis",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_execution_runtime owns budget execution aggregation.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/forecast",
        domain: "budgets-analysis",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_forecast_runtime owns historical bill aggregation, budget-map projection, forecast summary, and period-progress shape.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/budgets/history",
        domain: "budgets-history",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_history_runtime owns persisted snapshot lookup, exact-period preference, category enrichment, and on-demand fallback.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/history/snapshot",
        domain: "budgets-history",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_history_runtime owns budget_history replacement writes with canonical filter_summary and user scope.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/budgets/import",
        domain: "budgets-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::BudgetsCrud,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust budgets_import_runtime owns Flask-compatible array validation, per-item error accounting, and user-scoped upsert-by-name writes.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-statistics",
        domain: "statistics-read",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns DB-backed category/account cents aggregation with timestamp range and keyword filtering.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-statistics/trends",
        domain: "statistics-read",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns monthly category/account trend buckets for bounded and all-mode ranges.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/asset-trends",
        domain: "statistics-read",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns DB-backed daily asset trend balances with 365-day bounded-range guard.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category-pie",
        domain: "statistics-read",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns category pie aggregation for bill type/date filters.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/top-merchants",
        domain: "statistics-read",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns top merchant aggregation for date filters.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/amounts",
        domain: "statistics-read",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::StatisticsRead,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust statistics_read_runtime owns transaction amount period aggregation with CNY cents response semantics.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/overview",
        domain: "statistics-analyzer",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer overview remains Python-proxied until the full Analyzer report runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/trends",
        domain: "statistics-analyzer",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer trends remain Python-proxied until the full Analyzer trend runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/comparison",
        domain: "statistics-analyzer",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer comparison remains Python-proxied until the full Analyzer comparison runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/category",
        domain: "statistics-analyzer",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer category route remains Python-proxied until the full Analyzer category runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/trend",
        domain: "statistics-analyzer",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Analyzer trend alias remains Python-proxied until the full Analyzer trend runtime is Rust-owned.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/statistics/exchange-rates",
        domain: "statistics-exchange",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Live exchange provider fetch and custom-rate precedence remain Python-proxied until Rust owns provider execution and custom-rate persistence.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/statistics/exchange-rates/custom",
        domain: "statistics-exchange",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Custom exchange-rate writes remain Python-proxied until Rust owns provider execution and custom-rate persistence together.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/statistics/exchange-rates/custom/{currency}",
        domain: "statistics-exchange",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessResult,
        deletion_blocked_until_all_import_gates: false,
        notes: "Custom exchange-rate deletion remains Python-proxied until Rust owns provider execution and custom-rate persistence together.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/parse",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/parse_generic",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/dedup",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/confirm",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/session/{session_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/v2/session/{session_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportV2Stage,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview/{session_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview/{session_id}/index",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview/{session_id}/update",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/reclassify/{session_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewAction,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-candidates",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::ImportPreviewItemDecision,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/learning/{session_id}/suggestions",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this preview-bypass route; it applies supplied preview updates and returns an explicit empty suggestion list without provider/model execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/v2/learning/{session_id}/suggestions",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this preview-bypass route; it validates the session and returns an explicit empty suggestion list without provider/model execution.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/v2/learning/{session_id}/promote",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this DB-write decision route and promotes stored session suggestions into import learning rules.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/preview",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/confirm",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/batch",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/parse_import",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/upload",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/parsers",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/reclassify",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/configs",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs/match",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/bills/import/configs/suggest",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/configs/{config_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/bills/import/learning-rules",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/bills/import/learning-rules/{rule_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/bills/import/learning-rules/{rule_id}",
        domain: "bills-import",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "True LLM preview recommendation generation is proxied to Python until Rust owns provider execution; Rust keeps accept/reject/memory preview decisions.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend/accept",
        domain: "ai-learning-llm",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/preview-recommend/reject",
        domain: "ai-learning-llm",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/llm/memory",
        domain: "ai-learning-llm",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::LlmPreview,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns this route with provider-bypassed semantics; Python import deletion stays blocked until all five gates pass.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/analyze-transactions",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "True LLM transaction analysis and import-session rule induction are proxied to Python until Rust owns provider execution.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/llm/rule-synthesis",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "True rule-synthesis provider generation is proxied to Python until Rust owns provider execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/learning/suggestions",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center listing remains proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/generate",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::FlaskSuccessData,
        deletion_blocked_until_all_import_gates: false,
        notes: "Learning suggestion mining/generation remains proxied to Python until Rust owns the learning model loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/{suggestion_id}/accept",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center decisions remain proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/batch-accept",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center decisions remain proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/learning/suggestions/{suggestion_id}/reject",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center decisions remain proxied to Python until Rust owns suggestion mining, persistence, and decisions as one loop.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/learning/rules",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/learning/rules/{rule_id}/toggle",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/learning/rules/{rule_id}",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "DELETE",
        pattern: "/api/learning/rules/{rule_id}",
        domain: "ai-learning-llm",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::LearningRoute,
        deletion_blocked_until_all_import_gates: false,
        notes: "Global Learning Center rules remain proxied to Python until Rust owns the full Learning Center rule store contract.",
    },
    EndpointOwnership {
        method: "POST",
        pattern: "/api/ml/receipt-recognition",
        domain: "ai-ocr",
        owner: RouteOwner::PythonProxied,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: false,
        notes: "Receipt image recognition is deliberately proxied to Python until Rust owns real OCR provider execution.",
    },
    EndpointOwnership {
        method: "GET",
        pattern: "/api/ml/receipt-recognition/config",
        domain: "ai-ocr",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns OCR config; true receipt recognition remains Python-proxied.",
    },
    EndpointOwnership {
        method: "PUT",
        pattern: "/api/ml/receipt-recognition/config",
        domain: "ai-ocr",
        owner: RouteOwner::RustOwned,
        envelope: ResponseEnvelopeFamily::OcrMl,
        deletion_blocked_until_all_import_gates: true,
        notes: "Rust import_db_runtime owns OCR config; true receipt recognition remains Python-proxied.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://import-v2-envelope-oracle",
        domain: "bills-import",
        owner: RouteOwner::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins import envelope families for Rust-owned import_db_runtime routes.",
    },
    EndpointOwnership {
        method: "CONTRACT",
        pattern: "contract://sqlite-import-writer-policy",
        domain: "database-facade",
        owner: RouteOwner::ContractOnly,
        envelope: ResponseEnvelopeFamily::ContractOracle,
        deletion_blocked_until_all_import_gates: false,
        notes: "Rust core pins DB writer invariants for import_db_runtime takeover.",
    },
];

const RUST_ENVELOPE_CONTEXT: &[RouteOwner] = &[RouteOwner::RustOwned];
const PYTHON_PROXIED_ENVELOPE_CONTEXT: &[RouteOwner] = &[RouteOwner::PythonProxied];
const CONTRACT_ENVELOPE_CONTEXT: &[RouteOwner] = &[RouteOwner::ContractOnly];
const RUST_OR_PYTHON_PROXIED_ENVELOPE_CONTEXT: &[RouteOwner] =
    &[RouteOwner::RustOwned, RouteOwner::PythonProxied];

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
        route_contexts: PYTHON_PROXIED_ENVELOPE_CONTEXT,
        success_shape: "success=true result/message",
        error_shape: "success=false error/code/message as emitted by Flask route",
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

pub fn rust_http_shell_ownership_matrix() -> &'static [EndpointOwnership] {
    OWNERSHIP_MATRIX
}

pub fn endpoints_by_owner(owner: RouteOwner) -> Vec<&'static EndpointOwnership> {
    OWNERSHIP_MATRIX
        .iter()
        .filter(|endpoint| endpoint.owner == owner)
        .collect()
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
        invariants: &IMPORT_DB_WRITE_INVARIANTS,
    }
}

pub fn budgets_crud_db_writer_policy() -> DbWriterPolicy {
    DbWriterPolicy {
        domain: "budgets-crud",
        mode: DbWriterMode::RustDomainOwned,
        active_writer: "crates/bill-analyser-http/src/budget_routes.rs + crates/bill-analyser-db/src/budgets.rs via Rust budgets_crud_runtime",
        rust_write_allowed: true,
        invariants: &IMPORT_DB_WRITE_INVARIANTS,
    }
}
