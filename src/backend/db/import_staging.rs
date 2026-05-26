// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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

// import_staging facade 只聚合 session/template/preview/decision/LLM memory
// 与 confirm 分片。路由层可以编排这些函数，但 staging 生命周期和 row mapping
// 必须继续留在这些 include 分片中。
include!("import_staging/types.rs");
include!("import_staging/ledger_types.rs");
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
