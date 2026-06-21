/// 按固定顺序执行分类/周期/learning/account_rules 投影；账户规则必须在语义投影后最后运行。
#[tracing::instrument(level = "debug", skip_all)]
async fn apply_import_intelligence_chain(
    connection: &Connection,
    user_id: UserId,
    drafts: &mut [ImportPreviewDraft],
) -> Result<ImportIntelligenceStats, bill_analyser_db::DbError> {
    let user_id_i64 = user_id_i64_for_sql(user_id)?;
    let load_started_at = Instant::now();
    let categories = load_import_intelligence_categories(connection, user_id_i64).await?;
    let categories_by_id = categories
        .iter()
        .cloned()
        .map(|category| (category.id, category))
        .collect::<BTreeMap<_, _>>();
    let category_values = categories
        .iter()
        .map(import_intelligence_category_value)
        .collect::<Vec<_>>();
    let category_rules = ImportIntelligenceRuleSet::from_rules(
        load_import_intelligence_category_rules(connection, user_id_i64, &categories_by_id)
            .await?,
    );
    let accounts = load_import_intelligence_accounts(connection, user_id_i64).await?;
    let account_values = accounts
        .iter()
        .map(import_intelligence_account_value)
        .collect::<Vec<_>>();
    let account_rule_candidates =
        load_import_intelligence_account_rules(connection, user_id_i64).await?;
    let account_rules = compile_account_rule_candidates(&account_rule_candidates);
    let learning_rules = load_import_intelligence_learning_rules(connection, user_id_i64).await?;
    let recurring_templates =
        load_import_intelligence_recurring_templates(connection, user_id_i64).await?;
    let transfer_category = default_transfer_category(
        &categories,
        &categories_by_id,
        load_user_cash_transfer_category_id(connection, user_id_i64).await?,
    )
    .cloned();
    let mut stats = ImportIntelligenceStats {
        _elapsed_load_ms: import_stage_elapsed_ms(load_started_at),
        ..Default::default()
    };
    for draft in &mut *drafts {
        ensure_base_matching_feedback(draft);
        let before_category = (
            draft.preview_type.clone(),
            draft.preview_main_category.clone(),
            draft.preview_sub_category.clone(),
        );
        let category_started_at = Instant::now();
        let preview_rule_text = import_preview_rule_text(draft);
        demote_unauthorized_transfer_preview(draft, &categories);
        if is_transfer_protected_preview(draft) {
            if !apply_transfer_category_rule_match(draft, category_rules.for_type(4), &preview_rule_text) {
                apply_transfer_default_category(draft, &categories, transfer_category.as_ref());
            }
        } else if !apply_non_transfer_category_rule_match(draft, &category_rules, &preview_rule_text)
        {
            apply_builtin_category_rule_fallback(draft, &categories, &preview_rule_text);
        }
        stats.elapsed_category_rule_ns += category_started_at.elapsed().as_nanos();

        let recurring_started_at = Instant::now();
        if let Some(candidate) = best_recurring_candidate_for_draft(draft, &recurring_templates) {
            apply_recurring_candidate(draft, candidate);
            stats.recurring_projected += 1;
        }
        stats.elapsed_recurring_rule_ns += recurring_started_at.elapsed().as_nanos();
        let learning_started_at = Instant::now();
        if let Some(result) = apply_learning_rule_match(
            connection,
            user_id_i64,
            draft,
            &learning_rules,
            &categories_by_id,
            &category_values,
            &account_values,
        ) {
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
        apply_account_rule_match_after_semantic_projection(draft, &account_rules, &accounts);
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

fn user_id_i64_for_sql(user_id: UserId) -> Result<i64, bill_analyser_db::DbError> {
    i64::try_from(user_id.get())
        .map_err(|_| bill_analyser_db::DbError::InvalidOperation("invalid user id".to_string()))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_import_intelligence_categories(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceCategory>, bill_analyser_db::DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_type, path, name
        FROM categories
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id")?;
            let category_type: Option<String> = row.try_get("category_type")?;
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) =
                import_intelligence_category_parts(path.as_deref(), &name);
            Ok(ImportIntelligenceCategory {
                id,
                type_code: preview_type_code(category_type.as_deref().unwrap_or_default())
                    .unwrap_or(3),
                main_category,
                sub_category,
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_user_cash_transfer_category_id(
    connection: &Connection,
    user_id: i64,
) -> Result<Option<i64>, bill_analyser_db::DbError> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM categories
        WHERE user_id = $1
          AND is_active = true
          AND category_type IN ('4', 'transfer', '转账')
          AND (path ILIKE '%现金%' OR name ILIKE '%现金%')
        ORDER BY display_order ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(connection)
    .await
    .map_err(Into::into)
}

fn default_transfer_category<'a>(
    categories: &'a [ImportIntelligenceCategory],
    categories_by_id: &'a BTreeMap<i64, ImportIntelligenceCategory>,
    user_cash_transfer_category_id: Option<i64>,
) -> Option<&'a ImportIntelligenceCategory> {
    user_cash_transfer_category_id
        .and_then(|category_id| categories_by_id.get(&category_id))
        .filter(|category| category.type_code == 4)
        .or_else(|| categories.iter().find(|category| category.type_code == 4))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_import_intelligence_category_rules(
    connection: &Connection,
    user_id: i64,
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
) -> Result<Vec<ImportIntelligenceRule>, bill_analyser_db::DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_id, priority, rule_expression
        FROM category_rules
        WHERE user_id = $1 AND enabled = true
        ORDER BY priority ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .filter_map(|row| {
            let category_id = row.try_get::<i64, _>("category_id").ok()?;
            let category = categories_by_id.get(&category_id)?;
            let expression = row.try_get::<Value, _>("rule_expression").ok()?;
            let id = row.try_get("id").ok()?;
            let priority = row.try_get::<i32, _>("priority").ok()?;
            let regex_enabled = rule_expression_regex_enabled(&expression);
            let rule_expression = rule_expression_string(&expression);
            let compiled_expression = compile_rule_expression(&rule_expression, regex_enabled);
            Some(Ok(ImportIntelligenceRule {
                id,
                category_id,
                category_type: category.type_code,
                main_category: category.main_category.clone(),
                sub_category: category.sub_category.clone(),
                priority: i64::from(priority),
                rule_expression,
                regex_enabled,
                compiled_expression,
            }))
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_import_intelligence_accounts(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceAccount>, bill_analyser_db::DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, name
        FROM accounts
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportIntelligenceAccount {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_import_intelligence_account_rules(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<AccountRuleCandidate>, bill_analyser_db::DbError> {
    let rows = sqlx::query(
        r#"
        SELECT ar.id, ar.account_id, ar.rule_expression, ar.regex_enabled, ar.enabled, ar.priority
        FROM account_rules ar
        JOIN accounts a ON a.id = ar.account_id AND a.user_id = ar.user_id
        WHERE ar.user_id = $1 AND ar.enabled = true AND a.is_active = true
        ORDER BY ar.priority ASC, ar.id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            let expression: Value = row.try_get("rule_expression")?;
            Ok(AccountRuleCandidate {
                rule_id: row.try_get("id")?,
                account_id: row.try_get("account_id")?,
                rule_expression: rule_expression_string(&expression),
                regex_enabled: row.try_get("regex_enabled")?,
                enabled: row.try_get("enabled")?,
                priority: i64::from(row.try_get::<i32, _>("priority")?),
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_import_intelligence_learning_rules(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceLearningRule>, bill_analyser_db::DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, recommendation_type, recommendation_key, metadata
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND status IN ('accepted', 'auto_applied', 'green')
        ORDER BY updated_at DESC, id DESC
        LIMIT 1000
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            let metadata: Value = row.try_get("metadata")?;
            Ok(ImportIntelligenceLearningRule {
                id: row.try_get("id")?,
                parser_id: text_from_json(&metadata, "parser_id"),
                composite_hash: text_from_json(&metadata, "composite_hash"),
                match_features: string_map_from_json(metadata.get("match_features")),
                learned_type: optional_text_from_json(&metadata, "learned_type")
                    .or_else(|| row.try_get::<String, _>("recommendation_type").ok()),
                learned_category_id: optional_i64_from_json(&metadata, "learned_category_id"),
                learned_source_account_id: optional_i64_from_json(
                    &metadata,
                    "learned_source_account_id",
                ),
                learned_destination_account_id: optional_i64_from_json(
                    &metadata,
                    "learned_destination_account_id",
                ),
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_import_intelligence_recurring_templates(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceRecurringTemplate>, bill_analyser_db::DbError> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, transaction_type, source_amount_minor_units,
               source_account_id, scheduled_next_date, scheduled_start_date,
               metadata
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = 2
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            let metadata: Value = row.try_get("metadata").unwrap_or_else(|_| json!({}));
            let amount_minor: i64 = row.try_get("source_amount_minor_units").unwrap_or_default();
            let source_account_id: Option<i64> = row.try_get("source_account_id").ok().flatten();
            Ok(ImportIntelligenceRecurringTemplate {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                bill_type: row.try_get::<Option<String>, _>("transaction_type")?.unwrap_or_default(),
                amount_cents: amount_minor,
                account: source_account_id
                    .map(|value| value.to_string())
                    .or_else(|| optional_text_from_json(&metadata, "account"))
                    .unwrap_or_default(),
                counterparty: text_from_json(&metadata, "counterparty"),
                next_date: row
                    .try_get::<Option<String>, _>("scheduled_next_date")?
                    .unwrap_or_default(),
                start_date: row
                    .try_get::<Option<String>, _>("scheduled_start_date")?
                    .unwrap_or_default(),
            })
        })
        .collect()
}
