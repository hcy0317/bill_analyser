use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{HttpShellConfig, ImportRouteMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyFallback {
    Python,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpShellIdentity {
    pub crate_name: String,
    pub version: String,
    pub runtime_boundary: String,
    pub business_migration: String,
    pub api_takeover: bool,
    pub proxy_fallback: ProxyFallback,
}

impl HttpShellIdentity {
    pub fn current() -> Self {
        Self::for_import_route_mode(ImportRouteMode::ProxyOnly)
    }

    pub fn for_import_route_mode(import_route_mode: ImportRouteMode) -> Self {
        let (runtime_boundary, business_migration) = match import_route_mode {
            ImportRouteMode::ProxyOnly => ("rust-http-shell:proxy-only", "none"),
            ImportRouteMode::ImportRouteSkeleton => (
                "rust-http-shell:import-route-skeleton",
                "import-route-skeleton-no-db",
            ),
            ImportRouteMode::ImportDbRuntime => (
                "rust-http-shell:import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-recurring-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-export-runtime+auth-login-register-token-account-recovery-profile-cloud-external-auth-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime",
                "import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime-partial+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-export-runtime+auth-login-register-token-session-personal-refresh-logout-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime",
            ),
        };

        Self {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_boundary: runtime_boundary.to_string(),
            business_migration: business_migration.to_string(),
            api_takeover: true,
            proxy_fallback: ProxyFallback::Python,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpShellHealth {
    pub status: String,
    pub identity: HttpShellIdentity,
    pub details: BTreeMap<String, String>,
}

pub fn http_shell_health(config: &HttpShellConfig) -> HttpShellHealth {
    let mut details = BTreeMap::new();
    details.insert(
        "owned_routes".to_string(),
        if config.import_route_mode.intercepts_import_routes() {
            if config.import_route_mode == ImportRouteMode::ImportDbRuntime {
                "/api/health,/api/runtime,import/preview-adjacent runtime routes,bills CRUD runtime routes,bills picture runtime routes,bills export runtime route,bills recurring runtime routes,bills reconciliation runtime route,bills category actions runtime routes,budgets CRUD/execution/forecast/history/import runtime routes,statistics read runtime routes,taxonomy account CRUD/display-order/sync-balances/transaction-action runtime routes,taxonomy tag CRUD/display-order runtime routes,taxonomy category master-data/statistics/category-rule-list-test/rule-overview/settings-bundle-export runtime routes,taxonomy templates CRUD/display-order runtime routes,auth login/register/token/account-recovery/profile/cloud/external-auth/system/user-data-statistics-export-clear/2fa-status/2fa-verify/2fa-recovery-verify/2fa-write/step-up runtime routes".to_string()
            } else {
                "/api/health,/api/runtime,import/preview-adjacent runtime routes".to_string()
            }
        } else {
            "/api/health,/api/runtime".to_string()
        },
    );
    details.insert(
        "proxied_routes".to_string(),
        if config.import_route_mode == ImportRouteMode::ProxyOnly {
            "unowned /api/*".to_string()
        } else {
            "manifest PythonProxied endpoints only".to_string()
        },
    );
    details.insert(
        "import_route_mode".to_string(),
        config.import_route_mode.as_str().to_string(),
    );
    if config.import_route_mode.intercepts_import_routes() {
        details.insert(
            "import_skeleton_routes".to_string(),
            "first-phase deletion-blocked import/preview-adjacent endpoints".to_string(),
        );
    }
    if config.import_route_mode == ImportRouteMode::ImportDbRuntime {
        details.insert(
            "sqlite_db_path_configured".to_string(),
            config.sqlite_db_path.is_some().to_string(),
        );
        details.insert(
            "bills_crud_runtime".to_string(),
            "owned core bills/transactions CRUD, transaction picture upload/unused cleanup, bills CSV/XLSX export, recurring candidates/match, reconciliation statements, and category quick actions".to_string(),
        );
        details.insert(
            "budgets_crud_runtime".to_string(),
            "owned budgets CRUD/export/execution/forecast/history/snapshot/import".to_string(),
        );
        details.insert(
            "statistics_read_runtime".to_string(),
            "owned DB-backed category statistics, category trends, asset trends, category pie, top merchants, and transaction amounts; analyzer overview/comparison and live exchange providers proxied".to_string(),
        );
        details.insert(
            "taxonomy_accounts_runtime".to_string(),
            "owned account list/detail/create/update/delete/display-order, balance sync, and transaction move/clear routes with frontend cents to SQLite yuan conversion, bill-derived SQLite yuan balance recalculation, sensitive-operation password fallback, and account audit metadata".to_string(),
        );
        details.insert(
            "taxonomy_tags_runtime".to_string(),
            "owned tag list/detail/create/update/delete/display-order and batch-create routes with user-scoped DB writes".to_string(),
        );
        details.insert(
            "taxonomy_categories_runtime".to_string(),
            "owned category list/tree/flat/detail/create/update/delete/batch/move/import/export/all/statistics routes plus category-rule list/create/update/delete/reorder/defaults/migrate/test, rule overview, and settings bundle export routes with user-scoped DB reads and writes; settings bundle import/preview remain proxied".to_string(),
        );
        details.insert(
            "taxonomy_templates_runtime".to_string(),
            "owned template list/detail/create/update/delete/display-order routes with user-scoped DB reads and writes; recurring matching/binding and settings bundle import/preview remain proxied".to_string(),
        );
        details.insert(
            "auth_token_runtime".to_string(),
            "owned login, registration, token session list/revoke, API/MCP personal token generation, token refresh, logout, account-recovery email verification/resend/password forgot/reset, profile, avatar, profile cloud settings, profile external-auth list/unlink, profile verification resend, OAuth2 authorize disabled-safe/not-implemented, system version, user-data statistics/export/clear, 2FA status, 2FA TOTP login verification, 2FA recovery-code login verification, 2FA write management, and step-up verification routes; real OAuth provider exchange proxied".to_string(),
        );
    }
    details.insert(
        "python_upstream".to_string(),
        config.python_upstream.clone(),
    );
    details.insert(
        "business_api".to_string(),
        "rust-primary-http; unmigrated domains reverse-proxied".to_string(),
    );

    HttpShellHealth {
        status: "ok".to_string(),
        identity: HttpShellIdentity::for_import_route_mode(config.import_route_mode),
        details,
    }
}
