// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

use std::collections::BTreeSet;

use bill_analyser_core::adapters::transaction::{
    calculate_account_balance_from_bills, frontend_transaction_type_from_backend,
    sync_account_ids_for_batch_delete, sync_account_ids_for_bill, sync_account_ids_for_update,
    validate_batch_route_update_fields, validate_bill_create_fields, validate_bill_update_fields,
    AccountBalanceBill, BackendBillUpdateSnapshot, BillAccountSyncSnapshot, BILL_CREATE_COLUMNS,
    BILL_UPDATE_COLUMNS, ROUTE_ALLOWED_BATCH_UPDATE_FIELDS,
};
use bill_analyser_core::{classify_investment_pnl_change, Money, RuntimeError, UserId};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde_json::{Map, Number, Value};

use crate::{run_transaction, DbError, DbResult, UserScope};

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
    pub transaction_type: Option<String>,
    pub main_category: Option<String>,
    pub sub_category: Option<String>,
    pub batch_id: Option<String>,
    pub counterparty: Option<String>,
    pub description: Option<String>,
    pub keyword: Option<String>,
    pub account_ids: Vec<i64>,
    pub categories: Vec<BillCategoryFilter>,
    pub tag_ids: Vec<i64>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub amount_filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BillPage {
    pub bills: Vec<BillRecord>,
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
    pub old_balance: f64,
    pub new_balance: f64,
    pub diff: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncAllAccountBalancesResult {
    pub total_accounts: usize,
    pub synced_accounts: usize,
    pub discrepancies: Vec<AccountBalanceDiscrepancy>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct AccountBalanceSyncTarget {
    id: i64,
    name: String,
    balance: f64,
    initial_balance: f64,
}

const BILL_SELECT_COLUMNS: &[&str] = &[
    "id",
    "user_id",
    "date",
    "type",
    "amount",
    "counterparty",
    "description",
    "payment_method",
    "main_category",
    "sub_category",
    "batch_id",
    "hash",
    "created_at",
    "updated_at",
    "source_account_id",
    "destination_account_id",
    "destination_amount",
    "created_from_template",
    "created_from_recurring",
    "import_history_id",
];

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
        python_float_text(amount),
        counterparty,
        description
    );
    format!("{:x}", md5::compute(source.as_bytes()))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn calculate_bill_hash_from_record(record: &BillRecord) -> DbResult<String> {
    Ok(calculate_bill_hash_from_fields(
        &record_text(record, "date"),
        &record_text(record, "type"),
        record_f64(record, "amount")?,
        &record_text(record, "counterparty"),
        &record_text(record, "description"),
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn create_bill(
    connection: &mut Connection,
    user_id: UserId,
    draft: &BillCreateDraft,
) -> DbResult<i64> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "create_bill",
        "business operation entered"
    );
    let user_scope = UserScope::new(user_id);
    let user_id = user_scope.bind_value()?;
    run_transaction(connection, |tx| {
        let now = now_text();
        let (bill_id, snapshot) = insert_bill_on_tx(tx, user_id, draft, &now)?;
        sync_account_balances(tx, user_id, &sync_account_ids_for_bill(snapshot), &now)?;
        Ok(bill_id)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn batch_create_bills(
    connection: &mut Connection,
    user_id: UserId,
    drafts: &[BillCreateDraft],
) -> DbResult<Vec<i64>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "batch_create_bills",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    if drafts.is_empty() {
        return Ok(Vec::new());
    }
    run_transaction(connection, |tx| {
        let now = now_text();
        let mut bill_ids = Vec::with_capacity(drafts.len());
        let mut snapshots = Vec::with_capacity(drafts.len());
        for draft in drafts {
            let (bill_id, snapshot) = insert_bill_on_tx(tx, user_id, draft, &now)?;
            bill_ids.push(bill_id);
            snapshots.push(snapshot);
        }
        sync_account_balances(tx, user_id, &collect_account_ids(snapshots), &now)?;
        Ok(bill_ids)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_bill(
    connection: &mut Connection,
    user_id: UserId,
    bill_id: i64,
    draft: &BillUpdateDraft,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "update_bill",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(old_snapshot) = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)? else {
            return Ok(false);
        };
        if draft.fields.is_empty() && draft.tag_ids.is_none() {
            return Ok(false);
        }
        let now = now_text();
        if !draft.fields.is_empty() {
            let update_payload =
                prepare_update_payload(tx, user_id, bill_id, &draft.fields, &now, false)?;
            let (set_clause, mut values) = update_payload_to_sql(update_payload)?;
            values.push(SqlValue::Integer(bill_id));
            values.push(SqlValue::Integer(user_id));
            let updated = tx.execute(
                &format!("UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?"),
                params_from_iter(values),
            )?;
            if updated == 0 {
                return Ok(false);
            }
        }
        if let Some(tag_ids) = &draft.tag_ids {
            replace_bill_tags(tx, user_id, bill_id, tag_ids, &now)?;
        }
        delete_pairing_side_effects(tx, user_id, &[bill_id])?;
        let new_snapshot = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)?
            .ok_or_else(|| DbError::InvalidOperation("updated bill not found".to_string()))?;
        sync_account_balances(
            tx,
            user_id,
            &sync_account_ids_for_update(old_snapshot, new_snapshot),
            &now,
        )?;
        Ok(true)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn batch_update_bills(
    connection: &mut Connection,
    user_id: UserId,
    bill_ids: &[i64],
    fields: &BillRecord,
) -> DbResult<BatchUpdateBillsResult> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "batch_update_bills",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let bill_ids = normalize_bill_ids(bill_ids);
    if bill_ids.is_empty() || fields.is_empty() {
        return Ok(BatchUpdateBillsResult::default());
    }
    validate_batch_route_update_fields(fields.keys().map(String::as_str)).map_err(runtime_error)?;

    run_transaction(connection, |tx| {
        let now = now_text();
        let mut success_count = 0_usize;
        let mut failed_ids = Vec::new();
        let mut sync_snapshots = Vec::new();
        for bill_id in bill_ids {
            let Some(old_snapshot) = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)? else {
                failed_ids.push(bill_id);
                continue;
            };
            let update_payload = prepare_update_payload(tx, user_id, bill_id, fields, &now, true)?;
            let (set_clause, mut values) = update_payload_to_sql(update_payload)?;
            values.push(SqlValue::Integer(bill_id));
            values.push(SqlValue::Integer(user_id));
            let updated = tx.execute(
                &format!("UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?"),
                params_from_iter(values),
            )?;
            if updated == 0 {
                failed_ids.push(bill_id);
                continue;
            }
            delete_pairing_side_effects(tx, user_id, &[bill_id])?;
            let new_snapshot = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)?
                .ok_or_else(|| DbError::InvalidOperation("updated bill not found".to_string()))?;
            sync_snapshots.push(old_snapshot);
            sync_snapshots.push(new_snapshot);
            success_count += 1;
        }
        let account_ids = collect_account_ids(sync_snapshots);
        sync_account_balances(tx, user_id, &account_ids, &now)?;
        Ok(BatchUpdateBillsResult {
            success_count,
            failed_count: failed_ids.len(),
            failed_ids,
        })
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn delete_bill(connection: &mut Connection, user_id: UserId, bill_id: i64) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "delete_bill",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let now = now_text();
        delete_bill_on_tx(tx, user_id, bill_id, &now)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn update_bill_fields_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    fields: &BillRecord,
    now: &str,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "update_bill_fields_on_tx",
        "business operation entered"
    );
    let Some(old_snapshot) = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)? else {
        return Ok(false);
    };
    if fields.is_empty() {
        return Ok(true);
    }
    let update_payload = prepare_update_payload(tx, user_id, bill_id, fields, now, false)?;
    let (set_clause, mut values) = update_payload_to_sql(update_payload)?;
    values.push(SqlValue::Integer(bill_id));
    values.push(SqlValue::Integer(user_id));
    let updated = tx.execute(
        &format!("UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?"),
        params_from_iter(values),
    )?;
    if updated == 0 {
        return Ok(false);
    }
    delete_pairing_side_effects(tx, user_id, &[bill_id])?;
    let new_snapshot = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)?
        .ok_or_else(|| DbError::InvalidOperation("updated bill not found".to_string()))?;
    sync_account_balances(
        tx,
        user_id,
        &sync_account_ids_for_update(old_snapshot, new_snapshot),
        now,
    )?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn delete_bill_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    now: &str,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "delete_bill_on_tx",
        "business operation entered"
    );
    let Some(snapshot) = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)? else {
        return Ok(false);
    };
    delete_pairing_side_effects(tx, user_id, &[bill_id])?;
    delete_bill_tags(tx, &[bill_id])?;
    let deleted = tx.execute(
        "DELETE FROM bills WHERE id = ?1 AND user_id = ?2",
        params![bill_id, user_id],
    )?;
    sync_account_balances(tx, user_id, &sync_account_ids_for_bill(snapshot), now)?;
    Ok(deleted > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn batch_delete_bills(
    connection: &mut Connection,
    user_id: UserId,
    bill_ids: &[i64],
) -> DbResult<usize> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "batch_delete_bills",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let bill_ids = normalize_bill_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(0);
    }
    run_transaction(connection, |tx| {
        let snapshots = get_bill_account_snapshots_on_tx(tx, user_id, &bill_ids)?;
        if snapshots.is_empty() {
            return Ok(0);
        }
        let now = now_text();
        delete_pairing_side_effects(tx, user_id, &bill_ids)?;
        delete_bill_tags(tx, &bill_ids)?;
        let placeholders = placeholders(bill_ids.len());
        let mut params = bill_ids
            .iter()
            .copied()
            .map(SqlValue::Integer)
            .collect::<Vec<_>>();
        params.push(SqlValue::Integer(user_id));
        let deleted = tx.execute(
            &format!("DELETE FROM bills WHERE id IN ({placeholders}) AND user_id = ?"),
            params_from_iter(params),
        )?;
        sync_account_balances(
            tx,
            user_id,
            &sync_account_ids_for_batch_delete(&snapshots),
            &now,
        )?;
        Ok(deleted)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_bill_by_id(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> DbResult<Option<BillRecord>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    get_bill_by_id_on_connection(connection, user_id, bill_id)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_bill_update_snapshot(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> DbResult<Option<BackendBillUpdateSnapshot>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    get_bill_update_snapshot_on_connection(connection, user_id, bill_id)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_bill_tags(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> DbResult<Vec<Value>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    get_bill_tags_on_connection(connection, user_id, bill_id)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_bills(
    connection: &Connection,
    user_id: UserId,
    page: usize,
    page_size: usize,
    filters: &BillFilters,
) -> DbResult<BillPage> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "query_bills",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);
    let offset = (page - 1).saturating_mul(page_size);
    let (conditions, filter_params) = build_bill_filter_conditions(filters)?;

    let mut count_params = vec![SqlValue::Integer(user_id)];
    count_params.extend(filter_params.clone());
    let count_sql = format!(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?{}",
        where_suffix(&conditions)
    );
    let total = connection.query_row(&count_sql, params_from_iter(count_params), |row| {
        row.get::<_, i64>(0)
    })?;

    let mut list_params = vec![SqlValue::Integer(user_id)];
    list_params.extend(filter_params);
    list_params.push(SqlValue::Integer(usize_to_i64(page_size)));
    list_params.push(SqlValue::Integer(usize_to_i64(offset)));
    let list_sql = format!(
        "SELECT {} FROM bills WHERE user_id = ?{} ORDER BY date DESC LIMIT ? OFFSET ?",
        BILL_SELECT_COLUMNS.join(", "),
        where_suffix(&conditions)
    );
    let mut statement = connection.prepare(&list_sql)?;
    let rows = statement.query_map(params_from_iter(list_params), bill_record_from_row)?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(BillPage { bills, total })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_bill_recurring_candidates(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
    tolerance_days: i64,
) -> DbResult<Option<BillRecurringCandidates>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let Some(bill) = get_bill_by_id_on_connection(connection, user_id, bill_id)? else {
        return Ok(None);
    };
    let linked_recurring_id = record_optional_i64(&bill, "created_from_recurring");
    let linked_recurring_name = linked_recurring_id
        .and_then(|recurring_id| {
            connection
                .query_row(
                    "SELECT name FROM recurring_bills WHERE id = ?1 AND user_id = ?2",
                    params![recurring_id, user_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .transpose()
        })
        .transpose()?
        .flatten()
        .unwrap_or_default();
    let recurring_rows = list_enabled_recurring_templates(connection, user_id)?;
    let candidates = build_recurring_candidates_for_bill_data(
        &bill,
        &recurring_rows,
        linked_recurring_id,
        tolerance_days.clamp(0, 31),
    );
    Ok(Some(BillRecurringCandidates {
        linked_recurring_id,
        linked_recurring_name,
        candidates,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn bind_bill_to_recurring(
    connection: &mut Connection,
    user_id: UserId,
    bill_id: i64,
    recurring_id: i64,
) -> DbResult<Option<BillRecurringBindResult>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "bind_bill_to_recurring",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(bill) = get_bill_by_id_on_tx(tx, user_id, bill_id)? else {
            return Ok(None);
        };
        let previous_recurring_id = record_optional_i64(&bill, "created_from_recurring");
        let Some(recurring) = get_recurring_template_on_tx(tx, user_id, recurring_id)? else {
            return Ok(None);
        };
        let bill_date_text = record_text(&bill, "date");
        let next_occurrence = parse_date_value(&bill_date_text)
            .and_then(|bill_date| get_next_recurring_occurrence_after(&recurring, bill_date, 370))
            .map(|date| date.to_string());
        let fallback_next_date = recurring_text(&recurring, "next_date");
        let stored_next_date = next_occurrence
            .as_deref()
            .or_else(|| non_empty_str(fallback_next_date.as_str()));
        let now = now_text();
        tx.execute(
            "UPDATE bills SET created_from_recurring = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            params![recurring_id, now, bill_id, user_id],
        )?;
        tx.execute(
            "UPDATE recurring_bills SET next_date = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            params![stored_next_date, now, recurring_id, user_id],
        )?;
        if previous_recurring_id.is_some_and(|previous| previous != recurring_id) {
            recalculate_recurring_next_date_on_tx(
                tx,
                user_id,
                previous_recurring_id.unwrap_or_default(),
                &now,
            )?;
        }
        Ok(Some(BillRecurringBindResult {
            bill_id,
            recurring_id,
            next_scheduled_date: stored_next_date.map(ToString::to_string),
        }))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn unbind_bill_from_recurring(
    connection: &mut Connection,
    user_id: UserId,
    bill_id: i64,
) -> DbResult<Option<bool>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "unbind_bill_from_recurring",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(bill) = get_bill_by_id_on_tx(tx, user_id, bill_id)? else {
            return Ok(None);
        };
        let recurring_id = record_optional_i64(&bill, "created_from_recurring");
        let now = now_text();
        let updated = tx.execute(
            "UPDATE bills SET created_from_recurring = NULL, updated_at = ?1 WHERE id = ?2 AND user_id = ?3",
            params![now, bill_id, user_id],
        )?;
        if let Some(recurring_id) = recurring_id {
            recalculate_recurring_next_date_on_tx(tx, user_id, recurring_id, &now)?;
        }
        Ok(Some(updated > 0))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_bills(
    connection: &Connection,
    user_id: UserId,
    filters: &BillFilters,
) -> DbResult<Vec<BillRecord>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "list_bills",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    let (conditions, filter_params) = build_bill_filter_conditions(filters)?;
    let mut list_params = vec![SqlValue::Integer(user_id)];
    list_params.extend(filter_params);
    let list_sql = format!(
        "SELECT {} FROM bills WHERE user_id = ?{} ORDER BY date DESC",
        BILL_SELECT_COLUMNS.join(", "),
        where_suffix(&conditions)
    );
    let mut statement = connection.prepare(&list_sql)?;
    let rows = statement.query_map(params_from_iter(list_params), bill_record_from_row)?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(bills)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_first_account_id(connection: &Connection, user_id: UserId) -> DbResult<Option<i64>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    connection
        .query_row(
            "SELECT id FROM accounts WHERE user_id = ?1 ORDER BY id LIMIT 1",
            params![user_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn sync_all_account_balances(
    connection: &mut Connection,
    user_id: UserId,
) -> DbResult<SyncAllAccountBalancesResult> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "bills",
        operation = "sync_all_account_balances",
        "business operation entered"
    );
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let accounts = load_account_balance_sync_targets(tx, user_id)?;
        let now = now_text();
        let mut result = SyncAllAccountBalancesResult {
            total_accounts: accounts.len(),
            ..SyncAllAccountBalancesResult::default()
        };

        for account in accounts {
            match calculate_account_balance_yuan_on_tx(
                tx,
                user_id,
                account.id,
                account.initial_balance,
            ) {
                Ok(new_balance) => {
                    let diff = new_balance - account.balance;
                    if diff.abs() > 0.001 {
                        result.discrepancies.push(AccountBalanceDiscrepancy {
                            account_id: account.id,
                            name: account.name.clone(),
                            old_balance: round2(account.balance),
                            new_balance: round2(new_balance),
                            diff: round2(diff),
                        });
                    }
                    tx.execute(
                        "UPDATE accounts SET balance = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
                        params![new_balance, now, account.id, user_id],
                    )?;
                    result.synced_accounts += 1;
                }
                Err(error) => result.errors.push(format!(
                    "账户 '{}' (ID={}) 同步失败: {error}",
                    account.name, account.id
                )),
            }
        }
        Ok(result)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn insert_bill_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    draft: &BillCreateDraft,
    now: &str,
) -> DbResult<(i64, BillAccountSyncSnapshot)> {
    let mut payload = prepare_create_payload(user_id, &draft.fields, now)?;
    let columns = BILL_CREATE_COLUMNS
        .iter()
        .copied()
        .filter(|column| payload.contains_key(*column))
        .collect::<Vec<_>>();
    let placeholders = std::iter::repeat_n("?", columns.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO bills ({}) VALUES ({})",
        columns.join(", "),
        placeholders
    );
    let values = columns
        .iter()
        .map(|column| json_to_sql_value(payload.remove(*column).unwrap_or(Value::Null)))
        .collect::<Vec<_>>();
    tx.execute(&sql, params_from_iter(values))?;
    let bill_id = tx.last_insert_rowid();
    replace_bill_tags(tx, user_id, bill_id, &draft.tag_ids, now)?;
    let snapshot = get_bill_account_snapshot_on_tx(tx, user_id, bill_id)?
        .ok_or_else(|| DbError::InvalidOperation("created bill not found".to_string()))?;
    Ok((bill_id, snapshot))
}

#[tracing::instrument(level = "debug", skip_all)]
fn prepare_create_payload(user_id: i64, fields: &BillRecord, now: &str) -> DbResult<BillRecord> {
    let mut payload = fields.clone();
    normalize_create_aliases(&mut payload);
    payload.insert("user_id".to_string(), Value::Number(Number::from(user_id)));
    payload
        .entry("created_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    validate_bill_create_fields(payload.keys().map(String::as_str)).map_err(runtime_error)?;
    for field in ["date", "type", "amount", "counterparty", "description"] {
        if missing_required_field(&payload, field) {
            return Err(DbError::InvalidOperation(format!(
                "missing required bill field: {field}"
            )));
        }
    }
    if missing_required_field(&payload, "hash") {
        let hash = calculate_bill_hash_from_record(&payload)?;
        payload.insert("hash".to_string(), Value::String(hash));
    }
    Ok(payload)
}

#[tracing::instrument(level = "debug", skip_all)]
fn prepare_update_payload(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    fields: &BillRecord,
    now: &str,
    batch_route_fields: bool,
) -> DbResult<BillRecord> {
    if batch_route_fields {
        validate_batch_route_update_fields(fields.keys().map(String::as_str))
            .map_err(runtime_error)?;
    } else {
        validate_bill_update_fields(fields.keys().map(String::as_str)).map_err(runtime_error)?;
    }

    let mut payload = fields.clone();
    if update_requires_hash_recalculation(&payload) && missing_required_field(&payload, "hash") {
        let mut merged = get_bill_by_id_on_tx(tx, user_id, bill_id)?
            .ok_or_else(|| DbError::InvalidOperation("bill not found".to_string()))?;
        for (key, value) in &payload {
            merged.insert(key.clone(), value.clone());
        }
        let hash = calculate_bill_hash_from_record(&merged)?;
        payload.insert("hash".to_string(), Value::String(hash));
    }
    payload.insert("updated_at".to_string(), Value::String(now.to_string()));
    Ok(payload)
}

#[tracing::instrument(level = "debug", skip_all)]
fn update_payload_to_sql(payload: BillRecord) -> DbResult<(String, Vec<SqlValue>)> {
    if payload.is_empty() {
        return Err(DbError::InvalidOperation("empty bill update".to_string()));
    }
    let mut assignments = Vec::with_capacity(payload.len());
    let mut values = Vec::with_capacity(payload.len());
    let mut keys = payload.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        assignments.push(format!("{key} = ?"));
        values.push(json_to_sql_value(
            payload.get(&key).cloned().unwrap_or(Value::Null),
        ));
    }
    Ok((assignments.join(", "), values))
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_bill_by_id_on_connection(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<BillRecord>> {
    let sql = format!(
        "SELECT {} FROM bills WHERE id = ?1 AND user_id = ?2",
        BILL_SELECT_COLUMNS.join(", ")
    );
    connection
        .query_row(&sql, params![bill_id, user_id], bill_record_from_row)
        .optional()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_bill_by_id_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<BillRecord>> {
    let sql = format!(
        "SELECT {} FROM bills WHERE id = ?1 AND user_id = ?2",
        BILL_SELECT_COLUMNS.join(", ")
    );
    tx.query_row(&sql, params![bill_id, user_id], bill_record_from_row)
        .optional()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_bill_account_snapshot_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<BillAccountSyncSnapshot>> {
    tx.query_row(
        "SELECT source_account_id, destination_account_id FROM bills WHERE id = ?1 AND user_id = ?2",
        params![bill_id, user_id],
        |row| {
            Ok(BillAccountSyncSnapshot {
                source_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(0)?),
                destination_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(1)?),
            })
        },
    )
    .optional()
    .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_bill_account_snapshots_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<Vec<BillAccountSyncSnapshot>> {
    if bill_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = placeholders(bill_ids.len());
    let mut params = bill_ids
        .iter()
        .copied()
        .map(SqlValue::Integer)
        .collect::<Vec<_>>();
    params.push(SqlValue::Integer(user_id));
    let mut statement = tx.prepare(&format!(
        "SELECT source_account_id, destination_account_id FROM bills WHERE id IN ({placeholders}) AND user_id = ?"
    ))?;
    let rows = statement.query_map(params_from_iter(params), |row| {
        Ok(BillAccountSyncSnapshot {
            source_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(0)?),
            destination_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(1)?),
        })
    })?;
    let mut snapshots = Vec::new();
    for row in rows {
        snapshots.push(row?);
    }
    Ok(snapshots)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_bill_update_snapshot_on_connection(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<BackendBillUpdateSnapshot>> {
    connection
        .query_row(
            "SELECT type, source_account_id, destination_account_id, destination_amount FROM bills WHERE id = ?1 AND user_id = ?2",
            params![bill_id, user_id],
            bill_update_snapshot_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

fn bill_update_snapshot_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<BackendBillUpdateSnapshot> {
    let destination_amount = row
        .get::<_, Option<f64>>(3)?
        .map(|value| money_from_yuan(value).unwrap_or(Money::ZERO));
    Ok(BackendBillUpdateSnapshot {
        transaction_type: row.get::<_, String>(0)?,
        source_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(1)?),
        destination_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(2)?),
        destination_amount,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_bill_tags_on_connection(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Vec<Value>> {
    let mut statement = connection.prepare(
        "
        SELECT t.id, t.name, t.color, t.icon
        FROM tags t
        JOIN bill_tags bt ON t.id = bt.tag_id
        JOIN bills b ON b.id = bt.bill_id
        WHERE bt.bill_id = ?1 AND b.user_id = ?2 AND t.user_id = ?2
        ORDER BY t.name
        ",
    )?;
    let rows = statement.query_map(params![bill_id, user_id], tag_value_from_row)?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

fn replace_bill_tags(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    tag_ids: &[i64],
    now: &str,
) -> DbResult<()> {
    delete_bill_tags(tx, &[bill_id])?;
    for tag_id in normalize_bill_ids(tag_ids) {
        let exists = tx
            .query_row(
                "SELECT 1 FROM tags WHERE id = ?1 AND user_id = ?2",
                params![tag_id, user_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(DbError::InvalidOperation(format!(
                "tag not found: {tag_id}"
            )));
        }
        tx.execute(
            "INSERT INTO bill_tags (bill_id, tag_id, created_at) VALUES (?1, ?2, ?3)",
            params![bill_id, tag_id, now],
        )?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_bill_tags(tx: &Transaction<'_>, bill_ids: &[i64]) -> DbResult<()> {
    let bill_ids = normalize_bill_ids(bill_ids);
    if bill_ids.is_empty() || !table_exists(tx, "bill_tags")? {
        return Ok(());
    }
    let placeholders = placeholders(bill_ids.len());
    let params = bill_ids
        .iter()
        .copied()
        .map(SqlValue::Integer)
        .collect::<Vec<_>>();
    tx.execute(
        &format!("DELETE FROM bill_tags WHERE bill_id IN ({placeholders})"),
        params_from_iter(params),
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_pairing_side_effects(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<()> {
    let bill_ids = normalize_bill_ids(bill_ids);
    if bill_ids.is_empty() {
        return Ok(());
    }
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_pair_links")?;
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_transfer_pair_suppressions")?;
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_investment_pair_suppressions")?;
    delete_pair_table_by_pair_columns(tx, user_id, &bill_ids, "bill_duplicate_pair_suppressions")?;
    if table_exists(tx, "bill_learning_rule_suppressions")? {
        let placeholders = placeholders(bill_ids.len());
        let mut params = vec![SqlValue::Integer(user_id)];
        params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
        tx.execute(
            &format!(
                "DELETE FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id IN ({placeholders})"
            ),
            params_from_iter(params),
        )?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_pair_table_by_pair_columns(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
    table_name: &str,
) -> DbResult<()> {
    if !table_exists(tx, table_name)? {
        return Ok(());
    }
    let placeholders = placeholders(bill_ids.len());
    let mut params = vec![SqlValue::Integer(user_id)];
    params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    params.extend(bill_ids.iter().copied().map(SqlValue::Integer));
    tx.execute(
        &format!(
            "DELETE FROM {table_name} WHERE user_id = ? AND (left_bill_id IN ({placeholders}) OR right_bill_id IN ({placeholders}))"
        ),
        params_from_iter(params),
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn sync_account_balances(
    tx: &Transaction<'_>,
    user_id: i64,
    account_ids: &[i64],
    now: &str,
) -> DbResult<()> {
    let account_ids = normalize_bill_ids(account_ids);
    for account_id in account_ids {
        let Some(initial_balance) = tx
            .query_row(
                "SELECT initial_balance FROM accounts WHERE id = ?1 AND user_id = ?2",
                params![account_id, user_id],
                |row| row.get::<_, Option<f64>>(0),
            )
            .optional()?
            .flatten()
        else {
            continue;
        };
        let balance =
            calculate_account_balance_yuan_on_tx(tx, user_id, account_id, initial_balance)?;
        tx.execute(
            "UPDATE accounts SET balance = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            params![balance, now, account_id, user_id],
        )?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_account_balance_sync_targets(
    tx: &Transaction<'_>,
    user_id: i64,
) -> DbResult<Vec<AccountBalanceSyncTarget>> {
    let mut statement = tx.prepare(
        "
        SELECT id, name, COALESCE(balance, 0), COALESCE(initial_balance, 0)
        FROM accounts
        WHERE user_id = ?1
        ORDER BY id
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(AccountBalanceSyncTarget {
            id: row.get::<_, i64>(0)?,
            name: row.get::<_, String>(1)?,
            balance: row.get::<_, f64>(2)?,
            initial_balance: row.get::<_, f64>(3)?,
        })
    })?;
    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(row?);
    }
    Ok(accounts)
}

#[tracing::instrument(level = "debug", skip_all)]
fn calculate_account_balance_yuan_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    account_id: i64,
    initial_balance: f64,
) -> DbResult<f64> {
    let balance_bills = load_account_balance_bills(tx, user_id, account_id)?;
    let pnl_correction = calculate_same_account_investment_pnl_correction(tx, user_id, account_id)?;
    let balance = calculate_account_balance_from_bills(
        account_id,
        money_from_yuan(initial_balance)?,
        &balance_bills,
        pnl_correction,
    )
    .map_err(runtime_error)?;
    money_to_yuan_f64(balance)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_account_balance_bills(
    tx: &Transaction<'_>,
    user_id: i64,
    account_id: i64,
) -> DbResult<Vec<AccountBalanceBill>> {
    let mut statement = tx.prepare(
        "
        SELECT type, amount, destination_amount, source_account_id, destination_account_id
        FROM bills
        WHERE user_id = ?1 AND (source_account_id = ?2 OR destination_account_id = ?2)
        ",
    )?;
    let rows = statement.query_map(params![user_id, account_id], |row| {
        let transaction_type = row.get::<_, String>(0)?;
        let transaction_type = frontend_transaction_type_from_backend(&transaction_type)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let amount = money_from_yuan(row.get::<_, f64>(1)?)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let destination_amount = money_from_yuan(row.get::<_, Option<f64>>(2)?.unwrap_or(0.0))
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        Ok(AccountBalanceBill {
            transaction_type,
            amount,
            destination_amount,
            source_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(3)?),
            destination_account_id: nullable_positive_i64(row.get::<_, Option<i64>>(4)?),
        })
    })?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(bills)
}

#[tracing::instrument(level = "debug", skip_all)]
fn calculate_same_account_investment_pnl_correction(
    tx: &Transaction<'_>,
    user_id: i64,
    account_id: i64,
) -> DbResult<Money> {
    let mut statement = tx.prepare(
        "
        SELECT amount, destination_amount, type, counterparty, description,
               payment_method, main_category, sub_category
        FROM bills
        WHERE user_id = ?1
          AND type IN ('投资', 'investment', '5')
          AND source_account_id = ?2
          AND destination_account_id = ?2
        ",
    )?;
    let rows = statement.query_map(params![user_id, account_id], |row| {
        let amount = row.get::<_, f64>(0)?;
        let destination_amount = row.get::<_, Option<f64>>(1)?.unwrap_or(0.0);
        let mut bill = Map::new();
        bill.insert("amount".to_string(), json_real(amount));
        bill.insert(
            "destination_amount".to_string(),
            json_real(destination_amount),
        );
        for (index, key) in [
            "type",
            "counterparty",
            "description",
            "payment_method",
            "main_category",
            "sub_category",
        ]
        .iter()
        .enumerate()
        {
            let value = row.get::<_, Option<String>>(index + 2)?.unwrap_or_default();
            bill.insert((*key).to_string(), Value::String(value));
        }
        Ok((bill, amount, destination_amount))
    })?;

    let mut correction = 0.0_f64;
    for row in rows {
        let (bill, amount, destination_amount) = row?;
        let Some(signal) = classify_investment_pnl_change(&bill, None) else {
            continue;
        };
        let pnl_amount = amount.abs().max(destination_amount.abs());
        if pnl_amount <= 0.0 {
            continue;
        }
        let desired_effect = if signal.direction.as_deref() == Some("gain") {
            pnl_amount
        } else {
            -pnl_amount
        };
        let current_effect = -amount + destination_amount;
        correction += desired_effect - current_effect;
    }
    money_from_yuan(correction)
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_bill_filter_conditions(filters: &BillFilters) -> DbResult<(Vec<String>, Vec<SqlValue>)> {
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    if let Some(id) = filters.id.filter(|value| *value > 0) {
        conditions.push("id = ?".to_string());
        params.push(SqlValue::Integer(id));
    }
    if let Some(value) = text_filter(filters.date_from.as_deref()) {
        conditions.push("date >= ?".to_string());
        params.push(SqlValue::Text(value));
    }
    if let Some(value) = text_filter(filters.date_to.as_deref()) {
        conditions.push("date <= ?".to_string());
        params.push(SqlValue::Text(value));
    }
    if let Some(value) = text_filter(filters.transaction_type.as_deref()) {
        conditions.push("type = ?".to_string());
        params.push(SqlValue::Text(value));
    }
    if let Some(value) = text_filter(filters.main_category.as_deref()) {
        conditions.push("main_category = ?".to_string());
        params.push(SqlValue::Text(value));
    }
    if let Some(value) = text_filter(filters.sub_category.as_deref()) {
        conditions.push("sub_category = ?".to_string());
        params.push(SqlValue::Text(value));
    }
    if let Some(value) = text_filter(filters.batch_id.as_deref()) {
        conditions.push("batch_id = ?".to_string());
        params.push(SqlValue::Text(value));
    }
    if let Some(value) = text_filter(filters.counterparty.as_deref()) {
        conditions.push("counterparty LIKE ?".to_string());
        params.push(SqlValue::Text(format!("%{value}%")));
    }
    if let Some(value) = text_filter(filters.description.as_deref()) {
        conditions.push("description LIKE ?".to_string());
        params.push(SqlValue::Text(format!("%{value}%")));
    }
    if let Some(value) = text_filter(filters.keyword.as_deref()) {
        conditions.push("(description LIKE ? OR counterparty LIKE ?)".to_string());
        params.push(SqlValue::Text(format!("%{value}%")));
        params.push(SqlValue::Text(format!("%{value}%")));
    }
    if !filters.account_ids.is_empty() {
        let account_ids = normalize_bill_ids(&filters.account_ids);
        if !account_ids.is_empty() {
            let placeholders = placeholders(account_ids.len());
            conditions.push(format!(
                "(source_account_id IN ({placeholders}) OR destination_account_id IN ({placeholders}))"
            ));
            params.extend(account_ids.iter().copied().map(SqlValue::Integer));
            params.extend(account_ids.iter().copied().map(SqlValue::Integer));
        }
    }
    if !filters.categories.is_empty() {
        let mut category_conditions = Vec::new();
        for category in &filters.categories {
            let Some(main) = text_filter(Some(&category.main)) else {
                continue;
            };
            if let Some(sub) = text_filter(category.sub.as_deref()) {
                category_conditions.push("(main_category = ? AND sub_category = ?)".to_string());
                params.push(SqlValue::Text(main));
                params.push(SqlValue::Text(sub));
            } else {
                category_conditions.push("(main_category = ?)".to_string());
                params.push(SqlValue::Text(main));
            }
        }
        if !category_conditions.is_empty() {
            conditions.push(format!("({})", category_conditions.join(" OR ")));
        }
    }
    if !filters.tag_ids.is_empty() {
        let tag_ids = normalize_bill_ids(&filters.tag_ids);
        if !tag_ids.is_empty() {
            let placeholders = placeholders(tag_ids.len());
            conditions.push(format!(
                "id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN ({placeholders}))"
            ));
            params.extend(tag_ids.into_iter().map(SqlValue::Integer));
        }
    }
    if let Some(value) = filters.min_amount {
        conditions.push("amount >= ?".to_string());
        params.push(SqlValue::Real(value));
    }
    if let Some(value) = filters.max_amount {
        conditions.push("amount <= ?".to_string());
        params.push(SqlValue::Real(value));
    }
    if let Some(value) = text_filter(filters.amount_filter.as_deref()) {
        apply_amount_filter(&mut conditions, &mut params, &value);
    }
    Ok((conditions, params))
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_amount_filter(
    conditions: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    amount_filter: &str,
) {
    let parts = amount_filter.split(':').collect::<Vec<_>>();
    if parts.len() < 2 {
        return;
    }
    let amount = |index: usize| -> Option<f64> { parts.get(index)?.parse::<f64>().ok() };
    match parts[0].to_ascii_lowercase().as_str() {
        "eq" => push_amount_filter_condition(conditions, params, "amount = ?", amount(1)),
        "ne" => push_amount_filter_condition(conditions, params, "amount != ?", amount(1)),
        "gt" => push_amount_filter_condition(conditions, params, "amount > ?", amount(1)),
        "lt" => push_amount_filter_condition(conditions, params, "amount < ?", amount(1)),
        "gte" => push_amount_filter_condition(conditions, params, "amount >= ?", amount(1)),
        "lte" => push_amount_filter_condition(conditions, params, "amount <= ?", amount(1)),
        "between" => {
            if let (Some(minimum), Some(maximum)) = (amount(1), amount(2)) {
                conditions.push("amount BETWEEN ? AND ?".to_string());
                params.push(SqlValue::Real(minimum));
                params.push(SqlValue::Real(maximum));
            }
        }
        _ => {}
    }
}

fn push_amount_filter_condition(
    conditions: &mut Vec<String>,
    params: &mut Vec<SqlValue>,
    condition: &str,
    amount: Option<f64>,
) {
    if let Some(amount) = amount {
        conditions.push(condition.to_string());
        params.push(SqlValue::Real(amount));
    }
}

fn bill_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json_i64(row.get::<_, i64>("id")?));
    record.insert(
        "user_id".to_string(),
        json_i64(row.get::<_, i64>("user_id")?),
    );
    for key in [
        "date",
        "type",
        "counterparty",
        "description",
        "created_at",
        "updated_at",
    ] {
        record.insert(key.to_string(), Value::String(row.get::<_, String>(key)?));
    }
    record.insert(
        "amount".to_string(),
        json_real(row.get::<_, f64>("amount")?),
    );
    for key in [
        "payment_method",
        "main_category",
        "sub_category",
        "batch_id",
        "hash",
    ] {
        record.insert(
            key.to_string(),
            optional_string_value(row.get::<_, Option<String>>(key)?),
        );
    }
    record.insert(
        "source_account_id".to_string(),
        json_i64(row.get::<_, Option<i64>>("source_account_id")?.unwrap_or(0)),
    );
    record.insert(
        "destination_account_id".to_string(),
        json_i64(
            row.get::<_, Option<i64>>("destination_account_id")?
                .unwrap_or(0),
        ),
    );
    record.insert(
        "destination_amount".to_string(),
        json_real(
            row.get::<_, Option<f64>>("destination_amount")?
                .unwrap_or(0.0),
        ),
    );
    for key in [
        "created_from_template",
        "created_from_recurring",
        "import_history_id",
    ] {
        record.insert(
            key.to_string(),
            optional_i64_value(row.get::<_, Option<i64>>(key)?),
        );
    }
    Ok(record)
}

fn tag_value_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut tag = Map::new();
    tag.insert("id".to_string(), row.get::<_, i64>(0)?.to_string().into());
    tag.insert("name".to_string(), row.get::<_, String>(1)?.into());
    tag.insert(
        "color".to_string(),
        optional_string_value(row.get::<_, Option<String>>(2)?),
    );
    tag.insert(
        "icon".to_string(),
        optional_string_value(row.get::<_, Option<String>>(3)?),
    );
    Ok(Value::Object(tag))
}

#[tracing::instrument(level = "debug", skip_all)]
fn list_enabled_recurring_templates(
    connection: &Connection,
    user_id: i64,
) -> DbResult<Vec<BillRecord>> {
    let mut statement = connection.prepare(
        "
        SELECT id, user_id, template_id, name, description, type, category,
               amount, account, counterparty, destination_amount, hide_amount,
               tag, comment, frequency, scheduled_frequency_type, start_date,
               end_date, next_date, enabled, auto_create, display_order, hidden,
               utc_offset, created_at, updated_at
        FROM recurring_bills
        WHERE user_id = ?1 AND enabled = 1
        ORDER BY COALESCE(display_order, 0), name
        ",
    )?;
    let rows = statement.query_map(params![user_id], recurring_record_from_row)?;
    let mut recurring = Vec::new();
    for row in rows {
        recurring.push(row?);
    }
    Ok(recurring)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_recurring_template_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<Option<BillRecord>> {
    tx.query_row(
        "
        SELECT id, user_id, template_id, name, description, type, category,
               amount, account, counterparty, destination_amount, hide_amount,
               tag, comment, frequency, scheduled_frequency_type, start_date,
               end_date, next_date, enabled, auto_create, display_order, hidden,
               utc_offset, created_at, updated_at
        FROM recurring_bills
        WHERE id = ?1 AND user_id = ?2
        ",
        params![recurring_id, user_id],
        recurring_record_from_row,
    )
    .optional()
    .map_err(DbError::from)
}

fn recurring_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillRecord> {
    let mut record = Map::new();
    for key in [
        "id",
        "user_id",
        "template_id",
        "name",
        "description",
        "type",
        "category",
        "amount",
        "account",
        "counterparty",
        "destination_amount",
        "hide_amount",
        "tag",
        "comment",
        "frequency",
        "scheduled_frequency_type",
        "start_date",
        "end_date",
        "next_date",
        "enabled",
        "auto_create",
        "display_order",
        "hidden",
        "utc_offset",
        "created_at",
        "updated_at",
    ] {
        record.insert(key.to_string(), sql_value_ref_to_json(row.get_ref(key)?));
    }
    Ok(record)
}

fn sql_value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json_i64(value),
        ValueRef::Real(value) => json_real(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_recurring_candidates_for_bill_data(
    bill: &BillRecord,
    recurring_rows: &[BillRecord],
    linked_recurring_id: Option<i64>,
    tolerance_days: i64,
) -> Vec<Value> {
    let Some(bill_date) = parse_date_value(&record_text(bill, "date")) else {
        return Vec::new();
    };
    let bill_type = normalize_template_transaction_type(bill.get("type"));
    let bill_amount_cents =
        (record_f64(bill, "amount").unwrap_or(0.0).abs() * 100.0).round() as i64;
    let bill_source_account = record_text(bill, "source_account_id");
    let bill_destination_account = record_text(bill, "destination_account_id");
    let mut candidates = Vec::new();

    for recurring in recurring_rows {
        if normalize_template_transaction_type(recurring.get("type")) != bill_type {
            continue;
        }
        let recurring_amount_cents = recurring_f64(recurring, "amount").abs().round() as i64;
        if recurring_amount_cents != bill_amount_cents {
            continue;
        }
        let Some(matched_occurrence) =
            find_recurring_occurrence_near_date(recurring, bill_date, tolerance_days)
        else {
            continue;
        };

        let mut score = 80_i64;
        let mut reasons = vec![
            Value::String("type".to_string()),
            Value::String("amount".to_string()),
            Value::String("schedule".to_string()),
        ];
        if recurring_text(recurring, "account") == bill_source_account {
            reasons.push(Value::String("source_account".to_string()));
            score += 10;
        }
        if !matches!(bill_destination_account.as_str(), "" | "0")
            && recurring_text(recurring, "counterparty") == bill_destination_account
        {
            reasons.push(Value::String("destination_account".to_string()));
            score += 10;
        }
        let days_offset = (matched_occurrence - bill_date).num_days().abs();
        score += (10 - days_offset * 2).max(0);

        let mut candidate = serialize_recurring_template_row(recurring);
        candidate.insert("matchScore".to_string(), json_i64(score));
        candidate.insert("matchReasons".to_string(), Value::Array(reasons));
        candidate.insert(
            "matchedOccurrenceDate".to_string(),
            Value::String(matched_occurrence.to_string()),
        );
        candidate.insert("matchedDayOffset".to_string(), json_i64(days_offset));
        candidate.insert(
            "linked".to_string(),
            Value::Bool(
                linked_recurring_id
                    .zip(recurring_i64(recurring, "id"))
                    .is_some_and(|(linked, recurring_id)| linked == recurring_id),
            ),
        );
        candidates.push(Value::Object(candidate));
    }

    candidates.sort_by(|left, right| {
        let left = left.as_object().expect("candidate object");
        let right = right.as_object().expect("candidate object");
        recurring_i64(right, "matchScore")
            .cmp(&recurring_i64(left, "matchScore"))
            .then_with(|| {
                recurring_i64(left, "matchedDayOffset")
                    .unwrap_or(999)
                    .cmp(&recurring_i64(right, "matchedDayOffset").unwrap_or(999))
            })
            .then_with(|| recurring_text(left, "name").cmp(&recurring_text(right, "name")))
    });
    candidates
}

fn serialize_recurring_template_row(row: &BillRecord) -> Map<String, Value> {
    let mut value = Map::new();
    value.insert("id".to_string(), recurring_text(row, "id").into());
    value.insert("timeSequenceId".to_string(), String::new().into());
    value.insert("templateType".to_string(), json_i64(2));
    value.insert("name".to_string(), recurring_text(row, "name").into());
    value.insert(
        "type".to_string(),
        json_i64(normalize_template_transaction_type(row.get("type"))),
    );
    value.insert(
        "categoryId".to_string(),
        recurring_text(row, "category").into(),
    );
    value.insert("time".to_string(), json_i64(0));
    value.insert(
        "utcOffset".to_string(),
        json_i64(recurring_i64(row, "utc_offset").unwrap_or(0)),
    );
    value.insert(
        "sourceAccountId".to_string(),
        recurring_text(row, "account").into(),
    );
    value.insert(
        "destinationAccountId".to_string(),
        recurring_text(row, "counterparty").into(),
    );
    value.insert(
        "sourceAmount".to_string(),
        json_real(recurring_f64(row, "amount")),
    );
    value.insert(
        "destinationAmount".to_string(),
        json_real(recurring_f64(row, "destination_amount")),
    );
    value.insert(
        "hideAmount".to_string(),
        Value::Bool(recurring_i64(row, "hide_amount").unwrap_or(0) != 0),
    );
    value.insert(
        "tagIds".to_string(),
        Value::Array(
            recurring_text(row, "tag")
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect(),
        ),
    );
    value.insert("comment".to_string(), recurring_text(row, "comment").into());
    value.insert("editable".to_string(), Value::Bool(true));
    value.insert(
        "displayOrder".to_string(),
        json_i64(recurring_i64(row, "display_order").unwrap_or(0)),
    );
    value.insert(
        "hidden".to_string(),
        Value::Bool(recurring_i64(row, "hidden").unwrap_or(0) != 0),
    );
    value.insert(
        "scheduledFrequencyType".to_string(),
        json_i64(recurring_i64(row, "scheduled_frequency_type").unwrap_or(0)),
    );
    value.insert(
        "scheduledFrequency".to_string(),
        recurring_optional_text_json(row, "frequency"),
    );
    value.insert(
        "scheduledStartDate".to_string(),
        recurring_optional_text_json(row, "start_date"),
    );
    value.insert(
        "scheduledEndDate".to_string(),
        recurring_optional_text_json(row, "end_date"),
    );
    value.insert("scheduledAt".to_string(), Value::Null);
    value
}

#[tracing::instrument(level = "debug", skip_all)]
fn recalculate_recurring_next_date_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    recurring_id: i64,
    now: &str,
) -> DbResult<()> {
    let Some(recurring) = get_recurring_template_on_tx(tx, user_id, recurring_id)? else {
        return Ok(());
    };
    let latest_linked_date = tx
        .query_row(
            "
            SELECT date FROM bills
            WHERE user_id = ?1 AND created_from_recurring = ?2
            ORDER BY date DESC
            LIMIT 1
            ",
            params![user_id, recurring_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .and_then(|value| parse_date_value(&value));
    let fallback_next_date = recurring_text(&recurring, "next_date");
    let next_occurrence = latest_linked_date
        .and_then(|date| get_next_recurring_occurrence_after(&recurring, date, 370))
        .or_else(|| get_first_recurring_occurrence(&recurring, 370))
        .map(|date| date.to_string())
        .or_else(|| non_empty_str(fallback_next_date.as_str()).map(ToString::to_string));
    tx.execute(
        "UPDATE recurring_bills SET next_date = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
        params![next_occurrence, now, recurring_id, user_id],
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_date_value(value: &str) -> Option<NaiveDate> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_schedule_frequency_values(value: &str) -> Vec<u32> {
    let mut values = value
        .split(',')
        .filter_map(|item| item.trim().parse::<u32>().ok())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}

fn weekday_sunday_first(date: NaiveDate) -> u32 {
    date.weekday().num_days_from_sunday()
}

fn is_recurring_active_on_date(recurring: &BillRecord, target_date: NaiveDate) -> bool {
    if parse_date_value(&recurring_text(recurring, "start_date"))
        .is_some_and(|start_date| target_date < start_date)
    {
        return false;
    }
    if parse_date_value(&recurring_text(recurring, "end_date"))
        .is_some_and(|end_date| target_date > end_date)
    {
        return false;
    }
    true
}

fn is_recurring_due_on_date(recurring: &BillRecord, target_date: NaiveDate) -> bool {
    if !is_recurring_active_on_date(recurring, target_date) {
        return false;
    }
    let frequency_type = recurring_i64(recurring, "scheduled_frequency_type").unwrap_or(0);
    let frequency_values = parse_schedule_frequency_values(&recurring_text(recurring, "frequency"));
    let start_date = parse_date_value(&recurring_text(recurring, "start_date"));
    let next_date = parse_date_value(&recurring_text(recurring, "next_date"));

    if frequency_type == 1 {
        let valid_weekdays = if frequency_values.is_empty() {
            start_date
                .map(weekday_sunday_first)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            frequency_values
        };
        return valid_weekdays.contains(&weekday_sunday_first(target_date));
    }
    if frequency_type == 2 {
        let valid_days = if frequency_values.is_empty() {
            start_date
                .map(|date| date.day())
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            frequency_values
        };
        return valid_days.contains(&target_date.day());
    }
    next_date.is_some_and(|date| date == target_date)
        || start_date.is_some_and(|date| date == target_date)
}

#[tracing::instrument(level = "debug", skip_all)]
fn find_recurring_occurrence_near_date(
    recurring: &BillRecord,
    target_date: NaiveDate,
    tolerance_days: i64,
) -> Option<NaiveDate> {
    let mut nearest_date = None;
    let mut nearest_diff = None;
    for offset in -tolerance_days..=tolerance_days {
        let current_date = target_date + Duration::days(offset);
        if !is_recurring_due_on_date(recurring, current_date) {
            continue;
        }
        let diff = offset.abs();
        if nearest_date.is_none() || nearest_diff.is_some_and(|value| diff < value) {
            nearest_date = Some(current_date);
            nearest_diff = Some(diff);
        }
    }
    nearest_date
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_next_recurring_occurrence_after(
    recurring: &BillRecord,
    after_date: NaiveDate,
    max_search_days: i64,
) -> Option<NaiveDate> {
    (1..=max_search_days)
        .map(|offset| after_date + Duration::days(offset))
        .find(|candidate| is_recurring_due_on_date(recurring, *candidate))
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_first_recurring_occurrence(
    recurring: &BillRecord,
    max_search_days: i64,
) -> Option<NaiveDate> {
    let Some(start_date) = parse_date_value(&recurring_text(recurring, "start_date")) else {
        return parse_date_value(&recurring_text(recurring, "next_date"));
    };
    (0..=max_search_days)
        .map(|offset| start_date + Duration::days(offset))
        .find(|candidate| is_recurring_due_on_date(recurring, *candidate))
        .or_else(|| parse_date_value(&recurring_text(recurring, "next_date")))
}

fn collect_account_ids(snapshots: impl IntoIterator<Item = BillAccountSyncSnapshot>) -> Vec<i64> {
    let mut ids = BTreeSet::new();
    for snapshot in snapshots {
        if let Some(source_id) = snapshot.source_account_id.filter(|value| *value > 0) {
            ids.insert(source_id);
        }
        if let Some(destination_id) = snapshot.destination_account_id.filter(|value| *value > 0) {
            ids.insert(destination_id);
        }
    }
    ids.into_iter().collect()
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_bill_ids(values: &[i64]) -> Vec<i64> {
    values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_create_aliases(payload: &mut BillRecord) {
    if !payload.contains_key("payment_method") {
        if let Some(value) = payload.remove("channel").filter(is_non_empty_json_value) {
            payload.insert("payment_method".to_string(), value);
        }
    }
    if !payload.contains_key("main_category") {
        if let Some(value) = payload.remove("category").filter(is_non_empty_json_value) {
            payload.insert("main_category".to_string(), value);
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn update_requires_hash_recalculation(payload: &BillRecord) -> bool {
    ["date", "type", "amount", "counterparty", "description"]
        .iter()
        .any(|key| payload.contains_key(*key))
}

fn missing_required_field(payload: &BillRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn record_text(record: &BillRecord, key: &str) -> String {
    match record.get(key) {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn record_f64(record: &BillRecord, key: &str) -> DbResult<f64> {
    match record.get(key) {
        Some(Value::Number(value)) => value
            .as_f64()
            .ok_or_else(|| DbError::InvalidOperation(format!("invalid numeric field: {key}"))),
        Some(Value::String(value)) => value
            .trim()
            .parse::<f64>()
            .map_err(|_| DbError::InvalidOperation(format!("invalid numeric field: {key}"))),
        _ => Err(DbError::InvalidOperation(format!(
            "missing numeric field: {key}"
        ))),
    }
}

fn record_optional_i64(record: &BillRecord, key: &str) -> Option<i64> {
    value_as_i64(record.get(key)?)
}

fn recurring_text(record: &BillRecord, key: &str) -> String {
    match record.get(key) {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn recurring_i64(record: &BillRecord, key: &str) -> Option<i64> {
    value_as_i64(record.get(key)?)
}

fn recurring_f64(record: &BillRecord, key: &str) -> f64 {
    match record.get(key) {
        Some(Value::Number(value)) => value.as_f64().unwrap_or(0.0),
        Some(Value::String(value)) => value.trim().parse::<f64>().unwrap_or(0.0),
        Some(Value::Bool(value)) => f64::from(*value as u8),
        _ => 0.0,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_f64().map(|value| value as i64)),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        Value::Bool(value) => Some(i64::from(*value)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn recurring_optional_text_json(record: &BillRecord, key: &str) -> Value {
    match record.get(key) {
        Some(Value::String(value)) => Value::String(value.clone()),
        Some(Value::Number(value)) => Value::String(value.to_string()),
        Some(Value::Bool(value)) => Value::String(value.to_string()),
        Some(Value::Null) | Some(Value::Array(_)) | Some(Value::Object(_)) | None => Value::Null,
    }
}

fn non_empty_str(value: &str) -> Option<&str> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_template_transaction_type(value: Option<&Value>) -> i64 {
    let text = match value {
        Some(Value::String(value)) => value.trim().to_ascii_lowercase(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => return 3,
    };
    match text.as_str() {
        "2" | "income" | "收入" => 2,
        "3" | "expense" | "支出" => 3,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn json_to_sql_value(value: Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(value)),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_u64().and_then(|value| i64::try_from(value).ok())
            {
                SqlValue::Integer(value)
            } else {
                SqlValue::Real(number.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(value) => SqlValue::Text(value),
        Value::Array(_) | Value::Object(_) => SqlValue::Text(value.to_string()),
    }
}

fn money_from_yuan(value: f64) -> DbResult<Money> {
    Money::from_yuan_str(&python_float_text(value)).map_err(runtime_error)
}

fn money_to_yuan_f64(value: Money) -> DbResult<f64> {
    value
        .to_yuan_string()
        .parse::<f64>()
        .map_err(|_| DbError::InvalidOperation("money amount cannot be stored".to_string()))
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn python_float_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

fn runtime_error(error: RuntimeError) -> DbError {
    DbError::InvalidOperation(error.to_string())
}

fn table_exists(tx: &Transaction<'_>, table_name: &str) -> DbResult<bool> {
    tx.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn where_suffix(conditions: &[String]) -> String {
    if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    }
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn now_text() -> String {
    Utc::now().to_rfc3339()
}

fn text_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn json_i64(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn json_real(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn optional_string_value(value: Option<String>) -> Value {
    value
        .filter(|value| !value.is_empty())
        .map_or(Value::Null, Value::String)
}

fn optional_i64_value(value: Option<i64>) -> Value {
    value.map_or(Value::Null, json_i64)
}

fn nullable_positive_i64(value: Option<i64>) -> Option<i64> {
    value.filter(|value| *value > 0)
}

fn is_non_empty_json_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[allow(dead_code)]
fn _assert_update_fields_subset() {
    for field in ROUTE_ALLOWED_BATCH_UPDATE_FIELDS {
        debug_assert!(BILL_UPDATE_COLUMNS.contains(field));
    }
}
