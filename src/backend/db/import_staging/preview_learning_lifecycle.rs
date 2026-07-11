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
        let mut preview = preview_from_pg_row(&row)?;
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
        let mut payload = row
            .try_get::<Value, _>("preview_payload")
            .unwrap_or_else(|_| json!({}));
        for (field, value) in &patch.changes {
            apply_patch_value_to_preview(&mut preview, &mut payload, *field, value.clone());
        }
        if patch.clear_learning_decision {
            clear_feedback_key(&mut preview.preview_matching_feedback, "learning");
        }
        let identity_maps =
            load_import_identity_maps_in_transaction(&mut transaction, user_id).await?;
        apply_identity_validation_to_preview(&mut preview, &mut payload, &identity_maps);
        payload_set(
            &mut payload,
            "preview_matching_feedback",
            preview.preview_matching_feedback.clone(),
        );
        let amount_cents = preview.preview_amount_cents.abs();
        let direction = if preview.preview_type == "收入" || preview.preview_type == "income" {
            "income"
        } else {
            "expense"
        };
        let mut update = build_preview_row_update_query(
            &preview,
            payload.to_string(),
            amount_cents,
            direction,
            preview_id,
            session_db_id,
            user_id,
        );
        update.build().execute(&mut *transaction).await?;

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
    expected
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

async fn load_import_identity_maps_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(&mut **transaction)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
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

async fn load_import_learning_lifecycle_transition_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<bill_analyser_core::ImportLearningLifecycleTransition> {
    let feedback = bill_analyser_core::import_learning_lifecycle::normalize_import_learning_lifecycle_feedback(
        &input.feedback,
    )
    .ok_or_else(|| DbError::InvalidOperation("Invalid learning lifecycle feedback".to_string()))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
        .bind(&input.recommendation_key)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
    let row = sqlx::query(
        r#"
        SELECT status, accepted_count, rejected_count, auto_applied_count
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND recommendation_key = $2
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(&input.recommendation_key)
    .fetch_optional(&mut **transaction)
    .await?;
    let state = match row {
        Some(row) => ImportLearningLifecycleState {
            status: row.try_get("status")?,
            accepted_count: row.try_get::<i32, _>("accepted_count")? as i64,
            rejected_count: row.try_get::<i32, _>("rejected_count")? as i64,
            auto_applied_count: row.try_get::<i32, _>("auto_applied_count")? as i64,
        },
        None => ImportLearningLifecycleState::default(),
    };
    Ok(transition_import_learning_lifecycle(&state, feedback))
}

async fn persist_import_learning_lifecycle_transition_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
    next: &bill_analyser_core::ImportLearningLifecycleTransition,
) -> DbResult<()> {
    sqlx::query(
        r#"
        INSERT INTO import_learning_lifecycle (
            user_id, recommendation_key, recommendation_type, status,
            accepted_count, rejected_count, auto_applied_count, auto_apply_enabled,
            metadata, last_feedback_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, now(), now(), now())
        ON CONFLICT (user_id, recommendation_key) DO UPDATE SET
            recommendation_type = excluded.recommendation_type,
            status = excluded.status,
            accepted_count = excluded.accepted_count,
            rejected_count = excluded.rejected_count,
            auto_applied_count = excluded.auto_applied_count,
            auto_apply_enabled = excluded.auto_apply_enabled,
            last_feedback_at = now(),
            updated_at = now(),
            version = import_learning_lifecycle.version + 1
        "#,
    )
    .bind(user_id)
    .bind(&input.recommendation_key)
    .bind(&input.recommendation_type)
    .bind(&next.next_status)
    .bind(next.accepted_count)
    .bind(next.rejected_count)
    .bind(next.auto_applied_count)
    .bind(next.auto_apply_enabled)
    .execute(&mut **transaction)
    .await?;
    let lifecycle_id = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT id FROM import_learning_lifecycle WHERE user_id = $1 AND recommendation_key = $2",
    )
    .bind(user_id)
    .bind(&input.recommendation_key)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or_default();
    sqlx::query(
        r#"
        INSERT INTO import_learning_feedback_events (
            user_id, rule_id, suggestion_id, lifecycle_id, recommendation_key,
            event_type, session_id, preview_id, bill_id, candidate_id,
            previous_signal_state, next_signal_state, payload, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::jsonb,now())
        "#,
    )
    .bind(user_id)
    .bind(input.rule_id)
    .bind(input.suggestion_id)
    .bind(lifecycle_id)
    .bind(&input.recommendation_key)
    .bind(&next.event_type)
    .bind(&input.session_id)
    .bind(input.preview_id)
    .bind(input.bill_id)
    .bind(&input.candidate_id)
    .bind(&next.previous_status)
    .bind(&next.signal_state)
    .bind(input.payload_json.as_deref().unwrap_or("{}"))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ImportPreviewLlmCategoryMatch {
    id: i64,
    type_code: Option<i64>,
    main_category: String,
    sub_category: String,
}

/// 记录 learning 生命周期反馈，供后续自动应用 eligibility 与召回质量判断。
pub fn record_import_learning_lifecycle_feedback(
    pool: &PostgresPool,
    user_id: UserId,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<ImportLearningLifecycleView> {
    let feedback = bill_analyser_core::import_learning_lifecycle::normalize_import_learning_lifecycle_feedback(
        &input.feedback,
    )
    .ok_or_else(|| DbError::InvalidOperation("Invalid learning lifecycle feedback".to_string()))?;
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let existing =
            get_import_learning_lifecycle_view_async(pool, user_id, &input.recommendation_key)
                .await?;
        let state = ImportLearningLifecycleState {
            status: existing
                .as_ref()
                .map(|item| item.status.clone())
                .unwrap_or_else(|| "yellow".to_string()),
            accepted_count: existing
                .as_ref()
                .map(|item| item.accepted_count)
                .unwrap_or(0),
            rejected_count: existing
                .as_ref()
                .map(|item| item.rejected_count)
                .unwrap_or(0),
            auto_applied_count: existing
                .as_ref()
                .map(|item| item.auto_applied_count)
                .unwrap_or(0),
        };
        let next = transition_import_learning_lifecycle(&state, feedback);
        let signal_state = learning_lifecycle_signal_state(&next.next_status);
        sqlx::query(
            r#"
            INSERT INTO import_learning_lifecycle (
                user_id, recommendation_key, recommendation_type, status,
                accepted_count, rejected_count, auto_applied_count, auto_apply_enabled,
                metadata, last_feedback_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, now(), now(), now())
            ON CONFLICT (user_id, recommendation_key) DO UPDATE SET
                recommendation_type = excluded.recommendation_type,
                status = excluded.status,
                accepted_count = excluded.accepted_count,
                rejected_count = excluded.rejected_count,
                auto_applied_count = excluded.auto_applied_count,
                auto_apply_enabled = excluded.auto_apply_enabled,
                last_feedback_at = now(),
                updated_at = now(),
                version = import_learning_lifecycle.version + 1
            "#,
        )
        .bind(user_id)
        .bind(&input.recommendation_key)
        .bind(&input.recommendation_type)
        .bind(&next.next_status)
        .bind(next.accepted_count)
        .bind(next.rejected_count)
        .bind(next.auto_applied_count)
        .bind(learning_lifecycle_is_auto_eligible(&next.next_status))
        .execute(pool)
        .await?;
        let lifecycle_id = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT id FROM import_learning_lifecycle WHERE user_id = $1 AND recommendation_key = $2",
        )
        .bind(user_id)
        .bind(&input.recommendation_key)
        .fetch_one(pool)
        .await?
        .unwrap_or_default();
        sqlx::query(
            r#"
            INSERT INTO import_learning_feedback_events (
                user_id, rule_id, suggestion_id, lifecycle_id, recommendation_key,
                event_type, session_id, preview_id, bill_id, candidate_id,
                previous_signal_state, next_signal_state, payload, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::jsonb,now())
            "#,
        )
        .bind(user_id)
        .bind(input.rule_id)
        .bind(input.suggestion_id)
        .bind(lifecycle_id)
        .bind(&input.recommendation_key)
        .bind(&next.event_type)
        .bind(&input.session_id)
        .bind(input.preview_id)
        .bind(input.bill_id)
        .bind(&input.candidate_id)
        .bind(state.status)
        .bind(signal_state)
        .bind(input.payload_json.as_deref().unwrap_or("{}"))
        .execute(pool)
        .await?;
        get_import_learning_lifecycle_view_async(pool, user_id, &input.recommendation_key)
            .await?
            .ok_or_else(|| {
                DbError::InvalidOperation("learning lifecycle was not persisted".to_string())
            })
    })
}

/// 读取 learning 生命周期视图，HTTP 层用它构建可解释的学习建议状态。
pub fn get_import_learning_lifecycle_view(
    pool: &PostgresPool,
    user_id: UserId,
    recommendation_key: &str,
) -> DbResult<Option<ImportLearningLifecycleView>> {
    block_on_db(async move {
        get_import_learning_lifecycle_view_async(pool, user_id_i64(user_id)?, recommendation_key)
            .await
    })
}

async fn get_import_learning_lifecycle_view_async(
    pool: &PostgresPool,
    user_id: i64,
    recommendation_key: &str,
) -> DbResult<Option<ImportLearningLifecycleView>> {
    let row = sqlx::query(
        r#"
        SELECT recommendation_key, recommendation_type, status, accepted_count,
               rejected_count, auto_applied_count, auto_apply_enabled, suppressed_until
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND recommendation_key = $2
        "#,
    )
    .bind(user_id)
    .bind(recommendation_key)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let status: String = row.try_get("status")?;
        let state = ImportLearningLifecycleState {
            status: status.clone(),
            accepted_count: row.try_get::<i32, _>("accepted_count")? as i64,
            rejected_count: row.try_get::<i32, _>("rejected_count")? as i64,
            auto_applied_count: row.try_get::<i32, _>("auto_applied_count")? as i64,
        };
        let auto_apply_enabled = row.try_get("auto_apply_enabled")?;
        let suppressed = row
            .try_get::<Option<DateTime<Utc>>, _>("suppressed_until")?
            .is_some();
        Ok(ImportLearningLifecycleView {
            recommendation_key: row.try_get("recommendation_key")?,
            recommendation_type: row.try_get("recommendation_type")?,
            status,
            signal_state: learning_lifecycle_signal_state(&state.status).to_string(),
            accepted_count: state.accepted_count,
            rejected_count: state.rejected_count,
            auto_applied_count: state.auto_applied_count,
            auto_apply_enabled,
            suppressed,
        })
    })
    .transpose()
}

#[cfg(test)]
mod lifecycle_action_tests {
    use super::*;

    #[test]
    fn learning_review_status_uses_first_nonempty_authority() {
        let feedback = json!({
            "learning": {
                "review_status": "  ",
                "status": "Accepted",
                "lifecycle_status": "rejected",
                "signal_state": "pending"
            }
        });

        assert_eq!(
            preview_learning_review_status(&feedback).as_deref(),
            Some("accepted")
        );
    }

    #[test]
    fn repeated_terminal_learning_actions_are_idempotent() {
        assert!(learning_decision_is_idempotent(
            Some("accepted"),
            true,
            ImportPreviewDecision::Accept
        ));
        assert!(learning_decision_is_idempotent(
            Some("rejected"),
            true,
            ImportPreviewDecision::Reject
        ));
        assert!(learning_decision_is_idempotent(
            None,
            false,
            ImportPreviewDecision::Clear
        ));
        assert!(!learning_decision_is_idempotent(
            Some("pending"),
            true,
            ImportPreviewDecision::Accept
        ));
        assert!(!learning_decision_is_idempotent(
            Some("pending"),
            true,
            ImportPreviewDecision::Reject
        ));
    }

    #[test]
    fn learning_terminal_action_guard_covers_retries_opposites_clear_and_mobile_payloads() {
        let cases = [
            (Some("accepted"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Idempotent),
            (Some("accepted"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (Some("accepted"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Idempotent),
            (Some("rejected"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("pending"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Apply),
            (None, false, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Idempotent),
        ];

        for (status, has_signal, decision, expected) in cases {
            assert_eq!(preview_terminal_action(status, has_signal, decision), expected);
        }
    }

    #[test]
    fn learning_expected_state_rejects_stale_desktop_but_accepts_minimal_mobile_shape() {
        let mut preview = preview_row(1);
        preview.category_id = Some(42);
        preview.preview_matching_feedback = json!({
            "learning": {"review_status": "pending"}
        });
        let cases = [
            (
                ImportPreviewExpectedState {
                    session_id: Some("session".to_string()),
                    ..ImportPreviewExpectedState::default()
                },
                false,
            ),
            (
                ImportPreviewExpectedState {
                    session_id: Some("session".to_string()),
                    review_status: Some("pending".to_string()),
                    preview_category_id: Some(Some(42)),
                    ..ImportPreviewExpectedState::default()
                },
                false,
            ),
            (
                ImportPreviewExpectedState {
                    review_status: Some("accepted".to_string()),
                    preview_category_id: Some(Some(42)),
                    ..ImportPreviewExpectedState::default()
                },
                true,
            ),
            (
                ImportPreviewExpectedState {
                    review_status: Some("pending".to_string()),
                    preview_category_id: Some(Some(43)),
                    ..ImportPreviewExpectedState::default()
                },
                true,
            ),
        ];

        for (expected, conflicts) in cases {
            assert_eq!(
                recommendation_expected_state_conflicts(&preview, Some(&expected), Some("pending")),
                conflicts
            );
        }
    }

    #[test]
    fn serialized_concurrent_learning_opposites_and_identical_retry_emit_one_event() {
        for (first, opposite, terminal_status) in [
            (
                ImportPreviewDecision::Accept,
                ImportPreviewDecision::Reject,
                "accepted",
            ),
            (
                ImportPreviewDecision::Reject,
                ImportPreviewDecision::Accept,
                "rejected",
            ),
        ] {
            let mut event_count = 0;
            assert_eq!(
                preview_terminal_action(Some("pending"), true, first),
                ImportPreviewTerminalAction::Apply
            );
            event_count += 1;
            assert_eq!(
                preview_terminal_action(Some(terminal_status), true, first),
                ImportPreviewTerminalAction::Idempotent
            );
            assert_eq!(
                preview_terminal_action(Some(terminal_status), true, opposite),
                ImportPreviewTerminalAction::Conflict
            );
            assert_eq!(
                preview_terminal_action(
                    Some(terminal_status),
                    true,
                    ImportPreviewDecision::Clear,
                ),
                ImportPreviewTerminalAction::Conflict
            );
            assert_eq!(event_count, 1);
        }
    }

    #[test]
    fn learning_action_keeps_preview_and_lifecycle_writes_in_one_locked_transaction() {
        let source = include_str!("preview_learning_lifecycle.rs");

        assert!(source.contains("let mut transaction = pool.begin().await?;"));
        assert!(source.contains("FOR UPDATE OF p"));
        assert!(source.contains("pg_advisory_xact_lock"));
        assert!(source.contains("update.build().execute(&mut *transaction).await?;"));
        assert!(source.contains("persist_import_learning_lifecycle_transition_in_transaction("));
        assert!(source.contains("transaction.commit().await?;"));
        let lock = source.find("FOR UPDATE OF p").expect("preview row lock");
        let guard = source
            .find("match preview_terminal_action(")
            .expect("terminal action guard");
        let event = source
            .find("persist_import_learning_lifecycle_transition_in_transaction(")
            .expect("learning event persistence");
        let feedback = source
            .find("let lifecycle_input = preview_learning_lifecycle_input")
            .expect("learning feedback transition");
        assert!(lock < guard && guard < feedback && feedback < event);
    }
}
