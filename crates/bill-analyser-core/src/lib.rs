//! Internal Rust foundations for the Bill Analyser backend migration.
//!
//! This crate is deliberately limited to runtime shell contracts in S1. Flask
//! remains the REST shell and no business domain is migrated here.

pub mod error;
pub mod response;
pub mod runtime;

pub use error::{ErrorCode, RuntimeError};
pub use response::{ApiError, ApiResponse};
pub use runtime::{
    runtime_health, runtime_identity_json, RuntimeHealth, RuntimeIdentity, RuntimeStatus,
};
