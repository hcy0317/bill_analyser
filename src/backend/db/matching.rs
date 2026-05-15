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

const IMPORT_RECONCILIATION_FAMILY: &str = "import_reconciliation";
const PENDING_STATUS: &str = "pending";
const RECONCILIATION_STALE_PROJECTION_MESSAGE: &str =
    "Bill changed since reconciliation projection, please refresh";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatchingRuntimeError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Db(String),
}

impl MatchingRuntimeError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::Db(_) => 500,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::Db(message) => message,
        }
    }
}

impl From<DbError> for MatchingRuntimeError {
    fn from(error: DbError) -> Self {
        Self::Db(error.to_string())
    }
}

impl From<rusqlite::Error> for MatchingRuntimeError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error.to_string())
    }
}

pub type MatchingResult<T> = Result<T, MatchingRuntimeError>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReconciliationCandidateFilters {
    pub session_id: Option<String>,
    pub preview_id: Option<i64>,
    pub existing_bill_id: Option<i64>,
    pub candidate_type: Option<String>,
    pub status: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreviewMatchingActionRequest {
    pub expected_state: Option<ImportPreviewExpectedState>,
    pub response_mode_preview_item: bool,
    pub reviewed_type: Option<String>,
    pub recurring_id: Option<i64>,
    pub recurring_candidate_count: i64,
    pub recurring_candidate: Option<ImportPreviewRecurringCandidate>,
    pub learning_apply: Option<ImportPreviewLearningApply>,
}

pub fn init_matching_runtime_schema(connection: &Connection) -> DbResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bill_pair_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            pair_type TEXT NOT NULL,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(left_bill_id < right_bill_id),
            UNIQUE(user_id, pair_type, left_bill_id, right_bill_id)
        );
        CREATE TABLE IF NOT EXISTS bill_pair_feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            candidate_id TEXT NOT NULL,
            action TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS bill_transfer_pair_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK(left_bill_id < right_bill_id),
            UNIQUE(user_id, left_bill_id, right_bill_id)
        );
        CREATE TABLE IF NOT EXISTS bill_investment_pair_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK(left_bill_id < right_bill_id),
            UNIQUE(user_id, left_bill_id, right_bill_id)
        );
        CREATE TABLE IF NOT EXISTS bill_learning_rule_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            bill_id INTEGER NOT NULL,
            rule_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, bill_id, rule_id)
        );
        CREATE TABLE IF NOT EXISTS bill_reconciliation_candidates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            family TEXT NOT NULL DEFAULT 'import_reconciliation',
            candidate_id TEXT NOT NULL,
            candidate_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            session_id TEXT,
            preview_id INTEGER,
            import_bill_key TEXT NOT NULL,
            existing_bill_id INTEGER NOT NULL,
            group_key TEXT NOT NULL,
            amount_abs REAL NOT NULL DEFAULT 0,
            time_diff_seconds INTEGER,
            score REAL NOT NULL DEFAULT 0,
            level TEXT NOT NULL DEFAULT '',
            reason TEXT NOT NULL DEFAULT '',
            import_bill_snapshot_json TEXT NOT NULL DEFAULT '{}',
            existing_bill_snapshot_json TEXT NOT NULL DEFAULT '{}',
            source_payload_json TEXT NOT NULL DEFAULT '{}',
            seen_count INTEGER NOT NULL DEFAULT 1,
            first_seen_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            resolved_at TEXT,
            resolution_event_id INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(candidate_type IN ('transfer', 'duplicate')),
            CHECK(status IN ('pending', 'accepted', 'rejected', 'merged', 'rolled_back', 'superseded')),
            UNIQUE(user_id, candidate_id)
        );
        CREATE TABLE IF NOT EXISTS bill_merge_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            family TEXT NOT NULL DEFAULT 'import_reconciliation',
            group_key TEXT NOT NULL,
            group_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            canonical_bill_id INTEGER,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(group_type IN ('transfer', 'duplicate')),
            CHECK(status IN ('pending', 'accepted', 'merged', 'rolled_back', 'superseded')),
            UNIQUE(user_id, family, group_key)
        );
        CREATE TABLE IF NOT EXISTS bill_merge_members (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            member_key TEXT NOT NULL,
            member_type TEXT NOT NULL,
            bill_id INTEGER,
            import_bill_key TEXT,
            candidate_id TEXT,
            role TEXT NOT NULL DEFAULT 'candidate',
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(member_type IN ('existing_bill', 'import_bill')),
            UNIQUE(group_id, member_key)
        );
        CREATE TABLE IF NOT EXISTS bill_merge_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            group_id INTEGER NOT NULL,
            candidate_id TEXT,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_left
            ON bill_pair_links(user_id, left_bill_id);
        CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_right
            ON bill_pair_links(user_id, right_bill_id);
        CREATE INDEX IF NOT EXISTS idx_bill_pair_feedback_user_created_at
            ON bill_pair_feedback(user_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_status
            ON bill_reconciliation_candidates(user_id, family, status);
        CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_existing
            ON bill_reconciliation_candidates(user_id, existing_bill_id);
        ",
    )?;
    Ok(())
}

pub fn query_matching_session_candidates_payload(
    connection: &Connection,
    user_id: UserId,
    session_id: &str,
) -> MatchingResult<Option<Value>> {
    let Some(_) = get_import_session(connection, session_id, user_id)? else {
        return Ok(None);
    };
    let previews = get_preview_by_session(connection, session_id, user_id, false)?
        .into_iter()
        .map(preview_row_to_matching_input)
        .collect::<Vec<_>>();
    Ok(Some(build_matching_session_candidates(
        session_id, &previews,
    )))
}

pub fn query_matching_bill_candidates_payload(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> MatchingResult<Option<Value>> {
    init_matching_runtime_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let Some(anchor_bill) = get_bill_map(connection, user_id, bill_id)? else {
        return Ok(None);
    };
    let reconciliation = get_bill_reconciliation_projection(connection, user_id, bill_id)?;
    if let Some(pair) = get_bill_pair_link_for_bill(connection, user_id, bill_id, None)? {
        return Ok(Some(json!({
            "billId": bill_id,
            "linkedPair": serialize_bill_pair(&with_other_bill_id(pair, bill_id)),
            "candidates": [],
            "reconciliation": reconciliation,
        })));
    }

    let mut candidates = Vec::new();
    candidates.extend(list_reconciliation_candidates_for_bill(
        connection, user_id, bill_id,
    )?);
    candidates.extend(list_transfer_candidates_for_bill(
        connection,
        user_id,
        &anchor_bill,
    )?);
    candidates.extend(list_investment_candidates_for_bill(
        connection,
        user_id,
        &anchor_bill,
    )?);
    candidates.extend(list_learning_candidates_for_bill(
        connection,
        user_id,
        &anchor_bill,
    )?);

    Ok(Some(json!({
        "billId": bill_id,
        "linkedPair": Value::Null,
        "candidates": candidates.into_iter().map(serialize_matching_candidate).collect::<Vec<_>>(),
        "reconciliation": reconciliation,
    })))
}

pub fn query_matching_pairs_payload(
    connection: &Connection,
    user_id: UserId,
) -> MatchingResult<Value> {
    init_matching_runtime_schema(connection)?;
    let user_id_value = UserScope::new(user_id).bind_value()?;
    let mut statement = connection.prepare(
        "
        SELECT p.*,
               l.date AS left_bill_date, l.type AS left_bill_type, l.amount AS left_bill_amount,
               l.counterparty AS left_bill_counterparty, l.description AS left_bill_description,
               l.payment_method AS left_bill_payment_method, l.main_category AS left_bill_main_category,
               l.sub_category AS left_bill_sub_category, l.source_account_id AS left_bill_source_account_id,
               l.destination_account_id AS left_bill_destination_account_id,
               r.date AS right_bill_date, r.type AS right_bill_type, r.amount AS right_bill_amount,
               r.counterparty AS right_bill_counterparty, r.description AS right_bill_description,
               r.payment_method AS right_bill_payment_method, r.main_category AS right_bill_main_category,
               r.sub_category AS right_bill_sub_category, r.source_account_id AS right_bill_source_account_id,
               r.destination_account_id AS right_bill_destination_account_id
        FROM bill_pair_links p
        JOIN bills l ON l.id = p.left_bill_id AND l.user_id = p.user_id
        JOIN bills r ON r.id = p.right_bill_id AND r.user_id = p.user_id
        WHERE p.user_id = ? AND p.source = ?
        ORDER BY COALESCE(p.updated_at, p.created_at) DESC, p.id DESC
        ",
    )?;
    let rows = statement.query_map(params![user_id_value, MANUAL_PAIR_SOURCE], |row| {
        let pair = row_to_map(row, "p")?;
        let left_bill = prefixed_bill_from_row(row, "left_bill")?;
        let right_bill = prefixed_bill_from_row(row, "right_bill")?;
        Ok(json!({
            "id": map_i64(&pair, "id"),
            "pairType": map_string(&pair, "pair_type", TRANSFER_PAIR_TYPE),
            "source": map_string(&pair, "source", MANUAL_PAIR_SOURCE),
            "leftBillId": map_i64(&pair, "left_bill_id"),
            "rightBillId": map_i64(&pair, "right_bill_id"),
            "createdAt": map_string(&pair, "created_at", ""),
            "updatedAt": map_string(&pair, "updated_at", ""),
            "leftBill": serialize_bill_snapshot(&left_bill),
            "rightBill": serialize_bill_snapshot(&right_bill),
        }))
    })?;
    let pairs = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "pairs": pairs }))
}

pub fn query_matching_bill_feedback_payload(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> MatchingResult<Option<Value>> {
    init_matching_runtime_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    if get_bill_map(connection, user_id, bill_id)?.is_none() {
        return Ok(None);
    }
    let mut statement = connection.prepare(
        "
        SELECT id, candidate_id, action, payload_json, created_at
        FROM bill_pair_feedback
        WHERE user_id = ?
        ORDER BY COALESCE(created_at, '') DESC, id DESC
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        let payload_json: String = row.get("payload_json")?;
        Ok(json!({
            "id": row.get::<_, i64>("id")?,
            "candidateId": row.get::<_, String>("candidate_id")?,
            "action": row.get::<_, String>("action")?,
            "createdAt": row.get::<_, String>("created_at")?,
            "payload": parse_json_object(&payload_json),
        }))
    })?;
    let events = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|event| {
            event
                .get("payload")
                .is_some_and(|payload| bill_pair_feedback_payload_is_related(payload, bill_id))
        })
        .collect::<Vec<_>>();
    Ok(Some(json!({ "billId": bill_id, "events": events })))
}

pub fn list_reconciliation_candidates_payload(
    connection: &Connection,
    user_id: UserId,
    filters: &ReconciliationCandidateFilters,
) -> MatchingResult<Value> {
    init_matching_runtime_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let rows = list_reconciliation_candidates(connection, user_id, filters)?;
    Ok(json!({
        "candidates": rows.into_iter().map(serialize_reconciliation_candidate).collect::<Vec<_>>()
    }))
}

pub fn create_manual_matching_pair(
    connection: &mut Connection,
    user_id: UserId,
    bill_id: i64,
    candidate_bill_id: i64,
    pair_type: &str,
    feedback_candidate_id: Option<&str>,
) -> MatchingResult<Value> {
    init_matching_runtime_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    let pair_type = normalize_pair_type(pair_type)?;
    let (left_bill_id, right_bill_id) =
        normalize_transfer_pair_bill_ids(bill_id, candidate_bill_id)
            .map_err(|message| MatchingRuntimeError::BadRequest(message.to_string()))?;
    let now = utc_now();
    run_transaction(connection, |tx| {
        let bills = get_bills_by_ids_on_tx(tx, user_id, &[left_bill_id, right_bill_id])?;
        if bills.len() != 2 {
            return Err(DbError::InvalidOperation("Bill not found".to_string()));
        }
        if get_pair_for_bill_on_tx(tx, user_id, left_bill_id, None)?.is_some()
            || get_pair_for_bill_on_tx(tx, user_id, right_bill_id, None)?.is_some()
        {
            return Err(DbError::InvalidOperation(
                "Bills already belong to an existing transfer pair".to_string(),
            ));
        }
        if pair_type == TRANSFER_PAIR_TYPE
            && transfer_suppression_exists_on_tx(tx, user_id, left_bill_id, right_bill_id)?
        {
            return Err(DbError::InvalidOperation(
                "Bills already rejected for transfer pairing".to_string(),
            ));
        }
        if pair_type == INVESTMENT_PAIR_TYPE
            && (transfer_suppression_exists_on_tx(tx, user_id, left_bill_id, right_bill_id)?
                || investment_suppression_exists_on_tx(tx, user_id, left_bill_id, right_bill_id)?)
        {
            return Err(DbError::InvalidOperation(
                "Bills already rejected for investment pairing".to_string(),
            ));
        }
        let left = bills
            .iter()
            .find(|bill| map_i64(bill, "id") == left_bill_id)
            .expect("left bill");
        let right = bills
            .iter()
            .find(|bill| map_i64(bill, "id") == right_bill_id)
            .expect("right bill");
        if !pair_is_eligible(tx, user_id, left, right, pair_type)? {
            return Err(DbError::InvalidOperation(format!(
                "Bills are not eligible for {pair_type} pairing"
            )));
        }
        tx.execute(
            "
            INSERT INTO bill_pair_links(user_id, pair_type, left_bill_id, right_bill_id, source, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ",
            params![user_id, pair_type, left_bill_id, right_bill_id, MANUAL_PAIR_SOURCE, now, now],
        )?;
        let pair = json!({
            "id": tx.last_insert_rowid(),
            "pair_type": pair_type,
            "source": MANUAL_PAIR_SOURCE,
            "left_bill_id": left_bill_id,
            "right_bill_id": right_bill_id,
        });
        if let Some(candidate_id) = feedback_candidate_id.filter(|value| !value.trim().is_empty()) {
            record_feedback_on_tx(
                tx,
                user_id,
                candidate_id,
                "accept",
                &build_bill_pair_feedback_payload(
                    pair_type,
                    bill_id,
                    candidate_bill_id,
                    pair.as_object(),
                ),
                &now,
            )?;
        }
        Ok(pair)
    })
    .map(|pair| json!({ "pair": serialize_bill_pair(pair.as_object().expect("pair")) }))
    .map_err(map_write_error)
}

pub fn delete_manual_matching_pair(
    connection: &mut Connection,
    user_id: UserId,
    pair_id: i64,
) -> MatchingResult<Value> {
    init_matching_runtime_schema(connection)?;
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(pair) = get_pair_by_id_on_tx(tx, user_id, pair_id, None)? else {
            return Err(DbError::InvalidOperation("Pair not found".to_string()));
        };
        if map_string(&pair, "source", MANUAL_PAIR_SOURCE) != MANUAL_PAIR_SOURCE {
            return Err(DbError::InvalidOperation(
                "Only manual pairs can be deleted".to_string(),
            ));
        }
        tx.execute(
            "DELETE FROM bill_pair_links WHERE id = ? AND user_id = ?",
            params![pair_id, user_id],
        )?;
        Ok(pair)
    })
    .map(|pair| json!({ "pair": serialize_bill_pair(&pair) }))
    .map_err(map_write_error)
}

pub fn apply_matching_candidate_action(
    connection: &mut Connection,
    user_id: UserId,
    candidate_id: &str,
    action: &str,
    preview_request: &PreviewMatchingActionRequest,
) -> MatchingResult<Value> {
    init_matching_runtime_schema(connection)?;
    let user_id_value = UserScope::new(user_id).bind_value()?;
    let descriptor = parse_matching_candidate_id(candidate_id)
        .ok_or_else(|| MatchingRuntimeError::BadRequest("Invalid candidateId".to_string()))?;
    let action = action.trim().to_ascii_lowercase();
    match (
        descriptor.scope.as_str(),
        descriptor.kind.as_str(),
        action.as_str(),
    ) {
        ("bill", TRANSFER_PAIR_TYPE, "accept") | ("bill", INVESTMENT_PAIR_TYPE, "accept") => {
            let pair = create_manual_matching_pair(
                connection,
                user_id,
                descriptor.bill_id.unwrap_or_default(),
                descriptor.candidate_bill_id.unwrap_or_default(),
                &descriptor.kind,
                Some(candidate_id),
            )?;
            Ok(json!({"candidate_id": candidate_id, "action": "accept", "pair": pair["pair"]}))
        }
        ("bill", TRANSFER_PAIR_TYPE, "reject") | ("bill", INVESTMENT_PAIR_TYPE, "reject") => {
            reject_bill_pair_candidate(
                connection,
                user_id_value,
                descriptor.bill_id.unwrap_or_default(),
                descriptor.candidate_bill_id.unwrap_or_default(),
                &descriptor.kind,
                candidate_id,
            )?;
            Ok(json!({"candidate_id": candidate_id, "action": "reject"}))
        }
        ("bill", "learning", "accept") => accept_bill_learning_candidate(
            connection,
            user_id,
            candidate_id,
            descriptor.bill_id.unwrap_or_default(),
            descriptor.rule_id.unwrap_or_default(),
            descriptor.rule_revision.as_deref(),
        ),
        ("bill", "learning", "reject") => {
            reject_bill_learning_candidate(
                connection,
                user_id,
                descriptor.bill_id.unwrap_or_default(),
                descriptor.rule_id.unwrap_or_default(),
                descriptor.rule_revision.as_deref(),
            )?;
            Ok(json!({"candidate_id": candidate_id, "action": "reject"}))
        }
        ("preview", "transfer", "accept" | "reject" | "clear") => preview_transfer_action(
            connection,
            user_id,
            candidate_id,
            descriptor.preview_id.unwrap_or_default(),
            &action,
            preview_request,
        ),
        ("preview", "learning", "accept" | "reject" | "clear") => preview_learning_action(
            connection,
            user_id,
            candidate_id,
            descriptor.preview_id.unwrap_or_default(),
            &action,
            preview_request,
        ),
        ("preview", "recurring", "accept" | "reject") => preview_recurring_action(
            connection,
            user_id,
            candidate_id,
            descriptor.preview_id.unwrap_or_default(),
            &action,
            preview_request,
        ),
        ("reconciliation", "transfer" | "duplicate", "accept" | "reject" | "clear") => {
            reconciliation_action(connection, user_id_value, candidate_id, &action)
        }
        _ => Err(MatchingRuntimeError::BadRequest(
            "Candidate family not supported".to_string(),
        )),
    }
}

fn reject_bill_pair_candidate(
    connection: &mut Connection,
    user_id: i64,
    bill_id: i64,
    candidate_bill_id: i64,
    pair_type: &str,
    feedback_candidate_id: &str,
) -> MatchingResult<()> {
    let (left_bill_id, right_bill_id) =
        normalize_transfer_pair_bill_ids(bill_id, candidate_bill_id)
            .map_err(|message| MatchingRuntimeError::BadRequest(message.to_string()))?;
    let table = if pair_type == INVESTMENT_PAIR_TYPE {
        "bill_investment_pair_suppressions"
    } else {
        "bill_transfer_pair_suppressions"
    };
    let now = utc_now();
    run_transaction(connection, |tx| {
        let bills = get_bills_by_ids_on_tx(tx, user_id, &[left_bill_id, right_bill_id])?;
        if bills.len() != 2 {
            return Err(DbError::InvalidOperation("Bill not found".to_string()));
        }
        if get_pair_for_bill_on_tx(tx, user_id, left_bill_id, None)?.is_some()
            || get_pair_for_bill_on_tx(tx, user_id, right_bill_id, None)?.is_some()
        {
            return Err(DbError::InvalidOperation(
                "Bills already belong to an existing transfer pair".to_string(),
            ));
        }
        let left = bills
            .iter()
            .find(|bill| map_i64(bill, "id") == left_bill_id)
            .expect("left bill");
        let right = bills
            .iter()
            .find(|bill| map_i64(bill, "id") == right_bill_id)
            .expect("right bill");
        if !pair_is_eligible(tx, user_id, left, right, pair_type)? {
            return Err(DbError::InvalidOperation(format!(
                "Bills are not eligible for {pair_type} pairing"
            )));
        }
        tx.execute(
            &format!(
                "INSERT OR IGNORE INTO {table}(user_id, left_bill_id, right_bill_id, created_at) VALUES (?, ?, ?, ?)"
            ),
            params![user_id, left_bill_id, right_bill_id, now],
        )?;
        record_feedback_on_tx(
            tx,
            user_id,
            feedback_candidate_id,
            "reject",
            &build_bill_pair_feedback_payload(pair_type, bill_id, candidate_bill_id, None),
            &now,
        )?;
        Ok(())
    })
    .map_err(map_write_error)
}

fn accept_bill_learning_candidate(
    connection: &mut Connection,
    user_id: UserId,
    candidate_id: &str,
    bill_id: i64,
    rule_id: i64,
    expected_revision: Option<&str>,
) -> MatchingResult<Value> {
    let user_id_value = UserScope::new(user_id).bind_value()?;
    if !formal_learning_candidate_available(connection, user_id, bill_id, candidate_id)? {
        return Err(MatchingRuntimeError::BadRequest(
            "Learning candidate not available".to_string(),
        ));
    }
    let now = utc_now();
    run_transaction(connection, |tx| {
        let Some(bill) = get_bill_map_on_tx(tx, user_id_value, bill_id)? else {
            return Err(DbError::InvalidOperation("Bill not found".to_string()));
        };
        let Some(rule) = get_learning_rule_on_tx(tx, user_id_value, rule_id)? else {
            return Err(DbError::InvalidOperation(
                "Learning candidate not available".to_string(),
            ));
        };
        validate_learning_revision(&rule, expected_revision)?;
        let mut updates = Map::new();
        let learned_type = map_string(&rule, "learned_type", "");
        if !learned_type.trim().is_empty() && learned_type != map_string(&bill, "type", "") {
            updates.insert("type".to_string(), json!(learned_type));
        }
        if let Some(category_id) = map_optional_i64(&rule, "learned_category_id") {
            if let Some(category) = get_category_on_tx(tx, user_id_value, category_id)? {
                updates.insert(
                    "main_category".to_string(),
                    json!(map_string(&category, "main_category", "")),
                );
                updates.insert(
                    "sub_category".to_string(),
                    json!(map_string(&category, "sub_category", "")),
                );
            }
        }
        if let Some(source_id) = map_optional_i64(&rule, "learned_source_account_id") {
            if account_exists_on_tx(tx, user_id_value, source_id)? {
                updates.insert("source_account_id".to_string(), json!(source_id));
            }
        }
        if let Some(destination_id) = map_optional_i64(&rule, "learned_destination_account_id") {
            if account_exists_on_tx(tx, user_id_value, destination_id)? {
                updates.insert("destination_account_id".to_string(), json!(destination_id));
            }
        }
        if !updates.is_empty() {
            let mut assignments = Vec::new();
            let mut values = Vec::new();
            for (key, value) in &updates {
                assignments.push(format!("{key} = ?"));
                values.push(json_value_to_sql(value));
            }
            assignments.push("updated_at = ?".to_string());
            values.push(SqlValue::Text(now.clone()));
            values.push(SqlValue::Integer(bill_id));
            values.push(SqlValue::Integer(user_id_value));
            tx.execute(
                &format!(
                    "UPDATE bills SET {} WHERE id = ? AND user_id = ?",
                    assignments.join(", ")
                ),
                params_from_iter(values),
            )?;
        }
        tx.execute(
            "DELETE FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id = ? AND rule_id = ?",
            params![user_id_value, bill_id, rule_id],
        )?;
        let revision = build_learning_rule_revision(&rule);
        tx.execute(
            "INSERT OR IGNORE INTO bill_learning_rule_suppressions(user_id, bill_id, rule_id, created_at) VALUES (?, ?, ?, ?)",
            params![user_id_value, bill_id, rule_id, revision],
        )?;
        if table_exists_tx(tx, "import_learning_rule_logs")? {
            tx.execute(
                "INSERT INTO import_learning_rule_logs(user_id, rule_id, action, payload_json, created_at) VALUES (?, ?, 'accepted', ?, ?)",
                params![user_id_value, rule_id, json!({"bill_id": bill_id, "applied_updates": updates}).to_string(), now],
            )?;
        }
        if column_exists_tx(tx, "import_learning_rules", "applied_count")? {
            tx.execute(
                "
                UPDATE import_learning_rules
                SET applied_count = COALESCE(applied_count, 0) + 1,
                    last_applied_at = ?,
                    updated_at = ?
                WHERE id = ? AND user_id = ?
                ",
                params![now, now, rule_id, user_id_value],
            )?;
        }
        let updated_bill = get_bill_map_on_tx(tx, user_id_value, bill_id)?.unwrap_or_default();
        Ok(json!({
            "candidate_id": candidate_id,
            "action": "accept",
            "bill": updated_bill,
        }))
    })
    .map_err(map_write_error)
}

fn reject_bill_learning_candidate(
    connection: &mut Connection,
    user_id: UserId,
    bill_id: i64,
    rule_id: i64,
    expected_revision: Option<&str>,
) -> MatchingResult<()> {
    let user_id_value = UserScope::new(user_id).bind_value()?;
    if get_bill_map(connection, user_id_value, bill_id)?.is_none() {
        return Err(MatchingRuntimeError::NotFound("Bill not found".to_string()));
    }
    let rule = get_learning_rule(connection, user_id_value, rule_id)?.ok_or_else(|| {
        MatchingRuntimeError::BadRequest("Learning candidate not available".to_string())
    })?;
    validate_learning_revision(&rule, expected_revision).map_err(|_| {
        MatchingRuntimeError::BadRequest("Learning candidate not available".to_string())
    })?;
    let revision = build_learning_rule_revision(&rule);
    let now = utc_now();
    connection.execute(
        "
        INSERT INTO bill_learning_rule_suppressions(user_id, bill_id, rule_id, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(user_id, bill_id, rule_id) DO UPDATE SET created_at = excluded.created_at
        ",
        params![user_id_value, bill_id, rule_id, revision],
    )?;
    if table_exists(connection, "import_learning_rule_logs")? {
        connection.execute(
            "INSERT INTO import_learning_rule_logs(user_id, rule_id, action, payload_json, created_at) VALUES (?, ?, 'rejected', ?, ?)",
            params![user_id_value, rule_id, json!({"bill_id": bill_id}).to_string(), now],
        )?;
    }
    Ok(())
}

fn preview_transfer_action(
    connection: &mut Connection,
    user_id: UserId,
    candidate_id: &str,
    preview_id: i64,
    action: &str,
    request: &PreviewMatchingActionRequest,
) -> MatchingResult<Value> {
    let decision = decision_from_action(action)?;
    let reviewed_type = request.reviewed_type.as_deref().unwrap_or("转账");
    let result = apply_preview_transfer_decision(
        connection,
        preview_id,
        user_id,
        decision,
        reviewed_type,
        request.expected_state.as_ref(),
    )?;
    preview_decision_payload(
        candidate_id,
        action,
        result,
        request.response_mode_preview_item,
    )
}

fn preview_learning_action(
    connection: &mut Connection,
    user_id: UserId,
    candidate_id: &str,
    preview_id: i64,
    action: &str,
    request: &PreviewMatchingActionRequest,
) -> MatchingResult<Value> {
    let decision = decision_from_action(action)?;
    let result = apply_preview_learning_decision(
        connection,
        preview_id,
        user_id,
        decision,
        request.learning_apply.as_ref(),
        request.expected_state.as_ref(),
    )?;
    preview_decision_payload(
        candidate_id,
        action,
        result,
        request.response_mode_preview_item,
    )
}

fn preview_recurring_action(
    connection: &mut Connection,
    user_id: UserId,
    candidate_id: &str,
    preview_id: i64,
    action: &str,
    request: &PreviewMatchingActionRequest,
) -> MatchingResult<Value> {
    let update = if action == "accept" {
        let recurring_id = request
            .recurring_id
            .ok_or_else(|| MatchingRuntimeError::BadRequest("Missing recurringId".to_string()))?;
        ImportPreviewRecurringMatchUpdate {
            recurring_id: Some(recurring_id),
            candidate_count: request.recurring_candidate_count,
            target_candidate: request.recurring_candidate.clone(),
        }
    } else {
        ImportPreviewRecurringMatchUpdate {
            recurring_id: None,
            candidate_count: 0,
            target_candidate: None,
        }
    };
    let result = update_preview_recurring_match_decision(
        connection,
        preview_id,
        user_id,
        &update,
        request.expected_state.as_ref(),
    )?;
    let mut payload = preview_decision_payload(
        candidate_id,
        action,
        result,
        request.response_mode_preview_item,
    )?;
    if let Some(object) = payload.as_object_mut() {
        if let Some(recurring_id) = update.recurring_id {
            object.insert("recurring_id".to_string(), json!(recurring_id));
        }
    }
    Ok(payload)
}

fn preview_decision_payload(
    candidate_id: &str,
    action: &str,
    result: crate::ImportPreviewDecisionResult,
    preview_item_only: bool,
) -> MatchingResult<Value> {
    if result.state_conflict {
        return Err(MatchingRuntimeError::Conflict(
            "Preview row changed, please refresh".to_string(),
        ));
    }
    if result.invalid_recurring_id {
        return Err(MatchingRuntimeError::BadRequest(
            "Recurring candidate not available".to_string(),
        ));
    }
    let Some(preview) = result.preview else {
        return Err(MatchingRuntimeError::NotFound(
            "Preview recommendation is no longer available".to_string(),
        ));
    };
    let preview_item = preview_row_to_matching_input(preview.clone());
    let mut payload = json!({
        "candidate_id": candidate_id,
        "action": action,
        "preview_id": preview.id,
        "session_id": preview.session_id,
    });
    if preview_item_only {
        payload["preview_item"] = preview_item;
    } else {
        payload["preview"] = json!([preview_item]);
    }
    Ok(payload)
}

fn reconciliation_action(
    connection: &mut Connection,
    user_id: i64,
    candidate_id: &str,
    action: &str,
) -> MatchingResult<Value> {
    let now = utc_now();
    run_transaction(connection, |tx| {
        let candidate = get_reconciliation_candidate_on_tx(tx, user_id, candidate_id)?;
        let Some(candidate) = candidate else {
            return Err(DbError::InvalidOperation(
                "Reconciliation candidate not found".to_string(),
            ));
        };
        let group_id = map_i64(&candidate, "group_id");
        let group_key = map_string(&candidate, "group_key", "");
        let group_type = map_string(&candidate, "candidate_type", "");
        let was_applied = is_applied_reconciliation_status(&map_string(&candidate, "status", ""));
        let (base_bill, metadata) =
            prepare_reconciliation_base_snapshot_on_tx(tx, user_id, &candidate)?;
        let preview_id = find_reconciliation_preview_id_on_tx(tx, user_id, &candidate)?;
        let preview_selection_before =
            reconciliation_preview_selected_on_tx(tx, user_id, preview_id)?;
        let status = match action {
            "accept" => "merged",
            "reject" => "rejected",
            "clear" => PENDING_STATUS,
            _ => return Err(DbError::InvalidOperation("Invalid action".to_string())),
        };
        set_reconciliation_candidate_status_on_tx(tx, user_id, &candidate, status, &now, None)?;
        let projection = recompute_reconciliation_projection_on_tx(
            tx,
            user_id,
            ReconciliationProjectionInput {
                group_id,
                group_key: &group_key,
                group_type: &group_type,
                base_bill: &base_bill,
                metadata,
                now: &now,
            },
        )?;
        if action == "accept" {
            set_reconciliation_preview_selected_on_tx(tx, user_id, preview_id, false)?;
        } else if preview_id.is_some()
            && (action == "clear" || (action == "reject" && was_applied))
            && !same_preview_still_applied_on_tx(tx, user_id, &group_key, preview_id)?
        {
            set_reconciliation_preview_selected_on_tx(tx, user_id, preview_id, true)?;
        }
        let event_type = match action {
            "accept" => "merge_applied",
            "reject" => "candidate_rejected",
            _ => "merge_rolled_back",
        };
        let event_payload = match action {
            "accept" => json!({
                "candidate_type": map_string(&candidate, "candidate_type", ""),
                "base_bill": base_bill,
                "projection": projection,
                "preview_id": preview_id,
                "preview_selected_before": preview_selection_before,
            }),
            "clear" => json!({
                "candidate_type": map_string(&candidate, "candidate_type", ""),
                "projection": projection,
                "preview_id": preview_id,
            }),
            _ => json!({
                "candidate_type": map_string(&candidate, "candidate_type", ""),
                "projection": projection,
            }),
        };
        let event_id = append_merge_event_on_tx(
            tx,
            user_id,
            group_id,
            candidate_id,
            event_type,
            &event_payload,
            &now,
        )?;
        set_reconciliation_candidate_status_on_tx(
            tx,
            user_id,
            &candidate,
            status,
            &now,
            Some(event_id),
        )?;
        let mut result = json!({
            "candidate_id": candidate_id,
            "action": action,
            "group_id": group_id,
            "projection": projection,
        });
        if action == "accept" {
            let mut projected_bill = base_bill;
            if let Some(projection) = result
                .get("projection")
                .and_then(Value::as_object)
                .filter(|projection| !projection.is_empty())
            {
                projected_bill.insert(
                    "description".to_string(),
                    projection
                        .get("description")
                        .cloned()
                        .unwrap_or_else(|| json!("")),
                );
                projected_bill.insert(
                    "tag_ids".to_string(),
                    projection
                        .get("tag_ids")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                );
            }
            result["bill"] = Value::Object(projected_bill);
        }
        Ok(result)
    })
    .map_err(map_reconciliation_error)
}

fn list_transfer_candidates_for_bill(
    connection: &Connection,
    user_id: i64,
    anchor_bill: &Map<String, Value>,
) -> DbResult<Vec<Value>> {
    let bill_id = map_i64(anchor_bill, "id");
    let suppressed = suppressed_pair_candidate_ids(
        connection,
        user_id,
        bill_id,
        "bill_transfer_pair_suppressions",
    )?;
    let amount = map_f64(anchor_bill, "amount");
    let source_account_id = map_i64(anchor_bill, "source_account_id");
    if amount.abs() <= 0.0 || source_account_id <= 0 {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "
        SELECT * FROM bills b
        WHERE b.user_id = ?
          AND b.id != ?
          AND COALESCE(b.source_account_id, 0) > 0
          AND COALESCE(b.source_account_id, 0) != ?
          AND ABS(ABS(COALESCE(b.amount, 0)) - ?) <= ?
          AND COALESCE(b.amount, 0) * ? < 0
          AND COALESCE(b.type, '') NOT IN ('转账', 'transfer')
          AND NOT EXISTS (
              SELECT 1 FROM bill_pair_links links
              WHERE links.user_id = ? AND (links.left_bill_id = b.id OR links.right_bill_id = b.id)
          )
        ORDER BY b.id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![
            user_id,
            bill_id,
            source_account_id,
            amount.abs(),
            TRANSFER_AMOUNT_TOLERANCE,
            amount,
            user_id
        ],
        bill_from_row,
    )?;
    let bills = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|bill| !suppressed.contains(&map_i64(bill, "id")))
        .map(Value::Object)
        .collect::<Vec<_>>();
    Ok(build_transfer_pair_candidates(anchor_bill, &bills))
}

fn list_investment_candidates_for_bill(
    connection: &Connection,
    user_id: i64,
    anchor_bill: &Map<String, Value>,
) -> DbResult<Vec<Value>> {
    let bill_id = map_i64(anchor_bill, "id");
    let keyword_config = user_investment_keyword_config(connection, user_id)?;
    if score_investment_candidate(anchor_bill, true, Some(&keyword_config)).is_none() {
        return Ok(Vec::new());
    }
    let transfer_suppressed = suppressed_pair_candidate_ids(
        connection,
        user_id,
        bill_id,
        "bill_transfer_pair_suppressions",
    )?;
    let investment_suppressed = suppressed_pair_candidate_ids(
        connection,
        user_id,
        bill_id,
        "bill_investment_pair_suppressions",
    )?;
    let amount = map_f64(anchor_bill, "amount");
    let source_account_id = map_i64(anchor_bill, "source_account_id");
    let mut statement = connection.prepare(
        "
        SELECT * FROM bills b
        WHERE b.user_id = ?
          AND b.id != ?
          AND COALESCE(b.source_account_id, 0) > 0
          AND COALESCE(b.source_account_id, 0) != ?
          AND ABS(ABS(COALESCE(b.amount, 0)) - ?) <= ?
          AND COALESCE(b.amount, 0) * ? < 0
          AND COALESCE(b.type, '') NOT IN ('转账', 'transfer')
          AND NOT EXISTS (
              SELECT 1 FROM bill_pair_links links
              WHERE links.user_id = ? AND (links.left_bill_id = b.id OR links.right_bill_id = b.id)
          )
        ORDER BY b.id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![
            user_id,
            bill_id,
            source_account_id,
            amount.abs(),
            TRANSFER_AMOUNT_TOLERANCE,
            amount,
            user_id
        ],
        bill_from_row,
    )?;
    let bills = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|bill| {
            let candidate_id = map_i64(bill, "id");
            !transfer_suppressed.contains(&candidate_id)
                && !investment_suppressed.contains(&candidate_id)
                && score_investment_candidate(bill, true, Some(&keyword_config)).is_some()
        })
        .map(Value::Object)
        .collect::<Vec<_>>();
    Ok(build_investment_pair_candidates(
        anchor_bill,
        &bills,
        Some(&keyword_config),
    ))
}

fn list_learning_candidates_for_bill(
    connection: &Connection,
    user_id: i64,
    anchor_bill: &Map<String, Value>,
) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "import_learning_rules")? {
        return Ok(Vec::new());
    }
    if !import_learning_rules_candidate_columns_available(connection)? {
        return Ok(Vec::new());
    }
    if !user_import_learning_enabled(connection, user_id)? {
        return Ok(Vec::new());
    }
    let bill_id = map_i64(anchor_bill, "id");
    let suppression_revisions = learning_suppression_revision_map(connection, user_id, bill_id)?;
    let rules = load_learning_rules(connection, user_id)?;
    let categories = load_categories(connection, user_id)?;
    let accounts = load_accounts(connection, user_id)?;
    Ok(build_learning_candidates_for_bill(
        anchor_bill,
        &rules,
        &suppression_revisions,
        &categories,
        &accounts,
    ))
}

fn import_learning_rules_candidate_columns_available(connection: &Connection) -> DbResult<bool> {
    for column in ["id", "user_id", "enabled"] {
        if !column_exists(connection, "import_learning_rules", column)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn list_reconciliation_candidates_for_bill(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Vec<Value>> {
    let filters = ReconciliationCandidateFilters {
        existing_bill_id: Some(bill_id),
        status: Some(PENDING_STATUS.to_string()),
        limit: 50,
        ..Default::default()
    };
    Ok(list_reconciliation_candidates(connection, user_id, &filters)?
        .into_iter()
        .map(|candidate| {
            let import_snapshot = candidate
                .get("import_bill_snapshot")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let signal_label = map_string(&candidate, "signal_label", "");
            json!({
                "candidate_id": map_string(&candidate, "candidate_id", ""),
                "kind": format!("reconciliation_{}", map_string(&candidate, "candidate_type", "")),
                "bill_id": Value::Null,
                "score": map_f64(&candidate, "score"),
                "level": map_string(&candidate, "level", ""),
                "reason": map_string(&candidate, "reason", ""),
                "bill": serialize_bill_snapshot(&import_snapshot),
                "summary": signal_label,
                "suppressed": false,
                "status": map_string(&candidate, "status", ""),
                "reconciliation": {
                    "group_id": map_optional_i64(&candidate, "group_id"),
                    "candidate_type": map_string(&candidate, "candidate_type", ""),
                    "signal_label": signal_label,
                    "source_chain": candidate.get("source_chain").cloned().unwrap_or_else(|| json!([])),
                },
            })
        })
        .collect())
}

fn list_reconciliation_candidates(
    connection: &Connection,
    user_id: i64,
    filters: &ReconciliationCandidateFilters,
) -> DbResult<Vec<Map<String, Value>>> {
    let mut conditions = vec!["c.user_id = ?".to_string(), "c.family = ?".to_string()];
    let mut values = vec![
        SqlValue::Integer(user_id),
        SqlValue::Text(IMPORT_RECONCILIATION_FAMILY.to_string()),
    ];
    if let Some(session_id) = filters
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        conditions.push("c.session_id = ?".to_string());
        values.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(preview_id) = filters.preview_id {
        conditions.push("c.preview_id = ?".to_string());
        values.push(SqlValue::Integer(preview_id));
    }
    if let Some(existing_bill_id) = filters.existing_bill_id {
        conditions.push("c.existing_bill_id = ?".to_string());
        values.push(SqlValue::Integer(existing_bill_id));
    }
    if let Some(candidate_type) = filters
        .candidate_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        conditions.push("c.candidate_type = ?".to_string());
        values.push(SqlValue::Text(candidate_type.to_ascii_lowercase()));
    }
    if let Some(status) = filters
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        conditions.push("c.status = ?".to_string());
        values.push(SqlValue::Text(status.to_ascii_lowercase()));
    }
    values.push(SqlValue::Integer(filters.limit.clamp(1, 500)));
    let sql = format!(
        "
        SELECT c.*, g.id AS group_id, g.status AS group_status,
               g.canonical_bill_id AS canonical_bill_id, g.metadata_json AS group_metadata_json
        FROM bill_reconciliation_candidates c
        LEFT JOIN bill_merge_groups g
          ON g.user_id = c.user_id AND g.family = c.family AND g.group_key = c.group_key
        WHERE {}
        ORDER BY COALESCE(c.last_seen_at, c.created_at) DESC, c.id DESC
        LIMIT ?
        ",
        conditions.join(" AND ")
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        reconciliation_candidate_from_row(row)
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn get_bill_reconciliation_projection(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Value> {
    let mut statement = connection.prepare(
        "
        SELECT g.*
        FROM bill_merge_groups g
        JOIN bill_merge_members m ON m.group_id = g.id
        WHERE g.user_id = ?
          AND g.family = ?
          AND m.bill_id = ?
          AND m.member_type = 'existing_bill'
        ORDER BY g.updated_at DESC, g.id DESC
        ",
    )?;
    let rows = statement.query_map(
        params![user_id, IMPORT_RECONCILIATION_FAMILY, bill_id],
        |row| row_to_map(row, "g"),
    )?;
    for row in rows {
        let group = row?;
        let metadata = parse_json_object(&map_string(&group, "metadata_json", "{}"));
        let projection = metadata
            .get("projection")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let candidate_ids = projection
            .get("candidate_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if projection.is_empty() || candidate_ids.is_empty() {
            continue;
        }
        return Ok(json!({
            "group_id": map_i64(&group, "id"),
            "group_type": map_string(&group, "group_type", ""),
            "status": map_string(&group, "status", ""),
            "canonical_bill_id": map_optional_i64(&group, "canonical_bill_id"),
            "signal_label": value_string(projection.get("signal_label")),
            "source_chain": projection.get("source_chain").cloned().unwrap_or_else(|| json!([])),
            "candidate_ids": candidate_ids,
            "description": value_string(projection.get("description")),
            "tag_ids": projection.get("tag_ids").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    Ok(Value::Null)
}

struct ReconciliationProjectionInput<'a> {
    group_id: i64,
    group_key: &'a str,
    group_type: &'a str,
    base_bill: &'a Map<String, Value>,
    metadata: Map<String, Value>,
    now: &'a str,
}

fn recompute_reconciliation_projection_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    input: ReconciliationProjectionInput<'_>,
) -> DbResult<Value> {
    let ReconciliationProjectionInput {
        group_id,
        group_key,
        group_type,
        base_bill,
        mut metadata,
        now,
    } = input;
    let bill_id = map_i64(base_bill, "id");
    if bill_id <= 0 {
        return Err(DbError::InvalidOperation("Bill not found".to_string()));
    }
    assert_reconciliation_projection_current_on_tx(tx, user_id, bill_id, &metadata)?;

    let group_candidates = load_group_candidates_on_tx(tx, user_id, group_key)?;
    let applied_candidates = group_candidates
        .into_iter()
        .filter(|candidate| is_applied_reconciliation_status(&map_string(candidate, "status", "")))
        .collect::<Vec<_>>();
    let previous_projection = metadata
        .get("projection")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let had_applied_projection = !previous_projection.is_empty()
        && !projection_candidate_ids(&previous_projection).is_empty();
    if applied_candidates.is_empty() && !had_applied_projection {
        metadata.remove("base_bill_snapshot");
        metadata.remove("projection");
        tx.execute(
            "
            UPDATE bill_merge_groups
            SET status = ?, canonical_bill_id = NULL, metadata_json = ?, updated_at = ?
            WHERE id = ? AND user_id = ?
            ",
            params![
                PENDING_STATUS,
                Value::Object(metadata).to_string(),
                now,
                group_id,
                user_id
            ],
        )?;
        return Ok(json!({}));
    }

    let import_snapshots = applied_candidates
        .iter()
        .map(|candidate| {
            candidate
                .get("import_bill_snapshot")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let mut descriptions = vec![map_string(base_bill, "description", "")];
    descriptions.extend(
        import_snapshots
            .iter()
            .map(|snapshot| map_string(snapshot, "description", "")),
    );
    let merged_description = merge_description_values(descriptions);
    let mut merged_tag_ids = snapshot_tag_ids(base_bill);
    for snapshot in &import_snapshots {
        for tag_id in snapshot_tag_ids(snapshot) {
            if !merged_tag_ids.contains(&tag_id) {
                merged_tag_ids.push(tag_id);
            }
        }
    }
    tx.execute(
        "UPDATE bills SET description = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        params![merged_description, now, bill_id, user_id],
    )?;
    let merged_tag_ids =
        replace_bill_projection_tags_on_tx(tx, user_id, bill_id, &merged_tag_ids, now)?;
    let mut projection =
        build_reconciliation_projection_signal(group_type, base_bill, &import_snapshots);
    projection.insert("bill_id".to_string(), json!(bill_id));
    projection.insert("description".to_string(), json!(merged_description));
    projection.insert("tag_ids".to_string(), json!(merged_tag_ids));
    projection.insert(
        "candidate_ids".to_string(),
        Value::Array(
            applied_candidates
                .iter()
                .filter_map(|candidate| {
                    let candidate_id = map_string(candidate, "candidate_id", "");
                    (!candidate_id.is_empty()).then(|| json!(candidate_id))
                })
                .collect(),
        ),
    );
    if applied_candidates.is_empty() {
        metadata.remove("base_bill_snapshot");
        metadata.remove("projection");
    } else {
        metadata.insert(
            "base_bill_snapshot".to_string(),
            Value::Object(base_bill.clone()),
        );
        metadata.insert("projection".to_string(), Value::Object(projection.clone()));
    }
    let next_status = if applied_candidates.is_empty() {
        PENDING_STATUS
    } else {
        "merged"
    };
    tx.execute(
        "
        UPDATE bill_merge_groups
        SET status = ?, canonical_bill_id = ?, metadata_json = ?, updated_at = ?
        WHERE id = ? AND user_id = ?
        ",
        params![
            next_status,
            if applied_candidates.is_empty() {
                SqlValue::Null
            } else {
                SqlValue::Integer(bill_id)
            },
            Value::Object(metadata).to_string(),
            now,
            group_id,
            user_id
        ],
    )?;
    Ok(Value::Object(projection))
}

fn set_reconciliation_candidate_status_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    candidate: &Map<String, Value>,
    status: &str,
    now: &str,
    event_id: Option<i64>,
) -> DbResult<()> {
    let resolved_at = if status == PENDING_STATUS {
        None
    } else {
        Some(now)
    };
    tx.execute(
        "
        UPDATE bill_reconciliation_candidates
        SET status = ?, resolved_at = ?, resolution_event_id = ?, updated_at = ?
        WHERE id = ? AND user_id = ?
        ",
        params![
            status,
            resolved_at,
            event_id,
            now,
            map_i64(candidate, "id"),
            user_id
        ],
    )?;
    Ok(())
}

fn prepare_reconciliation_base_snapshot_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    candidate: &Map<String, Value>,
) -> DbResult<(Map<String, Value>, Map<String, Value>)> {
    let mut metadata = candidate
        .get("group_metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let base_bill = metadata
        .get("base_bill_snapshot")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if !base_bill.is_empty() {
        return Ok((base_bill, metadata));
    }
    let base_bill =
        get_bill_projection_snapshot_on_tx(tx, user_id, map_i64(candidate, "existing_bill_id"))?;
    metadata.insert(
        "base_bill_snapshot".to_string(),
        Value::Object(base_bill.clone()),
    );
    Ok((base_bill, metadata))
}

fn get_bill_projection_snapshot_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Map<String, Value>> {
    let Some(mut bill) = get_bill_map_on_tx(tx, user_id, bill_id)? else {
        return Err(DbError::InvalidOperation("Bill not found".to_string()));
    };
    bill.insert(
        "tag_ids".to_string(),
        Value::Array(
            load_bill_tag_ids_on_tx(tx, user_id, bill_id)?
                .into_iter()
                .map(|tag_id| json!(tag_id))
                .collect(),
        ),
    );
    Ok(bill)
}

fn load_bill_tag_ids_on_tx(tx: &Transaction<'_>, user_id: i64, bill_id: i64) -> DbResult<Vec<i64>> {
    if !table_exists_tx(tx, "bill_tags")? || !table_exists_tx(tx, "tags")? {
        return Ok(Vec::new());
    }
    let mut statement = tx.prepare(
        "
        SELECT bt.tag_id
        FROM bill_tags bt
        JOIN tags t ON t.id = bt.tag_id
        WHERE bt.bill_id = ? AND t.user_id = ?
        ORDER BY bt.tag_id
        ",
    )?;
    let rows = statement.query_map(params![bill_id, user_id], |row| row.get::<_, i64>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn replace_bill_projection_tags_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    tag_ids: &[i64],
    now: &str,
) -> DbResult<Vec<i64>> {
    if !table_exists_tx(tx, "bill_tags")? {
        return Ok(Vec::new());
    }
    let filtered_tag_ids = filter_existing_tag_ids_on_tx(tx, user_id, tag_ids)?;
    tx.execute("DELETE FROM bill_tags WHERE bill_id = ?", params![bill_id])?;
    for tag_id in &filtered_tag_ids {
        tx.execute(
            "INSERT OR IGNORE INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
            params![bill_id, tag_id, now],
        )?;
    }
    Ok(filtered_tag_ids)
}

fn filter_existing_tag_ids_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    tag_ids: &[i64],
) -> DbResult<Vec<i64>> {
    let normalized_tag_ids = normalize_tag_ids_from_iter(tag_ids.iter().copied());
    if normalized_tag_ids.is_empty() || !table_exists_tx(tx, "tags")? {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", normalized_tag_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut values = vec![SqlValue::Integer(user_id)];
    values.extend(
        normalized_tag_ids
            .iter()
            .map(|tag_id| SqlValue::Integer(*tag_id)),
    );
    let mut statement = tx.prepare(&format!(
        "SELECT id FROM tags WHERE user_id = ? AND id IN ({placeholders})"
    ))?;
    let rows = statement.query_map(params_from_iter(values), |row| row.get::<_, i64>(0))?;
    let existing_tag_ids = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<BTreeSet<_>>();
    Ok(normalized_tag_ids
        .into_iter()
        .filter(|tag_id| existing_tag_ids.contains(tag_id))
        .collect())
}

fn assert_reconciliation_projection_current_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    metadata: &Map<String, Value>,
) -> DbResult<()> {
    let projection = metadata
        .get("projection")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if projection.is_empty() || projection_candidate_ids(&projection).is_empty() {
        return Ok(());
    }
    let current_bill = get_bill_projection_snapshot_on_tx(tx, user_id, bill_id)?;
    let current_description = map_string(&current_bill, "description", "");
    let projected_description = value_string(projection.get("description"));
    let current_tag_ids = sorted_tag_ids(snapshot_tag_ids(&current_bill));
    let projected_tag_ids = sorted_tag_ids(normalize_tag_ids(projection.get("tag_ids")));
    if current_description != projected_description || current_tag_ids != projected_tag_ids {
        return Err(DbError::InvalidOperation(
            RECONCILIATION_STALE_PROJECTION_MESSAGE.to_string(),
        ));
    }
    Ok(())
}

fn find_reconciliation_preview_id_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    candidate: &Map<String, Value>,
) -> DbResult<Option<i64>> {
    if let Some(preview_id) = map_optional_i64(candidate, "preview_id") {
        return Ok(Some(preview_id));
    }
    if !table_exists_tx(tx, "bills_preview")? {
        return Ok(None);
    }
    let session_id = map_string(candidate, "session_id", "");
    let import_bill_key = map_string(candidate, "import_bill_key", "");
    let template_prefix = format!("session:{session_id}:template:");
    let Some(template_id) = import_bill_key
        .strip_prefix(&template_prefix)
        .and_then(|raw| raw.trim().parse::<i64>().ok())
        .filter(|template_id| *template_id > 0)
    else {
        return Ok(None);
    };
    let mut statement = tx.prepare(
        "
        SELECT id, dedup_source_ids
        FROM bills_preview
        WHERE session_id = ? AND user_id = ?
        ORDER BY id
        ",
    )?;
    let rows = statement.query_map(params![session_id, user_id], |row| {
        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, Option<String>>("dedup_source_ids")?,
        ))
    })?;
    for row in rows {
        let (preview_id, source_ids) = row?;
        let source_ids_value = source_ids.map(Value::String).unwrap_or(Value::Null);
        if normalize_tag_ids(Some(&source_ids_value)).contains(&template_id) {
            return Ok(Some(preview_id));
        }
    }
    Ok(None)
}

fn reconciliation_preview_selected_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    preview_id: Option<i64>,
) -> DbResult<Option<bool>> {
    let Some(preview_id) = preview_id else {
        return Ok(None);
    };
    if !table_exists_tx(tx, "bills_preview")? {
        return Ok(None);
    }
    tx.query_row(
        "SELECT preview_selected FROM bills_preview WHERE id = ? AND user_id = ?",
        params![preview_id, user_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map(|value| value.map(|value| value != 0))
    .map_err(DbError::from)
}

fn set_reconciliation_preview_selected_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    preview_id: Option<i64>,
    selected: bool,
) -> DbResult<()> {
    let Some(preview_id) = preview_id else {
        return Ok(());
    };
    if !table_exists_tx(tx, "bills_preview")? {
        return Ok(());
    }
    tx.execute(
        "UPDATE bills_preview SET preview_selected = ? WHERE id = ? AND user_id = ?",
        params![i64::from(selected), preview_id, user_id],
    )?;
    Ok(())
}

fn same_preview_still_applied_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    group_key: &str,
    preview_id: Option<i64>,
) -> DbResult<bool> {
    let Some(preview_id) = preview_id else {
        return Ok(false);
    };
    for candidate in load_group_candidates_on_tx(tx, user_id, group_key)? {
        if !is_applied_reconciliation_status(&map_string(&candidate, "status", "")) {
            continue;
        }
        if find_reconciliation_preview_id_on_tx(tx, user_id, &candidate)? == Some(preview_id) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn load_group_candidates_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    group_key: &str,
) -> DbResult<Vec<Map<String, Value>>> {
    let mut statement = tx.prepare(
        "
        SELECT c.*, g.id AS group_id, g.status AS group_status,
               g.canonical_bill_id AS canonical_bill_id, g.metadata_json AS group_metadata_json
        FROM bill_reconciliation_candidates c
        JOIN bill_merge_groups g
          ON g.user_id = c.user_id AND g.family = c.family AND g.group_key = c.group_key
        WHERE c.user_id = ? AND c.group_key = ?
        ORDER BY c.id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![user_id, group_key],
        reconciliation_candidate_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn is_applied_reconciliation_status(status: &str) -> bool {
    matches!(status, "accepted" | "merged")
}

fn projection_candidate_ids(projection: &Map<String, Value>) -> Vec<String> {
    projection
        .get("candidate_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|candidate_id| value_string(Some(candidate_id)))
        .filter(|candidate_id| !candidate_id.is_empty())
        .collect()
}

fn snapshot_tag_ids(snapshot: &Map<String, Value>) -> Vec<i64> {
    let tag_ids = normalize_tag_ids(snapshot.get("tag_ids"));
    if tag_ids.is_empty() {
        normalize_tag_ids(snapshot.get("tags"))
    } else {
        tag_ids
    }
}

fn normalize_tag_ids(raw_value: Option<&Value>) -> Vec<i64> {
    let Some(raw_value) = raw_value else {
        return Vec::new();
    };
    match raw_value {
        Value::Array(values) => normalize_tag_ids_from_iter(
            values
                .iter()
                .flat_map(|value| normalize_tag_ids(Some(value)).into_iter()),
        ),
        Value::Object(object) => normalize_tag_ids(object.get("id")),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                Vec::new()
            } else if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                normalize_tag_ids(Some(&parsed))
            } else {
                normalize_tag_ids_from_iter(
                    text.split(',')
                        .filter_map(|part| part.trim().parse::<i64>().ok()),
                )
            }
        }
        Value::Number(_) | Value::Bool(_) => {
            normalize_tag_ids_from_iter(value_i64(Some(raw_value)))
        }
        Value::Null => Vec::new(),
    }
}

fn normalize_tag_ids_from_iter(values: impl IntoIterator<Item = i64>) -> Vec<i64> {
    let mut tag_ids = Vec::new();
    for value in values {
        if value > 0 && !tag_ids.contains(&value) {
            tag_ids.push(value);
        }
    }
    tag_ids
}

fn sorted_tag_ids(mut tag_ids: Vec<i64>) -> Vec<i64> {
    tag_ids.sort_unstable();
    tag_ids
}

fn build_reconciliation_projection_signal(
    candidate_type: &str,
    base_bill: &Map<String, Value>,
    import_snapshots: &[Map<String, Value>],
) -> Map<String, Value> {
    let base_role = infer_bill_flow_role(base_bill);
    let import_sources = import_snapshots
        .iter()
        .map(|snapshot| {
            let mut source = Map::new();
            source.insert("role".to_string(), json!(infer_bill_flow_role(snapshot)));
            source.insert(
                "label".to_string(),
                json!(source_label_from_import_snapshot(snapshot)),
            );
            source.insert(
                "parser_id".to_string(),
                json!(map_string(snapshot, "parser_id", "")),
            );
            source.insert("source".to_string(), json!("parser"));
            source
        })
        .collect::<Vec<_>>();
    let signal_label = if candidate_type == TRANSFER_PAIR_TYPE {
        let mut sources_by_role = BTreeMap::<String, Vec<String>>::new();
        sources_by_role.insert("outgoing".to_string(), Vec::new());
        sources_by_role.insert("incoming".to_string(), Vec::new());
        for source in &import_sources {
            let role = map_string(source, "role", "primary");
            let label = map_string(source, "label", "");
            if let Some(labels) = sources_by_role.get_mut(&role) {
                if !label.is_empty() {
                    labels.push(label);
                }
            }
        }
        sources_by_role
            .entry(base_role.clone())
            .or_default()
            .push(manual_source_label().to_string());
        let side_labels = ["outgoing", "incoming"]
            .iter()
            .filter_map(|role| {
                let labels =
                    dedupe_text_items(sources_by_role.get(*role).cloned().unwrap_or_else(Vec::new));
                (!labels.is_empty()).then(|| labels.join("&"))
            })
            .collect::<Vec<_>>();
        if side_labels.is_empty() {
            manual_source_label().to_string()
        } else {
            format!("{}{}", match_prefix_label(), side_labels.join("|"))
        }
    } else {
        let mut labels = vec![manual_source_label().to_string()];
        labels.extend(
            import_sources
                .iter()
                .map(|source| map_string(source, "label", "")),
        );
        dedupe_text_items(labels).join("|")
    };
    let mut source_chain = Vec::new();
    source_chain.push(json!({
        "role": base_role,
        "label": manual_source_label(),
        "source": "manual",
    }));
    source_chain.extend(import_sources.into_iter().map(Value::Object));
    let mut signal = Map::new();
    signal.insert("signal_label".to_string(), json!(signal_label));
    signal.insert("source_chain".to_string(), Value::Array(source_chain));
    signal
}

fn source_label_from_import_snapshot(snapshot: &Map<String, Value>) -> String {
    for field_name in ["parser_id", "source", "payment_method"] {
        let raw_value = map_string(snapshot, field_name, "");
        if raw_value.is_empty() || raw_value == "0" {
            continue;
        }
        let label = parser_display_label(&raw_value);
        if !label.is_empty() {
            return label;
        }
    }
    import_source_label().to_string()
}

fn parser_display_label(parser_id: &str) -> String {
    match parser_id.trim().to_ascii_lowercase().as_str() {
        "wechat" => "\u{5fae}\u{4fe1}".to_string(),
        "alipay" => "\u{652f}\u{4ed8}\u{5b9d}".to_string(),
        "abc" => "\u{519c}\u{4e1a}\u{94f6}\u{884c}".to_string(),
        "ccb" => "\u{5efa}\u{8bbe}\u{94f6}\u{884c}".to_string(),
        "cmbc" => "\u{6c11}\u{751f}\u{94f6}\u{884c}".to_string(),
        "icbc" => "\u{5de5}\u{5546}\u{94f6}\u{884c}".to_string(),
        "generic" => "\u{901a}\u{7528}\u{6765}\u{6e90}".to_string(),
        value => value.to_string(),
    }
}

fn infer_bill_flow_role(snapshot: &Map<String, Value>) -> String {
    let bill_type = map_string(snapshot, "type", "").to_ascii_lowercase();
    let amount = map_f64(snapshot, "amount");
    if bill_type == "expense" || bill_type == "\u{652f}\u{51fa}" || amount < 0.0 {
        return "outgoing".to_string();
    }
    if bill_type == "income" || bill_type == "\u{6536}\u{5165}" || amount > 0.0 {
        return "incoming".to_string();
    }
    "primary".to_string()
}

fn dedupe_text_items(items: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for item in items {
        let item = item.trim();
        if !item.is_empty() && !deduped.iter().any(|existing: &String| existing == item) {
            deduped.push(item.to_string());
        }
    }
    deduped
}

fn manual_source_label() -> &'static str {
    "\u{4eba}\u{5de5}"
}

fn import_source_label() -> &'static str {
    "\u{5bfc}\u{5165}"
}

fn match_prefix_label() -> &'static str {
    "\u{5339}\u{914d}\u{ff1a}"
}

fn append_merge_event_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    group_id: i64,
    candidate_id: &str,
    event_type: &str,
    payload: &Value,
    now: &str,
) -> DbResult<i64> {
    tx.execute(
        "
        INSERT INTO bill_merge_events(user_id, group_id, candidate_id, event_type, payload_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ",
        params![user_id, group_id, candidate_id, event_type, payload.to_string(), now],
    )?;
    Ok(tx.last_insert_rowid())
}

fn get_reconciliation_candidate_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    candidate_id: &str,
) -> DbResult<Option<Map<String, Value>>> {
    tx.query_row(
        "
        SELECT c.*, g.id AS group_id, g.status AS group_status,
               g.canonical_bill_id AS canonical_bill_id, g.metadata_json AS group_metadata_json
        FROM bill_reconciliation_candidates c
        JOIN bill_merge_groups g
          ON g.user_id = c.user_id AND g.family = c.family AND g.group_key = c.group_key
        WHERE c.user_id = ? AND c.candidate_id = ?
        LIMIT 1
        ",
        params![user_id, candidate_id],
        reconciliation_candidate_from_row,
    )
    .optional()
    .map_err(DbError::from)
}

fn preview_row_to_matching_input(row: ImportPreviewRow) -> Value {
    let mut matching = Map::new();
    let feedback = row.preview_matching_feedback.clone();
    if let Some(transfer) = feedback.get("transfer").filter(|value| value.is_object()) {
        matching.insert("transfer".to_string(), transfer.clone());
    }
    if let Some(learning) = feedback.get("learning").filter(|value| value.is_object()) {
        matching.insert("learning".to_string(), learning.clone());
    }
    if row.preview_recurring_id.is_some()
        || row.preview_recurring_candidate_count > 0
        || !row.preview_recurring_name.trim().is_empty()
    {
        matching.insert(
            "recurring".to_string(),
            json!({
                "id": row.preview_recurring_id,
                "name": row.preview_recurring_name,
                "candidate_count": row.preview_recurring_candidate_count,
                "match_score": row.preview_recurring_match_score,
                "match_reasons": row.preview_recurring_match_reasons,
                "matched_date": row.preview_recurring_matched_date,
            }),
        );
    }
    matching.insert(
        "dedup".to_string(),
        json!({
            "type": row.dedup_type,
            "source_ids": row.dedup_source_ids,
        }),
    );
    matching.insert(
        "parser".to_string(),
        json!({
            "id": row.preview_parser_id,
            "tags": row.preview_parser_tags,
        }),
    );
    json!({
        "id": row.id,
        "session_id": row.session_id,
        "preview_date": row.preview_date,
        "preview_type": row.preview_type,
        "preview_amount": row.preview_amount,
        "preview_destination_amount": row.preview_destination_amount,
        "preview_main_category": row.preview_main_category,
        "preview_sub_category": row.preview_sub_category,
        "preview_source_account_id": row.preview_source_account_id,
        "preview_destination_account_id": row.preview_destination_account_id,
        "preview_counterparty": row.preview_counterparty,
        "preview_payment_method": row.preview_payment_method,
        "preview_description": row.preview_description,
        "preview_selected": row.preview_selected,
        "preview_matching_feedback": feedback,
        "matching": matching,
    })
}

fn serialize_matching_candidate(candidate: Value) -> Value {
    let Some(object) = candidate.as_object() else {
        return candidate;
    };
    let mut serialized = json!({
        "candidateId": value_string(object.get("candidate_id")),
        "kind": value_string(object.get("kind")),
        "score": value_f64(object.get("score")),
        "level": value_string(object.get("level")),
        "reason": value_string(object.get("reason")),
    });
    let target = serialized.as_object_mut().expect("candidate object");
    if let Some(bill_id) = value_i64(object.get("bill_id")) {
        target.insert("billId".to_string(), json!(bill_id));
    }
    if let Some(rule_id) = value_i64(object.get("rule_id")) {
        target.insert("ruleId".to_string(), json!(rule_id));
    }
    for (source, target_name) in [
        ("recommended_type", "recommendedType"),
        ("summary", "summary"),
        ("status", "status"),
    ] {
        let value = value_string(object.get(source));
        if !value.is_empty() {
            target.insert(target_name.to_string(), json!(value));
        }
    }
    if let Some(value) = object.get("suppressed").and_then(Value::as_bool) {
        target.insert("suppressed".to_string(), json!(value));
    }
    if let Some(bill) = object.get("bill").and_then(Value::as_object) {
        target.insert("bill".to_string(), serialize_bill_snapshot(bill));
    }
    if let Some(value) = object
        .get("reconciliation")
        .filter(|value| value.is_object())
    {
        target.insert("reconciliation".to_string(), value.clone());
    }
    serialized
}

fn serialize_reconciliation_candidate(candidate: Map<String, Value>) -> Value {
    let mut serialized = json!({
        "id": map_i64(&candidate, "id"),
        "candidateId": map_string(&candidate, "candidate_id", ""),
        "candidateType": map_string(&candidate, "candidate_type", ""),
        "status": map_string(&candidate, "status", ""),
        "sessionId": map_string(&candidate, "session_id", ""),
        "importBillKey": map_string(&candidate, "import_bill_key", ""),
        "existingBillId": map_i64(&candidate, "existing_bill_id"),
        "groupKey": map_string(&candidate, "group_key", ""),
        "amountAbs": map_f64(&candidate, "amount_abs"),
        "score": map_f64(&candidate, "score"),
        "level": map_string(&candidate, "level", ""),
        "reason": map_string(&candidate, "reason", ""),
        "groupId": map_i64(&candidate, "group_id"),
        "groupStatus": map_string(&candidate, "group_status", ""),
        "canonicalBillId": map_i64(&candidate, "canonical_bill_id"),
        "signalLabel": map_string(&candidate, "signal_label", ""),
        "sourceChain": candidate.get("source_chain").cloned().unwrap_or_else(|| json!([])),
        "seenCount": map_i64(&candidate, "seen_count"),
        "firstSeenAt": map_string(&candidate, "first_seen_at", ""),
        "lastSeenAt": map_string(&candidate, "last_seen_at", ""),
        "importBill": candidate.get("import_bill_snapshot").cloned().unwrap_or_else(|| json!({})),
        "existingBill": candidate.get("existing_bill_snapshot").cloned().unwrap_or_else(|| json!({})),
    });
    if let Some(preview_id) = map_optional_i64(&candidate, "preview_id") {
        serialized["previewId"] = json!(preview_id);
    }
    if let Some(time_diff) = map_optional_i64(&candidate, "time_diff_seconds") {
        serialized["timeDiffSeconds"] = json!(time_diff);
    }
    if let Some(source_payload) = candidate
        .get("source_payload")
        .filter(|value| value.is_object())
    {
        serialized["sourcePayload"] = source_payload.clone();
    }
    serialized
}

fn serialize_bill_pair(pair: &Map<String, Value>) -> Value {
    let mut serialized = json!({
        "id": map_i64(pair, "id"),
        "pairType": map_string(pair, "pair_type", TRANSFER_PAIR_TYPE),
        "source": map_string(pair, "source", MANUAL_PAIR_SOURCE),
        "leftBillId": map_i64(pair, "left_bill_id"),
        "rightBillId": map_i64(pair, "right_bill_id"),
    });
    if let Some(other) = map_optional_i64(pair, "other_bill_id") {
        serialized["otherBillId"] = json!(other);
    }
    serialized
}

fn serialize_bill_snapshot(snapshot: &Map<String, Value>) -> Value {
    json!({
        "id": map_i64(snapshot, "id"),
        "date": map_string(snapshot, "date", ""),
        "type": map_string(snapshot, "type", ""),
        "amount": map_f64(snapshot, "amount"),
        "counterparty": map_string(snapshot, "counterparty", ""),
        "description": map_string(snapshot, "description", ""),
        "paymentMethod": map_string(snapshot, "payment_method", ""),
        "mainCategory": map_string(snapshot, "main_category", ""),
        "subCategory": map_string(snapshot, "sub_category", ""),
        "sourceAccountId": map_i64(snapshot, "source_account_id"),
        "destinationAccountId": map_i64(snapshot, "destination_account_id"),
    })
}

fn get_bill_map(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    connection
        .query_row(
            "SELECT * FROM bills WHERE id = ? AND user_id = ? LIMIT 1",
            params![bill_id, user_id],
            bill_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

fn get_bill_map_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    tx.query_row(
        "SELECT * FROM bills WHERE id = ? AND user_id = ? LIMIT 1",
        params![bill_id, user_id],
        bill_from_row,
    )
    .optional()
    .map_err(DbError::from)
}

fn get_bills_by_ids_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_ids: &[i64],
) -> DbResult<Vec<Map<String, Value>>> {
    let placeholders = bill_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut values = vec![SqlValue::Integer(user_id)];
    values.extend(bill_ids.iter().map(|value| SqlValue::Integer(*value)));
    let mut statement = tx.prepare(&format!(
        "SELECT * FROM bills WHERE user_id = ? AND id IN ({placeholders})"
    ))?;
    let rows = statement.query_map(params_from_iter(values), bill_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn get_bill_pair_link_for_bill(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
    pair_type: Option<&str>,
) -> DbResult<Option<Map<String, Value>>> {
    let mut sql = "SELECT * FROM bill_pair_links WHERE user_id = ? AND (left_bill_id = ? OR right_bill_id = ?)".to_string();
    let mut values = vec![
        SqlValue::Integer(user_id),
        SqlValue::Integer(bill_id),
        SqlValue::Integer(bill_id),
    ];
    if let Some(pair_type) = pair_type {
        sql.push_str(" AND pair_type = ?");
        values.push(SqlValue::Text(pair_type.to_string()));
    }
    sql.push_str(" LIMIT 1");
    connection
        .query_row(&sql, params_from_iter(values), |row| row_to_map(row, ""))
        .optional()
        .map_err(DbError::from)
}

fn get_pair_for_bill_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    pair_type: Option<&str>,
) -> DbResult<Option<Map<String, Value>>> {
    let mut sql = "SELECT * FROM bill_pair_links WHERE user_id = ? AND (left_bill_id = ? OR right_bill_id = ?)".to_string();
    let mut values = vec![
        SqlValue::Integer(user_id),
        SqlValue::Integer(bill_id),
        SqlValue::Integer(bill_id),
    ];
    if let Some(pair_type) = pair_type {
        sql.push_str(" AND pair_type = ?");
        values.push(SqlValue::Text(pair_type.to_string()));
    }
    sql.push_str(" LIMIT 1");
    tx.query_row(&sql, params_from_iter(values), |row| row_to_map(row, ""))
        .optional()
        .map_err(DbError::from)
}

fn get_pair_by_id_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    pair_id: i64,
    pair_type: Option<&str>,
) -> DbResult<Option<Map<String, Value>>> {
    let mut sql = "SELECT * FROM bill_pair_links WHERE id = ? AND user_id = ?".to_string();
    let mut values = vec![SqlValue::Integer(pair_id), SqlValue::Integer(user_id)];
    if let Some(pair_type) = pair_type {
        sql.push_str(" AND pair_type = ?");
        values.push(SqlValue::Text(pair_type.to_string()));
    }
    sql.push_str(" LIMIT 1");
    tx.query_row(&sql, params_from_iter(values), |row| row_to_map(row, ""))
        .optional()
        .map_err(DbError::from)
}

fn with_other_bill_id(mut pair: Map<String, Value>, bill_id: i64) -> Map<String, Value> {
    let left = map_i64(&pair, "left_bill_id");
    let right = map_i64(&pair, "right_bill_id");
    pair.insert(
        "other_bill_id".to_string(),
        json!(if left == bill_id { right } else { left }),
    );
    pair
}

fn suppressed_pair_candidate_ids(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
    table: &str,
) -> DbResult<BTreeSet<i64>> {
    let mut statement = connection.prepare(&format!(
        "
        SELECT CASE WHEN left_bill_id = ? THEN right_bill_id ELSE left_bill_id END AS other_bill_id
        FROM {table}
        WHERE user_id = ? AND (left_bill_id = ? OR right_bill_id = ?)
        "
    ))?;
    let rows = statement.query_map(params![bill_id, user_id, bill_id, bill_id], |row| {
        row.get::<_, i64>("other_bill_id")
    })?;
    rows.collect::<Result<BTreeSet<_>, _>>()
        .map_err(DbError::from)
}

fn transfer_suppression_exists_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    left_bill_id: i64,
    right_bill_id: i64,
) -> DbResult<bool> {
    suppression_exists_on_tx(
        tx,
        "bill_transfer_pair_suppressions",
        user_id,
        left_bill_id,
        right_bill_id,
    )
}

fn investment_suppression_exists_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    left_bill_id: i64,
    right_bill_id: i64,
) -> DbResult<bool> {
    suppression_exists_on_tx(
        tx,
        "bill_investment_pair_suppressions",
        user_id,
        left_bill_id,
        right_bill_id,
    )
}

fn suppression_exists_on_tx(
    tx: &Transaction<'_>,
    table: &str,
    user_id: i64,
    left_bill_id: i64,
    right_bill_id: i64,
) -> DbResult<bool> {
    tx.query_row(
        &format!(
            "SELECT 1 FROM {table} WHERE user_id = ? AND left_bill_id = ? AND right_bill_id = ? LIMIT 1"
        ),
        params![user_id, left_bill_id, right_bill_id],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn pair_is_eligible(
    tx: &Transaction<'_>,
    user_id: i64,
    left: &Map<String, Value>,
    right: &Map<String, Value>,
    pair_type: &str,
) -> DbResult<bool> {
    if pair_type == TRANSFER_PAIR_TYPE {
        return Ok(build_transfer_pair_candidate(left, right).is_some());
    }
    let keyword_config = user_investment_keyword_config_tx(tx, user_id)?;
    Ok(
        score_investment_candidate(left, true, Some(&keyword_config)).is_some()
            && score_investment_candidate(right, true, Some(&keyword_config)).is_some()
            && build_investment_pair_candidates(
                left,
                &[Value::Object(right.clone())],
                Some(&keyword_config),
            )
            .len()
                == 1,
    )
}

fn formal_learning_candidate_available(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
    candidate_id: &str,
) -> MatchingResult<bool> {
    let Some(payload) = query_matching_bill_candidates_payload(connection, user_id, bill_id)?
    else {
        return Ok(false);
    };
    Ok(payload["candidates"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["candidateId"] == candidate_id)))
}

fn get_learning_rule(
    connection: &Connection,
    user_id: i64,
    rule_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    if !table_exists(connection, "import_learning_rules")? {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            params![rule_id, user_id],
            |row| row_to_map(row, ""),
        )
        .optional()
        .map_err(DbError::from)
}

fn get_learning_rule_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    rule_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    if !table_exists_tx(tx, "import_learning_rules")? {
        return Ok(None);
    }
    tx.query_row(
        "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
        params![rule_id, user_id],
        |row| row_to_map(row, ""),
    )
    .optional()
    .map_err(DbError::from)
}

fn validate_learning_revision(
    rule: &Map<String, Value>,
    expected_revision: Option<&str>,
) -> DbResult<()> {
    let Some(expected) = expected_revision
        .map(normalize_learning_rule_revision)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let current = build_learning_rule_revision(rule);
    if expected != current {
        return Err(DbError::InvalidOperation(
            "Learning candidate not available".to_string(),
        ));
    }
    Ok(())
}

fn learning_suppression_revision_map(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<BTreeMap<i64, String>> {
    let mut statement = connection.prepare(
        "SELECT rule_id, created_at FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id = ?",
    )?;
    let rows = statement.query_map(params![user_id, bill_id], |row| {
        Ok((
            row.get::<_, i64>("rule_id")?,
            row.get::<_, String>("created_at")?,
        ))
    })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(DbError::from)
}

fn load_learning_rules(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    let order_by = if column_exists(connection, "import_learning_rules", "confidence")? {
        "ORDER BY COALESCE(confidence, 0) DESC, id DESC"
    } else {
        "ORDER BY id DESC"
    };
    let sql = format!(
        "
        SELECT * FROM import_learning_rules
        WHERE user_id = ? AND COALESCE(enabled, 1) = 1
        {order_by}
        "
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![user_id], |row| row_to_map(row, ""))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(Value::Object)
        .collect())
}

fn load_categories(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare("SELECT * FROM categories WHERE user_id = ?")?;
    let rows = statement.query_map(params![user_id], |row| row_to_map(row, ""))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(Value::Object)
        .collect())
}

fn load_accounts(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "accounts")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare("SELECT * FROM accounts WHERE user_id = ?")?;
    let rows = statement.query_map(params![user_id], |row| row_to_map(row, ""))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(Value::Object)
        .collect())
}

fn user_import_learning_enabled(connection: &Connection, user_id: i64) -> DbResult<bool> {
    if !table_exists(connection, "users")?
        || !column_exists(connection, "users", "import_learning_enabled")?
    {
        return Ok(true);
    }
    connection
        .query_row(
            "SELECT COALESCE(import_learning_enabled, 1) FROM users WHERE id = ? LIMIT 1",
            params![user_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.unwrap_or(1) != 0)
        .map_err(DbError::from)
}

fn user_investment_keyword_config(connection: &Connection, user_id: i64) -> DbResult<Value> {
    if !table_exists(connection, "users")? {
        return Ok(build_user_investment_keyword_settings(None));
    }
    let user = connection
        .query_row(
            "SELECT * FROM users WHERE id = ? LIMIT 1",
            params![user_id],
            |row| row_to_map(row, ""),
        )
        .optional()?;
    Ok(build_user_investment_keyword_settings(user.as_ref()))
}

fn user_investment_keyword_config_tx(tx: &Transaction<'_>, user_id: i64) -> DbResult<Value> {
    if !table_exists_tx(tx, "users")? {
        return Ok(build_user_investment_keyword_settings(None));
    }
    let user = tx
        .query_row(
            "SELECT * FROM users WHERE id = ? LIMIT 1",
            params![user_id],
            |row| row_to_map(row, ""),
        )
        .optional()?;
    Ok(build_user_investment_keyword_settings(user.as_ref()))
}

fn get_category_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    if !table_exists_tx(tx, "categories")? {
        return Ok(None);
    }
    tx.query_row(
        "SELECT * FROM categories WHERE id = ? AND user_id = ? LIMIT 1",
        params![category_id, user_id],
        |row| row_to_map(row, ""),
    )
    .optional()
    .map_err(DbError::from)
}

fn account_exists_on_tx(tx: &Transaction<'_>, user_id: i64, account_id: i64) -> DbResult<bool> {
    if !table_exists_tx(tx, "accounts")? {
        return Ok(false);
    }
    tx.query_row(
        "SELECT 1 FROM accounts WHERE id = ? AND user_id = ? LIMIT 1",
        params![account_id, user_id],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn record_feedback_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    candidate_id: &str,
    action: &str,
    payload: &Value,
    now: &str,
) -> DbResult<i64> {
    tx.execute(
        "
        INSERT INTO bill_pair_feedback(user_id, candidate_id, action, payload_json, created_at)
        VALUES (?, ?, ?, ?, ?)
        ",
        params![user_id, candidate_id, action, payload.to_string(), now],
    )?;
    Ok(tx.last_insert_rowid())
}

fn reconciliation_candidate_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<Map<String, Value>> {
    let mut candidate = row_to_map(row, "")?;
    let import_snapshot =
        parse_json_object(&map_string(&candidate, "import_bill_snapshot_json", "{}"));
    let existing_snapshot =
        parse_json_object(&map_string(&candidate, "existing_bill_snapshot_json", "{}"));
    let source_payload = parse_json_object(&map_string(&candidate, "source_payload_json", "{}"));
    let group_metadata = parse_json_object(&map_string(&candidate, "group_metadata_json", "{}"));
    candidate.insert(
        "import_bill_snapshot".to_string(),
        Value::Object(import_snapshot),
    );
    candidate.insert(
        "existing_bill_snapshot".to_string(),
        Value::Object(existing_snapshot),
    );
    candidate.insert(
        "source_payload".to_string(),
        Value::Object(source_payload.clone()),
    );
    candidate.insert(
        "group_metadata".to_string(),
        Value::Object(group_metadata.clone()),
    );
    candidate.insert(
        "signal_label".to_string(),
        json!(source_payload
            .get("signal_label")
            .map(|value| value_string(Some(value)))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                group_metadata
                    .get("signal_label")
                    .map(|value| value_string(Some(value)))
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or_else(|| map_string(&candidate, "reason", ""))),
    );
    candidate.insert(
        "source_chain".to_string(),
        source_payload
            .get("source_chain")
            .cloned()
            .or_else(|| group_metadata.get("source_chain").cloned())
            .unwrap_or_else(|| json!([])),
    );
    Ok(candidate)
}

fn bill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Map<String, Value>> {
    row_to_map(row, "")
}

fn row_to_map(row: &rusqlite::Row<'_>, _prefix: &str) -> rusqlite::Result<Map<String, Value>> {
    let mut map = Map::new();
    for index in 0..row.as_ref().column_count() {
        let name = row.as_ref().column_name(index)?.to_string();
        map.insert(name, sql_value_ref_to_json(row.get_ref(index)?));
    }
    Ok(map)
}

fn prefixed_bill_from_row(
    row: &rusqlite::Row<'_>,
    prefix: &str,
) -> rusqlite::Result<Map<String, Value>> {
    let mut map = Map::new();
    for (field, key) in [
        ("id", "id"),
        ("date", "date"),
        ("type", "type"),
        ("amount", "amount"),
        ("counterparty", "counterparty"),
        ("description", "description"),
        ("payment_method", "payment_method"),
        ("main_category", "main_category"),
        ("sub_category", "sub_category"),
        ("source_account_id", "source_account_id"),
        ("destination_account_id", "destination_account_id"),
    ] {
        let column = format!("{prefix}_{field}");
        if let Ok(value) = row.get_ref(column.as_str()) {
            map.insert(key.to_string(), sql_value_ref_to_json(value));
        }
    }
    Ok(map)
}

fn sql_value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(_) => Value::Null,
    }
}

fn json_value_to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(*value)),
        Value::Number(number) => number
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| number.as_f64().map(SqlValue::Real))
            .unwrap_or(SqlValue::Null),
        Value::String(value) => SqlValue::Text(value.clone()),
        _ => SqlValue::Text(value.to_string()),
    }
}

fn parse_json_object(raw: &str) -> Map<String, Value> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn normalize_pair_type(pair_type: &str) -> MatchingResult<&'static str> {
    match pair_type.trim().to_ascii_lowercase().as_str() {
        TRANSFER_PAIR_TYPE => Ok(TRANSFER_PAIR_TYPE),
        INVESTMENT_PAIR_TYPE => Ok(INVESTMENT_PAIR_TYPE),
        _ => Err(MatchingRuntimeError::BadRequest(
            "Invalid pairType".to_string(),
        )),
    }
}

fn decision_from_action(action: &str) -> MatchingResult<ImportPreviewDecision> {
    match action {
        "accept" => Ok(ImportPreviewDecision::Accept),
        "reject" => Ok(ImportPreviewDecision::Reject),
        "clear" => Ok(ImportPreviewDecision::Clear),
        _ => Err(MatchingRuntimeError::BadRequest(
            "Invalid action".to_string(),
        )),
    }
}

fn map_write_error(error: DbError) -> MatchingRuntimeError {
    match error {
        DbError::InvalidOperation(message)
            if message.contains("not found") || message.contains("Bill not found") =>
        {
            MatchingRuntimeError::NotFound(message)
        }
        DbError::InvalidOperation(message) => MatchingRuntimeError::Conflict(message),
        other => MatchingRuntimeError::Db(other.to_string()),
    }
}

fn map_reconciliation_error(error: DbError) -> MatchingRuntimeError {
    match error {
        DbError::InvalidOperation(message) if message.contains("not found") => {
            MatchingRuntimeError::NotFound(message)
        }
        DbError::InvalidOperation(message) if message.contains("Invalid") => {
            MatchingRuntimeError::BadRequest(message)
        }
        DbError::InvalidOperation(message) => MatchingRuntimeError::Conflict(message),
        other => MatchingRuntimeError::Db(other.to_string()),
    }
}

fn table_exists(connection: &Connection, table: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
            params![table],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn table_exists_tx(tx: &Transaction<'_>, table: &str) -> DbResult<bool> {
    tx.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
        params![table],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> DbResult<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>("name"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == column))
}

fn column_exists_tx(tx: &Transaction<'_>, table: &str, column: &str) -> DbResult<bool> {
    let mut statement = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>("name"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == column))
}

fn map_string(map: &Map<String, Value>, key: &str, fallback: &str) -> String {
    map.get(key)
        .map(|value| value_string(Some(value)))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn map_i64(map: &Map<String, Value>, key: &str) -> i64 {
    map_optional_i64(map, key).unwrap_or(0)
}

fn map_optional_i64(map: &Map<String, Value>, key: &str) -> Option<i64> {
    value_i64(map.get(key)).filter(|value| *value > 0)
}

fn map_f64(map: &Map<String, Value>, key: &str) -> f64 {
    value_f64(map.get(key))
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(true)) => Some(1),
        Some(Value::Bool(false)) => Some(0),
        _ => None,
    }
}

fn value_f64(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn merge_description_values(values: impl IntoIterator<Item = String>) -> String {
    let mut merged = Vec::new();
    for value in values {
        for part in value.split('|') {
            let part = part.trim();
            if !part.is_empty() && !merged.iter().any(|item: &String| item == part) {
                merged.push(part.to_string());
            }
        }
    }
    merged.join("|")
}
