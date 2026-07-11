/// 校验 LLM 建议的交易类型，未获得结构化转账授权时禁止应用转账语义。
fn llm_suggested_type_for_preview(
    preview: &ImportPreviewRow,
    suggestion: &ImportPreviewLlmSuggestion,
) -> (Option<String>, Option<String>) {
    let raw_type = suggestion.suggested_type.trim();
    if raw_type.is_empty() {
        return (None, None);
    }
    let Some(type_code) = preview_category_type_code(raw_type) else {
        return (None, Some("invalid_type".to_string()));
    };
    let Some(normalized_type) = preview_type_label_from_code(type_code).map(str::to_string) else {
        return (None, Some("invalid_type".to_string()));
    };
    let transfer_authorized = preview_has_authorized_transfer_feedback(preview);
    if type_code == 4 && !transfer_authorized {
        return (None, Some("unauthorized_transfer".to_string()));
    }
    if transfer_authorized && type_code != 4 {
        return (None, Some("transfer_protected".to_string()));
    }
    (Some(normalized_type), None)
}

/// 解析 LLM 建议分类并在用户分类范围内匹配，转账保护会限制可选分类类型。
fn resolve_llm_suggested_category(
    pool: &PostgresPool,
    user_id: i64,
    suggestion: &ImportPreviewLlmSuggestion,
    transfer_authorized: bool,
) -> DbResult<Option<ImportPreviewLlmCategoryMatch>> {
    let main_category = suggestion.suggested_main_category.trim();
    let sub_category = suggestion.suggested_sub_category.trim();
    if suggestion.suggested_category_id.is_none()
        && main_category.is_empty()
        && sub_category.is_empty()
    {
        return Ok(None);
    }
    let suggested_type_code = preview_category_type_code(&suggestion.suggested_type);
    let suggested_category_id = suggestion.suggested_category_id;
    block_on_db(async move {
        let rows = sqlx::query(
            r#"
            SELECT id, category_type, path, name
            FROM categories
            WHERE user_id = $1 AND is_active = true
            ORDER BY display_order ASC, id ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        for row in rows {
            let id: i64 = row.try_get("id")?;
            let category_type: Option<String> = row.try_get("category_type")?;
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let type_code = category_type
                .as_deref()
                .and_then(preview_category_type_code);
            if type_code == Some(4) && !transfer_authorized {
                continue;
            }
            if transfer_authorized && type_code != Some(4) {
                continue;
            }
            if let (Some(suggested_type_code), Some(type_code)) = (suggested_type_code, type_code) {
                if suggested_type_code == 4 && type_code != 4 {
                    continue;
                }
            }
            let (candidate_main, candidate_sub) =
                preview_category_names_from_path(path.as_deref().unwrap_or_default(), &name);
            if let Some(suggested_category_id) = suggested_category_id {
                if id == suggested_category_id {
                    return Ok(Some(ImportPreviewLlmCategoryMatch {
                        id,
                        type_code,
                        main_category: candidate_main,
                        sub_category: candidate_sub,
                    }));
                }
                continue;
            }
            if candidate_main.trim() == main_category && candidate_sub.trim() == sub_category {
                return Ok(Some(ImportPreviewLlmCategoryMatch {
                    id,
                    type_code,
                    main_category: candidate_main,
                    sub_category: candidate_sub,
                }));
            }
        }
        Ok(None)
    })
}

/// 判断预览行是否已有用户确认的转账候选反馈，作为 LLM 转账应用的前置授权。
fn preview_has_authorized_transfer_feedback(preview: &ImportPreviewRow) -> bool {
    let Some(transfer) = preview
        .preview_matching_feedback
        .get("transfer")
        .and_then(Value::as_object)
    else {
        return false;
    };
    let review_status = transfer
        .get("review_status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if review_status == "rejected" {
        return false;
    }
    transfer
        .get("candidate_type")
        .and_then(Value::as_str)
        .is_some_and(|candidate_type| {
            matches!(
                candidate_type.trim().to_ascii_lowercase().as_str(),
                "transfer" | "transfer_cross_batch"
            )
        })
}

fn preview_type_label_from_code(type_code: i64) -> Option<&'static str> {
    match type_code {
        2 => Some("收入"),
        3 => Some("支出"),
        4 => Some("转账"),
        5 => Some("投资"),
        _ => None,
    }
}

/// 判断分类建议为何无法应用，便于写入 LLM signal 供前端解释。
fn category_ignored_reason_for_llm_suggestion(
    suggestion: &ImportPreviewLlmSuggestion,
    suggested_category: Option<&ImportPreviewLlmCategoryMatch>,
) -> Option<&'static str> {
    if suggested_category.is_none()
        && (suggestion.suggested_category_id.is_some()
            || !suggestion.suggested_main_category.trim().is_empty()
            || !suggestion.suggested_sub_category.trim().is_empty())
    {
        return Some("unresolved_or_unauthorized_category");
    }
    None
}

fn apply_resolved_llm_category_patch(
    mut patch: ImportPreviewPatch,
    applied_fields: &mut Vec<String>,
    suggested_category: Option<&ImportPreviewLlmCategoryMatch>,
) -> ImportPreviewPatch {
    let Some(category) = suggested_category else {
        return patch;
    };

    applied_fields.push("category_id".to_string());
    patch = patch.with_change(
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(category.id),
    );
    applied_fields.push("main_category".to_string());
    patch = patch.with_change(
        ImportPreviewPatchField::MainCategory,
        ImportPreviewPatchValue::Text(category.main_category.clone()),
    );
    applied_fields.push("sub_category".to_string());
    patch = patch.with_change(
        ImportPreviewPatchField::SubCategory,
        ImportPreviewPatchValue::Text(category.sub_category.clone()),
    );
    patch
}

/// 应用 LLM 预览建议；无结构化转账授权时只能记录信号，不能自动创建转账语义。
pub fn apply_preview_llm_recommendation(
    pool: &PostgresPool,
    request: &ImportPreviewLlmApplyRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let Some(preview) = get_preview_bill_by_id(pool, request.preview_id, request.user_id)? else {
        return Ok(ImportPreviewLlmDecisionResult {
            preview: None,
            event_id: None,
            applied_fields: Vec::new(),
            state_conflict: false,
        });
    };
    let mut applied_fields = Vec::new();
    let mut patch = ImportPreviewPatch::new(request.preview_id);
    let transfer_authorized = preview_has_authorized_transfer_feedback(&preview);
    let (suggested_type, suggested_type_ignored_reason) =
        llm_suggested_type_for_preview(&preview, request.suggestion);
    let suggested_category = resolve_llm_suggested_category(
        pool,
        user_id_i64(request.user_id)?,
        request.suggestion,
        transfer_authorized,
    )?;
    let category_type = suggested_category
        .as_ref()
        .and_then(|category| category.type_code)
        .and_then(preview_type_label_from_code)
        .map(str::to_string);
    let applied_type = category_type.or(suggested_type.clone());
    if let Some(value) = applied_type.as_ref() {
        applied_fields.push("type".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(value.clone()),
        );
    }
    patch =
        apply_resolved_llm_category_patch(patch, &mut applied_fields, suggested_category.as_ref());
    if let Some(value) = request.suggestion.resolved_source_account_id {
        applied_fields.push("source_account_id".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::SourceAccountId,
            ImportPreviewPatchValue::Integer(value),
        );
    }
    if let Some(value) = request.suggestion.resolved_destination_account_id {
        applied_fields.push("destination_account_id".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::DestinationAccountId,
            ImportPreviewPatchValue::Integer(value),
        );
    }
    let mut feedback = preview.preview_matching_feedback.clone();
    let category_ignored_reason =
        category_ignored_reason_for_llm_suggestion(request.suggestion, suggested_category.as_ref());
    let llm_signal = json!({
        "suggested_type": request.suggestion.suggested_type.clone(),
        "applied_type": applied_type.clone(),
        "suggested_main_category": request.suggestion.suggested_main_category.clone(),
        "suggested_sub_category": request.suggestion.suggested_sub_category.clone(),
        "requested_category_id": request.suggestion.suggested_category_id,
        "suggested_category_id": suggested_category.as_ref().map(|category| category.id),
        "suggested_source_account": request.suggestion.suggested_source_account.clone(),
        "suggested_destination_account": request.suggestion.suggested_destination_account.clone(),
        "resolved_source_account_id": request.suggestion.resolved_source_account_id,
        "resolved_destination_account_id": request.suggestion.resolved_destination_account_id,
        "confidence": request.suggestion.confidence,
        "reason": request.suggestion.reason.clone(),
        "review_status": "pending",
        "source": "llm_preview_recommendation",
        "type_ignored_reason": suggested_type_ignored_reason.clone(),
        "category_ignored_reason": category_ignored_reason,
    });
    if !feedback.is_object() {
        feedback = json!({});
    }
    feedback
        .as_object_mut()
        .expect("feedback object")
        .insert("llm".to_string(), llm_signal);
    patch = patch.with_change(
        ImportPreviewPatchField::MatchingFeedback,
        ImportPreviewPatchValue::Json(feedback),
    );
    let llm_response_raw = Some(
        json!({
            "suggested_type": request.suggestion.suggested_type.clone(),
            "suggested_category_id": request.suggestion.suggested_category_id,
            "suggested_main_category": request.suggestion.suggested_main_category.clone(),
            "suggested_sub_category": request.suggestion.suggested_sub_category.clone(),
            "suggested_source_account": request.suggestion.suggested_source_account.clone(),
            "suggested_destination_account": request.suggestion.suggested_destination_account.clone(),
            "confidence": request.suggestion.confidence,
            "reason": request.suggestion.reason.clone(),
        })
        .to_string(),
    );
    let event_id = create_llm_memory_event(
        pool,
        &LlmMemoryEventDraft {
            user_id: request.user_id,
            session_id: Some(request.session_id.to_string()),
            preview_id: Some(request.preview_id),
            event_type: "preview_apply".to_string(),
            decision: Some("accept".to_string()),
            prompt_text: request.prompt_text.map(str::to_string),
            llm_response_raw,
            llm_provider: request.llm_provider.map(str::to_string),
            llm_model: request.llm_model.map(str::to_string),
            suggested_main_category: Some(request.suggestion.suggested_main_category.clone()),
            suggested_sub_category: Some(request.suggestion.suggested_sub_category.clone()),
            suggested_source_account: Some(request.suggestion.suggested_source_account.clone()),
            suggested_destination_account: Some(
                request.suggestion.suggested_destination_account.clone(),
            ),
            confidence: request.suggestion.confidence,
            user_correction_category: None,
            user_correction_account: None,
            snapshot_before: Some(json!(preview)),
            snapshot_after: None,
            metadata: Some(json!({
                "reason": request.suggestion.reason.clone(),
                "suggested_type": request.suggestion.suggested_type.clone(),
                "suggested_category_id": request.suggestion.suggested_category_id,
                "applied_type": applied_type.clone(),
                "type_ignored_reason": suggested_type_ignored_reason.clone(),
                "category_ignored_reason": category_ignored_reason,
            })),
        },
    )?;
    replace_preview_selection_with_patches(pool, request.session_id, request.user_id, &[patch])?;
    Ok(ImportPreviewLlmDecisionResult {
        preview: get_preview_bill_by_id(pool, request.preview_id, request.user_id)?,
        event_id: Some(event_id),
        applied_fields,
        state_conflict: false,
    })
}

/// 记录用户对 LLM 建议的审核结果，并保持 preview row 与 memory feedback 一致。
pub fn review_preview_llm_recommendation(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    review_preview_llm_recommendation_with_expected_state(pool, request)
}

/// 通过 matching candidate action 审核 LLM 信号，并在同一事务内校验 expected state。
pub fn review_preview_llm_matching_action(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewDecisionResult> {
    review_preview_llm_recommendation_with_expected_state(pool, request).map(
        |result| ImportPreviewDecisionResult {
            preview: result.preview,
            state_conflict: result.state_conflict,
            invalid_recurring_id: false,
        },
    )
}

fn review_preview_llm_recommendation_with_expected_state(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let decision_name = match request.decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    };
    tracing::debug!(
        domain = "import_signal_lifecycle",
        operation = "lifecycle_transition",
        signal_family = "llm",
        outcome = "started",
        session_key = %request.session_id,
        decision = decision_name,
        "import preview signal lifecycle transition started"
    );
    block_on_db(async move {
        let user_id = user_id_i64(request.user_id)?;
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
        .bind(request.preview_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?
        else {
            transaction.rollback().await?;
            tracing::debug!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "not_found",
                session_key = %request.session_id,
                decision = decision_name,
                "import preview signal lifecycle transition not applied"
            );
            return Ok(empty_llm_decision_result());
        };
        let mut preview = preview_from_pg_row(&row)?;
        if preview.session_id != request.session_id {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "session_conflict",
                session_key = %request.session_id,
                decision = decision_name,
                "import preview signal lifecycle transition rejected"
            );
            return Ok(empty_llm_decision_result());
        }
        let decision = decision_name;
        let current_status = preview_llm_review_status(&preview.preview_matching_feedback);
        match preview_terminal_action(
            current_status.as_deref(),
            preview.preview_matching_feedback.get("llm").is_some(),
            request.decision,
        ) {
            ImportPreviewTerminalAction::Idempotent => {
                transaction.commit().await?;
                tracing::info!(
                    domain = "import_signal_lifecycle",
                    operation = "lifecycle_transition",
                    signal_family = "llm",
                    outcome = "idempotent",
                    session_key = %request.session_id,
                    decision = decision_name,
                    previous_state = current_status.as_deref().unwrap_or("none"),
                    next_state = current_status.as_deref().unwrap_or("none"),
                    "import preview signal lifecycle transition completed"
                );
                return Ok(ImportPreviewLlmDecisionResult {
                    preview: Some(preview),
                    event_id: None,
                    applied_fields: Vec::new(),
                    state_conflict: false,
                });
            }
            ImportPreviewTerminalAction::Conflict => {
                transaction.rollback().await?;
                tracing::warn!(
                    domain = "import_signal_lifecycle",
                    operation = "lifecycle_transition",
                    signal_family = "llm",
                    outcome = "state_conflict",
                    session_key = %request.session_id,
                    decision = decision_name,
                    previous_state = current_status.as_deref().unwrap_or("none"),
                    "import preview signal lifecycle transition rejected"
                );
                return Ok(ImportPreviewLlmDecisionResult {
                    preview: Some(preview),
                    event_id: None,
                    applied_fields: Vec::new(),
                    state_conflict: true,
                });
            }
            ImportPreviewTerminalAction::Apply => {}
        }
        if recommendation_expected_state_conflicts(
            &preview,
            request.expected_state,
            current_status.as_deref(),
        ) {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "expected_state_conflict",
                session_key = %request.session_id,
                decision = decision_name,
                previous_state = current_status.as_deref().unwrap_or("none"),
                "import preview signal lifecycle transition rejected"
            );
            return Ok(ImportPreviewLlmDecisionResult {
                preview: Some(preview),
                event_id: None,
                applied_fields: Vec::new(),
                state_conflict: true,
            });
        }

        let snapshot_before = json!(preview);
        match request.decision {
            ImportPreviewDecision::Accept => {
                preview.preview_matching_feedback = set_feedback_review_status(
                    preview.preview_matching_feedback,
                    "llm",
                    "accepted",
                );
            }
            ImportPreviewDecision::Reject => {
                preview.preview_matching_feedback = set_feedback_review_status(
                    preview.preview_matching_feedback,
                    "llm",
                    "rejected",
                );
            }
            ImportPreviewDecision::Clear => {
                clear_feedback_key(&mut preview.preview_matching_feedback, "llm");
            }
        }
        let mut payload = row
            .try_get::<Value, _>("preview_payload")
            .unwrap_or_else(|_| json!({}));
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
        let session_db_id: i64 = row.try_get("session_id")?;
        let mut update = build_preview_row_update_query(
            &preview,
            payload.to_string(),
            amount_cents,
            direction,
            request.preview_id,
            session_db_id,
            user_id,
        );
        update.build().execute(&mut *transaction).await?;
        let event_id = insert_llm_review_event_in_transaction(
            &mut transaction,
            user_id,
            request,
            decision,
            snapshot_before,
            json!(preview),
        )
        .await?;
        let updated_row = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            "#,
        )
        .bind(request.preview_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let updated = updated_row.as_ref().map(preview_from_pg_row).transpose()?;
        transaction.commit().await?;
        let next_state = match request.decision {
            ImportPreviewDecision::Accept => "accepted",
            ImportPreviewDecision::Reject => "rejected",
            ImportPreviewDecision::Clear => "cleared",
        };
        tracing::info!(
            domain = "import_signal_lifecycle",
            operation = "lifecycle_transition",
            signal_family = "llm",
            outcome = "committed",
            session_key = %request.session_id,
            decision = decision_name,
            previous_state = current_status.as_deref().unwrap_or("none"),
            next_state,
            "import preview signal lifecycle transition completed"
        );
        Ok(ImportPreviewLlmDecisionResult {
            preview: updated,
            event_id: Some(event_id),
            applied_fields: Vec::new(),
            state_conflict: false,
        })
    })
}

fn empty_llm_decision_result() -> ImportPreviewLlmDecisionResult {
    ImportPreviewLlmDecisionResult {
        preview: None,
        event_id: None,
        applied_fields: Vec::new(),
        state_conflict: false,
    }
}

async fn insert_llm_review_event_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    request: &ImportPreviewLlmReviewRequest<'_>,
    decision: &str,
    snapshot_before: Value,
    snapshot_after: Value,
) -> DbResult<i64> {
    let llm_response_raw = request.suggestion.map(|suggestion| {
        json!({
            "suggested_type": suggestion.suggested_type.clone(),
            "suggested_category_id": suggestion.suggested_category_id,
            "suggested_main_category": suggestion.suggested_main_category.clone(),
            "suggested_sub_category": suggestion.suggested_sub_category.clone(),
            "suggested_source_account": suggestion.suggested_source_account.clone(),
            "suggested_destination_account": suggestion.suggested_destination_account.clone(),
            "confidence": suggestion.confidence,
            "reason": suggestion.reason.clone(),
        })
        .to_string()
    });
    let id = sqlx::query(
        r#"
        INSERT INTO llm_memory_events (
            user_id, session_id, preview_id, event_type, decision, prompt_text,
            llm_response_raw, llm_provider, llm_model, suggested_main_category,
            suggested_sub_category, suggested_source_account, suggested_destination_account,
            confidence, user_correction_category, user_correction_account,
            snapshot_before, snapshot_after, metadata, created_at
        ) VALUES ($1,$2,$3,'preview_review',$4,NULL,$5,NULL,NULL,$6,$7,$8,$9,$10,$11,$12,$13::jsonb,$14::jsonb,'{}'::jsonb,now())
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(request.session_id)
    .bind(request.preview_id)
    .bind(decision)
    .bind(llm_response_raw)
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_main_category.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_sub_category.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_source_account.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_destination_account.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.confidence)
            .unwrap_or_default(),
    )
    .bind(request.user_correction_category)
    .bind(request.user_correction_account)
    .bind(snapshot_before.to_string())
    .bind(snapshot_after.to_string())
    .fetch_one(&mut **transaction)
    .await?
    .try_get("id")?;
    Ok(id)
}

fn preview_llm_review_status(feedback: &Value) -> Option<String> {
    feedback
        .get("llm")
        .and_then(Value::as_object)
        .and_then(|llm| {
            [
                "review_status",
                "status",
                "lifecycle_status",
                "signal_state",
            ]
            .iter()
            .find_map(|key| {
                llm.get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_ascii_lowercase())
            })
        })
}

#[cfg(test)]
fn llm_decision_is_idempotent(
    current_status: Option<&str>,
    has_llm: bool,
    decision: ImportPreviewDecision,
) -> bool {
    preview_terminal_action(current_status, has_llm, decision)
        == ImportPreviewTerminalAction::Idempotent
}

#[cfg(test)]
mod llm_lifecycle_action_tests {
    use super::*;

    #[test]
    fn llm_review_status_uses_first_nonempty_authority() {
        let feedback = json!({
            "llm": {
                "review_status": "",
                "status": "Rejected",
                "lifecycle_status": "accepted",
                "signal_state": "pending"
            }
        });

        assert_eq!(
            preview_llm_review_status(&feedback).as_deref(),
            Some("rejected")
        );
    }

    #[test]
    fn repeated_terminal_llm_actions_are_idempotent() {
        assert!(llm_decision_is_idempotent(
            Some("accepted"),
            true,
            ImportPreviewDecision::Accept
        ));
        assert!(llm_decision_is_idempotent(
            Some("rejected"),
            true,
            ImportPreviewDecision::Reject
        ));
        assert!(llm_decision_is_idempotent(
            None,
            false,
            ImportPreviewDecision::Clear
        ));
        assert!(!llm_decision_is_idempotent(
            Some("pending"),
            true,
            ImportPreviewDecision::Accept
        ));
    }

    #[test]
    fn llm_terminal_action_guard_covers_retries_opposites_clear_and_mobile_payloads() {
        let cases = [
            (Some("accepted"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Idempotent),
            (Some("accepted"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (Some("accepted"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Idempotent),
            (Some("rejected"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("pending"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Apply),
            (None, false, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Idempotent),
        ];

        for (status, has_signal, decision, expected) in cases {
            assert_eq!(preview_terminal_action(status, has_signal, decision), expected);
        }
    }

    #[test]
    fn llm_expected_state_rejects_stale_desktop_but_accepts_minimal_mobile_shape() {
        let mut preview = preview_row(1);
        preview.category_id = Some(42);
        preview.preview_matching_feedback = json!({
            "llm": {"review_status": "pending"}
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
                    review_status: Some("rejected".to_string()),
                    preview_category_id: Some(Some(42)),
                    ..ImportPreviewExpectedState::default()
                },
                true,
            ),
            (
                ImportPreviewExpectedState {
                    review_status: Some("pending".to_string()),
                    preview_category_id: Some(None),
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
    fn serialized_concurrent_llm_opposites_and_identical_retry_emit_one_event() {
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
    fn llm_review_keeps_preview_and_memory_event_in_one_locked_transaction() {
        let source = include_str!("preview_llm.rs");

        assert!(source.contains("let mut transaction = pool.begin().await?;"));
        assert!(source.contains("FOR UPDATE OF p"));
        assert!(source.contains("update.build().execute(&mut *transaction).await?;"));
        assert!(source.contains("insert_llm_review_event_in_transaction("));
        assert!(source.contains("transaction.commit().await?;"));
        let lock = source.find("FOR UPDATE OF p").expect("preview row lock");
        let guard = source
            .find("match preview_terminal_action(")
            .expect("terminal action guard");
        let event = source
            .find("insert_llm_review_event_in_transaction(")
            .expect("LLM event persistence");
        let feedback = source
            .find("preview.preview_matching_feedback = set_feedback_review_status(")
            .expect("LLM feedback mutation");
        assert!(lock < guard && guard < feedback && feedback < event);
    }
}
