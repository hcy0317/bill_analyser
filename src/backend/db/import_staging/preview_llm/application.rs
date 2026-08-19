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
                category_names_from_postgres_path(path.as_deref(), &name);
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
