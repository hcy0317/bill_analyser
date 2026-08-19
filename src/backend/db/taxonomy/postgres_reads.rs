// 中文导读：PostgreSQL taxonomy 仓储 facade，按账户、分类、规则、标签、模板与映射 helper 聚合功能文件。
// 维护重点：这里保持原有 public 函数路径和 user-scope SQL 合同；新增逻辑优先落到对应功能子文件。
// 不变式：只读/写路径必须按 user_id 过滤；金额字段从 Postgres 分显式投影为当前 cents DTO。

use std::collections::BTreeSet;

use bill_analyser_core::account_rules::{
    AccountRuleCandidate, AccountRuleMatch, AccountRuleMatchContext, DEFAULT_FIELD_SCOPES,
};
use bill_analyser_core::category_rules::escape_rule_expression_term;
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Number, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row, Transaction};

use crate::{
    auth_registration::{
        RegisterDefaultSeedSummary, DEFAULT_DAILY_CATEGORIES, DEFAULT_DAILY_CATEGORY_RULES,
    },
    taxonomy::{
        account_rules::AccountRuleRecord,
        accounts::{
            AccountDisplayOrder, AccountRecord, AccountTransactionsClearResult,
            AccountTransactionsMoveResult,
        },
        categories::{CategoryRecord, CategoryStatistic},
        category_rules::CategoryRuleRecord,
        tags::{TagDisplayOrder, TagRecord},
        templates::{TemplateDisplayOrder, TemplateRecord},
    },
    DbError, DbResult, PostgresPool,
};

include!("postgres_reads/accounts.rs");
include!("postgres_reads/account_transactions.rs");
include!("postgres_reads/categories.rs");
include!("postgres_reads/category_rules.rs");
include!("postgres_reads/learning_rules.rs");
include!("postgres_reads/account_rules.rs");
include!("postgres_reads/tags.rs");
include!("postgres_reads/templates.rs");
include!("postgres_reads/row_mapping.rs");
include!("postgres_reads/template_helpers.rs");
include!("postgres_reads/account_helpers.rs");
include!("postgres_reads/category_helpers.rs");
include!("postgres_reads/rule_helpers.rs");
#[cfg(test)]
include!("postgres_reads/tests.rs");
