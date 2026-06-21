#[derive(Debug, Clone, Default)]
struct ImportIntelligenceStats {
    category_matched: usize,
    account_matched: usize,
    learning_applied: usize,
    learning_vector_recalled: usize,
    learning_vector_status: String,
    recurring_projected: usize,
    _elapsed_load_ms: u128,
    elapsed_category_rule_ns: u128,
    elapsed_recurring_rule_ns: u128,
    elapsed_learning_rule_ns: u128,
    elapsed_account_rule_ns: u128,
    elapsed_stage2_baseline_ns: u128,
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
    scope_order: usize,
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
    compiled_expression: CompiledRuleDto,
}

#[derive(Debug, Clone, Default)]
struct ImportIntelligenceRuleSet {
    income: Vec<ImportIntelligenceRule>,
    expense: Vec<ImportIntelligenceRule>,
    transfer: Vec<ImportIntelligenceRule>,
    investment: Vec<ImportIntelligenceRule>,
}

impl ImportIntelligenceRuleSet {
    fn from_rules(rules: Vec<ImportIntelligenceRule>) -> Self {
        let mut rule_set = Self::default();
        for rule in rules {
            match rule.category_type {
                2 => rule_set.income.push(rule),
                3 => rule_set.expense.push(rule),
                4 => rule_set.transfer.push(rule),
                5 => rule_set.investment.push(rule),
                _ => {}
            }
        }
        rule_set
    }

    fn non_transfer_by_priority(&self) -> Vec<&ImportIntelligenceRule> {
        let mut rules = self
            .income
            .iter()
            .chain(self.expense.iter())
            .chain(self.investment.iter())
            .collect::<Vec<_>>();
        rules.sort_by_key(|rule| (rule.priority, rule.id));
        rules
    }

    fn for_type(&self, category_type: i64) -> &[ImportIntelligenceRule] {
        match category_type {
            2 => &self.income,
            3 => &self.expense,
            4 => &self.transfer,
            5 => &self.investment,
            _ => &[],
        }
    }
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
