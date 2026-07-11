use sha2::{Digest as _, Sha256};

const CONFIRM_RECEIPT_SCHEMA_VERSION: i64 = 1;
const CONFIRM_RESPONSE_SCHEMA_VERSION: i64 = 1;
const CONFIRM_HTTP_SUCCESS_STATUS: i64 = 200;

#[derive(Debug)]
struct LockedImportSession {
    id: i64,
    status: String,
    metadata: Value,
    version: i64,
}

#[derive(Debug, Clone)]
struct HistoryConfirmPlan {
    preview_id: i64,
    operation: bill_analyser_core::ImportHistoryRewriteOperation,
    operation_id: String,
    history_bill_id: i64,
    history_bill_version: i64,
    acknowledgement_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredConfirmReceipt {
    receipt_schema_version: i64,
    command_fingerprint: String,
    request_session_version: i64,
    response_schema_version: i64,
    http_status: i64,
    success_envelope: Value,
}

/// 兼容无历史改写 ack 的 confirm 入口，实际写入委托给带 ack 的事务实现。
pub fn confirm_preview_to_bills(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ConfirmPreviewResult> {
    confirm_preview_to_bills_with_ack(pool, session_id, user_id, None)
}

/// 锁定 user-scoped session 后，在同一事务内执行 ack/CAS、正式写入、回执和子表清理。
#[tracing::instrument(level = "debug", skip_all, fields(session_id = session_id))]
pub fn confirm_preview_to_bills_with_ack(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    history_acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> DbResult<ConfirmPreviewResult> {
    let receipt = confirm_import_command(
        pool,
        user_id,
        &ConfirmCommand {
            session_id: session_id.to_string(),
            expected_session_version: None,
            preview_patches: Vec::new(),
            selected_preview_ids: None,
            preserve_unpatched_selection: true,
            history_acknowledgement: history_acknowledgement.cloned(),
            declared_confirm_time_effects: Vec::new(),
        },
    )?;
    confirm_preview_result_from_envelope(&receipt.success_envelope)
}

/// Canonical HTTP-to-repository confirm entrypoint. The command is consumed by the locked
/// transaction; unsupported effect variants are rejected before any mutation.
#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(session_key = %command.session_id)
)]
pub fn confirm_import_command(
    pool: &PostgresPool,
    user_id: UserId,
    command: &ConfirmCommand,
) -> DbResult<ConfirmReceiptResponse> {
    tracing::info!(
        domain = "import_confirm",
        operation = "confirm",
        outcome = "started",
        session_key = %command.session_id,
        request_session_version = ?command.expected_session_version,
        response_schema_version = CONFIRM_RESPONSE_SCHEMA_VERSION,
        "import confirmation started"
    );
    let result = confirm_import_command_in_transaction(pool, user_id, command);
    match &result {
        Ok(receipt) => tracing::info!(
            domain = "import_confirm",
            operation = "confirm",
            outcome = if receipt.replayed { "replayed" } else { "committed" },
            session_key = %command.session_id,
            request_session_version = ?command.expected_session_version,
            response_schema_version = CONFIRM_RESPONSE_SCHEMA_VERSION,
            http_status = receipt.http_status,
            "import confirmation transaction completed"
        ),
        Err(error) => tracing::warn!(
            domain = "import_confirm",
            operation = "confirm",
            outcome = "rolled_back",
            session_key = %command.session_id,
            request_session_version = ?command.expected_session_version,
            rollback_reason = confirm_rollback_reason(error),
            "import confirmation transaction rolled back"
        ),
    }
    result
}

fn confirm_import_command_in_transaction(
    pool: &PostgresPool,
    user_id: UserId,
    command: &ConfirmCommand,
) -> DbResult<ConfirmReceiptResponse> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session =
            lock_import_session_for_confirm(&mut tx, &command.session_id, user_id).await?;
        let receipt = if session.status == "confirmed" {
            Some(stored_confirm_receipt(&session.metadata)?)
        } else {
            None
        };
        let request_session_version = command.expected_session_version.unwrap_or_else(|| {
            receipt
                .as_ref()
                .map(|receipt| receipt.request_session_version)
                .unwrap_or(session.version)
        });
        let command_fingerprint =
            confirm_command_fingerprint(command, request_session_version)?;
        tracing::debug!(
            domain = "import_confirm",
            operation = "fingerprint_classification",
            outcome = if receipt.is_some() { "terminal" } else { "new" },
            session_key = %command.session_id,
            request_session_version,
            session_version = session.version,
            "import confirmation fingerprint classified"
        );

        if let Some(receipt) = receipt {
            if receipt.command_fingerprint != command_fingerprint {
                tracing::warn!(
                    domain = "import_confirm",
                    operation = "fingerprint_classification",
                    outcome = "fingerprint_conflict",
                    session_key = %command.session_id,
                    request_session_version,
                    session_version = session.version,
                    "terminal import confirmation rejected a different command"
                );
                return Err(DbError::InvalidOperation(
                    "import confirm command fingerprint conflict".to_string(),
                ));
            }
            if receipt.http_status != CONFIRM_HTTP_SUCCESS_STATUS
                || receipt.response_schema_version != CONFIRM_RESPONSE_SCHEMA_VERSION
            {
                return Err(DbError::InvalidOperation(
                    "unsupported import confirm receipt response".to_string(),
                ));
            }
            tx.commit().await?;
            tracing::info!(
                domain = "import_confirm",
                operation = "fingerprint_classification",
                outcome = "replay",
                session_key = %command.session_id,
                request_session_version,
                session_version = session.version,
                "terminal import confirmation replayed stored receipt"
            );
            return Ok(ConfirmReceiptResponse {
                http_status: u16::try_from(receipt.http_status).unwrap_or(200),
                success_envelope: receipt.success_envelope,
                replayed: true,
            });
        }
        if command
            .expected_session_version
            .is_some_and(|expected| expected != session.version)
        {
            tracing::warn!(
                domain = "import_confirm",
                operation = "validation",
                outcome = "failed",
                validation_kind = "session_version",
                session_key = %command.session_id,
                request_session_version,
                session_version = session.version,
                "import confirmation validation failed"
            );
            return Err(DbError::InvalidOperation(format!(
                "import session version conflict: expected {}, current {}",
                command.expected_session_version.unwrap_or_default(),
                session.version
            )));
        }

        apply_confirm_command_mutations(&mut tx, session.id, user_id, command).await?;
        let previews =
            load_selected_preview_rows_for_confirm(&mut tx, session.id, user_id).await?;
        let history_plans = match validate_history_acknowledgement(
            &command.session_id,
            &previews,
            command.history_acknowledgement.as_ref(),
        ) {
            Ok(plans) => plans,
            Err(error) => {
                tracing::warn!(
                    domain = "import_confirm",
                    operation = "validation",
                    outcome = "failed",
                    validation_kind = "history_acknowledgement",
                    session_key = %command.session_id,
                    request_session_version,
                    session_version = session.version,
                    "import confirmation validation failed"
                );
                return Err(error);
            }
        };
        let history_preview_ids = history_plans
            .iter()
            .map(|plan| plan.preview_id)
            .collect::<BTreeSet<_>>();
        let identity_maps = load_import_identity_maps_for_confirm(&mut tx, user_id).await?;
        let identity_errors = previews
            .iter()
            .flat_map(|preview| preview_identity_error_messages(preview, &identity_maps))
            .collect::<Vec<_>>();
        if !identity_errors.is_empty() {
            tracing::warn!(
                domain = "import_confirm",
                operation = "validation",
                outcome = "failed",
                validation_kind = "identity",
                session_key = %command.session_id,
                request_session_version,
                session_version = session.version,
                validation_error_count = identity_errors.len(),
                "import confirmation validation failed"
            );
            return Err(DbError::InvalidOperation(format!(
                "import preview identity validation failed: {}",
                identity_errors.join("; ")
            )));
        }
        let review_errors = previews
            .iter()
            .filter(|preview| {
                preview_requires_review(preview) && !history_preview_ids.contains(&preview.id)
            })
            .map(|preview| format!("preview {} requires review before confirm", preview.id))
            .collect::<Vec<_>>();
        if !review_errors.is_empty() {
            tracing::warn!(
                domain = "import_confirm",
                operation = "validation",
                outcome = "failed",
                validation_kind = "review_state",
                session_key = %command.session_id,
                request_session_version,
                session_version = session.version,
                validation_error_count = review_errors.len(),
                "import confirmation validation failed"
            );
            return Err(DbError::InvalidOperation(format!(
                "import preview requires review: {}",
                review_errors.join("; ")
            )));
        }

        let previews_by_id = previews
            .iter()
            .map(|preview| (preview.id, preview))
            .collect::<BTreeMap<_, _>>();
        for plan in &history_plans {
            let preview = previews_by_id.get(&plan.preview_id).ok_or_else(|| {
                DbError::InvalidOperation("history acknowledgement preview not selected".to_string())
            })?;
            apply_history_confirm_plan(&mut tx, user_id, preview, plan).await?;
            tracing::debug!(
                domain = "import_confirm",
                operation = plan.operation.as_str(),
                outcome = "cas_applied",
                session_key = %command.session_id,
                request_session_version,
                session_version = session.version,
                request_history_version = plan.history_bill_version,
                "history confirmation operation CAS completed"
            );
        }
        let drafts = previews
            .iter()
            .filter(|preview| !history_preview_ids.contains(&preview.id))
            .map(|preview| BillCreateDraft {
                fields: bill_create_fields_from_preview(preview),
                tag_ids: Vec::new(),
            })
            .collect::<Vec<_>>();
        let created_bill_ids =
            batch_create_postgres_bills_in_transaction(pool, &mut tx, user_id, &drafts).await?;
        apply_confirm_time_effect_boundary(&mut tx, &command.declared_confirm_time_effects)
            .await?;
        let result = ConfirmPreviewResult {
            confirmed_count: created_bill_ids.len() + history_plans.len(),
            skipped_count: 0,
            duplicate_count: 0,
            errors: Vec::new(),
        };
        let response = bill_analyser_core::import_stage_confirm_success(
            bill_analyser_core::ImportStageConfirmData {
                imported_count: result.confirmed_count,
                skipped_count: result.skipped_count + result.duplicate_count,
                errors: result.errors.clone(),
            },
        );
        let receipt = StoredConfirmReceipt {
            receipt_schema_version: CONFIRM_RECEIPT_SCHEMA_VERSION,
            command_fingerprint,
            request_session_version: session.version,
            response_schema_version: CONFIRM_RESPONSE_SCHEMA_VERSION,
            http_status: i64::from(response.status_code),
            success_envelope: response.body.clone(),
        };
        persist_confirm_receipt(
            &mut tx,
            session.id,
            user_id,
            session.version,
            &receipt,
            result.confirmed_count,
        )
        .await?;
        tracing::debug!(
            domain = "import_confirm",
            operation = "receipt_persistence",
            outcome = "persisted",
            session_key = %command.session_id,
            request_session_version,
            session_version = session.version,
            receipt_schema_version = CONFIRM_RECEIPT_SCHEMA_VERSION,
            response_schema_version = CONFIRM_RESPONSE_SCHEMA_VERSION,
            "import confirmation receipt persisted"
        );
        clear_import_session_child_data_on_tx(
            &mut tx,
            user_id,
            session.id,
            &command.session_id,
        )
        .await?;
        tracing::debug!(
            domain = "import_confirm",
            operation = "child_cleanup",
            outcome = "completed",
            session_key = %command.session_id,
            request_session_version,
            session_version = session.version,
            "import confirmation child staging data cleared"
        );
        tx.commit().await?;
        Ok(ConfirmReceiptResponse {
            http_status: response.status_code,
            success_envelope: response.body,
            replayed: false,
        })
    })
}

fn confirm_rollback_reason(error: &DbError) -> &'static str {
    match error {
        DbError::InvalidOperation(message) if message.contains("fingerprint conflict") => {
            "fingerprint_conflict"
        }
        DbError::InvalidOperation(message) if message.contains("version conflict") => {
            "session_version_conflict"
        }
        DbError::InvalidOperation(message)
            if message.contains("identity validation") || message.contains("requires review") =>
        {
            "preview_validation"
        }
        DbError::InvalidOperation(message) if message.contains("history") => {
            "history_operation"
        }
        DbError::InvalidOperation(message) if message.contains("receipt") => {
            "receipt_persistence"
        }
        DbError::InvalidOperation(_) => "invalid_operation",
        DbError::Postgres(_) => "postgres",
        DbError::Io(_) => "io",
        DbError::UnsafePath(_) => "unsafe_path",
    }
}

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
        apply_patch_value_to_preview(&mut preview, &mut payload, *field, value.clone());
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

fn confirm_transaction_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "income" | "收入" | "2" => "income",
        "transfer" | "转账" | "4" => "transfer",
        "investment" | "投资" | "5" => "investment",
        _ => "expense",
    }
    .to_string()
}

fn confirm_balance_deltas(
    transaction_type: &str,
    direction: &str,
    amount_cents: i64,
    destination_amount_cents: Option<i64>,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
) -> Vec<(i64, i64)> {
    let amount = amount_cents.saturating_abs();
    let destination_amount = destination_amount_cents
        .unwrap_or(amount)
        .saturating_abs();
    match transaction_type {
        "income" => source_account_id
            .filter(|value| *value > 0)
            .map(|account_id| vec![(account_id, amount)])
            .unwrap_or_default(),
        "transfer" | "investment" => {
            let mut deltas = Vec::new();
            if let Some(account_id) = source_account_id.filter(|value| *value > 0) {
                deltas.push((account_id, -amount));
            }
            if let Some(account_id) = destination_account_id.filter(|value| *value > 0) {
                deltas.push((account_id, destination_amount));
            }
            deltas
        }
        _ => source_account_id
            .filter(|value| *value > 0)
            .map(|account_id| {
                vec![(
                    account_id,
                    if direction == "income" { amount } else { -amount },
                )]
            })
            .unwrap_or_default(),
    }
}

async fn apply_confirm_balance_delta_difference(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    old_deltas: &[(i64, i64)],
    new_deltas: &[(i64, i64)],
) -> DbResult<()> {
    let mut combined = BTreeMap::<i64, i64>::new();
    for (account_id, delta) in old_deltas {
        *combined.entry(*account_id).or_default() -= *delta;
    }
    for (account_id, delta) in new_deltas {
        *combined.entry(*account_id).or_default() += *delta;
    }
    for (account_id, delta) in combined {
        if delta == 0 {
            continue;
        }
        let changed = sqlx::query(
            r#"
            UPDATE accounts
            SET balance_cents = balance_cents + $3,
                updated_at = now(),
                version = version + 1
            WHERE user_id = $1 AND id = $2 AND is_active = true
            "#,
        )
        .bind(user_id)
        .bind(account_id)
        .bind(delta)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(DbError::InvalidOperation(format!(
                "history account CAS mismatch: {account_id}"
            )));
        }
    }
    Ok(())
}

async fn persist_confirm_receipt(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    request_session_version: i64,
    receipt: &StoredConfirmReceipt,
    confirmed_count: usize,
) -> DbResult<()> {
    let receipt = serde_json::to_value(receipt).map_err(|error| {
        DbError::InvalidOperation(format!("failed to serialize confirm receipt: {error}"))
    })?;
    let changed = sqlx::query(
        r#"
        UPDATE import_sessions
        SET status = 'confirmed',
            total_confirmed = $4,
            metadata = jsonb_set(metadata, '{confirm_receipt}', $5::jsonb, true),
            updated_at = now(),
            version = version + 1
        WHERE id = $1 AND user_id = $2 AND version = $3 AND status <> 'confirmed'
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .bind(request_session_version)
    .bind(i64::try_from(confirmed_count).unwrap_or(i64::MAX))
    .bind(receipt.to_string())
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(DbError::InvalidOperation(
            "import session confirm CAS mismatch".to_string(),
        ));
    }
    Ok(())
}

async fn load_selected_preview_rows_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<Vec<ImportPreviewRow>> {
    let rows = sqlx::query(
        r#"
        SELECT p.*, s.session_key
        FROM import_preview_rows p
        JOIN import_sessions s ON s.id = p.session_id
        WHERE p.session_id = $1 AND p.user_id = $2 AND p.selected = true
        ORDER BY p.occurred_at ASC, p.id ASC
        FOR UPDATE OF p
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(preview_from_pg_row).collect()
}

async fn load_import_identity_maps_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(&mut **tx)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut maps = ImportIdentityMaps::default();
    for row in account_rows {
        maps.active_accounts.insert(row.try_get("id")?);
    }
    for row in category_rows {
        let id = row.try_get::<i64, _>("id")?;
        let category_type = row
            .try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .and_then(preview_category_type_code);
        maps.active_categories.insert(id, category_type);
    }
    Ok(maps)
}

fn bill_create_fields_from_preview(preview: &ImportPreviewRow) -> BillRecord {
    let mut fields = Map::new();
    fields.insert("date".to_string(), json!(preview.preview_date));
    fields.insert("type".to_string(), json!(preview.preview_type));
    fields.insert(
        "amount_cents".to_string(),
        json!(preview.preview_amount_cents),
    );
    fields.insert(
        "destination_amount_cents".to_string(),
        json!(preview.preview_destination_amount_cents),
    );
    fields.insert(
        "counterparty".to_string(),
        json!(preview.preview_counterparty),
    );
    fields.insert(
        "description".to_string(),
        json!(preview.preview_description),
    );
    fields.insert(
        "payment_method".to_string(),
        json!(preview.preview_payment_method),
    );
    if let Some(value) = preview.category_id {
        fields.insert("category_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_source_account_id {
        fields.insert("source_account_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_destination_account_id {
        fields.insert("destination_account_id".to_string(), json!(value));
    }
    fields
}
