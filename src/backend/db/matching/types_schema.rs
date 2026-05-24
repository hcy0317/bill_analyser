// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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
    candidates.extend(list_duplicate_candidates_for_bill(
        connection,
        user_id,
        &anchor_bill,
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
