//! Internal SQLite runtime foundations for the Bill Analyser Rust migration.
//!
//! This crate is scaffolding only. Python/Flask keeps the business runtime
//! shell, and no business database write path is Rust-primary here.

pub mod connection;
pub mod error;
pub mod path;
pub mod schema;
pub mod taxonomy;
pub mod transaction;
pub mod user_scope;

pub use connection::{PragmaSnapshot, SqliteConnectionConfig, SqliteRuntime};
pub use error::{DbError, DbResult};
pub use path::SqliteDbPath;
pub use schema::{schema_inventory, SchemaDryRun, SchemaDryRunReport, SchemaResponsibility};
pub use transaction::run_transaction;
pub use user_scope::UserScope;
