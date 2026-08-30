// 中文导读：PostgreSQL bills DTO 与纯 helper。实际仓储读写在 `bills::postgres_reads`。
// 维护重点：保留 HTTP/Postgres 层共享的记录结构与 hash 计算，不保留 non-Postgres CRUD runtime。

use bill_analyser_core::adapters::transaction::ReconciliationBill;
use serde_json::{Map, Value};

use crate::{DbError, DbResult};

pub mod postgres_reads;

pub type BillRecord = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BillCreateDraft {
    pub fields: BillRecord,
    pub tag_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BillUpdateDraft {
    pub fields: BillRecord,
    pub tag_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BillCategoryFilter {
    pub main: String,
    pub sub: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BillFilters {
    pub id: Option<i64>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub date_before: Option<String>,
    pub transaction_type: Option<String>,
    pub flow_direction: Option<String>,
    pub main_category: Option<String>,
    pub sub_category: Option<String>,
    pub batch_id: Option<String>,
    pub counterparty: Option<String>,
    pub description: Option<String>,
    pub keyword: Option<String>,
    pub account_ids: Vec<i64>,
    pub categories: Vec<BillCategoryFilter>,
    pub tag_ids: Vec<i64>,
    pub min_amount_cents: Option<i64>,
    pub max_amount_cents: Option<i64>,
    pub amount_filter_cents: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BillPage {
    pub bills: Vec<BillRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresReconciliationBillRow {
    pub bill_id: i64,
    pub ledger_bill: ReconciliationBill,
    pub frontend_record: BillRecord,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PostgresReconciliationBillPage {
    pub rows: Vec<PostgresReconciliationBillRow>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchUpdateBillsResult {
    pub success_count: usize,
    pub failed_count: usize,
    pub failed_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BillRecurringCandidates {
    pub linked_recurring_id: Option<i64>,
    pub linked_recurring_name: String,
    pub candidates: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BillRecurringBindResult {
    pub bill_id: i64,
    pub recurring_id: i64,
    pub next_scheduled_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccountBalanceDiscrepancy {
    pub account_id: i64,
    pub name: String,
    pub old_balance_cents: i64,
    pub new_balance_cents: i64,
    pub diff_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncAllAccountBalancesResult {
    pub total_accounts: usize,
    pub synced_accounts: usize,
    pub discrepancies: Vec<AccountBalanceDiscrepancy>,
    pub errors: Vec<String>,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_bill_hash_from_fields(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
) -> String {
    let source = format!(
        "{}|{}|{}|{}|{}",
        date,
        bill_type,
        finite_float_text(amount),
        counterparty,
        description
    );
    format!("{:x}", md5::compute(source.as_bytes()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_bill_hash_from_record(record: &BillRecord) -> DbResult<String> {
    let source = format!(
        "{}|{}|{}|{}|{}",
        record_text(record, "date"),
        record_text(record, "type"),
        record_i64(record, "amount_cents")?,
        record_text(record, "counterparty"),
        record_text(record, "description"),
    );
    Ok(format!("{:x}", md5::compute(source.as_bytes())))
}

fn record_text(record: &BillRecord, key: &str) -> String {
    match record.get(key) {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn record_i64(record: &BillRecord, key: &str) -> DbResult<i64> {
    match record.get(key) {
        Some(Value::Number(value)) => value
            .as_i64()
            .ok_or_else(|| DbError::InvalidOperation(format!("invalid numeric field: {key}"))),
        Some(Value::String(value)) => value
            .trim()
            .parse::<i64>()
            .map_err(|_| DbError::InvalidOperation(format!("invalid numeric field: {key}"))),
        _ => Err(DbError::InvalidOperation(format!(
            "missing numeric field: {key}"
        ))),
    }
}

fn finite_float_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}
