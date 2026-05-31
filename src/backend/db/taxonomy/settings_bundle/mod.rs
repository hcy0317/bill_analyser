// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction as PgTransaction};

use crate::{DbError, DbResult, PostgresPool};

include!("types_and_normalization.rs");
include!("import_accounts_categories_tags.rs");
include!("import_templates_rules_llm.rs");
include!("export_formatters.rs");
include!("postgres_import_export.rs");
include!("value_helpers.rs");
