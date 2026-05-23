// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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
