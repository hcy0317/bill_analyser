// 中文导读：导入 Stage 2 的应用边界，负责为单批评估加载一次上下文并调用固定顺序的业务链。
// 维护重点：route 只调用 `ImportStage2::evaluate`；SQL 留在 DB repository，snapshot 不跨请求复用。
// 不变式：每次 evaluate 都读取当前 user-scoped 权威数据，不缓存 mutation 或 confirm 所需身份事实。

struct ImportStage2;

impl ImportStage2 {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn evaluate(
        connection: &Connection,
        user_id: UserId,
        drafts: &mut [ImportPreviewDraft],
    ) -> Result<ImportIntelligenceStats, bill_analyser_db::DbError> {
        let load_started_at = Instant::now();
        let rows = load_import_stage2_context(connection, user_id).await?;
        let context = ImportStage2ContextSnapshot::from_rows(rows);
        let mut stats = evaluate_import_intelligence_snapshot(connection, drafts, &context)?;
        stats._elapsed_load_ms = import_stage_elapsed_ms(load_started_at);
        Ok(stats)
    }
}

#[derive(Debug, Clone)]
struct ImportStage2ContextSnapshot {
    user_id: i64,
    categories: Vec<ImportIntelligenceCategory>,
    categories_by_id: BTreeMap<i64, ImportIntelligenceCategory>,
    category_values: Vec<Value>,
    category_rules: ImportIntelligenceRuleSet,
    accounts: Vec<ImportIntelligenceAccount>,
    account_values: Vec<Value>,
    account_rules: Vec<CompiledAccountRuleCandidate>,
    learning_rules: Vec<ImportIntelligenceLearningRule>,
    recurring_templates: Vec<ImportIntelligenceRecurringTemplate>,
    transfer_category: Option<ImportIntelligenceCategory>,
}

impl ImportStage2ContextSnapshot {
    fn from_rows(rows: ImportStage2ContextRows) -> Self {
        let categories = rows
            .categories
            .into_iter()
            .map(import_intelligence_category_from_record)
            .collect::<Vec<_>>();
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
            rows.category_rules
                .into_iter()
                .filter_map(|row| import_intelligence_rule_from_record(row, &categories_by_id))
                .collect(),
        );
        let accounts = rows
            .accounts
            .into_iter()
            .map(|row| ImportIntelligenceAccount {
                id: row.id,
                name: row.name,
            })
            .collect::<Vec<_>>();
        let account_values = accounts
            .iter()
            .map(import_intelligence_account_value)
            .collect::<Vec<_>>();
        let account_rules = compile_account_rule_candidates(&rows.account_rules);
        let learning_rules = rows
            .learning_rules
            .into_iter()
            .map(|row| ImportIntelligenceLearningRule {
                id: row.id,
                parser_id: text_from_json(&row.metadata, "parser_id"),
                composite_hash: text_from_json(&row.metadata, "composite_hash"),
                match_features: string_map_from_json(row.metadata.get("match_features")),
                learned_type: optional_text_from_json(&row.metadata, "learned_type")
                    .or(Some(row.recommendation_type)),
                learned_category_id: optional_i64_from_json(&row.metadata, "learned_category_id"),
                learned_source_account_id: optional_i64_from_json(
                    &row.metadata,
                    "learned_source_account_id",
                ),
                learned_destination_account_id: optional_i64_from_json(
                    &row.metadata,
                    "learned_destination_account_id",
                ),
            })
            .collect();
        let recurring_templates = rows
            .recurring_templates
            .into_iter()
            .map(import_intelligence_recurring_template_from_record)
            .collect();
        let transfer_category = default_transfer_category(
            &categories,
            &categories_by_id,
            rows.cash_transfer_category_id,
        )
        .cloned();
        Self {
            user_id: rows.user_id,
            categories,
            categories_by_id,
            category_values,
            category_rules,
            accounts,
            account_values,
            account_rules,
            learning_rules,
            recurring_templates,
            transfer_category,
        }
    }
}

fn import_intelligence_category_from_record(
    row: ImportStage2CategoryRecord,
) -> ImportIntelligenceCategory {
    let (main_category, sub_category) =
        import_intelligence_category_parts(row.path.as_deref(), &row.name);
    ImportIntelligenceCategory {
        id: row.id,
        type_code: preview_type_code(row.category_type.as_deref().unwrap_or_default()).unwrap_or(3),
        main_category,
        sub_category,
    }
}

fn import_intelligence_rule_from_record(
    row: ImportStage2CategoryRuleRecord,
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
) -> Option<ImportIntelligenceRule> {
    let category = categories_by_id.get(&row.category_id)?;
    let regex_enabled = rule_expression_regex_enabled(&row.rule_expression);
    let rule_expression = rule_expression_string(&row.rule_expression);
    Some(ImportIntelligenceRule {
        id: row.id,
        category_id: row.category_id,
        category_type: category.type_code,
        main_category: category.main_category.clone(),
        sub_category: category.sub_category.clone(),
        priority: i64::from(row.priority),
        compiled_expression: compile_rule_expression(&rule_expression, regex_enabled),
        rule_expression,
        regex_enabled,
    })
}

fn import_intelligence_recurring_template_from_record(
    row: ImportStage2RecurringTemplateRecord,
) -> ImportIntelligenceRecurringTemplate {
    ImportIntelligenceRecurringTemplate {
        id: row.id,
        name: row.name,
        bill_type: row.transaction_type.unwrap_or_default(),
        amount_cents: row.source_amount_minor_units,
        account: row
            .source_account_id
            .map(|value| value.to_string())
            .or_else(|| optional_text_from_json(&row.metadata, "account"))
            .unwrap_or_default(),
        counterparty: text_from_json(&row.metadata, "counterparty"),
        next_date: row.scheduled_next_date.unwrap_or_default(),
        start_date: row.scheduled_start_date.unwrap_or_default(),
    }
}

async fn load_import_intelligence_categories(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceCategory>, bill_analyser_db::DbError> {
    load_import_stage2_category_records(connection, user_id)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(import_intelligence_category_from_record)
                .collect()
        })
}

async fn load_import_intelligence_category_rules(
    connection: &Connection,
    user_id: i64,
    categories_by_id: &BTreeMap<i64, ImportIntelligenceCategory>,
) -> Result<Vec<ImportIntelligenceRule>, bill_analyser_db::DbError> {
    load_import_stage2_category_rule_records(connection, user_id)
        .await
        .map(|rows| {
            rows.into_iter()
                .filter_map(|row| import_intelligence_rule_from_record(row, categories_by_id))
                .collect()
        })
}

async fn load_import_intelligence_accounts(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceAccount>, bill_analyser_db::DbError> {
    load_import_stage2_account_records(connection, user_id)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| ImportIntelligenceAccount {
                    id: row.id,
                    name: row.name,
                })
                .collect()
        })
}

async fn load_import_intelligence_account_rules(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<AccountRuleCandidate>, bill_analyser_db::DbError> {
    load_import_stage2_account_rule_candidates(connection, user_id).await
}

async fn load_import_intelligence_recurring_templates(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<ImportIntelligenceRecurringTemplate>, bill_analyser_db::DbError> {
    load_import_stage2_recurring_template_records(connection, user_id)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(import_intelligence_recurring_template_from_record)
                .collect()
        })
}

fn user_id_i64_for_sql(user_id: UserId) -> Result<i64, bill_analyser_db::DbError> {
    i64::try_from(user_id.get())
        .map_err(|_| bill_analyser_db::DbError::InvalidOperation("invalid user id".to_string()))
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
