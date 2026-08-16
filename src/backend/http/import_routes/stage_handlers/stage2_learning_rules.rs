/// 尝试应用 deterministic learning 规则，必须尊重转账授权和 category/account 身份边界。
fn prepare_learning_rule_match(
    draft_index: usize,
    user_id: i64,
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceLearningRule],
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
) -> Result<Option<PreparedImportLearningRuleMatch>, bill_analyser_db::DbError> {
    let transfer_protected = is_transfer_protected_preview(draft);
    let Some(features) = build_composite_match_features(
        &draft.preview_parser_id,
        &draft.preview_counterparty,
        &draft.preview_description,
        &draft.preview_payment_method,
    ) else {
        return Ok(None);
    };
    let composite_hash = composite_hash_from_features(&features);
    let mut best: Option<(&ImportIntelligenceLearningRule, f64, String, String)> = None;
    for rule in rules {
        if !rule.parser_id.trim().is_empty()
            && !draft
                .preview_parser_id
                .trim()
                .eq_ignore_ascii_case(rule.parser_id.trim())
        {
            continue;
        }
        let candidate = if !rule.composite_hash.trim().is_empty()
            && rule.composite_hash.trim() == composite_hash
        {
            Some((
                1.0,
                "exact".to_string(),
                "composite exact match".to_string(),
            ))
        } else {
            score_learning_rule_similarity(&features, &rule.match_features).and_then(|score| {
                let score_value = score
                    .get("score")
                    .and_then(Value::as_f64)
                    .unwrap_or_default();
                (score_value >= 0.72 && learning_similarity_has_semantic_anchor(&score)).then(
                    || {
                        let reason = score
                            .get("reason_parts")
                            .cloned()
                            .unwrap_or_else(|| json!([]))
                            .to_string();
                        (score_value, "similar".to_string(), reason)
                    },
                )
            })
        };
        let Some((score, mode, reason)) = candidate else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(_, best_score, _, _)| score > *best_score)
        {
            best = Some((rule, score, mode, reason));
        }
    }
    let Some((rule, score, mode, reason)) = best else {
        return Ok(None);
    };
    let raw_learned_type = rule
        .learned_type
        .as_deref()
        .and_then(normalize_transaction_type_text);
    let learned_type =
        learning_projection_type_for_transfer_authority(raw_learned_type, transfer_protected);
    let candidate_preview_type = if transfer_protected {
        "转账"
    } else {
        learned_type
            .as_deref()
            .unwrap_or_else(|| draft.preview_type.trim())
    };
    let learned_category = if let Some(category_id) = rule.learned_category_id {
        let Some(category) = categories_by_id.get(&category_id) else {
            if !transfer_protected {
                annotate_learning_rule_skip(
                    draft,
                    rule.id,
                    "learned category is missing from current category table",
                );
            }
            return Ok(None);
        };
        if transfer_protected && category.type_code != 4 {
            None
        } else if !category_type_matches_learning_projection(
            category.type_code,
            candidate_preview_type,
        ) {
            if !transfer_protected {
                annotate_learning_rule_skip(
                    draft,
                    rule.id,
                    "learned category type is incompatible with preview type",
                );
            }
            return Ok(None);
        } else {
            Some(category)
        }
    } else {
        None
    };
    let recommended_category_id = learned_category.map(|category| category.id);
    if learned_type.is_none()
        && recommended_category_id.is_none()
        && rule.learned_source_account_id.is_none()
        && rule.learned_destination_account_id.is_none()
    {
        return Ok(None);
    }
    let mut recommended_draft = draft.clone();
    apply_learning_rule_projection(
        &mut recommended_draft,
        learned_type.as_deref(),
        learned_category,
        rule,
        transfer_protected,
    );
    if learned_type.is_none()
        && recommended_category_id.is_none()
        && import_preview_stage2_snapshot(&recommended_draft)
            == import_preview_stage2_snapshot(draft)
    {
        return Ok(None);
    }
    let recommendation_key =
        build_import_learning_recommendation_key(&ImportLearningRecommendationKeyInput {
            user_id,
            recommended_type: learned_type
                .clone()
                .unwrap_or_else(|| recommended_draft.preview_type.clone()),
            recommended_category_id,
            recommended_source_account_id: rule.learned_source_account_id,
            recommended_destination_account_id: rule.learned_destination_account_id,
            transaction_type_scope: draft.preview_type.clone(),
            parser_bucket: draft.preview_parser_id.clone(),
            counterparty_bucket: draft.preview_counterparty.clone(),
            payment_bucket: draft.preview_payment_method.clone(),
            description_bucket: draft.preview_description.clone(),
            amount_bucket: Some(
                amount_cents_bucket(Some(&json!(draft.preview_amount_cents))).to_string(),
            ),
            transfer_protected,
            ..ImportLearningRecommendationKeyInput::default()
        });
    u64::try_from(user_id)
        .map_err(|_| {
            bill_analyser_db::DbError::InvalidOperation(
                "learning lifecycle received an invalid user id".to_string(),
            )
        })
        .and_then(|value| {
            UserId::new(value).map_err(|_| {
                bill_analyser_db::DbError::InvalidOperation(
                    "learning lifecycle received an invalid user id".to_string(),
                )
            })
        })?;
    Ok(Some(PreparedImportLearningRuleMatch {
        draft_index,
        rule: rule.clone(),
        learned_type,
        learned_category: learned_category.cloned(),
        recommended_category_id,
        recommended_draft,
        recommendation_key,
        transfer_protected,
        score,
        mode,
        reason,
    }))
}

fn apply_prepared_learning_rule_match(
    draft: &mut ImportPreviewDraft,
    prepared: PreparedImportLearningRuleMatch,
    lifecycle: &ImportLearningLifecycleView,
    category_values: &[Value],
    account_values: &[Value],
) -> Option<ImportLearningRuleMatchResult> {
    if lifecycle.suppressed {
        return None;
    }
    let PreparedImportLearningRuleMatch {
        rule,
        learned_type,
        learned_category,
        recommended_category_id,
        recommended_draft,
        recommendation_key,
        transfer_protected,
        score,
        mode,
        reason,
        ..
    } = prepared;
    let mut rule_payload = Map::new();
    rule_payload.insert("learned_type".to_string(), json!(learned_type));
    rule_payload.insert(
        "learned_category_id".to_string(),
        json!(recommended_category_id),
    );
    rule_payload.insert(
        "learned_source_account_id".to_string(),
        json!(rule.learned_source_account_id),
    );
    rule_payload.insert(
        "learned_destination_account_id".to_string(),
        json!(rule.learned_destination_account_id),
    );
    let summary =
        build_learning_rule_result_summary(&rule_payload, category_values, account_values);
    let previous_preview = import_preview_stage2_snapshot(draft);
    let applied_preview = import_preview_stage2_snapshot(&recommended_draft);
    let auto_applied = lifecycle.auto_apply_enabled;
    if auto_applied {
        apply_learning_rule_projection(
            draft,
            learned_type.as_deref(),
            learned_category.as_ref(),
            &rule,
            transfer_protected,
        );
    }
    let recommended_type_value = applied_preview
        .get("preview_type")
        .cloned()
        .unwrap_or_else(|| json!(draft.preview_type.clone()));
    matching_feedback_object_mut(draft).insert(
        "learning".to_string(),
        json!({
            "rule_id": rule.id,
            "score": score,
            "mode": mode,
            "reason": reason,
            "summary": summary,
            "recommended_type": recommended_type_value,
            "review_status": if auto_applied { "auto_applied" } else { "pending" },
            "auto_apply": auto_applied,
            "source": "import_learning_rules",
            "recommendation_key": recommendation_key.clone(),
            "lifecycle_status": lifecycle.status.clone(),
            "signal_state": lifecycle.signal_state.clone(),
            "accepted_count": lifecycle.accepted_count,
            "rejected_count": lifecycle.rejected_count,
            "auto_applied_count": lifecycle.auto_applied_count,
            "previous_preview": previous_preview,
            "applied_preview": applied_preview,
        }),
    );
    Some(ImportLearningRuleMatchResult {
        auto_applied,
    })
}

/// 把 learning 命中投影到 preview draft，并写入可审核 feedback 与 expected-state 证据。
fn apply_learning_rule_projection(
    draft: &mut ImportPreviewDraft,
    learned_type: Option<&str>,
    learned_category: Option<&ImportIntelligenceCategory>,
    rule: &ImportIntelligenceLearningRule,
    transfer_protected: bool,
) {
    let manual_category = preview_manual_identity_field_owned(draft, "category_id");
    if let Some(learned_type) = learned_type.filter(|_| !manual_category) {
        draft.preview_type = learned_type.to_string();
    }
    if let Some(category) = learned_category.filter(|_| !manual_category) {
        draft.preview_main_category = category.main_category.clone();
        draft.preview_sub_category = category.sub_category.clone();
        draft.category_id = Some(category.id);
        normalize_preview_type_for_category(draft, category.type_code);
    }
    if let Some(account_id) = rule.learned_source_account_id {
        if !preview_manual_identity_field_owned(draft, "source_account_id")
            && (!transfer_protected || draft.preview_source_account_id.is_none())
        {
            draft.preview_source_account_id = Some(account_id);
        }
    }
    if let Some(account_id) = rule.learned_destination_account_id {
        if !preview_manual_identity_field_owned(draft, "destination_account_id")
            && (!transfer_protected || draft.preview_destination_account_id.is_none())
        {
            draft.preview_destination_account_id = Some(account_id);
        }
    }
}

fn learning_similarity_has_semantic_anchor(score: &Value) -> bool {
    score
        .get("matched_fields")
        .and_then(Value::as_array)
        .is_some_and(|fields| {
            fields
                .iter()
                .filter_map(Value::as_str)
                .any(|field| matches!(field, "counterparty" | "description"))
        })
}

fn annotate_learning_rule_skip(draft: &mut ImportPreviewDraft, rule_id: i64, reason: &str) {
    matching_feedback_object_mut(draft).insert(
        "learning".to_string(),
        json!({
            "rule_id": rule_id,
            "review_status": "skipped",
            "auto_apply": false,
            "reason": reason,
            "source": "import_learning_rules",
        }),
    );
}

fn apply_transfer_default_category(
    draft: &mut ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
    default_category: Option<&ImportIntelligenceCategory>,
) -> bool {
    if !is_transfer_protected_preview(draft) || preview_category_matches_type(draft, categories, 4)
    {
        return false;
    }

    let Some(category) = default_category else {
        return false;
    };
    draft.preview_main_category = category.main_category.clone();
    draft.preview_sub_category = category.sub_category.clone();
    draft.category_id = Some(category.id);
    true
}

fn preview_category_matches_type(
    draft: &ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
    expected_type: i64,
) -> bool {
    let main_category = draft.preview_main_category.trim();
    let sub_category = draft.preview_sub_category.trim();
    if main_category.is_empty() && sub_category.is_empty() {
        return false;
    }

    if let Some(category_id) = draft.category_id {
        return categories
            .iter()
            .any(|category| category.id == category_id && category.type_code == expected_type);
    }

    categories.iter().any(|category| {
        category.type_code == expected_type
            && category.main_category.trim() == main_category
            && category.sub_category.trim() == sub_category
    })
}

fn learning_projection_type_for_transfer_authority(
    raw_learned_type: Option<String>,
    transfer_authority: bool,
) -> Option<String> {
    match (transfer_authority, raw_learned_type.as_deref()) {
        (true, Some("转账")) => raw_learned_type,
        (true, _) => None,
        (false, Some("转账")) => None,
        (false, _) => raw_learned_type,
    }
}

fn category_type_matches_learning_projection(category_type: i64, preview_type: &str) -> bool {
    if matches!(category_type, 0 | 1) {
        return true;
    }
    let Some(preview_type) = preview_type_code(preview_type) else {
        return category_type != 4;
    };
    if category_type == 4 || preview_type == 4 {
        return category_type == preview_type;
    }
    matches!(category_type, 2 | 3 | 5) && matches!(preview_type, 2 | 3 | 5)
}

fn is_transfer_protected_preview(draft: &ImportPreviewDraft) -> bool {
    let Some(transfer_feedback) = draft
        .preview_matching_feedback
        .get("transfer")
        .and_then(Value::as_object)
    else {
        return false;
    };

    let review_status = transfer_feedback
        .get("review_status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if review_status == "rejected" {
        return false;
    }

    transfer_feedback
        .get("candidate_type")
        .and_then(Value::as_str)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "transfer" | "transfer_cross_batch"
            )
        })
        .unwrap_or(false)
}
