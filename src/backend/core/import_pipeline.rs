// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::import_pipeline_learning::LearningMatchingPayload;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};

pub const IMPORT_PREVIEW_SORT_KEYS: &[&str] = &[
    "time",
    "type",
    "sourceAmountCents",
    "counterparty",
    "paymentMethod",
    "comment",
];
// 这些字段是 Check Data 前端与 Rust preview page 的排序合同；新增字段时必须同步
// preview query、services.ts 映射和前端 contract 测试。
pub const IMPORT_PREVIEW_SELECTION_KEYS: &[&str] =
    &["selected", "isSelected", "is_selected", "preview_selected"];
pub const IMPORT_V2_PIPELINE_STEPS: &[&str] = &[
    "parse",
    "validation",
    "smart_dedup",
    "category_match",
    "account_match",
    "learning_replay",
    "recurring_projection",
    "preview",
    "confirm",
];
// 导入步骤名用于状态响应和排错，不代表 handler 可以跳过 staging/preview/confirm
// 的数据库生命周期。
pub const BILLS_PREVIEW_CONTRACT_FIELDS: &[&str] = &[
    "preview_parser_id",
    "preview_parser_tags_json",
    "preview_selected",
    "preview_is_manually_annotated",
    "dedup_type",
    "dedup_source_ids",
    "preview_recurring_id",
    "preview_recurring_name",
    "preview_recurring_candidate_count",
    "preview_recurring_match_score",
    "preview_recurring_match_reasons",
    "preview_recurring_matched_date",
    "preview_matching_feedback_json",
];
// preview row 的字段清单。任何删改都要同时复核后端投影、前端模型和
// matching feedback 的 sparse payload 契约一致性。
include!("import_history_rewrite.rs");

include!("import_pipeline/preview_query.rs");
include!("import_pipeline/preview_state.rs");
include!("import_pipeline/filter_index.rs");
include!("import_pipeline/matching_payload.rs");
include!("import_pipeline/type_mapping.rs");
include!("import_pipeline/response_types.rs");
include!("import_pipeline/responses.rs");
include!("import_pipeline/value_helpers.rs");
