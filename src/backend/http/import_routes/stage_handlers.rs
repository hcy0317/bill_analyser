// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_dedup_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_dedup_runtime_handler", "business operation entered");
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
    let session = match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(session)) => session,
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    };

    let _stage_started_at = Instant::now();
    let _template_query_started_at = Instant::now();
    let templates =
        match get_unprocessed_templates_for_dedup(runtime.connection(), &session_id, user_id) {
            Ok(templates) => templates,
            Err(error) => return route_response(db_error_response(error)),
        };
    if templates.is_empty() && session.status == "preview" {
        let persisted_preview =
            match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
                Ok(rows) => rows,
                Err(error) => return route_response(db_error_response(error)),
            };
        let persisted_preview_count = persisted_preview.len();
        let preview = if include_preview {
            persisted_preview
                .into_iter()
                .map(preview_row_to_value)
                .collect()
        } else {
            Vec::new()
        };
        let total = usize::try_from(session.total_parsed.max(0)).unwrap_or(usize::MAX);
        return route_response(import_stage_dedup_success(ImportStageDedupData {
            session_id,
            preview,
            preview_included: include_preview,
            total,
            after_dedup: persisted_preview_count,
            dedup_stats: json!({
                "removed": total.saturating_sub(persisted_preview_count),
                "duplicates": 0,
                "transfer_pairs": 0,
                "split_merge": 0,
            }),
            match_stats: json!({
                "runtime": "rust",
                "category_matched": 0,
                "account_matched": 0,
                "learning_applied": 0,
                "recurring_projected": 0,
                "database_candidates": 0,
                "provider_bypassed": false,
                "idempotent_replay": true,
            }),
        }));
    }
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        templates = templates.len(),
        elapsed_ms = import_stage_elapsed_ms(_template_query_started_at),
        "stage2 templates loaded"
    );
    let _dedup_started_at = Instant::now();
    let dedup_input = dedup_bills_from_parser_templates(&templates);
    let dedup_result = SmartDeduplicationEngine.process(dedup_input);
    let standard_rows =
        match get_import_standard_rows_by_session(runtime.connection(), &session_id, user_id) {
            Ok(rows) => rows,
            Err(error) => return route_response(db_error_response(error)),
        };
    let history_bills = match get_import_history_candidate_bills_for_session(
        runtime.connection(),
        &session_id,
        user_id,
    ) {
        Ok(rows) => rows,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut history_duplicate_plan =
        build_history_duplicate_preview_plan(&dedup_result.kept_bills, &history_bills);
    let history_duplicate_import_keys = history_duplicate_plan
        .iter()
        .map(|plan| plan.import_bill_key.clone())
        .collect::<HashSet<_>>();
    let history_duplicate_ids = history_duplicate_plan
        .iter()
        .map(|plan| plan.history_bill_id)
        .collect::<HashSet<_>>();
    let mut history_transfer_plan = build_history_transfer_preview_plan(
        &dedup_result.kept_bills,
        &history_bills,
        &history_duplicate_import_keys,
        &history_duplicate_ids,
    );
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        original = dedup_result.original_count,
        kept = dedup_result.kept_bills.len(),
        removed = dedup_result.removed_count,
        elapsed_ms = import_stage_elapsed_ms(_dedup_started_at),
        "stage2 smart dedup complete"
    );
    let history_import_keys = history_duplicate_plan
        .iter()
        .map(|plan| plan.import_bill_key.clone())
        .chain(
            history_transfer_plan
                .iter()
                .map(|plan| plan.import_bill_key.clone()),
        )
        .collect::<HashSet<_>>();
    let previewable_bills = dedup_result
        .kept_bills
        .iter()
        .filter(|bill| !history_import_keys.contains(&import_reconciliation_key_for_bill(bill)))
        .cloned()
        .collect::<Vec<_>>();
    let mut preview_drafts = preview_drafts_from_dedup_bills(&previewable_bills);
    preview_drafts.extend(
        history_duplicate_plan
            .iter()
            .map(|plan| plan.preview_draft.clone()),
    );
    preview_drafts.extend(
        history_transfer_plan
            .iter()
            .map(|plan| plan.preview_draft.clone()),
    );
    let mut intelligence_stats = match apply_import_intelligence_chain(
        runtime.connection_mut(),
        user_id,
        preview_drafts.as_mut_slice(),
    )
    .await
    {
        Ok(stats) => stats,
        Err(error) => return route_response(db_error_response(error)),
    };
    let user_id_i64 = match user_id_i64_for_sql(user_id) {
        Ok(user_id) => user_id,
        Err(error) => return route_response(db_error_response(error)),
    };
    match apply_import_learning_vector_recall_chain(
        runtime.connection(),
        &state.config,
        user_id_i64,
        preview_drafts.as_mut_slice(),
    )
    .await
    {
        Ok(vector_stats) => intelligence_stats.merge_vector_recall(vector_stats),
        Err(error) => return route_response(db_error_response(error)),
    }
    enforce_import_preview_invariants(preview_drafts.as_mut_slice());
    refresh_history_duplicate_materialization_payloads(
        &mut history_duplicate_plan,
        &preview_drafts,
    );
    refresh_history_transfer_materialization_payloads(&mut history_transfer_plan, &preview_drafts);
    let _preview_insert_started_at = Instant::now();
    if !templates.is_empty() {
        if let Err(error) =
            clear_import_preview_materialization_state(runtime.connection_mut(), &session_id, user_id)
        {
            return route_response(db_error_response(error));
        }
    }
    let inserted_preview = match insert_preview_bills_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &preview_drafts,
    ) {
        Ok(inserted) => inserted,
        Err(error) => return route_response(db_error_response(error)),
    };
    if let Err(error) = insert_import_history_materializations_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &history_duplicate_plan
            .iter()
            .map(|plan| plan.materialization.clone())
            .chain(
                history_transfer_plan
                    .iter()
                    .map(|plan| plan.materialization.clone()),
            )
            .collect::<Vec<_>>(),
    ) {
        return route_response(db_error_response(error));
    }
    let preview_rows_for_groups =
        match get_preview_by_session(runtime.connection(), &session_id, user_id, false) {
            Ok(rows) => rows,
            Err(error) => return route_response(db_error_response(error)),
        };
    let decision_groups = build_import_match_decision_groups(ImportMatchDecisionGroupInput {
        session_id: &session_id,
        duplicate_groups: &dedup_result.duplicate_groups,
        transfer_pairs: &dedup_result.transfer_pairs,
        templates: &templates,
        standard_rows: &standard_rows,
        preview_rows: &preview_rows_for_groups,
        history_duplicate_plan: &history_duplicate_plan,
        history_transfer_plan: &history_transfer_plan,
    });
    if let Err(error) = insert_import_decision_groups_batch(
        runtime.connection_mut(),
        &session_id,
        user_id,
        &decision_groups,
    ) {
        return route_response(db_error_response(error));
    }
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        preview_rows = inserted_preview,
        elapsed_ms = import_stage_elapsed_ms(_preview_insert_started_at),
        "stage2 preview inserted"
    );
    let _status_update_started_at = Instant::now();
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
    let _updated_templates = match mark_unprocessed_parser_templates_processed_for_session(
        runtime.connection_mut(),
        &session_id,
        user_id,
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_dedup_runtime_handler",
        user_id = user_id.get(),
        session_id = %session_id,
        template_rows = templates.len(),
        updated_template_rows = _updated_templates,
        total_elapsed_ms = import_stage_elapsed_ms(_stage_started_at),
        status_elapsed_ms = import_stage_elapsed_ms(_status_update_started_at),
        "stage2 status updated"
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
            "learning_vector_recalled": intelligence_stats.learning_vector_recalled,
            "learning_vector_status": intelligence_stats.learning_vector_status,
            "recurring_projected": intelligence_stats.recurring_projected,
            "database_candidates": history_duplicate_plan.len() + history_transfer_plan.len(),
            "provider_bypassed": false,
        }),
    }))
}

#[derive(Debug, Clone, Default)]
struct ImportIntelligenceStats {
    category_matched: usize,
    account_matched: usize,
    learning_applied: usize,
    learning_vector_recalled: usize,
    learning_vector_status: String,
    recurring_projected: usize,
}

impl ImportIntelligenceStats {
    fn merge_vector_recall(&mut self, vector_stats: ImportLearningVectorRecallStats) {
        self.learning_vector_recalled = vector_stats.recalled;
        self.learning_vector_status = vector_stats.status;
    }
}

#[derive(Debug, Clone)]
struct ImportLearningVectorRecallRequestDraft {
    draft_index: usize,
    features: BTreeMap<String, String>,
    transaction_type_scope: String,
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
    amount_cents: i64,
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

#[derive(Debug, Clone)]
struct ImportLearningRuleMatchResult {
    rule_id: Option<i64>,
    auto_applied: bool,
}

#[derive(Debug, Clone, Copy)]
struct ImportPreviewBuiltinCategoryFallback {
    category_type: i64,
    main_category: &'static str,
    sub_category: &'static str,
    keywords: &'static [&'static str],
}

const BUILTIN_CATEGORY_RULE_FALLBACKS: &[ImportPreviewBuiltinCategoryFallback] = &[
    ImportPreviewBuiltinCategoryFallback {
        category_type: 2,
        main_category: "投资收益",
        sub_category: "理财收益",
        keywords: &[
            "理财收益",
            "收益发放",
            "余额宝收益",
            "零钱通收益",
            "基金分红",
            "股息",
        ],
    },
    ImportPreviewBuiltinCategoryFallback {
        category_type: 3,
        main_category: "金融保险",
        sub_category: "投资支出",
        keywords: &[
            "理财通购买",
            "理财购买",
            "购买理财",
            "基金申购",
            "基金定投",
            "定投扣款",
            "基金买入",
            "证券买入",
            "投资支出",
        ],
    },
    ImportPreviewBuiltinCategoryFallback {
        category_type: 3,
        main_category: "交通出行",
        sub_category: "公交地铁",
        keywords: &["公共交通", "公交地铁", "轨道交通", "乘车码"],
    },
];

#[tracing::instrument(level = "debug", skip_all)]
async fn apply_import_intelligence_chain(
    connection: &Connection,
    user_id: UserId,
    drafts: &mut [ImportPreviewDraft],
) -> Result<ImportIntelligenceStats, bill_analyser_db::DbError> {
    let user_id_i64 = user_id_i64_for_sql(user_id)?;
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
    let category_rules =
        load_import_intelligence_category_rules(connection, user_id_i64, &categories_by_id).await?;
    let accounts = load_import_intelligence_accounts(connection, user_id_i64).await?;
    let account_values = accounts
        .iter()
        .map(import_intelligence_account_value)
        .collect::<Vec<_>>();
    let account_rules = load_import_intelligence_account_rules(connection, user_id_i64).await?;
    let learning_rules = load_import_intelligence_learning_rules(connection, user_id_i64).await?;
    let recurring_templates =
        load_import_intelligence_recurring_templates(connection, user_id_i64).await?;
    let transfer_category = default_transfer_category(
        &categories,
        &categories_by_id,
        load_user_cash_transfer_category_id(connection, user_id_i64).await?,
    )
    .cloned();
    let mut stats = ImportIntelligenceStats::default();
    for draft in &mut *drafts {
        ensure_base_matching_feedback(draft);
        let before_category = (
            draft.preview_type.clone(),
            draft.preview_main_category.clone(),
            draft.preview_sub_category.clone(),
        );
        if preview_type_code(&draft.preview_type) == Some(4) {
            if !apply_transfer_category_rule_match(draft, &category_rules) {
                apply_transfer_default_category(draft, &categories, transfer_category.as_ref());
            }
        } else if preview_type_code(&draft.preview_type) == Some(5) {
            if !apply_investment_category_rule_match(draft, &category_rules) {
                apply_builtin_category_rule_fallback(draft, &categories);
            }
        } else if !apply_income_expense_category_rule_match(draft, &category_rules) {
            apply_builtin_category_rule_fallback(draft, &categories);
        }

        if let Some(candidate) = best_recurring_candidate_for_draft(draft, &recurring_templates) {
            apply_recurring_candidate(draft, candidate);
            stats.recurring_projected += 1;
        }
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

        let before_account_rule = (
            draft.preview_source_account_id,
            draft.preview_destination_account_id,
        );
        // Account recognition is intentionally last in stage2: type/transfer,
        // recurring, and learning projections may still change the account
        // role, transaction type, or explicit accounts that rules must consume.
        apply_account_rule_match_after_semantic_projection(draft, &account_rules, &accounts);
        persist_stage2_actionable_baseline(draft);

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
    account_rules: &[AccountRuleCandidate],
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
            Some(Ok(ImportIntelligenceRule {
                id,
                category_id,
                category_type: category.type_code,
                main_category: category.main_category.clone(),
                sub_category: category.sub_category.clone(),
                priority: i64::from(priority),
                rule_expression: rule_expression_string(&expression),
                regex_enabled,
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
fn apply_transfer_category_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceRule],
) -> bool {
    let combined_text = import_preview_transfer_rule_text(draft);
    apply_category_rule_match_filtered(draft, rules, &combined_text, |rule, _| {
        rule.category_type == 4
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_investment_category_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceRule],
) -> bool {
    let combined_text = import_preview_visible_rule_text(draft);
    apply_category_rule_match_filtered(draft, rules, &combined_text, |rule, _| {
        rule.category_type == 5
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_income_expense_category_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceRule],
) -> bool {
    let Some(expected_type) = preview_type_code(&draft.preview_type).filter(|value| {
        matches!(*value, 2 | 3)
    }) else {
        return false;
    };
    let combined_text = import_preview_rule_text(draft);
    apply_category_rule_match_filtered(draft, rules, &combined_text, |rule, _| {
        rule.category_type == expected_type
    })
}

fn apply_category_rule_match_filtered<F>(
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceRule],
    combined_text: &str,
    mut rule_filter: F,
) -> bool
where
    F: FnMut(&ImportIntelligenceRule, &ImportPreviewDraft) -> bool,
{
    for rule in rules {
        if rule.rule_expression.trim().is_empty()
            || !rule_filter(rule, draft)
            || !match_rule_expression(combined_text, &rule.rule_expression, rule.regex_enabled)
        {
            continue;
        }
        if !rule.main_category.trim().is_empty() || !rule.sub_category.trim().is_empty() {
            draft.preview_main_category = rule.main_category.clone();
            draft.preview_sub_category = rule.sub_category.clone();
            draft.category_id = Some(rule.category_id);
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

#[tracing::instrument(level = "debug", skip_all)]
fn apply_builtin_category_rule_fallback(
    draft: &mut ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
) -> bool {
    if preview_type_code(&draft.preview_type)
        .is_some_and(|expected_type| preview_category_matches_type(draft, categories, expected_type))
    {
        return false;
    }

    let combined_text = normalize_category_fallback_text(&import_preview_rule_text(draft));
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

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_category_fallback_text(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(char::is_whitespace, "")
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
fn persist_stage2_actionable_baseline(draft: &mut ImportPreviewDraft) {
    let snapshot = import_preview_stage2_snapshot(draft);

    matching_feedback_object_mut(draft).insert(
        "stage2_baseline".to_string(),
        snapshot,
    );
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
fn apply_learning_rule_match(
    connection: &Connection,
    user_id: i64,
    draft: &mut ImportPreviewDraft,
    rules: &[ImportIntelligenceLearningRule],
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
    category_values: &[Value],
    account_values: &[Value],
) -> Option<ImportLearningRuleMatchResult> {
    let transfer_protected = is_transfer_protected_preview(draft);
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
                (score_value >= 0.72 && learning_similarity_has_semantic_anchor(&score)).then(|| {
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
    let raw_learned_type = rule
        .learned_type
        .as_deref()
        .and_then(normalize_transaction_type_text);
    let learned_type = if transfer_protected {
        raw_learned_type
            .as_deref()
            .filter(|transaction_type| *transaction_type == "转账")
            .map(ToOwned::to_owned)
    } else {
        raw_learned_type.clone()
    };
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
            return None;
        };
        if transfer_protected && category.type_code != 4 {
            None
        } else if !category_type_matches_preview(category.type_code, candidate_preview_type) {
            if !transfer_protected {
                annotate_learning_rule_skip(
                    draft,
                    rule.id,
                    "learned category type is incompatible with preview type",
                );
            }
            return None;
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
        return None;
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
        && import_preview_stage2_snapshot(&recommended_draft) == import_preview_stage2_snapshot(draft)
    {
        return None;
    }
    let recommendation_key = build_import_learning_recommendation_key(
        &ImportLearningRecommendationKeyInput {
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
        },
    );
    let lifecycle_user_id = u64::try_from(user_id).ok().and_then(|value| UserId::new(value).ok())?;
    let lifecycle = get_import_learning_lifecycle_view(
        connection,
        lifecycle_user_id,
        &recommendation_key,
    )
    .ok()
    .flatten()?;
    if lifecycle.suppressed {
        return None;
    }
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
            learned_category,
            rule,
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
        rule_id: Some(rule.id),
        auto_applied,
    })
}

fn apply_learning_rule_projection(
    draft: &mut ImportPreviewDraft,
    learned_type: Option<&str>,
    learned_category: Option<&ImportIntelligenceCategory>,
    rule: &ImportIntelligenceLearningRule,
    transfer_protected: bool,
) {
    if let Some(learned_type) = learned_type {
        draft.preview_type = learned_type.to_string();
    }
    if let Some(category) = learned_category {
        draft.preview_main_category = category.main_category.clone();
        draft.preview_sub_category = category.sub_category.clone();
        draft.category_id = Some(category.id);
        normalize_preview_type_for_category(draft, category.type_code);
    }
    if let Some(account_id) = rule.learned_source_account_id {
        if !transfer_protected || draft.preview_source_account_id.is_none() {
            draft.preview_source_account_id = Some(account_id);
        }
    }
    if let Some(account_id) = rule.learned_destination_account_id {
        if !transfer_protected || draft.preview_destination_account_id.is_none() {
            draft.preview_destination_account_id = Some(account_id);
        }
    }
}

fn learning_similarity_has_semantic_anchor(score: &Value) -> bool {
    score
        .get("matched_fields")
        .and_then(Value::as_array)
        .is_some_and(|fields| {
            fields.iter().filter_map(Value::as_str).any(|field| {
                matches!(field, "counterparty" | "description")
            })
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

#[tracing::instrument(level = "debug", skip_all)]
fn apply_transfer_default_category(
    draft: &mut ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
    default_category: Option<&ImportIntelligenceCategory>,
) -> bool {
    if !is_transfer_protected_preview(draft)
        || preview_category_matches_type(draft, categories, 4)
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

fn is_transfer_protected_preview(draft: &ImportPreviewDraft) -> bool {
    draft
        .dedup_type
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase().contains("transfer"))
        .unwrap_or(false)
        || draft.preview_matching_feedback.get("transfer").is_some()
}

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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

fn import_preview_visible_rule_text(draft: &ImportPreviewDraft) -> String {
    [
        draft.preview_counterparty.clone(),
        draft.preview_payment_method.clone(),
        draft.preview_description.clone(),
    ]
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

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_draft_from_row(row: &ImportPreviewRow) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: row.preview_date.clone(),
        preview_type: row.preview_type.clone(),
        preview_amount_cents: row.preview_amount_cents,
        preview_destination_amount_cents: row.preview_destination_amount_cents,
        category_id: row.category_id,
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
        preview_selected: row.preview_selected,
        dedup_type: (!row.dedup_type.trim().is_empty()).then(|| row.dedup_type.clone()),
        dedup_source_ids: row.dedup_source_ids.clone(),
        preview_matching_feedback: row.preview_matching_feedback.clone(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
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
            ImportPreviewPatchField::CategoryId,
            optional_i64_patch_value(draft.category_id),
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
        (
            ImportPreviewPatchField::Selected,
            ImportPreviewPatchValue::Bool(draft.preview_selected),
        ),
    ])
}

fn optional_i64_patch_value(value: Option<i64>) -> ImportPreviewPatchValue {
    value.map_or(ImportPreviewPatchValue::Null, ImportPreviewPatchValue::Integer)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_confirm_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_confirm_runtime_handler", "business operation entered");
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
            patches.push(match build_preview_patch_from_payload_with_category_lookup(
                runtime.connection(),
                user_id,
                preview_id,
                item,
            ) {
                Ok(patch) => patch,
                Err(response) => return route_response(response),
            });
        }
        let preserve_unpatched_selection = first_value(
            object,
            &["preserve_unpatched_selection", "preserveUnpatchedSelection"],
        )
        .and_then(Value::as_bool)
        .unwrap_or(false);
        let patch_result = if preserve_unpatched_selection {
            apply_preview_patches_preserving_selection(
                runtime.connection_mut(),
                &session_id,
                user_id,
                &patches,
            )
        } else {
            replace_preview_selection_with_patches(
                runtime.connection_mut(),
                &session_id,
                user_id,
                &patches,
            )
        };
        if let Err(error) = patch_result {
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

    let history_acknowledgement = match history_rewrite_acknowledgement_from_payload(object) {
        Ok(acknowledgement) => acknowledgement,
        Err(response) => return route_response(response),
    };
    let result = match confirm_preview_to_bills_with_ack(
        runtime.connection_mut(),
        &session_id,
        user_id,
        history_acknowledgement.as_ref(),
    ) {
        Ok(result) => result,
        Err(bill_analyser_db::DbError::InvalidOperation(message)) => {
            return route_response(import_v2_error_response(400, &message));
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    route_response(import_stage_confirm_success(ImportStageConfirmData {
        imported_count: result.confirmed_count,
        skipped_count: result.skipped_count + result.duplicate_count,
        errors: result.errors,
    }))
}

fn history_rewrite_acknowledgement_from_payload(
    object: &Map<String, Value>,
) -> Result<Option<ImportHistoryRewriteAcknowledgement>, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &[
            "history_rewrite_acknowledgement",
            "historyRewriteAcknowledgement",
            "history_rewrite_ack",
            "historyRewriteAck",
        ],
    ) else {
        return Ok(None);
    };
    serde_json::from_value::<ImportHistoryRewriteAcknowledgement>(value.clone())
        .map(Some)
        .map_err(|_| import_v2_error_response(400, "Invalid history rewrite acknowledgement"))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_session_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_session_runtime_handler", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_session_cancel_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_session_cancel_runtime_handler", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_page_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    Query(query): Query<PreviewPageQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_preview_page_runtime_handler", "business operation entered");
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
    if let Some(response) = invalid_import_preview_query_response(&query) {
        return route_response(response);
    }
    let requested_preview_ids =
        parse_preview_page_query_ids(query.preview_ids.as_deref().or(query.preview_ids_camel.as_deref()));
    let normalized_query = normalize_import_preview_page_query(
        query.page.map(|value| value as i64),
        query
            .page_size
            .or(query.page_size_camel)
            .map(|value| value as i64),
        query.sort_by.as_deref().or(query.sort_by_camel.as_deref()),
        query
            .sort_direction
            .as_deref()
            .or(query.sort_direction_camel.as_deref()),
        requested_preview_ids.as_slice(),
    );
    let request = ImportPreviewPageRequest {
        page: normalized_query.page,
        page_size: normalized_query.page_size,
        sort_by: normalized_query.sort_by,
        sort_direction: normalized_query.sort_direction.as_str().to_string(),
        preview_ids: normalized_query.preview_ids,
        filters: ImportPreviewQueryFilters {
            min_datetime: query.min_datetime.or(query.min_datetime_camel),
            max_datetime: query.max_datetime.or(query.max_datetime_camel),
            transaction_type: query.transaction_type.or(query.transaction_type_camel),
            category: query.category,
            account: query.account,
            tag: query.tag,
            signal: query.signal,
            annotation: query.annotation,
            description: query.description,
            selected_only: query
                .selected_only
                .or(query.selected_only_camel)
                .unwrap_or(false),
        },
    };
    match query_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        &request,
    ) {
        Ok(result) => {
            let preview = result
                .rows
                .into_iter()
                .map(preview_row_to_value)
                .collect::<Vec<_>>();
            let query = serde_json::to_value(&request).ok();
            let metadata = serde_json::to_value(result.metadata).ok();
            route_response(import_preview_page_success(ImportPreviewPageData {
                preview,
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                query,
                metadata,
            }))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

fn invalid_import_preview_query_response(query: &PreviewPageQuery) -> Option<ImportV2RouteResponse> {
    let sort_by = query
        .sort_by
        .as_deref()
        .or(query.sort_by_camel.as_deref())
        .unwrap_or("")
        .trim();
    if !sort_by.is_empty() && !IMPORT_PREVIEW_SORT_KEYS.contains(&sort_by) {
        return Some(import_v2_error_response(400, "Unsupported preview sort key"));
    }
    let sort_direction = query
        .sort_direction
        .as_deref()
        .or(query.sort_direction_camel.as_deref())
        .unwrap_or("")
        .trim();
    if !sort_direction.is_empty()
        && !sort_direction.eq_ignore_ascii_case("asc")
        && !sort_direction.eq_ignore_ascii_case("desc")
    {
        return Some(import_v2_error_response(
            400,
            "Unsupported preview sort direction",
        ));
    }
    None
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_preview_page_query_ids(value: Option<&str>) -> Vec<i64> {
    value
        .unwrap_or("")
        .split(',')
        .filter_map(|item| item.trim().parse::<i64>().ok())
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_index_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_preview_index_runtime_handler", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_selection_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_preview_selection_runtime_handler", "business operation entered");
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
    let action = match preview_selection_action_from_payload(object) {
        Ok(action) => action,
        Err(response) => return route_response(response),
    };
    let filters = import_preview_query_filters_from_payload(object);
    let selection_request = ImportPreviewPageRequest {
        page: 1,
        page_size: usize::MAX / 2,
        sort_by: String::new(),
        sort_direction: "asc".to_string(),
        preview_ids: Vec::new(),
        filters: filters.clone(),
    };
    let (selection_mode, selection_target) = match action.as_str() {
        "select_valid" => (
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::Valid,
        ),
        "select_invalid" | "select_needs_annotation" => (
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::NeedsReview,
        ),
        "select_none" => (
            ImportPreviewSelectionMode::Deselect,
            ImportPreviewSelectionTarget::All,
        ),
        "invert" => (
            ImportPreviewSelectionMode::Invert,
            ImportPreviewSelectionTarget::All,
        ),
        _ => (
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::All,
        ),
    };
    let updated = match update_session_preview_selection_by_query(
        runtime.connection(),
        &session_id,
        user_id,
        selection_mode,
        selection_target,
        &selection_request,
    ) {
        Ok(updated) => updated,
        Err(error) => return route_response(db_error_response(error)),
    };
    let metadata = match query_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 1,
            sort_by: String::new(),
            sort_direction: "asc".to_string(),
            preview_ids: Vec::new(),
            filters,
        },
    ) {
        Ok(result) => serde_json::to_value(result.metadata).unwrap_or_else(|_| json!({})),
        Err(error) => return route_response(db_error_response(error)),
    };

    route_response(import_v2_data_response(json!({
        "updated": updated,
        "selectionAction": action,
        "metadata": metadata,
    })))
}

fn preview_selection_action_from_payload(
    object: &Map<String, Value>,
) -> Result<String, ImportV2RouteResponse> {
    let action = first_value(
        object,
        &["selectionAction", "selection_action", "action"],
    )
    .and_then(value_to_text)
    .unwrap_or_default();
    match action
        .trim()
        .replace('-', "_")
        .to_ascii_lowercase()
        .as_str()
    {
        "selectall" | "select_all" | "all" => Ok("select_all".to_string()),
        "selectvalid" | "select_valid" | "valid" => Ok("select_valid".to_string()),
        "selectinvalid" | "select_invalid" | "invalid" => Ok("select_invalid".to_string()),
        "selectneedsannotation" | "select_needs_annotation" | "needs_annotation" => {
            Ok("select_needs_annotation".to_string())
        }
        "selectnone" | "select_none" | "none" => Ok("select_none".to_string()),
        "selectinvert" | "select_invert" | "invert" => Ok("invert".to_string()),
        _ => Err(import_v2_error_response(400, "Invalid selection action")),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_query_filters_from_payload(object: &Map<String, Value>) -> ImportPreviewQueryFilters {
    let filters = first_value(object, &["filters", "queryFilters", "query_filters"])
        .and_then(Value::as_object);
    ImportPreviewQueryFilters {
        min_datetime: preview_filter_text(filters, &["minDatetime", "min_datetime"]),
        max_datetime: preview_filter_text(filters, &["maxDatetime", "max_datetime"]),
        transaction_type: preview_filter_text(filters, &["transactionType", "transaction_type"]),
        category: preview_filter_text(filters, &["category"]),
        account: preview_filter_text(filters, &["account"]),
        tag: preview_filter_text(filters, &["tag"]),
        signal: preview_filter_text(filters, &["signal"]),
        annotation: preview_filter_text(filters, &["annotation"]),
        description: preview_filter_text(filters, &["description"]),
        selected_only: false,
    }
}

fn preview_filter_text(
    filters: Option<&Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    filters.and_then(|object| {
        first_value(object, keys)
            .and_then(value_to_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_preview_update_runtime_handler", "business operation entered");
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
    let patch = match build_preview_patch_from_payload_with_category_lookup(
        runtime.connection(),
        user_id,
        preview.id,
        object,
    ) {
        Ok(patch) => patch,
        Err(response) => return route_response(response),
    };
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_reclassify_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_reclassify_runtime_handler", "business operation entered");
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
        patches.push(match build_preview_patch_from_payload_with_category_lookup(
            runtime.connection(),
            user_id,
            preview_id,
            item,
        ) {
            Ok(patch) => patch,
            Err(response) => return route_response(response),
        });
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
    )
    .await
    {
        return route_response(db_error_response(error));
    }
    enforce_import_preview_invariants(intelligent_drafts.as_mut_slice());
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_recurring_candidates_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "preview_recurring_candidates_runtime_handler", "business operation entered");
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
        .await
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_recurring_match_put_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "preview_recurring_match_put_runtime_handler", "business operation entered");
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
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        &preview.session_id,
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_recurring_match_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "preview_recurring_match_delete_runtime_handler", "business operation entered");
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
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    match update_preview_recurring_match_decision(
        runtime.connection_mut(),
        &preview.session_id,
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn preview_transfer_decision_runtime_handler(
    State(state): State<HttpAppState>,
    Path(preview_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "preview_transfer_decision_runtime_handler", "business operation entered");
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
    let mut runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let preview = match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
        Ok(Some(preview)) => preview,
        Ok(None) => return route_response(import_v2_error_response(404, "Preview bill not found")),
        Err(error) => return route_response(db_error_response(error)),
    };
    match apply_preview_transfer_decision(
        runtime.connection_mut(),
        &preview.session_id,
        preview_id,
        user_id,
        decision,
        Some(&expected_state),
    ) {
        Ok(result) => route_response(preview_decision_result_response(
            result,
            json!({"decision": decision_name(decision)}),
        )),
        Err(error) => route_response(db_error_response(error)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_suggestions_get_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_learning_suggestions_get_runtime_handler", "business operation entered");
    import_learning_suggestions_response(state, session_id, headers, None).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_suggestions_post_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_learning_suggestions_post_runtime_handler", "business operation entered");
    import_learning_suggestions_response(state, session_id, headers, Some(payload)).await
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
                "recommendation_key": learning
                    .get("recommendation_key")
                    .cloned()
                    .unwrap_or_else(|| json!("")),
                "lifecycle_status": learning
                    .get("lifecycle_status")
                    .cloned()
                    .unwrap_or_else(|| json!("yellow")),
                "signal_state": learning
                    .get("signal_state")
                    .cloned()
                    .unwrap_or_else(|| json!("yellow")),
                "accepted_count": learning
                    .get("accepted_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
                "rejected_count": learning
                    .get("rejected_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
                "auto_applied_count": learning
                    .get("auto_applied_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
            }))
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_promote_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_learning_promote_runtime_handler", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_rules_list_runtime_handler(
    State(state): State<HttpAppState>,
    Query(query): Query<ImportLearningRulesQuery>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_learning_rules_list_runtime_handler", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_rule_update_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_learning_rule_update_runtime_handler", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_learning_rule_delete_runtime_handler(
    State(state): State<HttpAppState>,
    Path(rule_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "import_learning_rule_delete_runtime_handler", "business operation entered");
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_rule_regex_flag_comes_from_rule_expression_json() {
        assert!(rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regex_enabled": true
        })));
        assert!(rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regexEnabled": "1"
        })));
        assert!(!rule_expression_regex_enabled(&json!({
            "expression": "商户"
        })));
        assert!(rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regex_enabled": 1
        })));
        assert!(!rule_expression_regex_enabled(&json!({
            "expression": "商户",
            "regex_enabled": null
        })));
    }

    #[test]
    fn import_db_error_response_keeps_public_message_generic() {
        let response = db_error_response("column category_rules.regex_enabled does not exist");

        assert_eq!(response.status_code, 500);
        assert_eq!(response.body["success"], false);
        assert_eq!(
            response.body["error"],
            "Rust import route runtime DB error"
        );
    }

    #[test]
    fn preview_category_type_validation_prefers_canonical_category_id() {
        let categories = vec![
            ImportIntelligenceCategory {
                id: 42,
                type_code: 2,
                main_category: "理财".to_string(),
                sub_category: "理财收益".to_string(),
            },
            ImportIntelligenceCategory {
                id: 99,
                type_code: 3,
                main_category: "理财".to_string(),
                sub_category: "理财收益".to_string(),
            },
        ];
        let draft = ImportPreviewDraft {
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            ..ImportPreviewDraft::default()
        };

        assert!(preview_category_matches_type(&draft, &categories, 2));
        assert!(!preview_category_matches_type(&draft, &categories, 3));
    }

    #[test]
    fn import_preview_patch_from_draft_carries_canonical_category_id() {
        let draft = ImportPreviewDraft {
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            ..ImportPreviewDraft::default()
        };

        let patch = import_preview_patch_from_draft(7, &draft);

        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::CategoryId
                && *value == ImportPreviewPatchValue::Integer(42)
        }));
    }

    #[test]
    fn preview_snapshots_and_row_draft_preserve_canonical_category_id() {
        let draft = ImportPreviewDraft {
            preview_type: "收入".to_string(),
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            preview_source_account_id: Some(7),
            preview_destination_account_id: Some(8),
            ..ImportPreviewDraft::default()
        };

        let stage2 = import_preview_stage2_snapshot(&draft);
        let transfer = import_preview_transfer_applied_snapshot(&draft);

        assert_eq!(stage2["category_id"], json!(42));
        assert_eq!(transfer["category_id"], json!(42));

        let row = ImportPreviewRow {
            id: 7,
            session_id: "session".to_string(),
            user_id: 1,
            preview_date: "2026-01-01 09:00:00".to_string(),
            preview_type: "收入".to_string(),
            preview_amount_cents: 123,
            preview_destination_amount_cents: 0,
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            preview_source_account_id: Some(7),
            preview_destination_account_id: Some(8),
            preview_counterparty: "基金平台".to_string(),
            preview_payment_method: "招商卡".to_string(),
            preview_description: "收益".to_string(),
            preview_parser_id: "fixture".to_string(),
            preview_parser_tags: vec!["tag".to_string()],
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: true,
            dedup_type: String::new(),
            dedup_source_ids: vec![1],
            preview_matching_feedback: json!({}),
            created_at: "2026-01-01 09:00:00".to_string(),
        };

        let row_draft = import_preview_draft_from_row(&row);

        assert_eq!(row_draft.category_id, Some(42));
        assert_eq!(row_draft.preview_main_category, "理财");
        assert_eq!(row_draft.preview_sub_category, "理财收益");
    }
}
