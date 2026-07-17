async fn apply_confirm_time_effect_boundary(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    declared_effects: &[ConfirmTimeEffect],
) -> DbResult<()> {
    if !declared_effects.is_empty() {
        return Err(DbError::InvalidOperation(
            "unsupported confirm-time effect".to_string(),
        ));
    }
    sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut **tx)
        .await?;
    tracing::debug!(
        domain = "import_confirm",
        operation = "confirm_time_effect_boundary",
        declared_effect_count = 0,
        "no supported confirm-time effects declared"
    );
    Ok(())
}

fn confirm_preview_result_from_envelope(envelope: &Value) -> DbResult<ConfirmPreviewResult> {
    let data = envelope.get("data").ok_or_else(|| {
        DbError::InvalidOperation("invalid import confirm receipt envelope".to_string())
    })?;
    let data = serde_json::from_value::<bill_analyser_core::ImportStageConfirmData>(data.clone())
        .map_err(|_| {
            DbError::InvalidOperation("invalid import confirm receipt envelope".to_string())
        })?;
    Ok(ConfirmPreviewResult {
        confirmed_count: data.imported_count,
        skipped_count: data.skipped_count,
        duplicate_count: 0,
        errors: data.errors,
    })
}

async fn lock_import_session_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_id: &str,
    user_id: i64,
) -> DbResult<LockedImportSession> {
    let row = sqlx::query(
        r#"
        SELECT id, status, metadata, version
        FROM import_sessions
        WHERE session_key = $1 AND user_id = $2
        FOR UPDATE
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))?;
    let session = LockedImportSession {
        id: row.try_get("id")?,
        status: row.try_get("status")?,
        metadata: row.try_get("metadata")?,
        version: row.try_get("version")?,
    };
    tracing::debug!(
        domain = "import_confirm",
        operation = "session_locked",
        outcome = "locked",
        session_key = %session_id,
        session_version = session.version,
        terminal = session.status == "confirmed",
        "user-scoped import session locked"
    );
    Ok(session)
}

fn stored_confirm_receipt(metadata: &Value) -> DbResult<StoredConfirmReceipt> {
    let receipt = metadata.get("confirm_receipt").cloned().ok_or_else(|| {
        DbError::InvalidOperation("confirmed import session receipt is missing".to_string())
    })?;
    let receipt = serde_json::from_value::<StoredConfirmReceipt>(receipt).map_err(|_| {
        DbError::InvalidOperation("confirmed import session receipt is invalid".to_string())
    })?;
    if receipt.receipt_schema_version != CONFIRM_RECEIPT_SCHEMA_VERSION {
        return Err(DbError::InvalidOperation(
            "unsupported import confirm receipt schema".to_string(),
        ));
    }
    Ok(receipt)
}

async fn apply_confirm_command_mutations(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    command: &ConfirmCommand,
) -> DbResult<()> {
    let identity_maps = load_import_identity_maps_for_confirm(tx, user_id).await?;
    if !command.preview_patches.is_empty()
        && !command.preserve_unpatched_selection
        && command.selected_preview_ids.is_none()
    {
        set_confirm_session_selection(tx, session_db_id, user_id, &[]).await?;
    }
    for patch in &command.preview_patches {
        let patch = canonicalize_confirm_category_patch(tx, user_id, patch).await?;
        if !apply_confirm_preview_patch_on_tx(
            tx,
            session_db_id,
            user_id,
            &patch,
            &identity_maps,
        )
        .await?
        {
            return Err(DbError::InvalidOperation(format!(
                "preview bill not found: {}",
                patch.preview_id
            )));
        }
    }
    if let Some(selected_preview_ids) = &command.selected_preview_ids {
        let mut selected_preview_ids = selected_preview_ids.clone();
        selected_preview_ids.sort_unstable();
        selected_preview_ids.dedup();
        if !selected_preview_ids.is_empty() {
            let found = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)::BIGINT
                FROM import_preview_rows
                WHERE session_id = $1 AND user_id = $2 AND id = ANY($3)
                "#,
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(&selected_preview_ids)
            .fetch_one(&mut **tx)
            .await?;
            if found != i64::try_from(selected_preview_ids.len()).unwrap_or(i64::MAX) {
                return Err(DbError::InvalidOperation(
                    "preview bill not found in import session".to_string(),
                ));
            }
        }
        set_confirm_session_selection(tx, session_db_id, user_id, &selected_preview_ids).await?;
    }
    Ok(())
}

async fn set_confirm_session_selection(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    selected_preview_ids: &[i64],
) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE import_preview_rows
        SET selected = false,
            preview_payload = jsonb_set(preview_payload, '{preview_selected}', 'false'::jsonb, true),
            updated_at = now(),
            version = version + 1
        WHERE session_id = $1 AND user_id = $2 AND selected = true
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    if !selected_preview_ids.is_empty() {
        sqlx::query(
            r#"
            UPDATE import_preview_rows
            SET selected = true,
                preview_payload = jsonb_set(preview_payload, '{preview_selected}', 'true'::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE session_id = $1 AND user_id = $2 AND id = ANY($3)
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(selected_preview_ids)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn canonicalize_confirm_category_patch(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    patch: &ImportPreviewPatch,
) -> DbResult<ImportPreviewPatch> {
    let category_change = patch
        .changes
        .iter()
        .rev()
        .find(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
        .map(|(_, value)| value.clone());
    let Some(category_change) = category_change else {
        return Ok(patch.clone());
    };
    let mut patch = patch.clone();
    patch.changes.retain(|(field, _)| {
        !matches!(
            field,
            ImportPreviewPatchField::CategoryId
                | ImportPreviewPatchField::MainCategory
                | ImportPreviewPatchField::SubCategory
        )
    });
    let category_id = match category_change {
        ImportPreviewPatchValue::Null => None,
        ImportPreviewPatchValue::Integer(value) if value > 0 => Some(value),
        ImportPreviewPatchValue::Integer(_) => None,
        _ => {
            return Err(DbError::InvalidOperation(
                "invalid category patch".to_string(),
            ))
        }
    };
    let Some(category_id) = category_id else {
        patch
            .changes
            .push((ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Null));
        patch.changes.push((
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(String::new()),
        ));
        patch.changes.push((
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(String::new()),
        ));
        return Ok(patch);
    };
    let row = sqlx::query(import_preview_category_lookup_sql())
        .bind(category_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| DbError::InvalidOperation("invalid category".to_string()))?;
    let lookup = import_preview_category_lookup_from_values(
        &row.try_get::<String, _>("name")?,
        &row
            .try_get::<Option<String>, _>("path")?
            .unwrap_or_default(),
        row.try_get::<Option<String>, _>("category_type")?
            .as_deref(),
    );
    if let Some(preview_type) = confirm_category_type_name(lookup.type_code) {
        patch
            .changes
            .retain(|(field, _)| *field != ImportPreviewPatchField::Type);
        patch.changes.push((
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(preview_type.to_string()),
        ));
    }
    patch.changes.push((
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(category_id),
    ));
    patch.changes.push((
        ImportPreviewPatchField::MainCategory,
        ImportPreviewPatchValue::Text(lookup.main_category),
    ));
    patch.changes.push((
        ImportPreviewPatchField::SubCategory,
        ImportPreviewPatchValue::Text(lookup.sub_category),
    ));
    Ok(patch)
}

fn confirm_category_type_name(type_code: Option<i64>) -> Option<&'static str> {
    match type_code {
        Some(2) => Some("收入"),
        Some(3) => Some("支出"),
        Some(4) => Some("转账"),
        Some(5) => Some("投资"),
        _ => None,
    }
}

async fn apply_confirm_preview_patch_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    patch: &ImportPreviewPatch,
    identity_maps: &ImportIdentityMaps,
) -> DbResult<bool> {
    let Some(row) = sqlx::query(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.id = $1 AND p.session_id = $2 AND p.user_id = $3 FOR UPDATE OF p",
    )
    .bind(patch.preview_id)
    .bind(session_db_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(false);
    };
    let mut preview = preview_from_pg_row(&row)?;
    let mut payload = row
        .try_get::<Value, _>("preview_payload")
        .unwrap_or_else(|_| json!({}));
    for (field, value) in &patch.changes {
        apply_patch_value_to_preview(&mut preview, &mut payload, *field, value.clone())?;
    }
    if patch.clear_transfer_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "transfer");
    }
    if patch.clear_learning_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "learning");
    }
    if patch.clear_llm_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "llm");
    }
    apply_identity_validation_to_preview(&mut preview, &mut payload, identity_maps);
    payload_set(
        &mut payload,
        "preview_matching_feedback",
        preview.preview_matching_feedback.clone(),
    );
    let amount_cents = preview.preview_amount_cents.checked_abs().ok_or_else(|| {
        DbError::InvalidOperation("invalid preview amount".to_string())
    })?;
    let direction = if matches!(preview.preview_type.as_str(), "收入" | "income") {
        "income"
    } else {
        "expense"
    };
    let mut query = build_preview_row_update_query(
        &preview,
        payload.to_string(),
        amount_cents,
        direction,
        patch.preview_id,
        session_db_id,
        user_id,
    );
    Ok(query
        .build()
        .execute(&mut **tx)
        .await?
        .rows_affected()
        == 1)
}

fn confirm_command_fingerprint(
    command: &ConfirmCommand,
    request_session_version: i64,
) -> DbResult<String> {
    let acknowledgement = command.history_acknowledgement.as_ref().map(|acknowledgement| {
        let mut selected_preview_ids = acknowledgement.selected_preview_ids.clone();
        selected_preview_ids.sort_unstable();
        selected_preview_ids.dedup();
        let mut operations = acknowledgement.operations.clone();
        operations.sort_by(|left, right| {
            (
                left.preview_id,
                left.operation_id.as_str(),
                left.planned_operation.as_str(),
                left.history_bill_id,
                left.history_bill_version,
                left.acknowledgement_token.as_str(),
            )
                .cmp(&(
                    right.preview_id,
                    right.operation_id.as_str(),
                    right.planned_operation.as_str(),
                    right.history_bill_id,
                    right.history_bill_version,
                    right.acknowledgement_token.as_str(),
                ))
        });
        json!({
            "acknowledged": acknowledgement.acknowledged,
            "selected_preview_ids": selected_preview_ids,
            "operations": operations,
            "selection_scope": canonical_json(&acknowledgement.selection_scope),
        })
    });
    let selected_preview_ids = command.selected_preview_ids.as_ref().map(|values| {
        let mut values = values.clone();
        values.sort_unstable();
        values.dedup();
        values
    });
    let mut patches = command
        .preview_patches
        .iter()
        .map(canonical_confirm_patch)
        .collect::<DbResult<Vec<_>>>()?;
    patches.sort_by(|left, right| {
        left["preview_id"]
            .as_i64()
            .cmp(&right["preview_id"].as_i64())
    });
    let preserve_unpatched_selection =
        !command.preview_patches.is_empty() && command.preserve_unpatched_selection;
    let source = canonical_json(&json!({
        "schema": "import-confirm-command-v1",
        "session_id": command.session_id.trim(),
        "request_session_version": request_session_version,
        "preview_patches": patches,
        "selected_preview_ids": selected_preview_ids,
        "preserve_unpatched_selection": preserve_unpatched_selection,
        "history_acknowledgement": acknowledgement,
        "declared_confirm_time_effects": [],
    }));
    let encoded = serde_json::to_vec(&source).map_err(|error| {
        DbError::InvalidOperation(format!("failed to fingerprint confirm command: {error}"))
    })?;
    let digest = Sha256::digest(encoded);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn canonical_confirm_patch(patch: &ImportPreviewPatch) -> DbResult<Value> {
    let mut changes = BTreeMap::<&'static str, Value>::new();
    let category_change = patch
        .changes
        .iter()
        .rev()
        .find(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
        .map(|(_, value)| value);
    let positive_category =
        matches!(category_change, Some(ImportPreviewPatchValue::Integer(value)) if *value > 0);
    for (field, value) in &patch.changes {
        if category_change.is_some()
            && matches!(
                field,
                ImportPreviewPatchField::MainCategory | ImportPreviewPatchField::SubCategory
            )
        {
            continue;
        }
        if positive_category && *field == ImportPreviewPatchField::Type {
            continue;
        }
        let value = if *field == ImportPreviewPatchField::CategoryId
            && matches!(value, ImportPreviewPatchValue::Integer(value) if *value <= 0)
        {
            Value::Null
        } else {
            confirm_patch_value_json(value)?
        };
        changes.insert(confirm_patch_field_name(*field), value);
    }
    Ok(json!({
        "preview_id": patch.preview_id,
        "changes": changes,
        "clear_transfer_decision": patch.clear_transfer_decision,
        "clear_learning_decision": patch.clear_learning_decision,
        "clear_llm_decision": patch.clear_llm_decision,
    }))
}

fn confirm_patch_field_name(field: ImportPreviewPatchField) -> &'static str {
    match field {
        ImportPreviewPatchField::Date => "date",
        ImportPreviewPatchField::Type => "type",
        ImportPreviewPatchField::Amount => "amount",
        ImportPreviewPatchField::DestinationAmount => "destination_amount",
        ImportPreviewPatchField::MainCategory => "main_category",
        ImportPreviewPatchField::SubCategory => "sub_category",
        ImportPreviewPatchField::CategoryId => "category_id",
        ImportPreviewPatchField::SourceAccountId => "source_account_id",
        ImportPreviewPatchField::DestinationAccountId => "destination_account_id",
        ImportPreviewPatchField::Counterparty => "counterparty",
        ImportPreviewPatchField::PaymentMethod => "payment_method",
        ImportPreviewPatchField::Description => "description",
        ImportPreviewPatchField::RecurringId => "recurring_id",
        ImportPreviewPatchField::RecurringName => "recurring_name",
        ImportPreviewPatchField::RecurringCandidateCount => "recurring_candidate_count",
        ImportPreviewPatchField::RecurringMatchScore => "recurring_match_score",
        ImportPreviewPatchField::RecurringMatchReasons => "recurring_match_reasons",
        ImportPreviewPatchField::RecurringMatchedDate => "recurring_matched_date",
        ImportPreviewPatchField::Selected => "selected",
        ImportPreviewPatchField::ManualAnnotation => "manual_annotation",
        ImportPreviewPatchField::MatchingFeedback => "matching_feedback",
    }
}

fn confirm_patch_value_json(value: &ImportPreviewPatchValue) -> DbResult<Value> {
    match value {
        ImportPreviewPatchValue::Null => Ok(Value::Null),
        ImportPreviewPatchValue::Text(value) => Ok(Value::String(value.clone())),
        ImportPreviewPatchValue::Real(value) if value.is_finite() => Ok(json!(value)),
        ImportPreviewPatchValue::Real(_) => Err(DbError::InvalidOperation(
            "non-finite confirm patch value".to_string(),
        )),
        ImportPreviewPatchValue::Integer(value) => Ok(json!(value)),
        ImportPreviewPatchValue::Bool(value) => Ok(json!(value)),
        ImportPreviewPatchValue::Json(value) => Ok(canonical_json(value)),
    }
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut canonical = Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&object[key]));
            }
            Value::Object(canonical)
        }
        _ => value.clone(),
    }
}
