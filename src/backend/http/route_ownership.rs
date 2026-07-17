use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

pub const FORBIDDEN_LEGACY_VERIFIED_ROUTES: &[(&str, &str)] =
    &[("PUT", "/api/bills/import/learning-rules/{rule_id}")];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LiveRouteRecord {
    pub method: String,
    pub pattern: String,
}

impl LiveRouteRecord {
    pub fn new(method: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            pattern: pattern.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteOwnershipMismatch {
    pub missing: Vec<LiveRouteRecord>,
    pub extra: Vec<LiveRouteRecord>,
    pub legacy: Vec<LiveRouteRecord>,
}

impl fmt::Display for RouteOwnershipMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime route ownership mismatch: missing={:?}; extra={:?}; legacy={:?}",
            self.missing, self.extra, self.legacy
        )
    }
}

impl std::error::Error for RouteOwnershipMismatch {}

pub fn validate_live_route_ownership(
    live_routes: &[LiveRouteRecord],
    live_ownership: &[LiveRouteRecord],
    forbidden_legacy_routes: &[(&str, &str)],
) -> Result<(), RouteOwnershipMismatch> {
    let live = live_routes.iter().cloned().collect::<BTreeSet<_>>();
    let owned = live_ownership.iter().cloned().collect::<BTreeSet<_>>();
    let forbidden = forbidden_legacy_routes
        .iter()
        .map(|(method, pattern)| LiveRouteRecord::new(*method, *pattern))
        .collect::<BTreeSet<_>>();

    let missing = owned.difference(&live).cloned().collect::<Vec<_>>();
    let extra = live.difference(&owned).cloned().collect::<Vec<_>>();
    let legacy = owned.intersection(&forbidden).cloned().collect::<Vec<_>>();
    if missing.is_empty() && extra.is_empty() && legacy.is_empty() {
        Ok(())
    } else {
        Err(RouteOwnershipMismatch {
            missing,
            extra,
            legacy,
        })
    }
}
