// 中文导读：分类规则候选编译与选择内核，统一目标资格、表达式 fail-closed 和优先级决策。
// 维护重点：调用方只提供当前允许的分类类型；数据库读取、转账授权和结果写回仍由各 adapter 负责。

use super::{compile_rule_expression, match_compiled_rule_lowercase_text, CompiledRuleDto};

#[derive(Debug, Clone)]
pub struct CategoryRuleCandidateDraft {
    pub id: i64,
    pub category_id: i64,
    pub category_type: i32,
    pub main_category: String,
    pub sub_category: String,
    pub priority: i32,
    pub rule_expression: String,
    pub regex_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRuleCandidate {
    pub id: i64,
    pub category_id: i64,
    pub category_type: i32,
    pub main_category: String,
    pub sub_category: String,
    pub priority: i32,
    pub rule_expression: String,
    pub regex_enabled: bool,
    compiled_expression: CompiledRuleDto,
}

impl CategoryRuleCandidate {
    pub fn compile(draft: CategoryRuleCandidateDraft) -> Option<Self> {
        let main_category = draft.main_category.trim().to_string();
        let sub_category = draft.sub_category.trim().to_string();
        if draft.category_id <= 0 || (main_category.is_empty() && sub_category.is_empty()) {
            return None;
        }

        let rule_expression = draft.rule_expression.trim().to_string();
        let compiled_expression = compile_rule_expression(&rule_expression, draft.regex_enabled);
        if compiled_expression.is_empty {
            return None;
        }

        Some(Self {
            id: draft.id,
            category_id: draft.category_id,
            category_type: draft.category_type,
            main_category,
            sub_category,
            priority: draft.priority,
            rule_expression,
            regex_enabled: draft.regex_enabled,
            compiled_expression,
        })
    }
}

pub fn select_category_rule_candidate<'a>(
    candidates: &'a [CategoryRuleCandidate],
    allowed_category_types: &[i32],
    text: &str,
) -> Option<&'a CategoryRuleCandidate> {
    if text.is_empty() || allowed_category_types.is_empty() {
        return None;
    }

    let text_lower = text.to_lowercase();
    candidates
        .iter()
        .filter(|candidate| allowed_category_types.contains(&candidate.category_type))
        .filter(|candidate| {
            match_compiled_rule_lowercase_text(&text_lower, &candidate.compiled_expression)
        })
        .min_by_key(|candidate| (candidate.priority, candidate.id))
}
