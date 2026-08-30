// 中文导读：把语义等价的历史 OR 子句链收敛为一个可读、可编辑的 OR clause。
// 维护重点：只规范化纯 OR 表达式；含 AND/NOT/REGEX 或无法完整解析的输入必须原样保留。

use super::compile_rule_expression;

/// 将 `(OR={A})|(OR={B})` 这类历史恢复表达式规范化为 `OR={A,B}`。
pub fn canonicalize_or_only_rule_expression(expression: &str, regex_enabled: bool) -> String {
    let original = expression.trim();
    if original.is_empty() || regex_enabled || compile_rule_expression(original, false).is_empty {
        return original.to_string();
    }

    let Some(factors) = split_top_level_or_factors(original) else {
        return original.to_string();
    };
    if factors.len() < 2 {
        return original.to_string();
    }

    let mut clause_contents = Vec::with_capacity(factors.len());
    for factor in factors {
        let unwrapped = strip_outer_parentheses(factor.trim()).trim();
        let Some(content) = unwrapped
            .strip_prefix("OR={")
            .and_then(|value| value.strip_suffix('}'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return original.to_string();
        };
        clause_contents.push(content);
    }

    let canonical = format!("OR={{{}}}", clause_contents.join(","));
    if compile_rule_expression(&canonical, false).is_empty {
        original.to_string()
    } else {
        canonical
    }
}

fn split_top_level_or_factors(expression: &str) -> Option<Vec<&str>> {
    let mut factors = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut escaped = false;

    for (index, value) in expression.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if value == '\\' && brace_depth > 0 {
            escaped = true;
            continue;
        }
        match value {
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.checked_sub(1)?,
            '(' if brace_depth == 0 => paren_depth += 1,
            ')' if brace_depth == 0 => paren_depth = paren_depth.checked_sub(1)?,
            '|' | '/' if brace_depth == 0 && paren_depth == 0 => {
                factors.push(expression[start..index].trim());
                start = index + value.len_utf8();
            }
            _ => {}
        }
    }
    if brace_depth != 0 || paren_depth != 0 {
        return None;
    }
    factors.push(expression[start..].trim());
    (!factors.iter().any(|factor| factor.is_empty())).then_some(factors)
}

fn strip_outer_parentheses(value: &str) -> &str {
    if !value.starts_with('(') || !value.ends_with(')') {
        return value;
    }

    let mut depth = 0usize;
    let mut brace_depth = 0usize;
    let mut escaped = false;
    for (index, current) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if current == '\\' && brace_depth > 0 {
            escaped = true;
            continue;
        }
        match current {
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '(' if brace_depth == 0 => depth += 1,
            ')' if brace_depth == 0 => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index + current.len_utf8() != value.len() {
                    return value;
                }
            }
            _ => {}
        }
    }
    &value[1..value.len() - 1]
}
