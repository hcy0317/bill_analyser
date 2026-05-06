//! Internal Rust foundations for the Bill Analyser backend migration.
//!
//! Flask remains the REST shell. The crate currently contains migration
//! foundations and shared primitives only; no business API is migrated here.

pub mod adapters;
pub mod auth;
pub mod category_rules;
pub mod error;
pub mod parsers;
pub mod primitives;
pub mod response;
pub mod runtime;

pub use adapters::{account, api, category, transaction};
pub use error::{ErrorCode, RuntimeError};
pub use parsers::{
    aggregate_description, build_parser_tags, normalize_amount_text, normalize_parser_tags,
    normalize_transaction_type, parser_registry, parser_source_label, post_process_raw_bills,
    resolve_parser_tags, serialize_parser_tags, ParserInfo, RawBill, StandardBill,
};
pub use primitives::{
    normalize_bill_date_text, parse_bill_datetime, AuthContext, BillDateTime, CurrencyCode,
    EntityId, Money, Pagination, SortField, SortOrder, TransactionType, UnixTimestampSeconds,
    UserId, UtcOffsetMinutes,
};
pub use response::{ApiError, ApiResponse};
pub use runtime::{
    runtime_health, runtime_identity_json, RuntimeHealth, RuntimeIdentity, RuntimeStatus,
};
