// 中文导读：PostgreSQL recurring 仓储层，负责周期交易建议、模板候选与账单绑定。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑。

use std::collections::BTreeSet;

use bill_analyser_core::{matching::RecurringPattern, UserId};
use chrono::{Datelike, Duration, NaiveDate, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use sqlx::{postgres::PgRow, Row};

use crate::{
    category_path::category_names_from_postgres_path, get_postgres_bill_by_id, BillRecord,
    BillRecurringBindResult, BillRecurringCandidates, DbError, DbResult, PostgresPool,
};

include!("recurring/types.rs");
include!("recurring/suggestions.rs");
include!("recurring/bill_binding.rs");
include!("recurring/detection.rs");
include!("recurring/suggestion_actions.rs");
include!("recurring/linked_bills.rs");
include!("recurring/row_mapping.rs");
include!("recurring/templates.rs");
include!("recurring/schedule_dates.rs");
include!("recurring/value_helpers.rs");
#[cfg(test)]
include!("recurring/tests.rs");
