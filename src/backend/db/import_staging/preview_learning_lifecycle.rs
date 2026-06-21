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
    let Some(preview) = get_preview_bill_by_id(pool, preview_id, user_id)? else {
        return Ok(preview_decision_not_found());
    };
    if expected_state_conflicts(&preview, expected_state) {
        return Ok(preview_decision_state_conflict());
    }
    let mut patch = ImportPreviewPatch::new(preview_id);
    match decision {
        ImportPreviewDecision::Accept => {
            if let Some(applied) = applied_result {
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
                        value
                            .map(ImportPreviewPatchValue::Integer)
                            .unwrap_or(ImportPreviewPatchValue::Null),
                    );
                }
                if let Some(value) = applied.preview_destination_account_id {
                    patch = patch.with_change(
                        ImportPreviewPatchField::DestinationAccountId,
                        value
                            .map(ImportPreviewPatchValue::Integer)
                            .unwrap_or(ImportPreviewPatchValue::Null),
                    );
                }
            }
            let feedback = set_feedback_review_status(
                preview.preview_matching_feedback,
                "learning",
                "accepted",
            );
            patch = patch.with_change(
                ImportPreviewPatchField::MatchingFeedback,
                ImportPreviewPatchValue::Json(feedback),
            );
        }
        ImportPreviewDecision::Reject | ImportPreviewDecision::Clear => {
            patch = patch.with_learning_decision_cleared();
        }
    }
    replace_preview_selection_with_patches(pool, session_id, user_id, &[patch])?;
    Ok(ImportPreviewDecisionResult {
        preview: get_preview_bill_by_id(pool, preview_id, user_id)?,
        state_conflict: false,
        invalid_recurring_id: false,
    })
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
        let next = transition_import_learning_lifecycle(&state, &input.feedback);
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
        .bind(&input.feedback)
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
