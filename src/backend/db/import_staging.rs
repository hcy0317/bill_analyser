// 中文导读：Postgres import staging adapter，负责当前 import v2 session、source、standard row、preview、confirm、LLM memory 与学习反馈写读。
// 维护重点：只实现 Postgres 当前运行态；不读取历史 non-Postgres 数据，不提供历史 staging schema 路径。
// 不变式：外部 session key 映射到 Postgres import_sessions.id，所有查询必须 user scoped。

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use bill_analyser_core::{
    build_transfer_source_snapshot, import_preview_recommendation_feedback_family_is_meaningful,
    import_preview_signal_value_is_truthy, is_import_preview_visible_signal_family,
    learning_lifecycle_is_auto_eligible, learning_lifecycle_signal_state, normalize_bill_date_text,
    normalize_history_operation, resolve_first_nonempty_status, strict_decimal_is_positive,
    transition_import_learning_lifecycle, trim_import_preview_signal_text, DedupBill,
    DeduplicationType, ImportLearningLifecycleState, ImportPreviewSignalFamily, Money,
    TransferSourceSnapshot, UserId, IMPORT_PREVIEW_DECIMAL_HAS_NON_ZERO,
    IMPORT_PREVIEW_DECIMAL_NUMERIC_STRING_GRAMMAR, IMPORT_PREVIEW_HISTORY_OPERATION_NAMES,
    IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES, IMPORT_PREVIEW_LEARNING_NUMERIC_EVIDENCE_FIELDS,
    IMPORT_PREVIEW_LEARNING_TEXT_EVIDENCE_FIELDS, IMPORT_PREVIEW_LLM_NUMERIC_EVIDENCE_FIELDS,
    IMPORT_PREVIEW_LLM_TEXT_EVIDENCE_FIELDS, IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES,
    IMPORT_PREVIEW_SIGNAL_NON_PENDING_EXCLUDED_STATUSES, IMPORT_PREVIEW_SIGNAL_STATUS_FIELDS,
    IMPORT_PREVIEW_SIGNAL_SUPPRESSED_STATUSES, IMPORT_PREVIEW_SIGNAL_TERMINAL_STATUSES,
    IMPORT_PREVIEW_SIGNAL_TRIM_CHARS, IMPORT_PREVIEW_SIGNAL_TRUTHY_TEXT_VALUES,
    IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES,
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
const IMPORT_PREVIEW_TRANSFER_LEARNING_LEVELS: &[&str] = &["yellow", "green", "blue"];
const IMPORT_PREVIEW_TRANSFER_VISIBLE_STATUSES: &[&str] =
    &["pending", "accepted", "auto_applied", "auto-applied"];

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
include!("../../../tests/backend/core/transfer_signal_parity_corpus.rs");
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
include!("import_staging/tests_preview_performance_selection.rs");
#[cfg(test)]
include!("import_staging/tests_standard_rows.rs");
#[cfg(test)]
include!("import_staging/tests_patch_lookup.rs");
