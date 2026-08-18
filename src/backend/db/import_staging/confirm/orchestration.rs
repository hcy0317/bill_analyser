use sha2::{Digest as _, Sha256};

const CONFIRM_RECEIPT_SCHEMA_VERSION: i16 = 1;
const CONFIRM_RESPONSE_SCHEMA_VERSION: i16 = 1;
const CONFIRM_HTTP_SUCCESS_STATUS: i16 = 200;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredConfirmReceipt {
    receipt_schema_version: i16,
    command_fingerprint: String,
    request_session_version: i64,
    response_schema_version: i16,
    http_status: i16,
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
                http_status: receipt.http_status as u16,
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
            return Err(DbError::import_session_version_conflict(
                &command.session_id,
                command.expected_session_version.unwrap_or_default(),
                session.version,
            ));
        }

        apply_confirm_command_mutations(&mut tx, session.id, user_id, command).await?;
        let previews =
            load_selected_preview_rows_for_confirm(&mut tx, session.id, user_id).await?;
        let unknown_state_errors = previews
            .iter()
            .filter(|preview| {
                import_preview_matching_feedback_has_unknown_signal_status(
                    &preview.preview_matching_feedback,
                )
            })
            .map(|preview| format!("preview {} has an unknown signal state", preview.id))
            .collect::<Vec<_>>();
        if !unknown_state_errors.is_empty() {
            tracing::warn!(
                domain = "import_confirm",
                operation = "validation",
                outcome = "failed",
                validation_kind = "unknown_signal_state",
                session_key = %command.session_id,
                request_session_version,
                session_version = session.version,
                validation_error_count = unknown_state_errors.len(),
                "import confirmation validation failed"
            );
            return Err(DbError::InvalidOperation(format!(
                "import preview requires review: {}",
                unknown_state_errors.join("; ")
            )));
        }
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
            http_status: CONFIRM_HTTP_SUCCESS_STATUS,
            success_envelope: response.body.clone(),
        };
        persist_confirm_receipt(
            &mut tx,
            session.id,
            user_id,
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
        DbError::ImportSessionVersionConflict { .. } => "session_version_conflict",
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
        DbError::PreviewVersionConflict { .. } => "preview_version_conflict",
        DbError::PreviewSelectionConflict { .. } => "preview_selection_conflict",
        DbError::PreviewSelectionTargetMismatch { .. } => "preview_selection_target_mismatch",
        DbError::Postgres(_) => "postgres",
        DbError::Io(_) => "io",
        DbError::UnsafePath(_) => "unsafe_path",
    }
}
