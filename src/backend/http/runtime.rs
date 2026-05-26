// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
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
    pub business_migration: String,
    pub api_takeover: bool,
}

impl HttpShellIdentity {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn current() -> Self {
        Self::for_import_route_mode(ImportRouteMode::ImportDbRuntime)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn for_import_route_mode(import_route_mode: ImportRouteMode) -> Self {
        let (runtime_boundary, business_migration) = match import_route_mode {
            ImportRouteMode::ImportDbRuntime => (
                "rust-http:rust-only-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-recurring-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+matching-recurring-calendar-networth-runtime+statistics-read-runtime+statistics-analyzer-runtime+statistics-exchange-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-bundle-runtime+ai-learning-center-runtime+ai-llm-config-candidates-runtime+ai-llm-provider-generation-runtime+ai-ocr-recognition-runtime+auth-login-register-token-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime+backup-ops-sync-runtime",
                "import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-reconciliation-runtime+bills-category-actions-runtime+budgets-crud-execution-forecast-history-import-runtime+matching-recurring-calendar-networth-runtime+statistics-read-runtime+statistics-analyzer-runtime+statistics-exchange-runtime+taxonomy-accounts-runtime+taxonomy-tags-runtime+taxonomy-tags-batch-runtime+taxonomy-categories-runtime+taxonomy-templates-runtime+taxonomy-settings-bundle-runtime+ai-learning-center-runtime+ai-llm-config-candidates-runtime+ai-llm-provider-generation-runtime+ai-ocr-recognition-runtime+auth-login-register-token-session-personal-refresh-logout-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime+backup-ops-sync-runtime",
            ),
        };

        Self {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            runtime_boundary: runtime_boundary.to_string(),
            business_migration: business_migration.to_string(),
            api_takeover: true,
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
    let mut details = BTreeMap::new();
    details.insert(
        "owned_routes".to_string(),
        "/api/health,/api/runtime,import/preview-adjacent runtime routes,bills CRUD runtime routes,bills picture runtime routes,bills export runtime route,bills recurring runtime routes,bills reconciliation runtime route,bills category actions runtime routes,budgets CRUD/execution/forecast/history/import runtime routes,matching/recurring/calendar/networth runtime routes,statistics read/analyzer/exchange runtime routes,taxonomy account CRUD/display-order/sync-balances/transaction-action/account-rule list-create-update-delete-reorder-migrate-test runtime routes,taxonomy tag CRUD/display-order runtime routes,taxonomy category master-data/statistics/category-rule-list-test/rule-overview/settings-bundle import-export runtime routes,taxonomy templates CRUD/display-order runtime routes,global Learning Center suggestions/rules runtime routes,LLM config/candidates/provider-generation runtime routes,OCR config and receipt recognition runtime routes,auth login/register/token/account-recovery/OAuth2 authorize/profile/cloud/external-auth/system/user-data-statistics-export-clear/2fa-status/2fa-verify/2fa-recovery-verify/2fa-write/step-up runtime routes,backup ops runtime routes".to_string(),
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
    details.insert(
        "migration_mode".to_string(),
        config.migration_mode.as_str().to_string(),
    );
    details.insert(
        "migration_status".to_string(),
        "placeholder:not_started".to_string(),
    );
    details.insert(
        "weaviate_status".to_string(),
        "placeholder:disabled".to_string(),
    );
    details.insert(
        "require_postgres_after_cutover".to_string(),
        config.require_postgres_after_cutover.to_string(),
    );
    if config.import_route_mode == ImportRouteMode::ImportDbRuntime {
        details.insert(
            "sqlite_db_path_configured".to_string(),
            config.sqlite_db_path.is_some().to_string(),
        );
        details.insert(
            "sqlite_legacy_path_configured".to_string(),
            config.sqlite_legacy_path.is_some().to_string(),
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
            "owned DB-backed category statistics, category trends, asset trends, category pie, top merchants, transaction amounts, analyzer overview/trends/comparison/category/trend, exchange provider fallback, and user custom exchange rates".to_string(),
        );
        details.insert(
            "matching_recurring_calendar_networth_runtime".to_string(),
            "owned formal matching candidates, feedback, manual pairs, candidate actions, reconcile history, retired investment settings, recurring suggestion list/detect/accept/reject, calendar event aggregation with recurring projections, and net-worth snapshot routes".to_string(),
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
            "owned category list/tree/flat/detail/create/update/delete/batch/move/import/export/all/statistics routes plus category-rule list/create/update/delete/reorder/defaults/migrate/test, account-rule list/create/update/delete/reorder/migrate-aliases/test, rule overview, and settings bundle import/preview/export routes with user-scoped DB reads and writes".to_string(),
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
            "owned backup file list/create/download/delete/restore/verify/cleanup, job list/save, and cloud sync routes backed by Rust zip/Fernet file I/O, SQLite backup_ops schema, backup_records updates, safe restore validation, OSS/S3/COS/Azure/WebDAV upload execution, and backup audit log writes".to_string(),
        );
    }
    details.insert("business_api".to_string(), "rust-only-http".to_string());

    HttpShellHealth {
        status: "ok".to_string(),
        identity: HttpShellIdentity::for_import_route_mode(config.import_route_mode),
        details,
    }
}
