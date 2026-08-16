/// 按固定顺序执行分类/周期/learning/account_rules 投影；账户规则必须在语义投影后最后运行。
#[tracing::instrument(level = "debug", skip_all)]
fn evaluate_import_intelligence_snapshot(
    connection: &Connection,
    drafts: &mut [ImportPreviewDraft],
    context: &ImportStage2ContextSnapshot,
) -> Result<ImportIntelligenceStats, bill_analyser_db::DbError> {
    let user_id_i64 = context.user_id;
    let categories = &context.categories;
    let categories_by_id = &context.categories_by_id;
    let category_values = &context.category_values;
    let category_rules = &context.category_rules;
    let accounts = &context.accounts;
    let account_values = &context.account_values;
    let account_rules = &context.account_rules;
    let learning_rules = &context.learning_rules;
    let recurring_templates = &context.recurring_templates;
    let transfer_category = context.transfer_category.as_ref();
    let mut stats = ImportIntelligenceStats::default();
    for draft in &mut *drafts {
        ensure_base_matching_feedback(draft);
        canonicalize_manual_category_identity(draft, categories_by_id);
        let manual_category = manual_category_identity(draft);
        let before_category = (
            draft.preview_type.clone(),
            draft.preview_main_category.clone(),
            draft.preview_sub_category.clone(),
        );
        let category_started_at = Instant::now();
        let preview_rule_text = import_preview_rule_text(draft);
        demote_unauthorized_transfer_preview(draft, categories);
        if manual_category.is_some() {
            restore_manual_category_identity(draft, manual_category.as_ref());
        } else if is_transfer_protected_preview(draft) {
            if !apply_transfer_category_rule_match(
                draft,
                category_rules.for_type(4),
                &preview_rule_text,
            ) {
                apply_transfer_default_category(draft, categories, transfer_category);
            }
        } else if !apply_non_transfer_category_rule_match(
            draft,
            category_rules,
            &preview_rule_text,
        ) {
            apply_builtin_category_rule_fallback(draft, categories, &preview_rule_text);
        }
        stats.elapsed_category_rule_ns += category_started_at.elapsed().as_nanos();

        let recurring_started_at = Instant::now();
        if let Some(candidate) = best_recurring_candidate_for_draft(draft, recurring_templates) {
            apply_recurring_candidate(draft, candidate);
            stats.recurring_projected += 1;
        }
        stats.elapsed_recurring_rule_ns += recurring_started_at.elapsed().as_nanos();
        let learning_started_at = Instant::now();
        if let Some(result) = apply_learning_rule_match(
            connection,
            user_id_i64,
            draft,
            learning_rules,
            categories_by_id,
            category_values,
            account_values,
        )? {
            if result.auto_applied {
                stats.learning_applied += 1;
            }
            if let Some(rule_id) = result.rule_id {
                increment_applied_learning_rules(connection, user_id_i64, &[rule_id])?;
            }
        }
        stats.elapsed_learning_rule_ns += learning_started_at.elapsed().as_nanos();

        let before_account_rule = (
            draft.preview_source_account_id,
            draft.preview_destination_account_id,
        );
        // Account recognition is intentionally last in stage2: type/transfer,
        // recurring, and learning projections may still change the account
        // role, transaction type, or explicit accounts that rules must consume.
        let account_started_at = Instant::now();
        apply_account_rule_match_after_semantic_projection(draft, account_rules, accounts);
        stats.elapsed_account_rule_ns += account_started_at.elapsed().as_nanos();
        let baseline_started_at = Instant::now();
        persist_stage2_actionable_baseline(draft);
        stats.elapsed_stage2_baseline_ns += baseline_started_at.elapsed().as_nanos();

        if before_category
            != (
                draft.preview_type.clone(),
                draft.preview_main_category.clone(),
                draft.preview_sub_category.clone(),
            )
        {
            stats.category_matched += 1;
        }
        if before_account_rule
            != (
                draft.preview_source_account_id,
                draft.preview_destination_account_id,
            )
        {
            stats.account_matched += 1;
        }
    }

    Ok(stats)
}

fn apply_account_rule_match_after_semantic_projection(
    draft: &mut ImportPreviewDraft,
    account_rules: &[CompiledAccountRuleCandidate],
    accounts: &[ImportIntelligenceAccount],
) -> bool {
    match preview_type_code(&draft.preview_type) {
        Some(4) => apply_transfer_account_rule_match(draft, account_rules, accounts),
        Some(5) => apply_investment_account_rule_match(draft, account_rules, accounts),
        Some(2 | 3) => apply_standard_account_rule_match(draft, account_rules, accounts),
        _ => false,
    }
}

#[derive(Debug, Clone)]
struct ManualCategoryIdentity {
    preview_type: String,
    category_id: Option<i64>,
    main_category: String,
    sub_category: String,
}

fn preview_manual_identity_field_owned(draft: &ImportPreviewDraft, field: &str) -> bool {
    draft
        .preview_matching_feedback
        .pointer(&format!("/annotation/manual_fields/{field}"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn canonicalize_manual_category_identity(
    draft: &mut ImportPreviewDraft,
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
) {
    if !preview_manual_identity_field_owned(draft, "category_id") {
        return;
    }
    let Some(category) = draft
        .category_id
        .and_then(|category_id| categories_by_id.get(&category_id))
    else {
        return;
    };
    draft.preview_main_category = category.main_category.clone();
    draft.preview_sub_category = category.sub_category.clone();
}

fn manual_category_identity(draft: &ImportPreviewDraft) -> Option<ManualCategoryIdentity> {
    preview_manual_identity_field_owned(draft, "category_id").then(|| ManualCategoryIdentity {
        preview_type: draft.preview_type.clone(),
        category_id: draft.category_id,
        main_category: draft.preview_main_category.clone(),
        sub_category: draft.preview_sub_category.clone(),
    })
}

fn restore_manual_category_identity(
    draft: &mut ImportPreviewDraft,
    identity: Option<&ManualCategoryIdentity>,
) {
    let Some(identity) = identity else {
        return;
    };
    draft.preview_type = identity.preview_type.clone();
    draft.category_id = identity.category_id;
    draft.preview_main_category = identity.main_category.clone();
    draft.preview_sub_category = identity.sub_category.clone();
}
