pub async fn import_dedup_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match required_session_id_from_payload(object) {
        Ok(session_id) => session_id,
        Err(response) => return route_response(response),
    };
    let include_preview =
        bool_field_from_object(object, &["include_preview", "includePreview"]).unwrap_or(false);
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }

    let stage_started_at = Instant::now();
    let template_query_started_at = Instant::now();
    let templates =
        match get_unprocessed_templates_for_dedup(runtime.connection(), &session_id, user_id) {
            Ok(templates) => templates,
            Err(error) => return route_response(db_error_response(error)),
        };
    eprintln!(
        "[bill analyser import] stage2 templates loaded user_id={} session_id={} templates={} elapsed_ms={}",
        user_id.get(),
        session_id,
        templates.len(),
        import_stage_elapsed_ms(template_query_started_at)
    );
    let dedup_started_at = Instant::now();
    let dedup_input = dedup_bills_from_parser_templates(&templates);
    let dedup_result = SmartDeduplicationEngine.process(dedup_input);
    eprintln!(
        "[bill analyser import] stage2 smart dedup complete user_id={} session_id={} original={} kept={} removed={} dedup_elapsed_ms={}",
        user_id.get(),
        session_id,
        dedup_result.original_count,
        dedup_result.kept_bills.len(),
        dedup_result.removed_count,
        import_stage_elapsed_ms(dedup_started_at)
    );
    let mut preview_drafts = preview_drafts_from_dedup_bills(&dedup_result.kept_bills);
    let intelligence_stats = match apply_import_intelligence_chain(
        runtime.connection_mut(),
        user_id,
        preview_drafts.as_mut_slice(),
    ) {
        Ok(stats) => stats,
        Err(error) => return route_response(db_error_response(error)),
    };
    let preview_insert_started_at = Instant::now();
    let inserted_preview = match insert_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_drafts,
    ) {
        Ok(inserted) => inserted,
        Err(error) => return route_response(db_error_response(error)),
    };
    eprintln!(
        "[bill analyser import] stage2 preview inserted user_id={} session_id={} preview_rows={} insert_elapsed_ms={}",
        user_id.get(),
        session_id,
        inserted_preview,
        import_stage_elapsed_ms(preview_insert_started_at)
    );
    let status_update_started_at = Instant::now();
    let updated_templates = match mark_unprocessed_parser_templates_processed_for_session(
        runtime.connection_mut(),
        &session_id,
        user_id,
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    if let Err(error) = update_import_session_status(
        runtime.connection(),
        &ImportSessionStatusUpdate {
            session_id: session_id.clone(),
            user_id,
            status: "preview".to_string(),
            total_parsed: Some(usize_to_i64(templates.len())),
            total_preview: Some(usize_to_i64(inserted_preview)),
            total_confirmed: None,
        },
    ) {
        return route_response(db_error_response(error));
    }
    eprintln!(
        "[bill analyser import] stage2 status updated user_id={} session_id={} template_rows={} updated_template_rows={} total_elapsed_ms={} status_elapsed_ms={}",
        user_id.get(),
        session_id,
        templates.len(),
        updated_templates,
        import_stage_elapsed_ms(stage_started_at),
        import_stage_elapsed_ms(status_update_started_at)
    );
    let preview = if include_preview {
        match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
            Ok(rows) => rows.into_iter().map(preview_row_to_value).collect(),
            Err(error) => return route_response(db_error_response(error)),
        }
    } else {
        Vec::new()
    };
    route_response(import_stage_dedup_success(ImportStageDedupData {
        session_id,
        preview,
        preview_included: include_preview,
        total: dedup_result.original_count,
        after_dedup: inserted_preview,
        dedup_stats: json!({
            "removed": dedup_result.removed_count,
            "duplicates": dedup_result.duplicate_groups.len(),
            "transfer_pairs": dedup_result.transfer_pairs.len(),
            "split_merge": dedup_result.split_groups.len(),
        }),
        match_stats: json!({
            "runtime": "rust",
            "category_matched": intelligence_stats.category_matched,
            "account_matched": intelligence_stats.account_matched,
            "learning_applied": intelligence_stats.learning_applied,
            "recurring_projected": intelligence_stats.recurring_projected,
            "database_candidates": 0,
            "provider_bypassed": false,
        }),
    }))
}

#[derive(Debug, Clone, Default)]
struct ImportIntelligenceStats {
    category_matched: usize,
    account_matched: usize,
    learning_applied: usize,
    recurring_projected: usize,
}

#[derive(Debug, Clone)]
struct ImportIntelligenceCategory {
    id: i64,
    type_code: i64,
    main_category: String,
    sub_category: String,
}

#[derive(Debug, Clone)]
struct ImportIntelligenceRule {
    id: i64,
    category_id: i64,
    category_type: i64,
    main_category: String,
    sub_category: String,
    priority: i64,
    rule_expression: String,
    regex_enabled: bool,
}

#[derive(Debug, Clone)]
struct ImportIntelligenceAccount {
    id: i64,
    name: String,
    aliases: Vec<String>,
}

#[derive(Debug, Clone)]
struct ImportIntelligenceLearningRule {
    id: i64,
    parser_id: String,
    composite_hash: String,
    match_features: BTreeMap<String, String>,
    learned_type: Option<String>,
    learned_category_id: Option<i64>,
    learned_source_account_id: Option<i64>,
    learned_destination_account_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct ImportIntelligenceRecurringTemplate {
    id: i64,
    name: String,
    bill_type: String,
    amount: f64,
    account: String,
    counterparty: String,
    next_date: String,
    start_date: String,
}

#[derive(Debug, Clone)]
struct ImportRecurringCandidateMatch {
    id: i64,
    name: String,
    match_score: f64,
    match_reasons: Vec<String>,
    matched_occurrence_date: String,
}

fn apply_import_intelligence_chain(
    connection: &mut Connection,
    user_id: UserId,
    drafts: &mut [ImportPreviewDraft],
) -> rusqlite::Result<ImportIntelligenceStats> {
    let user_id_i64 = user_id_i64_for_sql(user_id)?;
    let categories = load_import_intelligence_categories(connection, user_id_i64)?;
    let category_values = categories
        .iter()
        .map(|category| {
            json!({
                "id": category.id,
                "main_category": category.main_category,
                "sub_category": category.sub_category,
                "type": category.type_code,
            })
        })
        .collect::<Vec<_>>();
    let categories_by_id = categories
        .iter()
        .cloned()
        .map(|category| (category.id, category))
        .collect::<BTreeMap<_, _>>();
    let category_rules =
        load_import_intelligence_category_rules(connection, user_id_i64, &categories_by_id)?;
    let accounts = load_import_intelligence_accounts(connection, user_id_i64)?;
    let account_values = accounts
        .iter()
        .map(|account| json!({"id": account.id, "name": account.name}))
        .collect::<Vec<_>>();
    let learning_rules = load_import_intelligence_learning_rules(connection, user_id_i64)?;
    let recurring_templates = load_import_intelligence_recurring_templates(connection, user_id_i64)?;

    let mut stats = ImportIntelligenceStats::default();
    let mut applied_learning_rule_ids = Vec::new();
    for draft in &mut *drafts {
        ensure_base_matching_feedback(draft);
        if apply_category_rule_match(draft, &category_rules) {
            stats.category_matched += 1;
        }
        if apply_account_alias_match(draft, &accounts) {
            stats.account_matched += 1;
        }
        if let Some(applied_rule_id) = apply_learning_rule_match(
            draft,
            &learning_rules,
            &categories_by_id,
            &category_values,
            &account_values,
        ) {
            stats.learning_applied += 1;
            applied_learning_rule_ids.push(applied_rule_id);
        }
        if let Some(candidate) = best_recurring_candidate_for_draft(draft, &recurring_templates) {
            apply_recurring_candidate(draft, candidate);
            stats.recurring_projected += 1;
        }
    }

    stats.category_matched = drafts
        .iter()
        .filter(|draft| {
            !draft.preview_main_category.trim().is_empty()
                || !draft.preview_sub_category.trim().is_empty()
        })
        .count();
    stats.account_matched = drafts
        .iter()
        .filter(|draft| {
            draft.preview_source_account_id.is_some()
                || draft.preview_destination_account_id.is_some()
        })
        .count();

    if !applied_learning_rule_ids.is_empty() {
        increment_applied_learning_rules(connection, user_id_i64, &applied_learning_rule_ids)?;
    }
    Ok(stats)
}

fn user_id_i64_for_sql(user_id: UserId) -> rusqlite::Result<i64> {
    i64::try_from(user_id.get()).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn import_intelligence_table_exists(
    connection: &Connection,
    table_name: &str,
) -> rusqlite::Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
}

fn sql_column_or_default(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    default_sql: &str,
) -> rusqlite::Result<String> {
    if table_has_column(connection, table_name, column_name)
        .map_err(|_| rusqlite::Error::InvalidQuery)?
    {
        Ok(column_name.to_string())
    } else {
        Ok(format!("{default_sql} AS {column_name}"))
    }
}

fn load_import_intelligence_categories(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<ImportIntelligenceCategory>> {
    if !import_intelligence_table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let type_expr = sql_column_or_default(connection, "categories", "type", "1")?;
    let priority_expr = sql_column_or_default(connection, "categories", "priority", "0")?;
    let mut statement = connection.prepare(&format!(
        "
        SELECT id, {type_expr}, main_category, sub_category, {priority_expr}
        FROM categories
        WHERE user_id = ?1
        ORDER BY priority ASC, id ASC
        "
    ))?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(ImportIntelligenceCategory {
            id: row.get("id")?,
            type_code: row.get::<_, Option<i64>>("type")?.unwrap_or(1),
            main_category: row
                .get::<_, Option<String>>("main_category")?
                .unwrap_or_default(),
            sub_category: row
                .get::<_, Option<String>>("sub_category")?
                .unwrap_or_default(),
        })
    })?;
    rows.collect()
}

fn load_import_intelligence_category_rules(
    connection: &Connection,
    user_id: i64,
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
) -> rusqlite::Result<Vec<ImportIntelligenceRule>> {
    if !import_intelligence_table_exists(connection, "category_rules")? {
        return Ok(Vec::new());
    }
    let regex_expr = sql_column_or_default(connection, "category_rules", "regex_enabled", "0")?;
    let has_enabled = table_has_column(connection, "category_rules", "enabled")?;
    let enabled_expr = if has_enabled {
        "enabled".to_string()
    } else {
        "1 AS enabled".to_string()
    };
    let enabled_filter = if has_enabled { "enabled != 0" } else { "1 = 1" };
    let has_priority = table_has_column(connection, "category_rules", "priority")?;
    let priority_expr = if has_priority {
        "priority".to_string()
    } else {
        "100 AS priority".to_string()
    };
    let priority_order = if has_priority { "priority" } else { "100" };
    let mut statement = connection.prepare(&format!(
        "
        SELECT id, category_id, rule_expression, {regex_expr}, {priority_expr}, {enabled_expr}
        FROM category_rules
        WHERE user_id = ?1 AND {enabled_filter}
        ORDER BY {priority_order} ASC, id ASC
        "
    ))?;
    let rows = statement.query_map(params![user_id], |row| {
        let category_id = row.get::<_, i64>("category_id")?;
        let category = categories_by_id.get(&category_id);
        Ok(ImportIntelligenceRule {
            id: row.get("id")?,
            category_id,
            category_type: category.map_or(1, |category| category.type_code),
            main_category: category
                .map(|category| category.main_category.clone())
                .unwrap_or_default(),
            sub_category: category
                .map(|category| category.sub_category.clone())
                .unwrap_or_default(),
            priority: row.get::<_, Option<i64>>("priority")?.unwrap_or(100),
            rule_expression: row
                .get::<_, Option<String>>("rule_expression")?
                .unwrap_or_default(),
            regex_enabled: row.get::<_, Option<i64>>("regex_enabled")?.unwrap_or(0) != 0,
        })
    })?;
    rows.collect()
}

fn load_import_intelligence_accounts(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<ImportIntelligenceAccount>> {
    if !import_intelligence_table_exists(connection, "accounts")? {
        return Ok(Vec::new());
    }
    let aliases_expr = sql_column_or_default(connection, "accounts", "aliases", "NULL")?;
    let has_hidden = table_has_column(connection, "accounts", "hidden")?;
    let hidden_expr = if has_hidden {
        "hidden".to_string()
    } else {
        "0 AS hidden".to_string()
    };
    let hidden_filter = if has_hidden { "hidden = 0" } else { "1 = 1" };
    let mut statement = connection.prepare(&format!(
        "
        SELECT id, name, {aliases_expr}, {hidden_expr}
        FROM accounts
        WHERE user_id = ?1 AND {hidden_filter}
        ORDER BY id ASC
        "
    ))?;
    let rows = statement.query_map(params![user_id], |row| {
        let name = row.get::<_, Option<String>>("name")?.unwrap_or_default();
        let raw_aliases = row.get::<_, Option<String>>("aliases")?;
        let mut aliases = parse_account_aliases(raw_aliases.as_deref());
        aliases.push(name.clone());
        Ok(ImportIntelligenceAccount {
            id: row.get("id")?,
            name,
            aliases,
        })
    })?;
    rows.collect()
}

fn load_import_intelligence_learning_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<ImportIntelligenceLearningRule>> {
    if !import_intelligence_table_exists(connection, "import_learning_rules")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "
        SELECT id, parser_id, composite_match_hash, normalized_match_value,
               match_features_json, learned_type, learned_category_id,
               learned_source_account_id, learned_destination_account_id
        FROM import_learning_rules
        WHERE user_id = ?1 AND enabled = 1 AND match_type = 'composite'
        ORDER BY applied_count DESC, id ASC
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        let raw_features = row
            .get::<_, Option<String>>("match_features_json")?
            .unwrap_or_default();
        Ok(ImportIntelligenceLearningRule {
            id: row.get("id")?,
            parser_id: row
                .get::<_, Option<String>>("parser_id")?
                .unwrap_or_default(),
            composite_hash: row
                .get::<_, Option<String>>("composite_match_hash")?
                .filter(|value| !value.trim().is_empty())
                .or_else(|| row.get::<_, Option<String>>("normalized_match_value").ok().flatten())
                .unwrap_or_default(),
            match_features: parse_learning_features(&raw_features),
            learned_type: row.get("learned_type")?,
            learned_category_id: row.get("learned_category_id")?,
            learned_source_account_id: row.get("learned_source_account_id")?,
            learned_destination_account_id: row.get("learned_destination_account_id")?,
        })
    })?;
    rows.collect()
}

fn load_import_intelligence_recurring_templates(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<ImportIntelligenceRecurringTemplate>> {
    if !import_intelligence_table_exists(connection, "recurring_bills")? {
        return Ok(Vec::new());
    }
    let account_expr = sql_column_or_default(connection, "recurring_bills", "account", "''")?;
    let counterparty_expr =
        sql_column_or_default(connection, "recurring_bills", "counterparty", "''")?;
    let start_date_expr = sql_column_or_default(connection, "recurring_bills", "start_date", "''")?;
    let next_date_expr = sql_column_or_default(connection, "recurring_bills", "next_date", "''")?;
    let has_enabled = table_has_column(connection, "recurring_bills", "enabled")?;
    let enabled_expr = if has_enabled {
        "enabled".to_string()
    } else {
        "1 AS enabled".to_string()
    };
    let enabled_filter = if has_enabled { "enabled != 0" } else { "1 = 1" };
    let mut statement = connection.prepare(&format!(
        "
        SELECT id, name, type, amount, {account_expr}, {counterparty_expr},
               {start_date_expr}, {next_date_expr}, {enabled_expr}
        FROM recurring_bills
        WHERE user_id = ?1 AND {enabled_filter}
        ORDER BY id ASC
        "
    ))?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(ImportIntelligenceRecurringTemplate {
            id: row.get("id")?,
            name: row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            bill_type: row.get::<_, Option<String>>("type")?.unwrap_or_default(),
            amount: row.get::<_, Option<f64>>("amount")?.unwrap_or_default(),
            account: row.get::<_, Option<String>>("account")?.unwrap_or_default(),
            counterparty: row
                .get::<_, Option<String>>("counterparty")?
                .unwrap_or_default(),
            next_date: row
                .get::<_, Option<String>>("next_date")?
                .unwrap_or_default(),
            start_date: row
                .get::<_, Option<String>>("start_date")?
                .unwrap_or_default(),
        })
    })?;
    rows.collect()
}

fn ensure_base_matching_feedback(draft: &mut ImportPreviewDraft) {
    let parser_id = draft.preview_parser_id.clone();
    let parser_tags = draft.preview_parser_tags.clone().unwrap_or_else(|| json!([]));
    let payment_method = draft.preview_payment_method.clone();
    let counterparty = draft.preview_counterparty.clone();
    let dedup_type = draft
        .dedup_type
        .clone()
        .unwrap_or_else(|| "remaining".to_string());
    let dedup_source_ids = draft.dedup_source_ids.clone();
    let dedup_source_count = dedup_source_ids.len();
    let feedback = matching_feedback_object_mut(draft);
    feedback.entry("parser".to_string()).or_insert_with(|| {
        json!({
            "parser_id": parser_id,
            "parser_tags": parser_tags,
            "payment_method": payment_method,
            "counterparty": counterparty,
        })
    });
    feedback.entry("dedup".to_string()).or_insert_with(|| {
        json!({
            "type": dedup_type,
            "source_ids": dedup_source_ids,
            "source_count": dedup_source_count,
        })
    });
}

fn matching_feedback_object_mut(draft: &mut ImportPreviewDraft) -> &mut Map<String, Value> {
    if !draft.preview_matching_feedback.is_object() {
        draft.preview_matching_feedback = json!({});
    }
    draft
        .preview_matching_feedback
        .as_object_mut()
        .expect("matching feedback object")
}

fn apply_category_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceRule],
) -> bool {
    let combined_text = import_preview_rule_text(draft);
    for rule in rules {
        if rule.rule_expression.trim().is_empty()
            || !category_type_matches_preview(rule.category_type, &draft.preview_type)
            || !match_rule_expression(&combined_text, &rule.rule_expression, rule.regex_enabled)
        {
            continue;
        }
        if !rule.main_category.trim().is_empty() || !rule.sub_category.trim().is_empty() {
            draft.preview_main_category = rule.main_category.clone();
            draft.preview_sub_category = rule.sub_category.clone();
            normalize_preview_type_for_category(draft, rule.category_type);
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
            return true;
        }
    }
    false
}

fn apply_account_alias_match(
    draft: &mut ImportPreviewDraft,
    accounts: &[ImportIntelligenceAccount],
) -> bool {
    if draft.preview_source_account_id.is_some() {
        return false;
    }
    let tokens = import_preview_account_tokens(draft);
    let Some(account) = accounts.iter().find(|account| account_matches_tokens(account, &tokens))
    else {
        return false;
    };
    draft.preview_source_account_id = Some(account.id);
    matching_feedback_object_mut(draft).insert(
        "account".to_string(),
        json!({
            "source_account_id": account.id,
            "source_account_name": account.name,
            "reason": "account alias matched parser payment method",
            "review_status": "auto_applied",
        }),
    );
    true
}

fn apply_learning_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceLearningRule],
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
    category_values: &[Value],
    account_values: &[Value],
) -> Option<i64> {
    let features = build_composite_match_features(
        &draft.preview_parser_id,
        &draft.preview_counterparty,
        &draft.preview_description,
        &draft.preview_payment_method,
    )?;
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
            Some((1.0, "exact".to_string(), "composite exact match".to_string()))
        } else {
            score_learning_rule_similarity(&features, &rule.match_features).and_then(|score| {
                let score_value = score
                    .get("score")
                    .and_then(Value::as_f64)
                    .unwrap_or_default();
                (score_value >= 0.72).then(|| {
                    let reason = score
                        .get("reason_parts")
                        .cloned()
                        .unwrap_or_else(|| json!([]))
                        .to_string();
                    (score_value, "similar".to_string(), reason)
                })
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
    let (rule, score, mode, reason) = best?;
    if let Some(learned_type) = rule
        .learned_type
        .as_deref()
        .and_then(normalize_transaction_type_text)
    {
        draft.preview_type = learned_type;
    }
    if let Some(category_id) = rule.learned_category_id {
        if let Some(category) = categories_by_id.get(&category_id) {
            draft.preview_main_category = category.main_category.clone();
            draft.preview_sub_category = category.sub_category.clone();
            normalize_preview_type_for_category(draft, category.type_code);
        }
    }
    if let Some(account_id) = rule.learned_source_account_id {
        draft.preview_source_account_id = Some(account_id);
    }
    if let Some(account_id) = rule.learned_destination_account_id {
        draft.preview_destination_account_id = Some(account_id);
    }
    let mut rule_payload = Map::new();
    rule_payload.insert("learned_type".to_string(), json!(rule.learned_type));
    rule_payload.insert(
        "learned_category_id".to_string(),
        json!(rule.learned_category_id),
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
    matching_feedback_object_mut(draft).insert(
        "learning".to_string(),
        json!({
            "rule_id": rule.id,
            "score": score,
            "mode": mode,
            "reason": reason,
            "summary": summary,
            "review_status": "auto_applied",
            "auto_apply": true,
            "source": "import_learning_rules",
        }),
    );
    Some(rule.id)
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
    if recurring_amount_matches(draft.preview_amount, template.amount) {
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
    connection: &Connection,
    user_id: i64,
    rule_ids: &[i64],
) -> rusqlite::Result<()> {
    let now = Utc::now().to_rfc3339();
    for rule_id in rule_ids {
        connection.execute(
            "
            UPDATE import_learning_rules
            SET applied_count = COALESCE(applied_count, 0) + 1,
                last_applied_at = ?1,
                updated_at = ?1
            WHERE id = ?2 AND user_id = ?3
            ",
            params![now, rule_id, user_id],
        )?;
    }
    Ok(())
}

fn parse_account_aliases(raw_aliases: Option<&str>) -> Vec<String> {
    let raw_aliases = raw_aliases.unwrap_or("").trim();
    if raw_aliases.is_empty() {
        return Vec::new();
    }
    if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(raw_aliases) {
        return values
            .iter()
            .filter_map(value_to_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }
    raw_aliases
        .split([',', ';', '|', '，', '；'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_learning_features(raw_features: &str) -> BTreeMap<String, String> {
    serde_json::from_str::<BTreeMap<String, String>>(raw_features.trim())
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| (key, value.trim().to_ascii_lowercase()))
        .filter(|(_, value)| !value.is_empty())
        .collect()
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
                normalized.strip_prefix("parser:").unwrap_or(&normalized).to_string(),
                normalized.strip_prefix("channel:").unwrap_or(&normalized).to_string(),
            ]
        })
        .filter(|token| !token.is_empty())
        .collect()
}

fn account_matches_tokens(account: &ImportIntelligenceAccount, tokens: &[String]) -> bool {
    account.aliases.iter().any(|alias| {
        let alias = normalize_account_match_text(alias);
        !alias.is_empty()
            && tokens
                .iter()
                .any(|token| token == &alias || token.contains(&alias) || alias.contains(token))
    })
}

fn normalize_account_match_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn recurring_type_matches(preview_type: &str, recurring_type: &str) -> bool {
    let recurring_type = recurring_type.trim();
    recurring_type.is_empty()
        || preview_type_code(preview_type) == preview_type_code(recurring_type)
        || preview_type.trim() == recurring_type
}

fn recurring_amount_matches(preview_amount: f64, template_amount: f64) -> bool {
    let preview_cents = (preview_amount.abs() * 100.0).round() as i64;
    let template_as_cents = template_amount.abs().round() as i64;
    let template_yuan_as_cents = (template_amount.abs() * 100.0).round() as i64;
    preview_cents == template_as_cents || preview_cents == template_yuan_as_cents
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

fn parse_import_preview_date(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.len() >= 10 {
        NaiveDate::parse_from_str(&trimmed[..10], "%Y-%m-%d").ok()
    } else {
        None
    }
}

fn import_preview_draft_from_row(row: &ImportPreviewRow) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: row.preview_date.clone(),
        preview_type: row.preview_type.clone(),
        preview_amount: row.preview_amount,
        preview_destination_amount: row.preview_destination_amount,
        preview_main_category: row.preview_main_category.clone(),
        preview_sub_category: row.preview_sub_category.clone(),
        preview_source_account_id: row.preview_source_account_id,
        preview_destination_account_id: row.preview_destination_account_id,
        preview_counterparty: row.preview_counterparty.clone(),
        preview_payment_method: row.preview_payment_method.clone(),
        preview_description: row.preview_description.clone(),
        preview_parser_id: row.preview_parser_id.clone(),
        preview_parser_tags: Some(json!(row.preview_parser_tags)),
        preview_recurring_id: row.preview_recurring_id,
        preview_recurring_name: row.preview_recurring_name.clone(),
        preview_recurring_candidate_count: row.preview_recurring_candidate_count,
        preview_recurring_match_score: row.preview_recurring_match_score,
        preview_recurring_match_reasons: row.preview_recurring_match_reasons.clone(),
        preview_recurring_matched_date: row.preview_recurring_matched_date.clone(),
        dedup_type: (!row.dedup_type.trim().is_empty()).then(|| row.dedup_type.clone()),
        dedup_source_ids: row.dedup_source_ids.clone(),
        preview_matching_feedback: row.preview_matching_feedback.clone(),
    }
}

fn import_preview_patch_from_draft(preview_id: i64, draft: &ImportPreviewDraft) -> ImportPreviewPatch {
    ImportPreviewPatch::new(preview_id).with_changes([
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(draft.preview_type.clone()),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(draft.preview_main_category.clone()),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(draft.preview_sub_category.clone()),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            optional_i64_patch_value(draft.preview_source_account_id),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            optional_i64_patch_value(draft.preview_destination_account_id),
        ),
        (
            ImportPreviewPatchField::RecurringId,
            optional_i64_patch_value(draft.preview_recurring_id),
        ),
        (
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text(draft.preview_recurring_name.clone()),
        ),
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(draft.preview_recurring_candidate_count),
        ),
        (
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(draft.preview_recurring_match_score),
        ),
        (
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text(draft.preview_recurring_match_reasons.clone()),
        ),
        (
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text(draft.preview_recurring_matched_date.clone()),
        ),
        (
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Json(draft.preview_matching_feedback.clone()),
        ),
    ])
}

fn optional_i64_patch_value(value: Option<i64>) -> ImportPreviewPatchValue {
    value.map_or(ImportPreviewPatchValue::Null, ImportPreviewPatchValue::Integer)
}

pub async fn import_confirm_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = match required_session_id_from_payload(object) {
        Ok(session_id) => session_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }

    if first_value(object, &["preview_updates", "previewUpdates"]).is_some() {
        let update_items = match preview_update_items_from_payload(&payload) {
            Ok(items) => items,
            Err(response) => return route_response(response),
        };
        let mut patches = Vec::with_capacity(update_items.len());
        for item in update_items {
            let preview_id = match preview_id_from_payload(item) {
                Ok(preview_id) => preview_id,
                Err(response) => return route_response(response),
            };
            match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
                Ok(Some(preview)) if preview.session_id == session_id => {}
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(404, "Preview bill not found"));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
            patches.push(build_preview_patch_from_payload(preview_id, item));
        }
        if let Err(error) = replace_preview_selection_with_patches(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &patches,
        ) {
            return route_response(db_error_response(error));
        }
    } else if let Some(selected_ids) =
        id_list_field_from_object(object, &["selected_ids", "selectedIds"])
    {
        for preview_id in &selected_ids {
            match get_preview_bill_by_id(runtime.connection(), *preview_id, user_id) {
                Ok(Some(preview)) if preview.session_id == session_id => {}
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(404, "Preview bill not found"));
                }
                Err(error) => return route_response(db_error_response(error)),
            }
        }
        if let Err(error) =
            reset_session_preview_selection(runtime.connection(), &session_id, user_id)
        {
            return route_response(db_error_response(error));
        }
        if let Err(error) =
            update_preview_selection(runtime.connection_mut(), &selected_ids, true, user_id)
        {
            return route_response(db_error_response(error));
        }
    }

    let result = match confirm_preview_to_bills(runtime.connection_mut(), &session_id, user_id) {
        Ok(result) => result,
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_stage_confirm_success(ImportStageConfirmData {
        imported_count: result.confirmed_count,
        skipped_count: result.skipped_count + result.duplicate_count,
        errors: result.errors,
    }))
}

pub async fn import_session_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(session)) => route_response(import_session_success(ImportSessionSummary {
            session_id: session.session_id,
            status: session.status,
            created_at: session.created_at,
            parsed_count: non_negative_usize(session.total_parsed),
            preview_count: non_negative_usize(session.total_preview),
            file_paths: json!([]),
        })),
        Ok(None) => route_response(import_session_not_found_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_session_cancel_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => match clear_session_data(runtime.connection_mut(), &session_id, user_id) {
            Ok(_) => route_response(import_session_cancel_success_response()),
            Err(error) => route_response(db_error_response(error)),
        },
        Ok(None) => route_response(import_session_cancel_missing_response()),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_preview_page_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    Query(query): Query<PreviewPageQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .or(query.page_size_camel)
        .unwrap_or(100)
        .clamp(1, 500);
    let selected_only = query
        .selected_only
        .or(query.selected_only_camel)
        .unwrap_or(false);
    match get_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        page as i64,
        page_size as i64,
        selected_only,
    ) {
        Ok((preview, total)) => {
            let preview = preview
                .into_iter()
                .map(preview_row_to_value)
                .collect();
            route_response(import_preview_page_success(ImportPreviewPageData {
                preview,
                total: non_negative_usize(total),
                page,
                page_size,
            }))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_preview_index_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let rows = match get_preview_filter_index_by_session(runtime.connection(), &session_id, user_id)
    {
        Ok(rows) => rows,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categories_by_id = BTreeMap::new();
    let accounts_by_id = BTreeMap::new();
    let items = rows
        .into_iter()
        .filter_map(|row| serde_json::to_value(row).ok()?.as_object().cloned())
        .map(|row| build_import_preview_filter_index_item(&row, &categories_by_id, &accounts_by_id))
        .collect::<Vec<_>>();
    route_response(import_preview_index_success(ImportPreviewIndexData {
        total: items.len(),
        items,
    }))
}

pub async fn import_preview_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let preview_id = match preview_id_from_payload(object) {
        Ok(preview_id) => preview_id,
        Err(response) => return route_response(response),
    };
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) if preview.session_id == session_id => preview,
        Ok(Some(_)) | Ok(None) => {
            return route_response(import_v2_error_response(404, "Preview bill not found"));
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    let patch = build_preview_patch_from_payload(preview.id, object);
    let updated = match update_preview_bill(runtime.connection(), &session_id, user_id, &patch) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    if response_mode_is_preview_item(object) {
        let preview_item = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => preview_row_to_value(preview),
            Ok(Some(_)) | Ok(None) => Value::Null,
            Err(error) => return route_response(db_error_response(error)),
        };
        route_response(import_v2_data_response(json!({
            "updated": updated,
            "previewItem": preview_item,
        })))
    } else {
        route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": updated}),
        })
    }
}

pub async fn import_reclassify_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let update_items = match preview_update_items_from_payload(&payload) {
        Ok(update_items) => update_items,
        Err(response) => return route_response(response),
    };
    let mut patches = Vec::with_capacity(update_items.len());
    for item in update_items {
        let preview_id = match preview_id_from_payload(item) {
            Ok(preview_id) => preview_id,
            Err(response) => return route_response(response),
        };
        match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => {}
            Ok(Some(_)) | Ok(None) => {
                return route_response(import_v2_error_response(404, "Preview bill not found"));
            }
            Err(error) => return route_response(db_error_response(error)),
        }
        patches.push(build_preview_patch_from_payload(preview_id, item));
    }
    let updated = match update_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        patches.as_slice(),
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut preview = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
        Ok(preview) => preview,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut intelligent_drafts = preview
        .iter()
        .map(import_preview_draft_from_row)
        .collect::<Vec<_>>();
    if let Err(error) = apply_import_intelligence_chain(
        runtime.connection_mut(),
        user_id,
        intelligent_drafts.as_mut_slice(),
    ) {
        return route_response(db_error_response(error));
    }
    let intelligence_patches = preview
        .iter()
        .zip(intelligent_drafts.iter())
        .map(|(row, draft)| import_preview_patch_from_draft(row.id, draft))
        .collect::<Vec<_>>();
    if let Err(error) = update_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        intelligence_patches.as_slice(),
    ) {
        return route_response(db_error_response(error));
    }
    preview = match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
        Ok(preview) => preview,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categorized = preview.iter().filter(preview_row_is_categorized).count();
    let account_matched = preview.iter().filter(preview_row_has_account).count();
    let preview: Vec<Value> = preview.into_iter().map(preview_row_to_value).collect();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "total": preview.len(),
        "categorized": categorized,
        "account_matched": account_matched,
        "session_samples_saved": 0,
        "annotation_applied": 0,
        "updated": updated,
        "preview": preview,
    })))
}

pub async fn preview_recurring_candidates_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_global_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    let user_id_i64 = match user_id_i64_for_sql(user_id) {
        Ok(value) => value,
        Err(error) => return route_response(db_error_response(error)),
    };
    let draft = import_preview_draft_from_row(&preview);
    let candidates = match load_import_intelligence_recurring_templates(runtime.connection(), user_id_i64)
    {
        Ok(templates) => {
            let mut candidates = templates
                .iter()
                .filter_map(|template| recurring_template_match(&draft, template))
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                right
                    .match_score
                    .partial_cmp(&left.match_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            candidates
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    let candidate_count = candidates.len();
    let candidates = candidates
        .into_iter()
        .map(|candidate| {
            json!({
                "id": candidate.id,
                "name": candidate.name,
                "matchScore": candidate.match_score,
                "match_score": candidate.match_score,
                "matchReasons": candidate.match_reasons,
                "match_reasons": candidate.match_reasons,
                "matchedOccurrenceDate": candidate.matched_occurrence_date,
                "matched_occurrence_date": candidate.matched_occurrence_date,
            })
        })
        .collect::<Vec<_>>();
    route_response(import_v2_data_response(json!({
        "previewId": preview.id,
        "sessionId": preview.session_id,
        "linkedRecurringId": preview.preview_recurring_id,
        "linkedRecurringName": preview.preview_recurring_name,
        "candidates": candidates,
        "candidate_count": candidate_count,
        "provider_bypassed": false,
        "runtime": "rust-import-db-runtime",
    })))
}

pub async fn preview_recurring_match_put_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let recurring_id = match first_value(object, &["recurringId", "recurring_id"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => return route_response(import_v2_error_response(400, "Missing recurringId")),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let target_candidate = recurring_candidate_from_payload(object, recurring_id);
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: Some(recurring_id),
        candidate_count: recurring_candidate_count_from_payload(object, target_candidate.as_ref()),
        target_candidate,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": recurring_id}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn preview_recurring_match_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let update = ImportPreviewRecurringMatchUpdate {
        recurring_id: None,
        candidate_count: 0,
        target_candidate: None,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        &update,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"recurringId": Value::Null}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn preview_transfer_decision_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let decision = match decision_from_payload(object) {
        Ok(decision) => decision,
        Err(response) => return route_response(response),
    };
    let expected_state = match expected_state_from_payload(object) {
        Ok(expected_state) => expected_state,
        Err(response) => return route_response(response),
    };
    let reviewed_type = first_value(object, &["reviewedType", "reviewed_type"])
        .and_then(value_to_text)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "转账".to_string());
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview_id,
        user_id,
        decision,
        &reviewed_type,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"decision": decision_name(decision)}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

pub async fn import_learning_suggestions_get_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    import_learning_suggestions_response(state, session_id, headers, None).await
}

pub async fn import_learning_suggestions_post_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    import_learning_suggestions_response(state, session_id, headers, Some(payload)).await
}

async fn import_learning_suggestions_response(
    state: HttpAppState,
    session_id: String,
    headers: HeaderMap,
    payload: Option<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates = match payload.as_ref() {
        Some(payload) => {
            match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, payload) {
                Ok(updated) => updated,
                Err(response) => return route_response(response),
            }
        }
        None => 0,
    };
    let preview_ids = payload
        .as_ref()
        .map(preview_ids_from_payload)
        .unwrap_or_default();
    let suggestions = match get_preview_by_session(runtime.connection(), &session_id, user_id, false)
    {
        Ok(preview) => build_import_learning_suggestions_from_preview(&preview, &preview_ids),
        Err(error) => return route_response(db_error_response(error)),
    };
    let suggestion_count = suggestions.len();
    route_response(import_v2_data_response(json!({
        "session_id": session_id,
        "suggestions": suggestions,
        "count": suggestion_count,
        "preview_ids": preview_ids,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": false,
        "runtime": "rust-import-db-runtime",
    })))
}

fn build_import_learning_suggestions_from_preview(
    preview: &[ImportPreviewRow],
    preview_ids: &[i64],
) -> Vec<Value> {
    let selected_ids = preview_ids.iter().copied().collect::<BTreeSet<_>>();
    preview
        .iter()
        .filter(|row| selected_ids.is_empty() || selected_ids.contains(&row.id))
        .filter_map(|row| {
            let learning = row.preview_matching_feedback.get("learning")?.as_object()?;
            Some(json!({
                "preview_id": row.id,
                "previewId": row.id,
                "rule_id": learning.get("rule_id").cloned().unwrap_or(Value::Null),
                "score": learning.get("score").cloned().unwrap_or_else(|| json!(0)),
                "level": learning.get("level").cloned().unwrap_or_else(|| json!("")),
                "reason": learning.get("reason").cloned().unwrap_or_else(|| json!("")),
                "recommended_type": learning
                    .get("recommended_type")
                    .cloned()
                    .unwrap_or_else(|| json!(row.preview_type.clone())),
                "summary": learning.get("summary").cloned().unwrap_or_else(|| json!("")),
                "review_status": learning
                    .get("review_status")
                    .cloned()
                    .unwrap_or_else(|| json!("pending")),
                "source": learning
                    .get("source")
                    .cloned()
                    .unwrap_or_else(|| json!("import_learning_rules")),
                "mode": learning.get("mode").cloned().unwrap_or_else(|| json!("")),
                "auto_apply": learning
                    .get("auto_apply")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
            }))
        })
        .collect()
}

pub async fn import_learning_promote_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let applied_preview_updates =
        match apply_preview_updates_from_payload(&mut runtime, &session_id, user_id, &payload) {
            Ok(updated) => updated,
            Err(response) => return route_response(response),
        };
    let annotation_samples = annotation_samples_from_payload(&payload);
    let saved_samples = if annotation_samples.is_empty() {
        0
    } else {
        match save_import_annotation_samples(
            runtime.connection_mut(),
            &session_id,
            user_id,
            &annotation_samples,
        ) {
            Ok(saved) => saved,
            Err(error) => return route_response(db_error_response(error)),
        }
    };
    let mut preview_ids = preview_ids_from_payload(&payload);
    preview_ids.extend(annotation_samples.iter().map(|sample| sample.preview_id));
    preview_ids.sort_unstable();
    preview_ids.dedup();
    let promoted = match promote_import_learning_rules(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_ids,
    ) {
        Ok(result) => result,
        Err(response) => return route_response(response),
    };
    route_response(import_v2_data_response(json!({
        "success": true,
        "session_id": session_id,
        "selected_samples": preview_ids.len(),
        "saved_samples": saved_samples,
        "rules_total": promoted.rules_total,
        "created": promoted.created,
        "updated": promoted.updated,
        "applied_preview_updates": applied_preview_updates,
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })))
}

pub async fn import_learning_rules_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<ImportLearningRulesQuery>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size().clamp(1, 500);
    let enabled_only = query.enabled_only();
    let total = match count_import_learning_rules(runtime.connection(), user_id, enabled_only) {
        Ok(total) => total,
        Err(response) => return route_response(response),
    };
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    let rules = match load_import_learning_rules(
        runtime.connection(),
        user_id,
        enabled_only,
        page_size,
        offset,
    ) {
        Ok(rules) => rules,
        Err(response) => return route_response(response),
    };
    let total_pages = if total <= 0 {
        0
    } else {
        (total as usize).div_ceil(page_size) as i64
    };
    route_response(ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": rules,
            "totalCount": total,
            "page": page,
            "pageSize": page_size,
            "totalPages": total_pages,
        }),
    })
}

pub async fn import_learning_rule_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    if let Err(response) =
        update_import_learning_rule(runtime.connection(), rule_id, user_id, object)
    {
        return route_response(response);
    }
    match get_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(Some(rule)) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": rule}),
        }),
        Ok(None) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

pub async fn import_learning_rule_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_learning_runtime_schema(&runtime) {
        return route_response(response);
    }
    match delete_import_learning_rule(runtime.connection(), rule_id, user_id) {
        Ok(true) => route_response(ImportV2RouteResponse {
            status_code: 200,
            body: json!({"success": true, "result": true}),
        }),
        Ok(false) => route_response(import_v2_error_response(404, "Rule not found")),
        Err(response) => route_response(response),
    }
}

