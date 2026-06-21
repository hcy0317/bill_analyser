// 中文导读：PostgreSQL bills 读仓储，负责 当前交易列表与详情投影。
// 维护重点：金额从 PostgreSQL 分直接投影为显式 cents 字段，handler 不复制 SQL。
// 不变式：所有查询必须按 user_id 过滤且忽略 is_deleted，不允许回退 non-Postgres。

use bill_analyser_core::adapters::transaction::{
    validate_batch_route_update_fields, ReconciliationCategoryRecord,
};
use bill_analyser_core::{parse_bill_datetime, Money};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Number, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row};

use crate::{
    calculate_bill_hash_from_record, BatchUpdateBillsResult, BillCategoryFilter, BillCreateDraft,
    BillFilters, BillPage, BillRecord, BillUpdateDraft, DbError, DbResult, PostgresPool,
};

const MAIN_CATEGORY_EXPR: &str = "COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(c.path, '/', 1), ''), c.name, '')";
const SUB_CATEGORY_EXPR: &str = "COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), CASE WHEN position('/' in COALESCE(c.path, '')) > 0 THEN substring(c.path from position('/' in c.path) + 1) ELSE '' END, '')";

include!("postgres_reads/types.rs");
include!("postgres_reads/category_queries.rs");
include!("postgres_reads/query.rs");
include!("postgres_reads/tags_accounts.rs");
include!("postgres_reads/create.rs");
include!("postgres_reads/update.rs");
include!("postgres_reads/delete.rs");
include!("postgres_reads/mutation_types.rs");
include!("postgres_reads/mutation_prepare.rs");
include!("postgres_reads/tags.rs");
include!("postgres_reads/mutation_sql.rs");
include!("postgres_reads/mutation_value_helpers.rs");
include!("postgres_reads/filters.rs");
include!("postgres_reads/row_mapping.rs");
include!("postgres_reads/value_helpers.rs");
#[cfg(test)]
include!("postgres_reads/tests.rs");
