// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
pub fn apply_preview_transfer_decision(
    connection: &mut Connection,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    reviewed_type: &str,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_decision_not_found());
        };
        if !preview_matches_expected_state(&preview, expected_state) {
            return Ok(preview_decision_state_conflict());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        let transfer_feedback = feedback
            .get("transfer")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let previous_snapshot = transfer_feedback
            .get("previous_preview")
            .and_then(normalize_transfer_snapshot);
        let applied_snapshot = transfer_feedback
            .get("applied_preview")
            .and_then(normalize_transfer_snapshot);
        let current_review_status = transfer_feedback
            .get("review_status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let should_restore_accepted = current_review_status == "accepted"
            && previous_snapshot.as_ref().is_some()
            && applied_snapshot
                .as_ref()
                .is_none_or(|snapshot| transfer_preview_matches_snapshot(&preview, snapshot));
        let should_restore_pending = current_review_status != "accepted"
            && applied_snapshot
                .as_ref()
                .is_some_and(|snapshot| transfer_preview_matches_snapshot(&preview, snapshot));

        let mut patch = ImportPreviewPatch::new(preview_id);
        match decision {
            ImportPreviewDecision::Accept => {
                let snapshot =
                    previous_snapshot.unwrap_or_else(|| build_transfer_previous_snapshot(&preview));
                let transfer_category = transfer_decision_category(tx, user_id)?;
                let transfer_type = transfer_category
                    .as_ref()
                    .and_then(|category| preview_type_name_for_category_type(category.type_code))
                    .unwrap_or(reviewed_type)
                    .to_string();
                let transfer_main_category = transfer_category
                    .as_ref()
                    .map(|category| category.main_category.clone())
                    .unwrap_or_default();
                let transfer_sub_category = transfer_category
                    .as_ref()
                    .map(|category| category.sub_category.clone())
                    .unwrap_or_default();
                let account_resolution =
                    resolve_transfer_accounts_for_preview(tx, user_id, &preview)?;
                patch = patch
                    .with_change(
                        ImportPreviewPatchField::Type,
                        ImportPreviewPatchValue::Text(transfer_type.clone()),
                    )
                    .with_change(
                        ImportPreviewPatchField::MainCategory,
                        ImportPreviewPatchValue::Text(transfer_main_category.clone()),
                    )
                    .with_change(
                        ImportPreviewPatchField::SubCategory,
                        ImportPreviewPatchValue::Text(transfer_sub_category.clone()),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringId,
                        ImportPreviewPatchValue::Null,
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringName,
                        ImportPreviewPatchValue::Text(String::new()),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringCandidateCount,
                        ImportPreviewPatchValue::Integer(0),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringMatchScore,
                        ImportPreviewPatchValue::Real(0.0),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringMatchReasons,
                        ImportPreviewPatchValue::Text(String::new()),
                    )
                    .with_change(
                        ImportPreviewPatchField::RecurringMatchedDate,
                        ImportPreviewPatchValue::Text(String::new()),
                    );
                if let Some(source_account_id) = account_resolution.source_account_id {
                    patch = patch.with_change(
                        ImportPreviewPatchField::SourceAccountId,
                        ImportPreviewPatchValue::Integer(source_account_id),
                    );
                }
                if let Some(destination_account_id) = account_resolution.destination_account_id {
                    patch = patch.with_change(
                        ImportPreviewPatchField::DestinationAccountId,
                        ImportPreviewPatchValue::Integer(destination_account_id),
                    );
                }
                let mut transfer_review = serde_json::json!({
                    "review_status": "accepted",
                    "reviewed_type": transfer_type.clone(),
                    "suppressed": false,
                    "previous_preview": snapshot,
                });
                if let Some(category) = transfer_category.as_ref() {
                    transfer_review["category_id"] = serde_json::json!(category.id);
                }
                if let Some(source_account_id) = account_resolution.source_account_id {
                    transfer_review["resolved_source_account_id"] =
                        serde_json::json!(source_account_id);
                }
                if let Some(destination_account_id) = account_resolution.destination_account_id {
                    transfer_review["resolved_destination_account_id"] =
                        serde_json::json!(destination_account_id);
                }
                if account_resolution.source_account_id.is_some()
                    || account_resolution.destination_account_id.is_some()
                {
                    transfer_review["account_resolution"] = serde_json::json!("source_chain");
                }
                transfer_review["applied_preview"] = serde_json::json!({
                    "preview_type": transfer_type,
                    "preview_main_category": transfer_main_category,
                    "preview_sub_category": transfer_sub_category,
                    "preview_source_account_id": account_resolution
                        .source_account_id
                        .or(preview.preview_source_account_id),
                    "preview_destination_account_id": account_resolution
                        .destination_account_id
                        .or(preview.preview_destination_account_id),
                    "preview_recurring_id": Value::Null,
                    "preview_recurring_name": "",
                    "preview_recurring_candidate_count": 0,
                    "preview_recurring_match_score": 0.0,
                    "preview_recurring_match_reasons": "",
                    "preview_recurring_matched_date": "",
                });
                feedback["transfer"] = transfer_review;
            }
            ImportPreviewDecision::Reject => {
                if should_restore_accepted {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(transfer_snapshot_restore_changes(snapshot));
                    }
                } else if should_restore_pending {
                    if let Some(baseline) =
                        category_rule_account_baseline_snapshot(tx, user_id, &preview)?
                    {
                        patch = patch.with_changes(learning_snapshot_restore_changes(&baseline));
                    }
                }
                feedback["transfer"] = serde_json::json!({
                    "review_status": "rejected",
                    "reviewed_type": "",
                    "suppressed": true,
                });
            }
            ImportPreviewDecision::Clear => {
                if should_restore_accepted {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(transfer_snapshot_restore_changes(snapshot));
                    }
                }
                remove_json_object_key(&mut feedback, "transfer");
            }
        }

        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, &preview.session_id, user_id, &patch)? < 1 {
            return Ok(preview_decision_not_found());
        }
        Ok(ImportPreviewDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_preview_recurring_match_decision(
    connection: &mut Connection,
    preview_id: i64,
    user_id: UserId,
    update: &ImportPreviewRecurringMatchUpdate,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "update_preview_recurring_match_decision", "business operation entered");
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_decision_not_found());
        };
        if !preview_matches_expected_state(&preview, expected_state) {
            return Ok(preview_decision_state_conflict());
        }

        let target_candidate = match update.recurring_id {
            Some(recurring_id) => match update.target_candidate.as_ref() {
                Some(candidate) if candidate.id == recurring_id => Some(candidate),
                _ => {
                    return Ok(ImportPreviewDecisionResult {
                        preview: None,
                        state_conflict: false,
                        invalid_recurring_id: true,
                    })
                }
            },
            None => None,
        };

        let mut feedback = preview.preview_matching_feedback.clone();
        if preview.preview_recurring_id != update.recurring_id {
            remove_json_object_key(&mut feedback, "transfer");
        }
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
                    target_candidate
                        .map(|candidate| candidate.name.clone())
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringCandidateCount,
                ImportPreviewPatchValue::Integer(update.candidate_count),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchScore,
                ImportPreviewPatchValue::Real(
                    target_candidate
                        .map(|candidate| candidate.match_score)
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchReasons,
                ImportPreviewPatchValue::Text(
                    target_candidate
                        .map(|candidate| candidate.match_reasons.join("|"))
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::RecurringMatchedDate,
                ImportPreviewPatchValue::Text(
                    target_candidate
                        .map(|candidate| candidate.matched_occurrence_date.clone())
                        .unwrap_or_default(),
                ),
            )
            .with_change(
                ImportPreviewPatchField::MatchingFeedback,
                ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
            );
        if execute_preview_patch(tx, &preview.session_id, user_id, &patch)? < 1 {
            return Ok(preview_decision_not_found());
        }
        Ok(ImportPreviewDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn apply_preview_learning_decision(
    connection: &mut Connection,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    applied_result: Option<&ImportPreviewLearningApply>,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_decision_not_found());
        };
        if !preview_matches_expected_state(&preview, expected_state) {
            return Ok(preview_decision_state_conflict());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        let learning_feedback = feedback
            .get("learning")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let previous_snapshot = learning_feedback
            .get("previous_preview")
            .and_then(normalize_learning_snapshot);
        let applied_snapshot = learning_feedback
            .get("applied_preview")
            .and_then(normalize_learning_snapshot);
        let current_review_status = learning_feedback
            .get("review_status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let should_restore = current_review_status == "accepted"
            && previous_snapshot.as_ref().is_some()
            && applied_snapshot
                .as_ref()
                .is_none_or(|snapshot| learning_preview_matches_snapshot(&preview, snapshot));
        let should_restore_pending = current_review_status != "accepted"
            && applied_snapshot
                .as_ref()
                .is_some_and(|snapshot| learning_preview_matches_snapshot(&preview, snapshot));

        let mut patch = ImportPreviewPatch::new(preview_id);
        match decision {
            ImportPreviewDecision::Accept => {
                let previous =
                    previous_snapshot.unwrap_or_else(|| build_learning_previous_snapshot(&preview));
                let rule_id = applied_result.and_then(|value| normalize_rule_id(value.rule_id));
                let rule_category = match rule_id {
                    Some(rule_id) => learning_rule_category(tx, user_id, rule_id)?,
                    None => None,
                };
                let mut applied = learning_accept_preview_snapshot(applied_result, &preview);
                if let Some(category) = rule_category.as_ref() {
                    apply_category_to_learning_snapshot(&mut applied, category);
                } else {
                    canonicalize_learning_snapshot_category(tx, user_id, &mut applied)?;
                }
                patch = patch.with_changes(learning_snapshot_restore_changes(&applied));
                feedback["learning"] = serde_json::json!({
                    "review_status": "accepted",
                    "suppressed": false,
                    "previous_preview": previous,
                    "applied_preview": applied,
                });
                if let Some(rule_id) = rule_id {
                    feedback["learning"]["rule_id"] = serde_json::json!(rule_id);
                }
                if let Some(category) = rule_category.as_ref() {
                    feedback["learning"]["category_id"] = serde_json::json!(category.id);
                }
            }
            ImportPreviewDecision::Reject => {
                if should_restore {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(learning_snapshot_restore_changes(snapshot));
                    }
                } else if should_restore_pending {
                    if let Some(baseline) =
                        category_rule_account_baseline_snapshot(tx, user_id, &preview)?
                    {
                        patch = patch.with_changes(learning_snapshot_restore_changes(&baseline));
                    }
                }
                feedback["learning"] = serde_json::json!({
                    "review_status": "rejected",
                    "suppressed": true,
                });
                let rule_id = applied_result
                    .and_then(|value| normalize_rule_id(value.rule_id))
                    .or_else(|| feedback_rule_id(&learning_feedback));
                if let Some(rule_id) = rule_id {
                    feedback["learning"]["rule_id"] = serde_json::json!(rule_id);
                }
            }
            ImportPreviewDecision::Clear => {
                if should_restore {
                    if let Some(snapshot) = previous_snapshot.as_ref() {
                        patch = patch.with_changes(learning_snapshot_restore_changes(snapshot));
                    }
                }
                remove_json_object_key(&mut feedback, "learning");
            }
        }

        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, &preview.session_id, user_id, &patch)? < 1 {
            return Ok(preview_decision_not_found());
        }
        Ok(ImportPreviewDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            state_conflict: false,
            invalid_recurring_id: false,
        })
    })
}
