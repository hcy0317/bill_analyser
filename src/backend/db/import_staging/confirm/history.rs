fn validate_history_acknowledgement(
    session_id: &str,
    previews: &[ImportPreviewRow],
    acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> DbResult<Vec<HistoryConfirmPlan>> {
    let mut plans = Vec::new();
    for preview in previews {
        let Some(plan) = history_confirm_plan_from_preview(session_id, preview)? else {
            continue;
        };
        plans.push(plan);
    }
    if plans.is_empty() {
        if acknowledgement.is_some_and(|value| !value.operations.is_empty()) {
            return Err(DbError::InvalidOperation(
                "invalid history acknowledgement operation set".to_string(),
            ));
        }
        return Ok(plans);
    }
    let acknowledgement = acknowledgement.filter(|value| value.acknowledged).ok_or_else(|| {
        DbError::InvalidOperation(
            "invalid history acknowledgement: acknowledgement token is required".to_string(),
        )
    })?;
    let selected_ids = acknowledgement
        .selected_preview_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut acknowledged_preview_ids = BTreeSet::new();
    for plan in &plans {
        if !selected_ids.is_empty() && !selected_ids.contains(&plan.preview_id) {
            return Err(DbError::InvalidOperation(
                "invalid history acknowledgement selection".to_string(),
            ));
        }
        let operation = acknowledgement
            .operations
            .iter()
            .find(|operation| operation.preview_id == plan.preview_id)
            .ok_or_else(|| {
                DbError::InvalidOperation(
                    "invalid history acknowledgement: operation is missing".to_string(),
                )
            })?;
        if !acknowledged_preview_ids.insert(operation.preview_id) {
            return Err(DbError::InvalidOperation(
                "invalid history acknowledgement: duplicate operation".to_string(),
            ));
        }
        let normalized_operation = normalize_history_operation(&operation.planned_operation)
            .ok_or_else(|| {
                DbError::InvalidOperation(
                    "invalid history acknowledgement operation".to_string(),
                )
            })?;
        if normalized_operation != plan.operation
            || operation.operation_id != plan.operation_id
            || operation.history_bill_id != plan.history_bill_id
            || operation.history_bill_version != plan.history_bill_version
            || operation.acknowledgement_token != plan.acknowledgement_token
        {
            return Err(DbError::InvalidOperation(
                "invalid history acknowledgement token or CAS identity".to_string(),
            ));
        }
    }
    if acknowledgement.operations.len() != plans.len() {
        return Err(DbError::InvalidOperation(
            "invalid history acknowledgement operation set".to_string(),
        ));
    }
    Ok(plans)
}

fn history_confirm_plan_from_preview(
    session_id: &str,
    preview: &ImportPreviewRow,
) -> DbResult<Option<HistoryConfirmPlan>> {
    let Some(reconciliation) = preview
        .preview_matching_feedback
        .get("reconciliation")
        .and_then(Value::as_object)
    else {
        return Ok(None);
    };
    let raw_operation = reconciliation
        .get("planned_operation")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(operation) = normalize_history_operation(raw_operation) else {
        return Ok(None);
    };
    let history_bill_id = reconciliation
        .get("history_bill_id")
        .and_then(confirm_value_i64)
        .unwrap_or_default();
    let history_bill_version = reconciliation
        .get("history_bill_version")
        .and_then(confirm_value_i64)
        .unwrap_or_default();
    if history_bill_id <= 0 || history_bill_version <= 0 {
        return Err(DbError::InvalidOperation(
            "invalid history acknowledgement CAS identity".to_string(),
        ));
    }
    let group_key = reconciliation
        .get("group_key")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let operation_id = bill_analyser_core::build_import_history_rewrite_operation_id(
        operation,
        history_bill_id,
        history_bill_version,
        group_key,
    );
    let acknowledgement_token = bill_analyser_core::build_import_history_rewrite_ack_token(
        session_id,
        &operation_id,
        operation,
        history_bill_id,
        history_bill_version,
    );
    Ok(Some(HistoryConfirmPlan {
        preview_id: preview.id,
        operation,
        operation_id,
        history_bill_id,
        history_bill_version,
        acknowledgement_token,
    }))
}

fn confirm_value_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<i64>().ok())
    })
}

async fn apply_history_confirm_plan(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    preview: &ImportPreviewRow,
    plan: &HistoryConfirmPlan,
) -> DbResult<()> {
    let materialized = sqlx::query(
        r#"
        SELECT materialized_payload
        FROM import_history_materializations
        WHERE session_id = (
                SELECT session_id FROM import_preview_rows
                WHERE id = $1 AND user_id = $2
            )
          AND user_id = $2
          AND history_bill_id = $3
          AND history_bill_version = $4
        FOR UPDATE
        "#,
    )
    .bind(preview.id)
    .bind(user_id)
    .bind(plan.history_bill_id)
    .bind(plan.history_bill_version)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        DbError::InvalidOperation("history materialization CAS mismatch".to_string())
    })?;
    let materialized_payload = materialized.try_get::<Value, _>("materialized_payload")?;
    let stored_operation = materialized_payload
        .get("planned_operation")
        .or_else(|| materialized_payload.get("operation"))
        .and_then(Value::as_str)
        .and_then(normalize_history_operation)
        .ok_or_else(|| {
            DbError::InvalidOperation("invalid history materialization operation".to_string())
        })?;
    if stored_operation != plan.operation {
        return Err(DbError::InvalidOperation(
            "history materialization operation mismatch".to_string(),
        ));
    }
    let old_bill = sqlx::query(
        r#"
        SELECT amount_cents, direction, transaction_type,
               COALESCE(source_account_id, account_id) AS source_account_id,
               COALESCE(target_account_id, transfer_target_account_id) AS destination_account_id,
               standard_payload
        FROM bills
        WHERE user_id = $1 AND id = $2 AND version = $3 AND is_deleted = false
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(plan.history_bill_id)
    .bind(plan.history_bill_version)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| DbError::InvalidOperation("history bill CAS mismatch".to_string()))?;
    let old_payload = old_bill.try_get::<Value, _>("standard_payload")?;
    let old_deltas = confirm_balance_deltas(
        &old_bill.try_get::<String, _>("transaction_type")?,
        &old_bill.try_get::<String, _>("direction")?,
        old_bill.try_get("amount_cents")?,
        payload_i64(&old_payload, "destination_amount_cents"),
        old_bill.try_get("source_account_id")?,
        old_bill.try_get("destination_account_id")?,
    );
    let transaction_type = confirm_transaction_type(&preview.preview_type);
    let direction = if transaction_type == "income" {
        "income"
    } else {
        "expense"
    };
    let amount_cents = preview.preview_amount_cents.checked_abs().ok_or_else(|| {
        DbError::InvalidOperation("invalid history preview amount".to_string())
    })?;
    let destination_amount_cents = preview
        .preview_destination_amount_cents
        .checked_abs()
        .ok_or_else(|| {
            DbError::InvalidOperation("invalid history destination amount".to_string())
        })?;
    let standard_payload = Value::Object(bill_create_fields_from_preview(preview));
    let changed = sqlx::query(
        r#"
        UPDATE bills
        SET occurred_at = $4::timestamptz,
            amount_cents = $5,
            direction = $6,
            transaction_type = $7,
            account_id = $8,
            source_account_id = $8,
            target_account_id = $9,
            transfer_target_account_id = $9,
            category_id = $10,
            merchant = $11,
            payment_method = $12,
            description = $13,
            standard_payload = $14::jsonb,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND version = $3 AND is_deleted = false
        "#,
    )
    .bind(user_id)
    .bind(plan.history_bill_id)
    .bind(plan.history_bill_version)
    .bind(normalize_bill_date_text(&preview.preview_date))
    .bind(amount_cents)
    .bind(direction)
    .bind(&transaction_type)
    .bind(preview.preview_source_account_id)
    .bind(preview.preview_destination_account_id)
    .bind(preview.category_id)
    .bind(&preview.preview_counterparty)
    .bind(&preview.preview_payment_method)
    .bind(&preview.preview_description)
    .bind(standard_payload.to_string())
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(DbError::InvalidOperation(
            "history bill CAS mismatch".to_string(),
        ));
    }
    let new_deltas = confirm_balance_deltas(
        &transaction_type,
        direction,
        amount_cents,
        Some(destination_amount_cents),
        preview.preview_source_account_id,
        preview.preview_destination_account_id,
    );
    apply_confirm_balance_delta_difference(tx, user_id, &old_deltas, &new_deltas).await?;
    Ok(())
}
