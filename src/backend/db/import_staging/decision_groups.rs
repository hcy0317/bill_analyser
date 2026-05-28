// 中文导读：导入 decision group 与历史账单 materialization 的持久化边界。
// 维护重点：HTTP 只提交结构化 draft；本文件负责 user/session scope、幂等写入和历史账单候选查询。
// 不变式：历史账单只 materialize 到预览证据，不在本切片改写正式 bills。

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_import_decision_groups_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportDecisionGroupDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let now = now_text();
        let mut inserted = 0;
        for draft in drafts {
            tx.execute(
                "
                INSERT INTO import_decision_groups (
                    session_id, user_id, group_type, group_key,
                    decision_status, base_preview_row_id,
                    signal_payload_json, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                ON CONFLICT(session_id, user_id, group_type, group_key)
                DO UPDATE SET
                    decision_status = excluded.decision_status,
                    base_preview_row_id = excluded.base_preview_row_id,
                    signal_payload_json = excluded.signal_payload_json,
                    updated_at = excluded.updated_at
                ",
                params![
                    session_id,
                    user_id,
                    draft.group_type,
                    draft.group_key,
                    draft.decision_status,
                    draft.base_preview_row_id,
                    serialize_json_object(&draft.signal_payload),
                    now,
                ],
            )?;
            let group_id = tx.query_row(
                "
                SELECT id FROM import_decision_groups
                WHERE session_id = ?1 AND user_id = ?2
                  AND group_type = ?3 AND group_key = ?4
                ",
                params![session_id, user_id, draft.group_type, draft.group_key],
                |row| row.get::<_, i64>(0),
            )?;
            tx.execute(
                "DELETE FROM import_decision_group_members WHERE group_id = ?1",
                params![group_id],
            )?;
            let mut member_statement = tx.prepare(
                "
                INSERT INTO import_decision_group_members (
                    group_id, preview_row_id, standard_row_id, history_bill_id,
                    member_role, parser_name, metadata_json, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
            )?;
            for member in &draft.members {
                member_statement.execute(params![
                    group_id,
                    member.preview_row_id,
                    member.standard_row_id,
                    member.history_bill_id,
                    member.member_role,
                    member.parser_name,
                    serialize_json_object(&member.metadata),
                    now,
                ])?;
            }
            inserted += 1;
        }
        Ok(inserted)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_decision_groups_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportDecisionGroupRow>> {
    let user_id = user_id_i64(user_id)?;
    let mut statement = connection.prepare(
        "
        SELECT id, session_id, user_id, group_type, group_key, decision_status,
               base_preview_row_id, signal_payload_json, created_at, updated_at
        FROM import_decision_groups
        WHERE session_id = ?1 AND user_id = ?2
        ORDER BY id ASC
        ",
    )?;
    let rows = statement.query_map(params![session_id, user_id], |row| {
        let signal_payload_json: Option<String> = row.get("signal_payload_json")?;
        Ok(ImportDecisionGroupRow {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            user_id: row.get("user_id")?,
            group_type: row.get("group_type")?,
            group_key: row.get("group_key")?,
            decision_status: row.get("decision_status")?,
            base_preview_row_id: row.get("base_preview_row_id")?,
            signal_payload: parse_json_object(signal_payload_json.as_deref()),
            members: Vec::new(),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })?;
    let mut groups = rows.collect::<Result<Vec<_>, _>>()?;
    for group in &mut groups {
        group.members = get_import_decision_group_members(connection, group.id)?;
    }
    Ok(groups)
}

fn get_import_decision_group_members(
    connection: &Connection,
    group_id: i64,
) -> DbResult<Vec<ImportDecisionGroupMemberRow>> {
    let mut statement = connection.prepare(
        "
        SELECT id, group_id, preview_row_id, standard_row_id, history_bill_id,
               member_role, parser_name, metadata_json, created_at
        FROM import_decision_group_members
        WHERE group_id = ?1
        ORDER BY id ASC
        ",
    )?;
    let rows = statement.query_map(params![group_id], |row| {
        let metadata_json: Option<String> = row.get("metadata_json")?;
        Ok(ImportDecisionGroupMemberRow {
            id: row.get("id")?,
            group_id: row.get("group_id")?,
            preview_row_id: row.get("preview_row_id")?,
            standard_row_id: row.get("standard_row_id")?,
            history_bill_id: row.get("history_bill_id")?,
            member_role: row.get("member_role")?,
            parser_name: row
                .get::<_, Option<String>>("parser_name")?
                .unwrap_or_default(),
            metadata: parse_json_object(metadata_json.as_deref()),
            created_at: row.get("created_at")?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_import_history_materializations_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportHistoryMaterializationDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let now = now_text();
        let mut inserted = 0;
        for draft in drafts {
            tx.execute(
                "
                INSERT INTO import_history_materializations (
                    session_id, user_id, history_bill_id, history_bill_version,
                    materialized_payload_json, rewrite_reason, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(session_id, user_id, history_bill_id)
                DO UPDATE SET
                    history_bill_version = excluded.history_bill_version,
                    materialized_payload_json = excluded.materialized_payload_json,
                    rewrite_reason = excluded.rewrite_reason,
                    created_at = excluded.created_at
                ",
                params![
                    session_id,
                    user_id,
                    draft.history_bill_id,
                    draft.history_bill_version,
                    serialize_json_object(&draft.materialized_payload),
                    draft.rewrite_reason,
                    now,
                ],
            )?;
            inserted += 1;
        }
        Ok(inserted)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_history_materializations_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportHistoryMaterializationRow>> {
    let mut statement = connection.prepare(
        "
        SELECT id, session_id, user_id, history_bill_id, history_bill_version,
               materialized_payload_json, rewrite_reason, created_at
        FROM import_history_materializations
        WHERE session_id = ?1 AND user_id = ?2
        ORDER BY id ASC
        ",
    )?;
    let rows = statement.query_map(params![session_id, user_id_i64(user_id)?], |row| {
        let payload_json: Option<String> = row.get("materialized_payload_json")?;
        Ok(ImportHistoryMaterializationRow {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            user_id: row.get("user_id")?,
            history_bill_id: row.get("history_bill_id")?,
            history_bill_version: row.get("history_bill_version")?,
            materialized_payload: parse_json_object(payload_json.as_deref()),
            rewrite_reason: row.get("rewrite_reason")?,
            created_at: row.get("created_at")?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn clear_import_preview_materialization_state(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<()> {
    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        tx.execute(
            "
            DELETE FROM import_decision_group_members
            WHERE group_id IN (
                SELECT id FROM import_decision_groups
                WHERE session_id = ?1 AND user_id = ?2
            )
            ",
            params![session_id, user_id],
        )?;
        tx.execute(
            "DELETE FROM import_decision_groups WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        delete_import_history_materializations_for_session(tx, session_id, user_id)?;
        tx.execute(
            "DELETE FROM import_annotation_samples WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        tx.execute(
            "DELETE FROM bills_preview WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        Ok(())
    })
}

fn delete_import_history_materializations_for_session(
    connection: &Connection,
    session_id: &str,
    user_id: i64,
) -> DbResult<()> {
    connection.execute(
        "DELETE FROM import_history_materializations WHERE session_id = ?1 AND user_id = ?2",
        params![session_id, user_id],
    )?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_history_candidate_bills_for_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportHistoryBillRow>> {
    if !import_staging_table_exists(connection, "bills")? {
        return Ok(Vec::new());
    }

    let standard_rows = get_import_standard_rows_by_session(connection, session_id, user_id)?;
    let mut day_prefixes = std::collections::BTreeSet::new();
    for row in standard_rows {
        if let Some(day) = import_history_day_prefix(&row.occurred_at) {
            day_prefixes.insert(day);
        }
    }
    if day_prefixes.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", day_prefixes.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut params = vec![SqlValue::Integer(user_id_i64(user_id)?)];
    params.extend(day_prefixes.iter().cloned().map(SqlValue::Text));
    let mut statement = connection.prepare(&format!(
        "
        SELECT id, date, type, amount, counterparty, description,
               payment_method, main_category, sub_category,
               source_account_id, destination_account_id, destination_amount,
               updated_at, import_history_id
        FROM bills
        WHERE user_id = ? AND substr(date, 1, 10) IN ({placeholders})
        ORDER BY date ASC, id ASC
        "
    ))?;
    let rows = statement.query_map(params_from_iter(params), import_history_bill_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn import_history_bill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportHistoryBillRow> {
    let history_bill_id = row.get::<_, i64>("id")?;
    let date = row.get::<_, String>("date")?;
    let transaction_type = row.get::<_, String>("type")?;
    let amount = row.get::<_, f64>("amount")?;
    let counterparty = row.get::<_, String>("counterparty")?;
    let description = row.get::<_, String>("description")?;
    let payment_method = row
        .get::<_, Option<String>>("payment_method")?
        .unwrap_or_default();
    let main_category = row
        .get::<_, Option<String>>("main_category")?
        .unwrap_or_default();
    let sub_category = row
        .get::<_, Option<String>>("sub_category")?
        .unwrap_or_default();
    let source_account_id = row
        .get::<_, Option<i64>>("source_account_id")?
        .filter(|value| *value > 0)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let destination_account_id = row
        .get::<_, Option<i64>>("destination_account_id")?
        .filter(|value| *value > 0)
        .map(|value| value.to_string());
    let destination_amount = row.get::<_, Option<f64>>("destination_amount")?.unwrap_or(0.0);
    let updated_at = row
        .get::<_, Option<String>>("updated_at")?
        .unwrap_or_default();
    let history_bill_version = row.get::<_, Option<i64>>("import_history_id")?.unwrap_or(1).max(1);
    let bill = DedupBill {
        id: Some(history_bill_id.to_string()),
        date: date.clone(),
        amount: Money::from_yuan_str(&python_float_text(amount)).unwrap_or(Money::ZERO),
        transaction_type: transaction_type.clone(),
        source_account_id: source_account_id.clone(),
        parser_id: "history_db".to_string(),
        source: payment_method.clone(),
        counterparty: counterparty.clone(),
        payment_method: payment_method.clone(),
        description: description.clone(),
        main_category: main_category.clone(),
        sub_category: sub_category.clone(),
        destination_account_id: destination_account_id.clone(),
        destination_account_name: destination_account_id.clone().unwrap_or_default(),
        ..DedupBill::default()
    };
    Ok(ImportHistoryBillRow {
        history_bill_id,
        history_bill_version,
        bill,
        snapshot: serde_json::json!({
            "id": history_bill_id,
            "date": date,
            "type": transaction_type,
            "amount": amount,
            "counterparty": counterparty,
            "description": description,
            "payment_method": payment_method,
            "main_category": main_category,
            "sub_category": sub_category,
            "source_account_id": source_account_id,
            "destination_account_id": destination_account_id,
            "destination_amount": destination_amount,
            "updated_at": updated_at,
            "history_bill_version": history_bill_version,
        }),
    })
}

fn import_history_day_prefix(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() >= 10 {
        Some(trimmed[..10].to_string())
    } else {
        None
    }
}

fn import_staging_table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn serialize_json_object(value: &Value) -> String {
    if value.is_object() {
        value.to_string()
    } else {
        "{}".to_string()
    }
}
