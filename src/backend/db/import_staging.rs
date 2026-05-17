use bill_analyser_core::{normalize_bill_date_text, DedupBill, Money, UserId};
use bill_analyser_parsers::{serialize_parser_tags, StandardBill};
use chrono::Utc;
use rusqlite::ffi::{SQLITE_CONSTRAINT_PRIMARYKEY, SQLITE_CONSTRAINT_UNIQUE};
use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, ErrorCode, OptionalExtension,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{run_transaction, DbError, DbResult};

const LLM_MEMORY_PROMPT_TEXT_MAX_BYTES: usize = 16_384;

include!("import_staging/types.rs");
include!("import_staging/schema_sessions.rs");
include!("import_staging/preview_reads.rs");
include!("import_staging/parser_templates.rs");
include!("import_staging/preview_updates.rs");
include!("import_staging/decisions.rs");
include!("import_staging/llm_memory.rs");
include!("import_staging/selection_confirm.rs");
include!("import_staging/rows.rs");
include!("import_staging/row_value_helpers.rs");
include!("import_staging/decision_snapshot_helpers.rs");
include!("import_staging/llm_payload_helpers.rs");
include!("import_staging/import_value_helpers.rs");
include!("import_staging/schema_legacy_helpers.rs");
