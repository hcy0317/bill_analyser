// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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

fn list_duplicate_candidates_for_bill(
    connection: &Connection,
    user_id: i64,
    anchor_bill: &Map<String, Value>,
) -> DbResult<Vec<Value>> {
    let bill_id = map_i64(anchor_bill, "id");
    let suppressed = suppressed_pair_candidate_ids(
        connection,
        user_id,
        bill_id,
        "bill_duplicate_pair_suppressions",
    )?;
    let amount = map_f64(anchor_bill, "amount");
    let destination_amount = map_f64(anchor_bill, "destination_amount");
    let mut statement = connection.prepare(
        "
        SELECT * FROM bills b
        WHERE b.user_id = ?
          AND b.id != ?
          AND COALESCE(b.date, '') = ?
          AND COALESCE(b.type, '') = ?
          AND ABS(COALESCE(b.amount, 0) - ?) <= ?
          AND ABS(COALESCE(b.destination_amount, 0) - ?) <= ?
          AND COALESCE(b.source_account_id, 0) = ?
          AND COALESCE(b.destination_account_id, 0) = ?
          AND COALESCE(b.counterparty, '') = ?
          AND COALESCE(b.description, '') = ?
          AND COALESCE(b.payment_method, '') = ?
          AND COALESCE(b.main_category, '') = ?
          AND COALESCE(b.sub_category, '') = ?
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
            map_string(anchor_bill, "date", ""),
            map_string(anchor_bill, "type", ""),
            amount,
            TRANSFER_AMOUNT_TOLERANCE,
            destination_amount,
            TRANSFER_AMOUNT_TOLERANCE,
            map_i64(anchor_bill, "source_account_id"),
            map_i64(anchor_bill, "destination_account_id"),
            map_string(anchor_bill, "counterparty", ""),
            map_string(anchor_bill, "description", ""),
            map_string(anchor_bill, "payment_method", ""),
            map_string(anchor_bill, "main_category", ""),
            map_string(anchor_bill, "sub_category", ""),
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
    Ok(build_duplicate_bill_candidates(anchor_bill, &bills))
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
