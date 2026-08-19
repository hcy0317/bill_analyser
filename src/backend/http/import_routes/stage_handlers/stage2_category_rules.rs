fn apply_transfer_category_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[CategoryRuleCandidate],
    preview_rule_text: &str,
) -> bool {
    if !is_transfer_protected_preview(draft) {
        return false;
    }
    let combined_text = import_preview_transfer_rule_text(preview_rule_text, draft);
    apply_category_rule_match_filtered(draft, rules, &[4], &combined_text)
}

fn apply_non_transfer_category_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[CategoryRuleCandidate],
    preview_rule_text: &str,
) -> bool {
    if is_transfer_protected_preview(draft) {
        return false;
    }
    apply_category_rule_match_filtered(draft, rules, &[2, 3, 5], preview_rule_text)
}

fn apply_category_rule_match_filtered(
    draft: &mut ImportPreviewDraft,
    rules: &[CategoryRuleCandidate],
    allowed_category_types: &[i32],
    combined_text: &str,
) -> bool {
    let Some(rule) =
        select_category_rule_candidate(rules, allowed_category_types, combined_text)
    else {
        return false;
    };
    draft.preview_main_category = rule.main_category.clone();
    draft.preview_sub_category = rule.sub_category.clone();
    draft.category_id = Some(rule.category_id);
    normalize_preview_type_for_category(draft, i64::from(rule.category_type));
    matching_feedback_object_mut(draft).insert(
        "category_rule".to_string(),
        json!({
            "rule_id": rule.id,
            "category_id": rule.category_id,
            "priority": rule.priority,
            "reason": "category rule expression matched",
            "review_status": "auto_applied",
        }),
    );
    true
}

fn apply_builtin_category_rule_fallback(
    draft: &mut ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
    preview_rule_text: &str,
) -> bool {
    if preview_type_code(&draft.preview_type)
        .is_some_and(|expected_type| preview_category_matches_type(draft, categories, expected_type))
    {
        return false;
    }

    let combined_text = normalize_category_fallback_text(preview_rule_text);
    let current_category_text = normalize_category_fallback_text(&format!(
        "{} {}",
        draft.preview_main_category, draft.preview_sub_category
    ));

    for fallback in BUILTIN_CATEGORY_RULE_FALLBACKS {
        if !category_type_matches_preview(fallback.category_type, &draft.preview_type)
            || !fallback.keywords.iter().any(|keyword| {
                let keyword = normalize_category_fallback_text(keyword);
                !keyword.is_empty()
                    && (combined_text.contains(&keyword)
                        || current_category_text.contains(&keyword))
            })
        {
            continue;
        }

        let Some(category) = find_import_intelligence_category(
            categories,
            fallback.category_type,
            fallback.main_category,
            fallback.sub_category,
        ) else {
            continue;
        };

        draft.preview_main_category = category.main_category.clone();
        draft.preview_sub_category = category.sub_category.clone();
        draft.category_id = Some(category.id);
        normalize_preview_type_for_category(draft, category.type_code);
        matching_feedback_object_mut(draft).insert(
            "category_rule".to_string(),
            json!({
                "rule_id": Value::Null,
                "category_id": category.id,
                "priority": Value::Null,
                "reason": "built-in category rule fallback matched import text",
                "review_status": "auto_applied",
            }),
        );
        return true;
    }

    false
}

fn normalize_category_fallback_text(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(char::is_whitespace, "")
}

fn find_import_intelligence_category<'a>(
    categories: &'a [ImportIntelligenceCategory],
    category_type: i64,
    main_category: &str,
    sub_category: &str,
) -> Option<&'a ImportIntelligenceCategory> {
    categories.iter().find(|category| {
        category.type_code == category_type
            && category.main_category.trim() == main_category
            && category.sub_category.trim() == sub_category
    })
}

fn persist_stage2_actionable_baseline(draft: &mut ImportPreviewDraft) {
    let snapshot = import_preview_stage2_snapshot(draft);

    matching_feedback_object_mut(draft).insert(
        "stage2_baseline".to_string(),
        snapshot,
    );
}

fn import_preview_stage2_snapshot(draft: &ImportPreviewDraft) -> Value {
    json!({
        "preview_type": draft.preview_type,
        "category_id": draft.category_id,
        "preview_main_category": draft.preview_main_category,
        "preview_sub_category": draft.preview_sub_category,
        "preview_source_account_id": draft.preview_source_account_id,
        "preview_destination_account_id": draft.preview_destination_account_id,
    })
}

fn import_preview_transfer_applied_snapshot(draft: &ImportPreviewDraft) -> Value {
    json!({
        "preview_type": draft.preview_type,
        "category_id": draft.category_id,
        "preview_main_category": draft.preview_main_category,
        "preview_sub_category": draft.preview_sub_category,
        "preview_source_account_id": draft.preview_source_account_id,
        "preview_destination_account_id": draft.preview_destination_account_id,
        "preview_recurring_id": draft.preview_recurring_id,
        "preview_recurring_name": draft.preview_recurring_name,
        "preview_recurring_candidate_count": draft.preview_recurring_candidate_count,
        "preview_recurring_match_score": draft.preview_recurring_match_score,
        "preview_recurring_match_reasons": draft.preview_recurring_match_reasons,
        "preview_recurring_matched_date": draft.preview_recurring_matched_date,
    })
}

fn transfer_chain_entry_for_roles<'a>(
    source_chain: &'a [Value],
    roles: &[&str],
    fallback_index: Option<usize>,
) -> Option<&'a Value> {
    source_chain
        .iter()
        .find(|entry| {
            let role = transfer_entry_text(entry.get("role"))
                .map(|value| value.trim().to_ascii_lowercase())
                .unwrap_or_default();
            roles.iter().any(|candidate| role == *candidate)
        })
        .or_else(|| fallback_index.and_then(|index| source_chain.get(index)))
}

fn transfer_entry_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    value.as_f64().map(|number| number.to_string())
}
