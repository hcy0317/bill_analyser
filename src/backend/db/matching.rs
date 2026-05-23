// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::{
    matching::{
        bill_pair_feedback_payload_is_related, build_bill_pair_feedback_payload,
        build_investment_pair_candidates, build_learning_candidates_for_bill,
        build_learning_rule_revision, build_matching_session_candidates,
        build_transfer_pair_candidate, build_transfer_pair_candidates,
        build_user_investment_keyword_settings, normalize_learning_rule_revision,
        normalize_transfer_pair_bill_ids, parse_matching_candidate_id, score_investment_candidate,
        INVESTMENT_PAIR_TYPE, MANUAL_PAIR_SOURCE, TRANSFER_AMOUNT_TOLERANCE, TRANSFER_PAIR_TYPE,
    },
    UserId,
};
use chrono::{SecondsFormat, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Value};

use crate::{
    apply_preview_learning_decision, apply_preview_transfer_decision, get_import_session,
    get_preview_by_session, run_transaction, update_preview_recurring_match_decision, DbError,
    DbResult, ImportPreviewDecision, ImportPreviewExpectedState, ImportPreviewLearningApply,
    ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate, ImportPreviewRow,
    UserScope,
};

include!("matching/types_schema.rs");
include!("matching/actions.rs");
include!("matching/candidate_queries.rs");
include!("matching/reconciliation_projection.rs");
include!("matching/serialization.rs");
include!("matching/repositories_and_helpers.rs");
