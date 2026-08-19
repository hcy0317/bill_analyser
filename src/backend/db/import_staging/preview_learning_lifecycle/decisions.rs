#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportPreviewTerminalAction {
    Apply,
    Idempotent,
    Conflict,
}

fn preview_terminal_action(
    current_status: Option<&str>,
    has_signal: bool,
    decision: ImportPreviewDecision,
) -> ImportPreviewTerminalAction {
    match current_status {
        Some("accepted") => match decision {
            ImportPreviewDecision::Accept => ImportPreviewTerminalAction::Idempotent,
            ImportPreviewDecision::Reject | ImportPreviewDecision::Clear => {
                ImportPreviewTerminalAction::Conflict
            }
        },
        Some("rejected") => match decision {
            ImportPreviewDecision::Reject => ImportPreviewTerminalAction::Idempotent,
            ImportPreviewDecision::Accept | ImportPreviewDecision::Clear => {
                ImportPreviewTerminalAction::Conflict
            }
        },
        None if !has_signal => match decision {
            ImportPreviewDecision::Clear => ImportPreviewTerminalAction::Idempotent,
            ImportPreviewDecision::Accept | ImportPreviewDecision::Reject => {
                ImportPreviewTerminalAction::Conflict
            }
        },
        _ => ImportPreviewTerminalAction::Apply,
    }
}

/// 应用 learning 建议的接受/拒绝决策，负责状态冲突检测和 preview patch 落库。
pub fn apply_preview_learning_decision(
    pool: &PostgresPool,
    session_id: &str,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    applied_result: Option<&ImportPreviewLearningApply>,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    let decision_name = match decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    };
    tracing::debug!(
        domain = "import_signal_lifecycle",
        operation = "lifecycle_transition",
        signal_family = "learning",
        outcome = "started",
        session_key = %session_id,
        decision = decision_name,
        "import preview signal lifecycle transition started"
    );
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut transaction = pool.begin().await?;
        let Some(row) = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            FOR UPDATE OF p
            "#,
        )
        .bind(preview_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?
        else {
            transaction.rollback().await?;
            tracing::debug!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "learning",
                outcome = "not_found",
                session_key = %session_id,
                decision = decision_name,
                "import preview signal lifecycle transition not applied"
            );
            return Ok(preview_decision_not_found());
        };
        let preview = preview_from_pg_row(&row)?;
        if preview.session_id != session_id {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "learning",
                outcome = "session_conflict",
                session_key = %session_id,
                decision = decision_name,
                "import preview signal lifecycle transition rejected"
            );
            return Ok(preview_decision_not_found());
        }
        let current_status = preview_learning_review_status(&preview.preview_matching_feedback);
        match preview_terminal_action(
            current_status.as_deref(),
            preview.preview_matching_feedback.get("learning").is_some(),
            decision,
        ) {
            ImportPreviewTerminalAction::Idempotent => {
                transaction.commit().await?;
                tracing::info!(
                    domain = "import_signal_lifecycle",
                    operation = "lifecycle_transition",
                    signal_family = "learning",
                    outcome = "idempotent",
                    session_key = %session_id,
                    decision = decision_name,
                    previous_state = current_status.as_deref().unwrap_or("none"),
                    next_state = current_status.as_deref().unwrap_or("none"),
                    "import preview signal lifecycle transition completed"
                );
                return Ok(ImportPreviewDecisionResult {
                    preview: Some(preview),
                    state_conflict: false,
                    invalid_recurring_id: false,
                });
            }
            ImportPreviewTerminalAction::Conflict => {
                transaction.rollback().await?;
                tracing::warn!(
                    domain = "import_signal_lifecycle",
                    operation = "lifecycle_transition",
                    signal_family = "learning",
                    outcome = "state_conflict",
                    session_key = %session_id,
                    decision = decision_name,
                    previous_state = current_status.as_deref().unwrap_or("none"),
                    "import preview signal lifecycle transition rejected"
                );
                return Ok(preview_decision_state_conflict());
            }
            ImportPreviewTerminalAction::Apply => {}
        }
        if let Some(expected_row_version) = expected_state
            .and_then(|expected| expected.expected_row_version)
            .filter(|expected| *expected != preview.version)
        {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "learning",
                outcome = "row_version_conflict",
                session_key = %session_id,
                decision = decision_name,
                expected_row_version,
                actual_row_version = preview.version,
                "import preview signal lifecycle transition rejected"
            );
            return Err(DbError::preview_version_conflict(
                preview_id,
                expected_row_version,
                preview.version,
            ));
        }
        if recommendation_expected_state_conflicts(
            &preview,
            expected_state,
            current_status.as_deref(),
        ) {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "learning",
                outcome = "expected_state_conflict",
                session_key = %session_id,
                decision = decision_name,
                previous_state = current_status.as_deref().unwrap_or("none"),
                "import preview signal lifecycle transition rejected"
            );
            return Ok(preview_decision_state_conflict());
        }

        let lifecycle_input = preview_learning_lifecycle_input(&preview, decision);
        let lifecycle_transition = match lifecycle_input.as_ref() {
            Some(input) => Some(
                load_import_learning_lifecycle_transition_in_transaction(
                    &mut transaction,
                    user_id,
                    input,
                )
                .await?,
            ),
            None => None,
        };
        let mut patch = ImportPreviewPatch::new(preview_id);
        match decision {
            ImportPreviewDecision::Accept => {
                if let Some(applied) = applied_result {
                    patch = learning_accept_patch_fields(patch, applied);
                }
                let feedback = learning_feedback_after_decision(
                    preview.preview_matching_feedback.clone(),
                    "accepted",
                    lifecycle_transition.as_ref(),
                );
                patch = patch.with_change(
                    ImportPreviewPatchField::MatchingFeedback,
                    ImportPreviewPatchValue::Json(feedback),
                );
            }
            ImportPreviewDecision::Reject => {
                let feedback = learning_feedback_after_decision(
                    preview.preview_matching_feedback.clone(),
                    "rejected",
                    lifecycle_transition.as_ref(),
                );
                patch = patch.with_change(
                    ImportPreviewPatchField::MatchingFeedback,
                    ImportPreviewPatchValue::Json(feedback),
                );
            }
            ImportPreviewDecision::Clear => {
                patch = patch.with_learning_decision_cleared();
            }
        }

        let session_db_id: i64 = row.try_get("session_id")?;
        let payload = row
            .try_get::<Value, _>("preview_payload")
            .unwrap_or_else(|_| json!({}));
        let identity_maps =
            load_import_identity_maps_in_transaction(&mut transaction, user_id).await?;
        let projection = project_preview_patch(preview, payload, &patch, &identity_maps)?;
        let mut update = build_preview_row_update_query(
            &projection.preview,
            projection.payload.to_string(),
            projection.signal_projection,
            projection.amount_cents,
            projection.direction,
            PreviewRowUpdateTarget {
                preview_id,
                session_db_id,
                user_id,
                expected_row_version: expected_state
                    .and_then(|expected| expected.expected_row_version),
            },
        );
        update.build().execute(&mut *transaction).await?;
        observe_import_preview_signal_projection_parity(
            &mut transaction,
            &[preview_id],
            "learning_decision",
        )
        .await?;

        if let (Some(input), Some(next)) = (lifecycle_input.as_ref(), lifecycle_transition.as_ref())
        {
            persist_import_learning_lifecycle_transition_in_transaction(
                &mut transaction,
                user_id,
                input,
                next,
            )
            .await?;
        }
        let updated_row = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            "#,
        )
        .bind(preview_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let updated = updated_row.as_ref().map(preview_from_pg_row).transpose()?;
        transaction.commit().await?;
        let next_state = lifecycle_transition
            .as_ref()
            .map(|transition| transition.next_status.as_str())
            .unwrap_or(match decision {
                ImportPreviewDecision::Accept => "accepted",
                ImportPreviewDecision::Reject => "rejected",
                ImportPreviewDecision::Clear => "cleared",
            });
        tracing::info!(
            domain = "import_signal_lifecycle",
            operation = "lifecycle_transition",
            signal_family = "learning",
            outcome = "committed",
            session_key = %session_id,
            decision = decision_name,
            previous_state = current_status.as_deref().unwrap_or("none"),
            next_state,
            "import preview signal lifecycle transition completed"
        );
        Ok(ImportPreviewDecisionResult {
            preview: updated,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

fn learning_accept_patch_fields(
    mut patch: ImportPreviewPatch,
    applied: &ImportPreviewLearningApply,
) -> ImportPreviewPatch {
    if let Some(value) = &applied.preview_type {
        patch = patch.with_change(
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(value.clone()),
        );
    }
    if let Some(value) = &applied.preview_main_category {
        patch = patch.with_change(
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(value.clone()),
        );
    }
    if let Some(value) = &applied.preview_sub_category {
        patch = patch.with_change(
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(value.clone()),
        );
    }
    if let Some(value) = applied.preview_source_account_id {
        patch = patch.with_change(
            ImportPreviewPatchField::SourceAccountId,
            value.map_or(
                ImportPreviewPatchValue::Null,
                ImportPreviewPatchValue::Integer,
            ),
        );
    }
    if let Some(value) = applied.preview_destination_account_id {
        patch = patch.with_change(
            ImportPreviewPatchField::DestinationAccountId,
            value.map_or(
                ImportPreviewPatchValue::Null,
                ImportPreviewPatchValue::Integer,
            ),
        );
    }
    patch
}

fn recommendation_expected_state_conflicts(
    preview: &ImportPreviewRow,
    expected: Option<&ImportPreviewExpectedState>,
    current_review_status: Option<&str>,
) -> bool {
    if expected_state_conflicts(preview, expected) {
        return true;
    }
    let Some(expected) = expected else {
        return false;
    };
    expected.expected_row_version.is_some_and(|value| value != preview.version)
        || expected
        .review_status
        .as_deref()
        .is_some_and(|value| {
            !value
                .trim()
                .eq_ignore_ascii_case(current_review_status.unwrap_or_default())
        })
        || expected
            .preview_category_id
            .is_some_and(|value| value != preview.category_id)
        || expected
        .preview_recurring_id
        .is_some_and(|value| value != preview.preview_recurring_id)
        || expected
            .preview_source_account_id
            .is_some_and(|value| value != preview.preview_source_account_id)
        || expected
            .preview_destination_account_id
            .is_some_and(|value| value != preview.preview_destination_account_id)
        || expected
            .preview_matching_feedback
            .as_ref()
            .is_some_and(|value| value != &preview.preview_matching_feedback)
}

fn preview_learning_review_status(feedback: &Value) -> Option<String> {
    feedback
        .get("learning")
        .and_then(Value::as_object)
        .and_then(|learning| {
            [
                "review_status",
                "status",
                "lifecycle_status",
                "signal_state",
            ]
            .iter()
            .find_map(|key| {
                learning
                    .get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_ascii_lowercase())
            })
        })
}

#[cfg(test)]
fn learning_decision_is_idempotent(
    current_status: Option<&str>,
    has_learning: bool,
    decision: ImportPreviewDecision,
) -> bool {
    preview_terminal_action(current_status, has_learning, decision)
        == ImportPreviewTerminalAction::Idempotent
}

fn preview_learning_lifecycle_input(
    preview: &ImportPreviewRow,
    decision: ImportPreviewDecision,
) -> Option<ImportLearningLifecycleRecordInput> {
    let learning = preview
        .preview_matching_feedback
        .get("learning")?
        .as_object()?;
    let feedback = match decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => return None,
    };
    let recommendation_key = learning
        .get("recommendation_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("preview:{}:learning", preview.id));
    Some(ImportLearningLifecycleRecordInput {
        recommendation_key,
        recommendation_type: "import_preview".to_string(),
        feedback: feedback.to_string(),
        rule_id: learning.get("rule_id").and_then(Value::as_i64),
        suggestion_id: None,
        session_id: Some(preview.session_id.clone()),
        preview_id: Some(preview.id),
        bill_id: None,
        candidate_id: Some(format!("preview:{}:learning", preview.id)),
        payload_json: Some("{}".to_string()),
    })
}

fn learning_feedback_after_decision(
    mut feedback: Value,
    review_status: &str,
    transition: Option<&bill_analyser_core::ImportLearningLifecycleTransition>,
) -> Value {
    feedback = set_feedback_review_status(feedback, "learning", review_status);
    let Some(learning) = feedback.get_mut("learning").and_then(Value::as_object_mut) else {
        return feedback;
    };
    if let Some(transition) = transition {
        learning.insert(
            "lifecycle_status".to_string(),
            json!(transition.next_status),
        );
        learning.insert("signal_state".to_string(), json!(transition.signal_state));
        learning.insert(
            "accepted_count".to_string(),
            json!(transition.accepted_count),
        );
        learning.insert(
            "rejected_count".to_string(),
            json!(transition.rejected_count),
        );
        learning.insert(
            "auto_applied_count".to_string(),
            json!(transition.auto_applied_count),
        );
        learning.insert(
            "auto_apply".to_string(),
            json!(transition.auto_apply_enabled),
        );
        learning.insert("suppressed".to_string(), json!(transition.suppressed));
    }
    feedback
}
