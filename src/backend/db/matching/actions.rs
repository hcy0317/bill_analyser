// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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
