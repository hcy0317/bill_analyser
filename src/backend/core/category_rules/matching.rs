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

/// 直接编译并匹配分类规则表达式，适用于单次规则测试和低频调用。
pub fn match_rule_expression(text: &str, expr: &str, regex_enabled: bool) -> bool {
    let compiled = compile_rule_expression(expr, regex_enabled);
    match_compiled_rule(text, &compiled)
}

/// 匹配已编译分类规则，外层负责大小写归一和空表达式 fail closed。
pub fn match_compiled_rule(text: &str, compiled: &CompiledRuleDto) -> bool {
    if compiled.is_empty || text.is_empty() {
        return false;
    }

    let text_lower = text.to_lowercase();
    match_compiled_rule_lowercase_text(&text_lower, compiled)
}

/// 在已转小写文本上匹配 compiled 结构，优先使用 AST，缺失时保留旧字段语义。
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

/// 递归执行分类规则 AST，集中维护 all/any/not/clause 的布尔语义。
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

/// 匹配单个 pattern，正则 pattern 使用缓存并对不可编译正则 fail closed。
fn match_rule_pattern(text_lower: &str, pattern: &str) -> bool {
    if let Some(regex_pattern) = pattern.strip_prefix("regex:") {
        if regex_pattern_is_plain_literal(regex_pattern) {
            return plain_regex_literal_matches(text_lower, regex_pattern);
        }
        return cached_rule_regex(regex_pattern).is_some_and(|regex| regex.is_match(text_lower));
    }
    text_lower.contains(pattern)
}

/// 对没有正则元字符的 `regex:` pattern 走普通 contains，避免无意义正则编译。
fn plain_regex_literal_matches(text_lower: &str, pattern: &str) -> bool {
    if pattern.is_ascii() {
        return text_lower.contains(&pattern.to_ascii_lowercase());
    }
    text_lower.contains(pattern)
}

/// 判断 regex pattern 是否只是普通字面量，用于正则匹配的快速路径。
fn regex_pattern_is_plain_literal(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.chars().any(|ch| {
            matches!(
                ch,
                '\\' | '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
            )
        })
}

/// 编译并缓存规则正则，缓存 `None` 以避免非法正则在批量导入时重复编译。
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
