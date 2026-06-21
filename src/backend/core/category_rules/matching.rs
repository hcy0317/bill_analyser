// 中文导读：分类规则表达式匹配器，负责 AST、普通关键词和正则表达式的命中判断。
// 维护重点：保持大小写、正则缓存和 NOT/AND/OR 语义稳定，避免影响导入与规则中心。

use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

use regex::Regex;

use super::{
    parser::compile_rule_expression,
    types::{CompiledRuleDto, RuleExpressionNodeDto},
};

static RULE_REGEX_CACHE: OnceLock<RwLock<HashMap<String, Option<Regex>>>> = OnceLock::new();

pub fn match_rule_expression(text: &str, expr: &str, regex_enabled: bool) -> bool {
    let compiled = compile_rule_expression(expr, regex_enabled);
    match_compiled_rule(text, &compiled)
}

pub fn match_compiled_rule(text: &str, compiled: &CompiledRuleDto) -> bool {
    if compiled.is_empty || text.is_empty() {
        return false;
    }

    let text_lower = text.to_lowercase();
    match_compiled_rule_lowercase_text(&text_lower, compiled)
}

pub fn match_compiled_rule_lowercase_text(text_lower: &str, compiled: &CompiledRuleDto) -> bool {
    if compiled.is_empty || text_lower.is_empty() {
        return false;
    }

    if let Some(expression_ast) = &compiled.expression_ast {
        return match_rule_expression_node(text_lower, expression_ast);
    }

    if compiled
        .not_patterns
        .iter()
        .any(|pattern| match_rule_pattern(text_lower, pattern))
    {
        return false;
    }

    if compiled
        .and_patterns
        .iter()
        .any(|pattern| !match_rule_pattern(text_lower, pattern))
    {
        return false;
    }

    if !compiled.or_blocks.is_empty() {
        return compiled.or_blocks.iter().all(|block| {
            block
                .iter()
                .any(|pattern| match_rule_pattern(text_lower, pattern))
        });
    }

    !compiled.and_patterns.is_empty() || !compiled.not_patterns.is_empty()
}

fn match_rule_expression_node(text_lower: &str, node: &RuleExpressionNodeDto) -> bool {
    match node.kind.as_str() {
        "all" => {
            !node.children.is_empty()
                && node
                    .children
                    .iter()
                    .all(|child| match_rule_expression_node(text_lower, child))
        }
        "any" => node
            .children
            .iter()
            .any(|child| match_rule_expression_node(text_lower, child)),
        "not" => {
            node.children.len() == 1 && !match_rule_expression_node(text_lower, &node.children[0])
        }
        "clause" if node.operator == "OR" => node
            .patterns
            .iter()
            .any(|pattern| match_rule_pattern(text_lower, pattern)),
        "clause" if node.operator == "AND" => {
            !node.patterns.is_empty()
                && node
                    .patterns
                    .iter()
                    .all(|pattern| match_rule_pattern(text_lower, pattern))
        }
        "clause" if node.operator == "NOT" => !node
            .patterns
            .iter()
            .any(|pattern| match_rule_pattern(text_lower, pattern)),
        _ => false,
    }
}

fn match_rule_pattern(text_lower: &str, pattern: &str) -> bool {
    if let Some(regex_pattern) = pattern.strip_prefix("regex:") {
        if regex_pattern_is_plain_literal(regex_pattern) {
            return plain_regex_literal_matches(text_lower, regex_pattern);
        }
        return cached_rule_regex(regex_pattern).is_some_and(|regex| regex.is_match(text_lower));
    }
    text_lower.contains(pattern)
}

fn plain_regex_literal_matches(text_lower: &str, pattern: &str) -> bool {
    if pattern.is_ascii() {
        return text_lower.contains(&pattern.to_ascii_lowercase());
    }
    text_lower.contains(pattern)
}

fn regex_pattern_is_plain_literal(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.chars().any(|ch| {
            matches!(
                ch,
                '\\' | '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
            )
        })
}

fn cached_rule_regex(pattern: &str) -> Option<Regex> {
    let cache = RULE_REGEX_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Ok(reader) = cache.read() {
        if let Some(regex) = reader.get(pattern) {
            return regex.clone();
        }
    }

    let compiled = regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .ok();
    if let Ok(mut writer) = cache.write() {
        writer
            .entry(pattern.to_string())
            .or_insert_with(|| compiled.clone());
    }
    compiled
}
