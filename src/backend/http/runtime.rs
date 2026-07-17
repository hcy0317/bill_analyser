// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    config::{HttpShellConfig, ImportRouteMode},
    database_runtime::DatabaseRuntimeBoundary,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpShellIdentity {
    pub crate_name: String,
    pub version: String,
    pub runtime_boundary: String,
}

impl HttpShellIdentity {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn current() -> Self {
        Self::for_import_route_mode(ImportRouteMode::ImportDbRuntime)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn for_import_route_mode(import_route_mode: ImportRouteMode) -> Self {
        let runtime_boundary = match import_route_mode {
            ImportRouteMode::ImportDbRuntime =>
                "rust-http:postgres-authority+weaviate-required+bills+budgets+matching+statistics+taxonomy+import+ai+auth+backup-runtime",
        };

        Self {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_boundary: runtime_boundary.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpShellHealth {
    pub status: String,
    pub identity: HttpShellIdentity,
    pub details: BTreeMap<String, String>,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn http_shell_health(config: &HttpShellConfig) -> HttpShellHealth {
    http_shell_health_with_weaviate_status(config, config.weaviate.status_without_probe())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn http_shell_health_with_weaviate_status(
    config: &HttpShellConfig,
    weaviate_status: &str,
) -> HttpShellHealth {
    let mut details = BTreeMap::new();
    details.insert(
        "owned_routes".to_string(),
        "/api/health,/api/health/live,/api/health/ready,/api/runtime,import/preview-adjacent runtime routes,bills CRUD runtime routes,bills picture runtime routes,bills export runtime route,bills recurring runtime routes,bills reconciliation runtime route,bills category actions runtime routes,budgets CRUD/execution/forecast/history/import runtime routes,matching/recurring/calendar/networth runtime routes,statistics read/analyzer/exchange runtime routes,taxonomy account CRUD/display-order/sync-balances/transaction-action/account-rule list-create-update-delete-reorder-test runtime routes,taxonomy tag CRUD/display-order runtime routes,taxonomy category master-data/statistics/category-rule-list-test/rule-overview/settings-bundle import-export runtime routes,taxonomy templates CRUD/display-order runtime routes,global Learning Center suggestions/rules runtime routes,LLM config/candidates/provider-generation runtime routes,OCR config and receipt recognition runtime routes,auth login/register/token/OAuth2 authorize/profile/cloud/external-auth/system/user-data-statistics-export-clear/2FA status/verify/recovery-code/write/step-up runtime routes,backup ops runtime routes".to_string(),
    );
    details.insert(
        "import_route_mode".to_string(),
        config.import_route_mode.as_str().to_string(),
    );
    details.insert(
        "database_backend".to_string(),
        config.database_backend.as_str().to_string(),
    );
    let database_boundary = DatabaseRuntimeBoundary::from_config(config);
    details.insert(
        "route_repository_backend".to_string(),
        database_boundary.route_repository_backend_str().to_string(),
    );
    let postgres_authority_status = DatabaseRuntimeBoundary::postgres_authority_status(config);
    details.insert(
        "postgres_authority_status".to_string(),
        postgres_authority_status.to_string(),
    );
    details.insert(
        "postgres_configured".to_string(),
        config.postgres_configured().to_string(),
    );
    details.insert(
        "postgres_url_redacted".to_string(),
        config
            .redacted_postgres_url()
            .unwrap_or_else(|| "unconfigured".to_string()),
    );
    details.insert("weaviate_status".to_string(), weaviate_status.to_string());
    details.insert(
        "weaviate_endpoint_redacted".to_string(),
        config.weaviate.redacted_endpoint(),
    );
    details.insert(
        "weaviate_api_key_configured".to_string(),
        config.weaviate.api_key_configured().to_string(),
    );
    details.insert(
        "weaviate_collection_prefix".to_string(),
        config.weaviate.collection_prefix.clone(),
    );
    details.insert("weaviate_required".to_string(), "true".to_string());
    if config.import_route_mode == ImportRouteMode::ImportDbRuntime {
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
            "owned DB-backed category statistics, category trends, asset trends, category pie, top merchants, transaction amounts, analyzer overview/trends/comparison/category/trend, exchange provider fallback, and user custom exchange rates".to_string(),
        );
        details.insert(
            "matching_recurring_calendar_networth_runtime".to_string(),
            "owned formal matching candidates, feedback, manual pairs, candidate actions, reconcile history, retired investment settings, recurring suggestion list/detect/accept/reject, calendar event aggregation with recurring projections, and net-worth snapshot routes".to_string(),
        );
        details.insert(
            "taxonomy_accounts_runtime".to_string(),
            "owned account list/detail/create/update/delete/display-order, balance sync, and transaction move/clear routes with frontend cents to PostgreSQL balance_cents conversion, bill-derived balance recalculation, sensitive-operation password fallback, and account audit metadata".to_string(),
        );
        details.insert(
            "taxonomy_tags_runtime".to_string(),
            "owned tag list/detail/create/update/delete/display-order and batch-create routes with user-scoped DB writes".to_string(),
        );
        details.insert(
            "taxonomy_categories_runtime".to_string(),
            "owned category list/tree/flat/detail/create/update/delete/batch/move/import/export/all/statistics routes plus category-rule list/create/update/delete/reorder/defaults/test, account-rule list/create/update/delete/reorder/test, rule overview, and settings bundle import/preview/export routes with user-scoped DB reads and writes".to_string(),
        );
        details.insert(
            "taxonomy_templates_runtime".to_string(),
            "owned template list/detail/create/update/delete/display-order routes with user-scoped DB reads and writes; recurring matching/binding is handled by Rust-owned matching routes".to_string(),
        );
        details.insert(
            "auth_token_runtime".to_string(),
            "owned login, registration, token session list/revoke, API/MCP personal token generation, token refresh, logout, account-recovery email verification/resend/password forgot/reset, OAuth2 authorize disabled-safe/not-implemented, profile, avatar, profile cloud settings, profile external-auth list/unlink, profile verification resend, system version, user-data statistics/export/clear, 2FA status, 2FA TOTP login verification, 2FA recovery-code login verification, 2FA write management, and step-up verification routes; no OAuth provider exchange fallback remains in the current workspace contract".to_string(),
        );
        details.insert(
            "backup_ops_runtime".to_string(),
            "owned backup file list/create/download/delete/verify/cleanup, job list/save, and cloud sync routes backed by Rust zip/Fernet file I/O plus PostgreSQL backup_records, backup_jobs, and backup_audit_logs metadata; restore and historical database recovery are not runtime capabilities".to_string(),
        );
    }
    details.insert("business_api".to_string(), "rust-only-http".to_string());

    HttpShellHealth {
        status: if DatabaseRuntimeBoundary::route_repository_runtime_is_healthy(config)
            && weaviate_status == "healthy"
        {
            "ok".to_string()
        } else {
            "unhealthy".to_string()
        },
        identity: HttpShellIdentity::for_import_route_mode(config.import_route_mode),
        details,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn http_shell_readiness_with_dependency_statuses(
    config: &HttpShellConfig,
    postgres_status: &str,
    weaviate_status: &str,
) -> HttpShellHealth {
    let mut health = http_shell_health_with_weaviate_status(config, weaviate_status);
    health
        .details
        .insert("probe".to_string(), "readiness".to_string());
    health.details.insert(
        "postgres_authority_status".to_string(),
        postgres_status.to_string(),
    );
    health.status = if postgres_status == "healthy" && weaviate_status == "healthy" {
        "ok".to_string()
    } else {
        "unhealthy".to_string()
    };
    health
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn http_shell_liveness(config: &HttpShellConfig) -> HttpShellHealth {
    let mut health = http_shell_health_with_weaviate_status(config, "not_probed:liveness");
    health.status = "ok".to_string();
    health
        .details
        .insert("probe".to_string(), "liveness".to_string());
    health.details.insert(
        "postgres_authority_status".to_string(),
        "not_probed:liveness".to_string(),
    );
    health
}

#[cfg(test)]
mod tests {
    use super::{
        http_shell_health_with_weaviate_status, http_shell_readiness_with_dependency_statuses,
    };
    use crate::HttpShellConfig;

    #[test]
    fn health_details_expose_current_postgres_authority_status_only() {
        let config = HttpShellConfig::default();
        let health = http_shell_health_with_weaviate_status(&config, "healthy");

        assert_eq!(health.status, "ok");
        assert_eq!(
            health.details.get("route_repository_backend"),
            Some(&"postgres_authority".to_string())
        );
        assert_eq!(
            health.details.get("postgres_authority_status"),
            Some(&"complete:postgres_authority".to_string())
        );
    }

    #[test]
    fn readiness_does_not_allow_healthy_weaviate_to_mask_postgres_outage() {
        let config = HttpShellConfig::default();
        let health = http_shell_readiness_with_dependency_statuses(
            &config,
            "unhealthy:unavailable",
            "healthy",
        );

        assert_eq!(health.status, "unhealthy");
        assert_eq!(
            health.details.get("postgres_authority_status"),
            Some(&"unhealthy:unavailable".to_string())
        );
        assert_eq!(
            health.details.get("weaviate_status"),
            Some(&"healthy".to_string())
        );
        assert_eq!(health.details.get("probe"), Some(&"readiness".to_string()));
    }
}
