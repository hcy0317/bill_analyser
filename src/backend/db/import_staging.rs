// 中文导读：Postgres import staging adapter，负责当前 import v2 session、source、standard row、preview、confirm、LLM memory 与学习反馈写读。
// 维护重点：只实现 Postgres 当前运行态；不读取历史 non-Postgres 数据，不提供历史 staging schema 路径。
// 不变式：外部 session key 映射到 Postgres import_sessions.id，所有查询必须 user scoped。

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use bill_analyser_core::{
    build_transfer_source_snapshot, learning_lifecycle_is_auto_eligible,
    learning_lifecycle_signal_state, normalize_bill_date_text,
    transition_import_learning_lifecycle, DedupBill, DeduplicationType,
    ImportLearningLifecycleState, Money, TransferSourceSnapshot, UserId,
};
use bill_analyser_parsers::{parser_source_label, serialize_parser_tags, StandardBill};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row};

use crate::{
    bills::postgres_reads::batch_create_postgres_bills_in_transaction, BillCreateDraft, BillRecord,
    DbError, DbResult, PostgresPool,
};

const LLM_MEMORY_PROMPT_TEXT_MAX_BYTES: usize = 16_384;
const IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE: usize = 500;
const IMPORT_PREVIEW_FACET_LIMIT: i64 = 100;

include!("import_staging/types.rs");
include!("import_staging/ledger_types.rs");

include!("import_staging/session_lifecycle.rs");
include!("import_staging/parser_templates.rs");
include!("import_staging/sources_standard_rows.rs");
include!("import_staging/preview_write.rs");
include!("import_staging/identity_validation.rs");
include!("import_staging/preview_query.rs");
include!("import_staging/preview_predicates.rs");
include!("import_staging/preview_selection.rs");
include!("import_staging/preview_decisions.rs");
include!("import_staging/preview_learning_lifecycle.rs");
include!("import_staging/preview_llm.rs");
include!("import_staging/llm_memory_annotations.rs");
include!("import_staging/confirm.rs");
include!("import_staging/materialization_ledger.rs");
include!("import_staging/row_mapping.rs");
include!("import_staging/patch_payload_helpers.rs");
include!("import_staging/filter_matching.rs");
#[cfg(test)]
include!("import_staging/test_support.rs");
#[cfg(test)]
include!("import_staging/tests_filters.rs");
#[cfg(test)]
include!("import_staging/tests_preview_write.rs");
#[cfg(test)]
include!("import_staging/tests_sql_query.rs");
#[cfg(test)]
include!("import_staging/tests_identity_validation.rs");
#[cfg(test)]
include!("import_staging/tests_llm.rs");
#[cfg(test)]
include!("import_staging/tests_preview_page_confirm.rs");
#[cfg(test)]
include!("import_staging/tests_standard_rows.rs");
#[cfg(test)]
include!("import_staging/tests_patch_lookup.rs");
