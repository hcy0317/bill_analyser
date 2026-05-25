// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

fn duplicate_suppression_exists_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    left_bill_id: i64,
    right_bill_id: i64,
) -> DbResult<bool> {
    suppression_exists_on_tx(
        tx,
        "bill_duplicate_pair_suppressions",
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
    if pair_type == DUPLICATE_CANDIDATE_KIND {
        return Ok(build_duplicate_bill_candidate(left, right).is_some());
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
