fn enforce_import_preview_invariants(drafts: &mut [ImportPreviewDraft]) {
    for draft in drafts {
        if !is_transfer_protected_preview(draft) {
            continue;
        }

        draft.preview_type = "转账".to_string();
        if transfer_preview_requires_account_review(draft) {
            draft.preview_selected = false;
            annotate_transfer_account_review(draft);
        } else {
            clear_transfer_account_review_annotation(draft);
        }
        persist_transfer_pending_applied_snapshot(draft);
    }
}

fn persist_transfer_pending_applied_snapshot(draft: &mut ImportPreviewDraft) {
    let applied_snapshot = import_preview_transfer_applied_snapshot(draft);
    let feedback = matching_feedback_object_mut(draft);
    let Some(transfer) = feedback.get_mut("transfer").and_then(Value::as_object_mut) else {
        return;
    };
    let review_status = transfer
        .get("review_status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if matches!(review_status.as_str(), "accepted" | "rejected") {
        return;
    }
    transfer
        .entry("applied_preview".to_string())
        .or_insert(applied_snapshot);
}

fn transfer_preview_requires_account_review(draft: &ImportPreviewDraft) -> bool {
    draft.preview_source_account_id.is_none()
        || draft.preview_destination_account_id.is_none()
        || draft.preview_source_account_id == draft.preview_destination_account_id
}

fn annotate_transfer_account_review(draft: &mut ImportPreviewDraft) {
    if draft
        .preview_matching_feedback
        .pointer("/annotation/type")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "no_income_expenditure")
    {
        return;
    }

    matching_feedback_object_mut(draft).insert(
        "annotation".to_string(),
        json!({
            "status": "needs_review",
            "type": "transfer_account_direction",
            "review_status": "requires_account_review",
            "suppressed": true,
            "reason": "transfer preview is missing source or destination account",
        }),
    );
}

fn clear_transfer_account_review_annotation(draft: &mut ImportPreviewDraft) {
    if draft
        .preview_matching_feedback
        .pointer("/annotation/type")
        .and_then(Value::as_str)
        .is_none_or(|value| value != "transfer_account_direction")
    {
        return;
    }

    matching_feedback_object_mut(draft).remove("annotation");
}

fn best_recurring_candidate_for_draft(
    draft: &ImportPreviewDraft,
    templates: &[ImportIntelligenceRecurringTemplate],
) -> Option<ImportRecurringCandidateMatch> {
    templates
        .iter()
        .filter_map(|template| recurring_template_match(draft, template))
        .max_by(|left, right| {
            left.match_score
                .partial_cmp(&right.match_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn recurring_template_match(
    draft: &ImportPreviewDraft,
    template: &ImportIntelligenceRecurringTemplate,
) -> Option<ImportRecurringCandidateMatch> {
    if !recurring_type_matches(&draft.preview_type, &template.bill_type) {
        return None;
    }
    let mut score: f64 = 0.0;
    let mut reasons = Vec::new();
    if recurring_amount_matches(draft.preview_amount_cents, template.amount_cents) {
        score += 0.45;
        reasons.push("amount".to_string());
    } else {
        return None;
    }
    let matched_date = recurring_matched_date(&draft.preview_date, template)?;
    score += 0.35;
    reasons.push("schedule".to_string());
    if recurring_account_matches(draft, template) {
        score += 0.10;
        reasons.push("account".to_string());
    }
    if !template.counterparty.trim().is_empty()
        && draft
            .preview_counterparty
            .trim()
            .contains(template.counterparty.trim())
    {
        score += 0.10;
        reasons.push("counterparty".to_string());
    }
    Some(ImportRecurringCandidateMatch {
        id: template.id,
        name: template.name.clone(),
        match_score: (score * 100.0).round() / 100.0,
        match_reasons: reasons,
        matched_occurrence_date: matched_date,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_recurring_candidate(draft: &mut ImportPreviewDraft, candidate: ImportRecurringCandidateMatch) {
    draft.preview_recurring_id = Some(candidate.id);
    draft.preview_recurring_name = candidate.name.clone();
    draft.preview_recurring_candidate_count = 1;
    draft.preview_recurring_match_score = candidate.match_score;
    draft.preview_recurring_match_reasons = candidate.match_reasons.join("|");
    draft.preview_recurring_matched_date = candidate.matched_occurrence_date.clone();
    matching_feedback_object_mut(draft).insert(
        "recurring".to_string(),
        json!({
            "id": candidate.id,
            "name": candidate.name,
            "candidate_count": 1,
            "match_score": candidate.match_score,
            "match_reasons": candidate.match_reasons,
            "matched_date": candidate.matched_occurrence_date,
            "review_status": "pending",
        }),
    );
}

fn increment_applied_learning_rules(
    _connection: &Connection,
    _user_id: i64,
    _rule_ids: &[i64],
) -> Result<(), bill_analyser_db::DbError> {
    Ok(())
}

fn import_preview_rule_text(draft: &ImportPreviewDraft) -> String {
    let mut parts = vec![
        draft.preview_counterparty.clone(),
        draft.preview_description.clone(),
        draft.preview_payment_method.clone(),
        draft.preview_parser_id.clone(),
    ];
    if let Some(tags) = draft.preview_parser_tags.as_ref() {
        if let Some(values) = tags.as_array() {
            parts.extend(values.iter().filter_map(value_to_text));
        }
    }
    parts
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn category_type_matches_preview(category_type: i64, preview_type: &str) -> bool {
    if matches!(category_type, 0 | 1) {
        return true;
    }
    match preview_type_code(preview_type) {
        Some(preview_type) => category_type == preview_type || category_type == 5 && preview_type == 3,
        None => true,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_preview_type_for_category(draft: &mut ImportPreviewDraft, category_type: i64) {
    let normalized = match category_type {
        2 => Some("收入"),
        3 => Some("支出"),
        4 => Some("转账"),
        5 => Some("投资"),
        _ => None,
    };
    if let Some(normalized) = normalized {
        draft.preview_type = normalized.to_string();
    }
}

fn preview_type_code(preview_type: &str) -> Option<i64> {
    match preview_type.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => Some(2),
        "支出" | "expense" | "3" => Some(3),
        "转账" | "transfer" | "4" => Some(4),
        "投资" | "investment" | "5" => Some(5),
        _ => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_account_tokens(draft: &ImportPreviewDraft) -> Vec<String> {
    let mut tokens = vec![
        draft.preview_parser_id.clone(),
        draft.preview_payment_method.clone(),
        draft.preview_counterparty.clone(),
        draft.preview_description.clone(),
    ];
    if let Some(tags) = draft.preview_parser_tags.as_ref() {
        if let Some(values) = tags.as_array() {
            tokens.extend(values.iter().filter_map(value_to_text));
        }
    }
    tokens
        .into_iter()
        .flat_map(|token| {
            let normalized = normalize_account_match_text(&token);
            [
                normalized.clone(),
                normalized
                    .strip_prefix("parser:")
                    .unwrap_or(&normalized)
                    .to_string(),
                normalized
                    .strip_prefix("channel:")
                    .unwrap_or(&normalized)
                    .to_string(),
            ]
        })
        .filter(|token| !token.is_empty())
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_account_match_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn recurring_type_matches(preview_type: &str, recurring_type: &str) -> bool {
    let recurring_type = recurring_type.trim();
    recurring_type.is_empty()
        || preview_type_code(preview_type) == preview_type_code(recurring_type)
        || preview_type.trim() == recurring_type
}

fn recurring_amount_matches(preview_amount_cents: i64, template_amount_cents: i64) -> bool {
    preview_amount_cents.abs() == template_amount_cents.abs()
}

fn recurring_account_matches(
    draft: &ImportPreviewDraft,
    template: &ImportIntelligenceRecurringTemplate,
) -> bool {
    let account = template.account.trim();
    if account.is_empty() {
        return false;
    }
    if let Ok(account_id) = account.parse::<i64>() {
        return draft.preview_source_account_id == Some(account_id)
            || draft.preview_destination_account_id == Some(account_id);
    }
    let account = normalize_account_match_text(account);
    import_preview_account_tokens(draft)
        .iter()
        .any(|token| token == &account || token.contains(&account))
}

fn recurring_matched_date(
    preview_date: &str,
    template: &ImportIntelligenceRecurringTemplate,
) -> Option<String> {
    let preview_date = parse_import_preview_date(preview_date)?;
    for candidate in [&template.next_date, &template.start_date] {
        let candidate_date = parse_import_preview_date(candidate)?;
        let delta_days = (preview_date - candidate_date).num_days().abs();
        if delta_days <= 3 {
            return Some(candidate_date.format("%Y-%m-%d").to_string());
        }
    }
    None
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_import_preview_date(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.len() >= 10 {
        NaiveDate::parse_from_str(&trimmed[..10], "%Y-%m-%d").ok()
    } else {
        None
    }
}
