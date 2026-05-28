#[tracing::instrument(level = "debug", skip_all)]
pub fn confirm_preview_to_bills_with_ack(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    history_acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> DbResult<ConfirmPreviewResult> {
    let user_scope = user_id;
    let user_id = user_id_i64(user_scope)?;
    let now = now_text();
    let batch_id = Utc::now().format("%Y%m%d%H%M%S").to_string();

    run_transaction(connection, |tx| {
        if get_import_session(tx, session_id, user_scope)?.is_none() {
            return Err(DbError::InvalidOperation(
                "import session not found or already cleaned".to_string(),
            ));
        }
        let previews = get_preview_by_session(tx, session_id, user_scope, true)?;
        let materializations = get_import_history_materializations_by_session(
            tx,
            session_id,
            user_scope,
        )?
        .into_iter()
        .map(|row| (row.history_bill_id, row))
        .collect::<std::collections::HashMap<_, _>>();
        let history_operations =
            collect_selected_history_rewrite_operations(session_id, &previews, &materializations)?;
        validate_history_rewrite_acknowledgement(
            session_id,
            &previews,
            &history_operations,
            history_acknowledgement,
        )?;
        let history_operations_by_preview = history_operations
            .into_iter()
            .map(|operation| (operation.preview_id, operation))
            .collect::<std::collections::HashMap<_, _>>();
        let mut touched_account_ids = Vec::new();
        let mut result = ConfirmPreviewResult {
            confirmed_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            errors: Vec::new(),
        };
        let apply_meta = HistoryRewriteApplyMeta {
            session_id,
            user_id,
            now: &now,
            batch_id: &batch_id,
        };

        for preview in &previews {
            if let Some(operation) = history_operations_by_preview.get(&preview.id) {
                apply_history_rewrite_operation(
                    tx,
                    preview,
                    operation,
                    &apply_meta,
                    &mut touched_account_ids,
                )?;
                result.confirmed_count += 1;
                continue;
            }

            let bill_type = normalize_confirm_bill_type(&preview.preview_type);
            if confirm_preview_requires_review(preview, &bill_type) {
                result.skipped_count += 1;
                result.errors.push(format!(
                    "preview {} requires review before confirm",
                    preview.id
                ));
                continue;
            }
            let preview_date_text = normalize_bill_date_text(&preview.preview_date);
            let amount = confirm_amount_for_type(&bill_type, preview.preview_amount);
            let bill_hash = calculate_import_bill_hash(
                &preview_date_text,
                &bill_type,
                amount,
                &preview.preview_counterparty,
                &preview.preview_description,
            );
            let insert_result = tx.execute(
                "
                INSERT INTO bills (
                    user_id, date, type, amount, counterparty, description,
                    payment_method, main_category, sub_category,
                    source_account_id, destination_account_id, destination_amount,
                    batch_id, hash, created_from_recurring, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                ",
                params![
                    user_id,
                    preview_date_text,
                    bill_type,
                    amount,
                    preview.preview_counterparty,
                    preview.preview_description,
                    preview.preview_payment_method,
                    preview.preview_main_category,
                    preview.preview_sub_category,
                    preview.preview_source_account_id,
                    preview.preview_destination_account_id,
                    preview.preview_destination_amount,
                    batch_id,
                    bill_hash,
                    preview.preview_recurring_id,
                    now,
                    now,
                ],
            );

            match insert_result {
                Ok(_) => {
                    result.confirmed_count += 1;
                    collect_preview_account_ids(preview, &mut touched_account_ids);
                }
                Err(error) if is_sqlite_constraint_error(&error) => result.duplicate_count += 1,
                Err(error) => return Err(DbError::from(error)),
            }
        }

        sync_confirm_account_balances(tx, user_id, &touched_account_ids, &now)?;
        delete_import_ledger_for_session(tx, session_id, user_id)?;
        tx.execute(
            "DELETE FROM import_annotation_samples WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        tx.execute(
            "DELETE FROM bills_preview WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        tx.execute(
            "DELETE FROM bills_parser_template WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        tx.execute(
            "DELETE FROM import_sessions WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        Ok(result)
    })
}

#[derive(Debug, Clone)]
struct PlannedHistoryRewriteOperation {
    preview_id: i64,
    operation_id: String,
    planned_operation: String,
    history_bill_id: i64,
    history_bill_version: i64,
    history_role: String,
    acknowledgement_token: String,
    materialization: ImportHistoryMaterializationRow,
}

fn collect_selected_history_rewrite_operations(
    session_id: &str,
    previews: &[ImportPreviewRow],
    materializations: &std::collections::HashMap<i64, ImportHistoryMaterializationRow>,
) -> DbResult<Vec<PlannedHistoryRewriteOperation>> {
    let mut operations = Vec::new();
    for preview in previews {
        if let Some(operation) =
            history_rewrite_operation_from_preview(session_id, preview, materializations)?
        {
            operations.push(operation);
        }
    }
    Ok(operations)
}

fn history_rewrite_operation_from_preview(
    session_id: &str,
    preview: &ImportPreviewRow,
    materializations: &std::collections::HashMap<i64, ImportHistoryMaterializationRow>,
) -> DbResult<Option<PlannedHistoryRewriteOperation>> {
    let matching = &preview.preview_matching_feedback;
    let planned_operation = matching
        .pointer("/reconciliation/planned_operation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if !matches!(
        planned_operation.as_str(),
        "update_history" | "merge_transfer_history"
    ) {
        return Ok(None);
    }
    let annotation_type = matching
        .pointer("/annotation/type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let notice = matching
        .pointer("/reconciliation/notice")
        .or_else(|| matching.pointer("/annotation/history_rewrite_notice"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if annotation_type != "history_rewrite_pending" || notice != HISTORY_REWRITE_NOTICE {
        return Err(DbError::InvalidOperation(
            "history rewrite operation requires visible preview marker".to_string(),
        ));
    }
    let history_bill_id = value_pointer_i64(matching, "/reconciliation/history_bill_id");
    let history_bill_version =
        value_pointer_i64(matching, "/reconciliation/history_bill_version").max(1);
    if history_bill_id <= 0 {
        return Err(DbError::InvalidOperation(
            "history rewrite operation missing history bill id".to_string(),
        ));
    }
    let Some(materialization) = materializations.get(&history_bill_id).cloned() else {
        return Err(DbError::InvalidOperation(
            "history rewrite materialization is missing".to_string(),
        ));
    };
    if materialization.history_bill_version != history_bill_version {
        return Err(DbError::InvalidOperation(
            "history bill version is stale".to_string(),
        ));
    }
    let materialized_operation = materialization
        .materialized_payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if materialized_operation != planned_operation {
        return Err(DbError::InvalidOperation(
            "history rewrite materialization operation mismatch".to_string(),
        ));
    }
    let group_key = matching
        .pointer("/reconciliation/group_key")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let operation_id = matching
        .pointer("/reconciliation/operation_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            build_import_history_rewrite_operation_id(
                &planned_operation,
                history_bill_id,
                history_bill_version,
                group_key,
            )
        });
    let acknowledgement_token = build_import_history_rewrite_ack_token(
        session_id,
        &operation_id,
        &planned_operation,
        history_bill_id,
        history_bill_version,
    );
    Ok(Some(PlannedHistoryRewriteOperation {
        preview_id: preview.id,
        operation_id,
        planned_operation,
        history_bill_id,
        history_bill_version,
        history_role: matching
            .pointer("/reconciliation/history_role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        acknowledgement_token,
        materialization,
    }))
}

fn validate_history_rewrite_acknowledgement(
    session_id: &str,
    selected_previews: &[ImportPreviewRow],
    operations: &[PlannedHistoryRewriteOperation],
    acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> DbResult<()> {
    if operations.is_empty() {
        return Ok(());
    }
    let Some(acknowledgement) = acknowledgement else {
        return Err(DbError::InvalidOperation(
            "history rewrite acknowledgement is required".to_string(),
        ));
    };
    if !acknowledgement.acknowledged || acknowledgement.selection_scope.is_null() {
        return Err(DbError::InvalidOperation(
            "history rewrite acknowledgement is incomplete".to_string(),
        ));
    }
    let mut expected_selected_ids: Vec<i64> = selected_previews.iter().map(|row| row.id).collect();
    expected_selected_ids.sort_unstable();
    let mut actual_selected_ids = acknowledgement.selected_preview_ids.clone();
    actual_selected_ids.sort_unstable();
    if actual_selected_ids != expected_selected_ids {
        return Err(DbError::InvalidOperation(
            "history rewrite acknowledgement selected preview ids mismatch".to_string(),
        ));
    }
    let ack_operations = acknowledgement
        .operations
        .iter()
        .map(|operation| (operation.preview_id, operation))
        .collect::<std::collections::HashMap<_, _>>();
    if ack_operations.len() != operations.len() {
        return Err(DbError::InvalidOperation(
            "history rewrite acknowledgement operations mismatch".to_string(),
        ));
    }
    for operation in operations {
        let Some(ack_operation) = ack_operations.get(&operation.preview_id) else {
            return Err(DbError::InvalidOperation(
                "history rewrite acknowledgement operation is missing".to_string(),
            ));
        };
        if ack_operation.operation_id != operation.operation_id
            || ack_operation.planned_operation != operation.planned_operation
            || ack_operation.history_bill_id != operation.history_bill_id
            || ack_operation.history_bill_version != operation.history_bill_version
            || ack_operation.acknowledgement_token != operation.acknowledgement_token
        {
            return Err(DbError::InvalidOperation(format!(
                "history rewrite acknowledgement operation mismatch for session {session_id}"
            )));
        }
    }
    Ok(())
}

fn value_pointer_i64(value: &Value, pointer: &str) -> i64 {
    value
        .pointer(pointer)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or_default()
}
