/// 批量更新分类字段并重新执行身份校验，防止无效分类进入 confirm。
pub fn batch_update_preview_classification(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    updates: &[ImportPreviewClassificationUpdate],
) -> DbResult<usize> {
    let patches = updates
        .iter()
        .map(|update| {
            ImportPreviewPatch::new(update.preview_id)
                .with_change(
                    ImportPreviewPatchField::Type,
                    ImportPreviewPatchValue::Text(update.preview_type.clone()),
                )
                .with_change(
                    ImportPreviewPatchField::MainCategory,
                    ImportPreviewPatchValue::Text(update.preview_main_category.clone()),
                )
                .with_change(
                    ImportPreviewPatchField::SubCategory,
                    ImportPreviewPatchValue::Text(update.preview_sub_category.clone()),
                )
                .with_change(
                    ImportPreviewPatchField::SourceAccountId,
                    update
                        .preview_source_account_id
                        .map(ImportPreviewPatchValue::Integer)
                        .unwrap_or(ImportPreviewPatchValue::Null),
                )
                .with_change(
                    ImportPreviewPatchField::DestinationAccountId,
                    update
                        .preview_destination_account_id
                        .map(ImportPreviewPatchValue::Integer)
                        .unwrap_or(ImportPreviewPatchValue::Null),
                )
        })
        .collect::<Vec<_>>();
    replace_preview_selection_with_patches(pool, session_id, user_id, &patches)
}

/// 落库人工转账决策并写入 matching feedback，保留 selected 状态用于后续 confirm。
pub fn apply_preview_transfer_decision(
    pool: &PostgresPool,
    session_id: &str,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let Some(preview) =
            load_preview_bill_by_id_on_tx(&mut tx, session_db_id, preview_id, user_id).await?
        else {
            tx.commit().await?;
            return Ok(preview_decision_not_found());
        };
        if expected_state_conflicts(&preview, expected_state) {
            tx.commit().await?;
            return Ok(preview_decision_state_conflict());
        }
        let patch = match decision {
            ImportPreviewDecision::Accept => {
                let feedback = set_feedback_review_status(
                    preview.preview_matching_feedback,
                    "transfer",
                    "accepted",
                );
                ImportPreviewPatch::new(preview_id).with_change(
                    ImportPreviewPatchField::MatchingFeedback,
                    ImportPreviewPatchValue::Json(feedback),
                )
            }
            ImportPreviewDecision::Reject | ImportPreviewDecision::Clear => {
                ImportPreviewPatch::new(preview_id).with_transfer_decision_cleared()
            }
        };
        apply_preview_patches_on_tx(&mut tx, session_db_id, user_id, &[patch]).await?;
        let updated =
            load_preview_bill_by_id_on_tx(&mut tx, session_db_id, preview_id, user_id).await?;
        tx.commit().await?;
        Ok(ImportPreviewDecisionResult {
            preview: updated,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

/// 落库周期模板匹配接受/拒绝结果，并同步更新 recurring 相关预览字段。
pub fn update_preview_recurring_match_decision(
    pool: &PostgresPool,
    session_id: &str,
    preview_id: i64,
    user_id: UserId,
    update: &ImportPreviewRecurringMatchUpdate,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let Some(preview) =
            load_preview_bill_by_id_on_tx(&mut tx, session_db_id, preview_id, user_id).await?
        else {
            tx.commit().await?;
            return Ok(preview_decision_not_found());
        };
        if expected_row_version_conflicts(&preview, expected_state) {
            tx.commit().await?;
            return Ok(preview_decision_state_conflict_with_preview(preview));
        }
        if expected_state_conflicts(&preview, expected_state) {
            tx.commit().await?;
            return Ok(preview_decision_state_conflict());
        }
        let candidate = update.target_candidate.as_ref();
        let patch = ImportPreviewPatch::new(preview_id)
            .with_change(
                ImportPreviewPatchField::RecurringId,
                update
                    .recurring_id
                    .map(ImportPreviewPatchValue::Integer)
                    .unwrap_or(ImportPreviewPatchValue::Null),
            )
            .with_change(
                ImportPreviewPatchField::RecurringName,
                ImportPreviewPatchValue::Text(
                    candidate.map(|item| item.name.clone()).unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringCandidateCount,
                ImportPreviewPatchValue::Integer(update.candidate_count),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchScore,
                ImportPreviewPatchValue::Real(
                    candidate.map(|item| item.match_score).unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchReasons,
                ImportPreviewPatchValue::Text(
                    candidate
                        .map(|item| item.match_reasons.join("；"))
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchedDate,
                ImportPreviewPatchValue::Text(
                    candidate
                        .map(|item| item.matched_occurrence_date.clone())
                        .unwrap_or_default(),
                ),
            );
        apply_preview_patches_on_tx(&mut tx, session_db_id, user_id, &[patch]).await?;
        let updated =
            load_preview_bill_by_id_on_tx(&mut tx, session_db_id, preview_id, user_id).await?;
        tx.commit().await?;
        Ok(ImportPreviewDecisionResult {
            preview: updated,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

fn expected_state_conflicts(
    preview: &ImportPreviewRow,
    expected: Option<&ImportPreviewExpectedState>,
) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    if let Some(session_id) = &expected.session_id {
        if session_id != &preview.session_id {
            return true;
        }
    }
    if expected
        .expected_row_version
        .is_some_and(|value| value != preview.version)
    {
        return true;
    }
    if let Some(value) = &expected.preview_type {
        if value != &preview.preview_type {
            return true;
        }
    }
    if let Some(value) = &expected.preview_main_category {
        if value != &preview.preview_main_category {
            return true;
        }
    }
    if let Some(value) = &expected.preview_sub_category {
        if value != &preview.preview_sub_category {
            return true;
        }
    }
    false
}

fn expected_row_version_conflicts(
    preview: &ImportPreviewRow,
    expected: Option<&ImportPreviewExpectedState>,
) -> bool {
    expected
        .and_then(|value| value.expected_row_version)
        .is_some_and(|value| value != preview.version)
}

fn preview_decision_not_found() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: false,
        invalid_recurring_id: false,
    }
}

fn preview_decision_state_conflict() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: true,
        invalid_recurring_id: false,
    }
}

fn preview_decision_state_conflict_with_preview(
    preview: ImportPreviewRow,
) -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: Some(preview),
        state_conflict: true,
        invalid_recurring_id: false,
    }
}
