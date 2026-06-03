use std::sync::LazyLock;

use super::types::EndpointOwnership;

mod auth_contracts;
mod auth_taxonomy_backup;
mod import_ai;
mod matching_taxonomy;
mod runtime_bills_budget_statistics;

pub(super) static OWNERSHIP_MATRIX: LazyLock<Vec<EndpointOwnership>> = LazyLock::new(|| {
    let mut routes = Vec::new();
    routes.extend_from_slice(runtime_bills_budget_statistics::ROUTES);
    routes.extend_from_slice(import_ai::ROUTES);
    routes.extend_from_slice(auth_taxonomy_backup::ROUTES);
    routes.extend_from_slice(matching_taxonomy::ROUTES);
    routes.extend_from_slice(auth_contracts::ROUTES);
    routes
});
