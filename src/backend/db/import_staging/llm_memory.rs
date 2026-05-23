// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

pub fn create_llm_memory_event(
    connection: &Connection,
    draft: &LlmMemoryEventDraft,
) -> DbResult<i64> {
    connection.execute(
        "
        INSERT INTO llm_memory_events (
            user_id, session_id, preview_id, event_type, decision,
            prompt_text, llm_response_raw, llm_provider, llm_model,
            suggested_main_category, suggested_sub_category,
            suggested_source_account, suggested_destination_account,
            confidence, user_correction_category, user_correction_account,
            snapshot_before, snapshot_after, metadata, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
        ",
        params![
            user_id_i64(draft.user_id)?,
            draft.session_id,
            draft.preview_id,
            draft.event_type,
            draft.decision,
            draft.prompt_text,
            draft.llm_response_raw,
            draft.llm_provider,
            draft.llm_model,
            draft.suggested_main_category,
            draft.suggested_sub_category,
            draft.suggested_source_account,
            draft.suggested_destination_account,
            draft.confidence,
            draft.user_correction_category,
            draft.user_correction_account,
            json_option_text(draft.snapshot_before.as_ref()),
            json_option_text(draft.snapshot_after.as_ref()),
            json_option_text(draft.metadata.as_ref()),
            now_text(),
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn get_llm_memory_events(
    connection: &Connection,
    user_id: UserId,
    session_id: Option<&str>,
    event_type: Option<&str>,
    limit: usize,
    offset: usize,
) -> DbResult<Vec<LlmMemoryEventRow>> {
    let mut query = "SELECT * FROM llm_memory_events WHERE user_id = ?".to_string();
    let mut params = vec![SqlValue::Integer(user_id_i64(user_id)?)];
    if let Some(session_id) = session_id {
        query.push_str(" AND session_id = ?");
        params.push(SqlValue::Text(session_id.to_string()));
    }
    if let Some(event_type) = event_type {
        query.push_str(" AND event_type = ?");
        params.push(SqlValue::Text(event_type.to_string()));
    }
    query.push_str(" ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?");
    params.push(SqlValue::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    params.push(SqlValue::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(
        rusqlite::params_from_iter(params),
        llm_memory_event_from_row,
    )?;
    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    Ok(events)
}

pub fn apply_preview_llm_recommendation(
    connection: &mut Connection,
    request: ImportPreviewLlmApplyRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let ImportPreviewLlmApplyRequest {
        session_id,
        preview_id,
        user_id,
        suggestion,
        prompt_text,
        llm_provider,
        llm_model,
    } = request;
    run_transaction(connection, |tx| {
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_llm_not_found());
        };
        if preview.session_id != session_id {
            return Ok(preview_llm_not_found());
        }

        let previous_snapshot = build_llm_previous_snapshot(&preview);
        let current_main_category = preview.preview_main_category.trim().to_string();
        let current_sub_category = preview.preview_sub_category.trim().to_string();
        let mut next_main_category = current_main_category.clone();
        let mut next_sub_category = current_sub_category.clone();
        if current_main_category.is_empty()
            && current_sub_category.is_empty()
            && !suggestion.suggested_main_category.trim().is_empty()
        {
            next_main_category = suggestion.suggested_main_category.trim().to_string();
            next_sub_category = suggestion.suggested_sub_category.trim().to_string();
        }

        let mut next_source_account_id = preview.preview_source_account_id;
        if next_source_account_id.is_none() && suggestion.resolved_source_account_id.is_some() {
            next_source_account_id = suggestion.resolved_source_account_id;
        }
        let mut next_destination_account_id = preview.preview_destination_account_id;
        if next_destination_account_id.is_none()
            && suggestion.resolved_destination_account_id.is_some()
        {
            next_destination_account_id = suggestion.resolved_destination_account_id;
        }

        let applied_snapshot = serde_json::json!({
            "preview_main_category": next_main_category,
            "preview_sub_category": next_sub_category,
            "preview_source_account_id": next_source_account_id,
            "preview_destination_account_id": next_destination_account_id,
        });
        let mut applied_fields = Vec::new();
        let mut patch = ImportPreviewPatch::new(preview_id);
        if next_main_category != current_main_category {
            patch = patch.with_change(
                ImportPreviewPatchField::MainCategory,
                ImportPreviewPatchValue::Text(next_main_category.clone()),
            );
            applied_fields.push("preview_main_category".to_string());
        }
        if next_sub_category != current_sub_category {
            patch = patch.with_change(
                ImportPreviewPatchField::SubCategory,
                ImportPreviewPatchValue::Text(next_sub_category.clone()),
            );
            applied_fields.push("preview_sub_category".to_string());
        }
        if next_source_account_id != preview.preview_source_account_id {
            patch = patch.with_change(
                ImportPreviewPatchField::SourceAccountId,
                next_source_account_id
                    .map(ImportPreviewPatchValue::Integer)
                    .unwrap_or(ImportPreviewPatchValue::Null),
            );
            applied_fields.push("preview_source_account_id".to_string());
        }
        if next_destination_account_id != preview.preview_destination_account_id {
            patch = patch.with_change(
                ImportPreviewPatchField::DestinationAccountId,
                next_destination_account_id
                    .map(ImportPreviewPatchValue::Integer)
                    .unwrap_or(ImportPreviewPatchValue::Null),
            );
            applied_fields.push("preview_destination_account_id".to_string());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        if !applied_fields.is_empty() {
            remove_json_object_key(&mut feedback, "transfer");
        }
        feedback["llm"] = build_llm_feedback_payload(
            suggestion,
            "pending",
            false,
            &previous_snapshot,
            &applied_snapshot,
        );
        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, session_id, user_id, &patch)? < 1 {
            return Ok(preview_llm_not_found());
        }

        let event_id = create_llm_memory_event(
            tx,
            &LlmMemoryEventDraft {
                user_id,
                session_id: Some(session_id.to_string()),
                preview_id: Some(preview_id),
                event_type: "recommendation".to_string(),
                decision: None,
                prompt_text: prompt_text.map(truncated_llm_memory_prompt_text),
                llm_response_raw: Some(llm_suggestion_payload(suggestion).to_string()),
                llm_provider: llm_provider.map(str::to_string),
                llm_model: llm_model.map(str::to_string),
                suggested_main_category: optional_non_empty(&suggestion.suggested_main_category),
                suggested_sub_category: optional_non_empty(&suggestion.suggested_sub_category),
                suggested_source_account: optional_non_empty(&suggestion.suggested_source_account),
                suggested_destination_account: optional_non_empty(
                    &suggestion.suggested_destination_account,
                ),
                confidence: suggestion.confidence,
                user_correction_category: None,
                user_correction_account: None,
                snapshot_before: Some(previous_snapshot),
                snapshot_after: Some(applied_snapshot),
                metadata: Some(serde_json::json!({
                    "reason": suggestion.reason,
                    "counterparty": preview.preview_counterparty,
                    "payment_method": preview.preview_payment_method,
                    "description": preview.preview_description,
                    "applied_fields": applied_fields,
                    "resolved_source_account_id": suggestion.resolved_source_account_id,
                    "resolved_destination_account_id": suggestion.resolved_destination_account_id,
                })),
            },
        )?;
        Ok(ImportPreviewLlmDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            event_id: Some(event_id),
            applied_fields,
            restored: false,
        })
    })
}

pub fn review_preview_llm_recommendation(
    connection: &mut Connection,
    request: ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let ImportPreviewLlmReviewRequest {
        session_id,
        preview_id,
        user_id,
        decision,
        suggestion,
        user_correction_category,
        user_correction_account,
    } = request;
    run_transaction(connection, |tx| {
        if decision == ImportPreviewDecision::Clear {
            return Ok(preview_llm_not_found());
        }
        let Some(preview) = get_preview_bill_by_id(tx, preview_id, user_id)? else {
            return Ok(preview_llm_not_found());
        };
        if preview.session_id != session_id {
            return Ok(preview_llm_not_found());
        }

        let mut feedback = preview.preview_matching_feedback.clone();
        ensure_json_object(&mut feedback);
        let llm_feedback = feedback
            .get("llm")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let previous_snapshot = llm_feedback
            .get("previous_preview")
            .and_then(normalize_llm_snapshot);
        let applied_snapshot = llm_feedback
            .get("applied_preview")
            .and_then(normalize_llm_snapshot);
        let should_restore = decision == ImportPreviewDecision::Reject
            && previous_snapshot.as_ref().is_some()
            && applied_snapshot
                .as_ref()
                .is_none_or(|snapshot| llm_preview_matches_snapshot(&preview, snapshot));
        let current_snapshot = build_llm_previous_snapshot(&preview);
        let refreshed_snapshot = if should_restore {
            previous_snapshot
                .clone()
                .unwrap_or_else(|| current_snapshot.clone())
        } else {
            applied_snapshot
                .clone()
                .unwrap_or_else(|| current_snapshot.clone())
        };
        let resolved_suggestion = suggestion
            .cloned()
            .unwrap_or_else(|| llm_suggestion_from_feedback(&llm_feedback));

        let mut patch = ImportPreviewPatch::new(preview_id);
        if should_restore {
            patch = patch.with_changes(llm_snapshot_restore_changes(&refreshed_snapshot));
        }
        let review_status = if decision == ImportPreviewDecision::Accept {
            "accepted"
        } else {
            "rejected"
        };
        feedback["llm"] = build_llm_feedback_payload(
            &resolved_suggestion,
            review_status,
            decision == ImportPreviewDecision::Reject,
            &previous_snapshot.unwrap_or_else(|| current_snapshot.clone()),
            &applied_snapshot.unwrap_or_else(|| current_snapshot.clone()),
        );
        patch = patch.with_change(
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Text(serialize_preview_matching_feedback(&feedback)),
        );
        if execute_preview_patch(tx, session_id, user_id, &patch)? < 1 {
            return Ok(preview_llm_not_found());
        }

        let event_id = create_llm_memory_event(
            tx,
            &LlmMemoryEventDraft {
                user_id,
                session_id: Some(session_id.to_string()),
                preview_id: Some(preview_id),
                event_type: "feedback".to_string(),
                decision: Some(review_status.trim_end_matches("ed").to_string()),
                prompt_text: None,
                llm_response_raw: Some(llm_suggestion_payload(&resolved_suggestion).to_string()),
                llm_provider: None,
                llm_model: None,
                suggested_main_category: optional_non_empty(
                    &resolved_suggestion.suggested_main_category,
                ),
                suggested_sub_category: optional_non_empty(
                    &resolved_suggestion.suggested_sub_category,
                ),
                suggested_source_account: optional_non_empty(
                    &resolved_suggestion.suggested_source_account,
                ),
                suggested_destination_account: optional_non_empty(
                    &resolved_suggestion.suggested_destination_account,
                ),
                confidence: resolved_suggestion.confidence,
                user_correction_category: user_correction_category.and_then(optional_non_empty),
                user_correction_account: user_correction_account.and_then(optional_non_empty),
                snapshot_before: Some(current_snapshot),
                snapshot_after: Some(refreshed_snapshot),
                metadata: Some(serde_json::json!({
                    "reason": resolved_suggestion.reason,
                    "rollback": should_restore,
                })),
            },
        )?;
        Ok(ImportPreviewLlmDecisionResult {
            preview: get_preview_bill_by_id(tx, preview_id, user_id)?,
            event_id: Some(event_id),
            applied_fields: Vec::new(),
            restored: should_restore,
        })
    })
}
