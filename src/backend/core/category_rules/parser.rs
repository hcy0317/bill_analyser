// 中文导读：分类规则表达式解析器，负责把用户输入解析为 AST 和旧版 compiled 字段。
// 维护重点：只处理表达式语法，不执行文本匹配；匹配语义在 matching.rs。

use super::{
    terms::{
        is_escapable_char, is_factor_terminator, is_or_connector, split_rule_expression_terms,
    },
    types::{CompiledRuleDto, RuleExpressionNodeDto},
};

pub fn compile_rule_expression(expr: &str, regex_enabled: bool) -> CompiledRuleDto {
    if expr.is_empty() {
        return CompiledRuleDto::empty();
    }

    let mut parser = RuleExpressionParser::new(expr, regex_enabled);
    match parser.parse() {
        Ok(Some(expression_ast)) => {
            let mut compiled = CompiledRuleDto {
                or_blocks: Vec::new(),
                not_patterns: Vec::new(),
                and_patterns: Vec::new(),
                is_empty: false,
                expression_ast: Some(expression_ast),
            };
            if let Some(node) = compiled.expression_ast.clone() {
                collect_compiled_fields(&node, &mut compiled);
            }
            compiled
        }
        Ok(None) | Err(_) => CompiledRuleDto::empty(),
    }
}

struct RuleExpressionParser {
    chars: Vec<char>,
    regex_enabled: bool,
}

impl RuleExpressionParser {
    fn new(expr: &str, regex_enabled: bool) -> Self {
        Self {
            chars: expr.chars().collect(),
            regex_enabled,
        }
    }

    fn parse(&mut self) -> Result<Option<RuleExpressionNodeDto>, String> {
        let (node, index) = self.parse_or_expression(0)?;
        let index = self.skip_space(index);
        if index != self.chars.len() {
            return Err(format!("unexpected token at offset {index}"));
        }
        Ok(node)
    }

    fn parse_or_expression(
        &self,
        mut index: usize,
    ) -> Result<(Option<RuleExpressionNodeDto>, usize), String> {
        let mut children = Vec::new();
        let (left, next_index) = self.parse_and_expression(index)?;
        index = next_index;
        if let Some(node) = left {
            children.push(node);
        }

        loop {
            index = self.skip_space(index);
            if index >= self.chars.len() || !is_or_connector(self.chars[index]) {
                break;
            }
            let connector_index = index;
            index += 1;
            let (right, next_index) = self.parse_and_expression(index)?;
            index = next_index;
            let Some(node) = right else {
                return Err(format!(
                    "missing expression after OR connector at offset {connector_index}"
                ));
            };
            children.push(node);
        }

        Ok((collapse_children("any", children), index))
    }

    fn parse_and_expression(
        &self,
        mut index: usize,
    ) -> Result<(Option<RuleExpressionNodeDto>, usize), String> {
        let mut children = Vec::new();
        let mut pending_negated_connector = false;
        let mut pending_connector = false;

        loop {
            index = self.skip_space(index);
            if index >= self.chars.len()
                || self.chars[index] == ')'
                || is_or_connector(self.chars[index])
            {
                if pending_connector {
                    return Err(format!(
                        "missing expression after AND/NOT connector at offset {index}"
                    ));
                }
                break;
            }

            if let Some((negated, next_index)) =
                self.read_and_connector(index, !children.is_empty())
            {
                if pending_connector {
                    return Err(format!(
                        "missing expression after AND/NOT connector at offset {index}"
                    ));
                }
                pending_negated_connector = negated;
                pending_connector = true;
                index = next_index;
                continue;
            }
            if children.is_empty() && self.read_not_connector(index).is_some() {
                return Err(format!(
                    "unexpected leading NOT connector at offset {index}"
                ));
            }

            let (child, next_index) = self.parse_factor(index)?;
            index = next_index;
            if let Some(mut node) = child {
                if pending_negated_connector {
                    node = apply_not_connector(node);
                }
                pending_connector = false;
                children.push(node);
            } else if pending_connector {
                return Err(format!(
                    "missing expression after AND/NOT connector at offset {index}"
                ));
            } else {
                return Err(format!("expected rule expression factor at offset {index}"));
            }

            index = self.skip_space(index);
            if index >= self.chars.len()
                || self.chars[index] == ')'
                || is_or_connector(self.chars[index])
            {
                break;
            }
            if let Some((negated, next_index)) = self.read_and_connector(index, true) {
                if pending_connector {
                    return Err(format!(
                        "missing expression after AND/NOT connector at offset {index}"
                    ));
                }
                pending_negated_connector = negated;
                pending_connector = true;
                index = next_index;
                continue;
            }
            return Err(format!("expected AND/OR connector at offset {index}"));
        }

        Ok((collapse_children("all", children), index))
    }

    fn parse_factor(
        &self,
        mut index: usize,
    ) -> Result<(Option<RuleExpressionNodeDto>, usize), String> {
        index = self.skip_space(index);
        if index >= self.chars.len() {
            return Ok((None, index));
        }

        if self.chars[index] == '(' {
            let (node, next_index) = self.parse_or_expression(index + 1)?;
            index = self.skip_space(next_index);
            if index >= self.chars.len() || self.chars[index] != ')' {
                return Err("missing closing ')' in rule expression".to_string());
            }
            return Ok((node, index + 1));
        }

        let start = index;
        let mut brace_depth = 0usize;
        while index < self.chars.len() {
            let char_value = self.chars[index];
            if char_value == '\\'
                && brace_depth > 0
                && index + 1 < self.chars.len()
                && is_escapable_char(self.chars[index + 1])
            {
                index += 2;
                continue;
            }
            if char_value == '{' {
                brace_depth += 1;
            } else if char_value == '}' && brace_depth > 0 {
                brace_depth -= 1;
            } else if brace_depth == 0 {
                if is_factor_terminator(char_value) {
                    break;
                }
                if index > start && self.read_not_connector(index).is_some() {
                    break;
                }
            }
            index += 1;
        }

        let block = self.slice(start, index).trim().to_string();
        if block.is_empty() {
            return Ok((None, index));
        }
        Ok((Some(self.parse_clause(&block)?), index))
    }

    fn parse_clause(&self, block: &str) -> Result<RuleExpressionNodeDto, String> {
        let Some(eq_idx) = block.find("={") else {
            let pattern = if self.regex_enabled {
                format!("regex:{block}")
            } else {
                block.to_lowercase()
            };
            return Ok(clause("OR", vec![pattern]));
        };

        let prefix = block[..eq_idx].trim().to_uppercase();
        let mut content = block[eq_idx + 2..].trim().to_string();
        if !content.ends_with('}') {
            return Err(format!("missing closing '}}' in clause {block:?}"));
        }
        content.pop();

        let keywords = split_rule_expression_terms(&content);
        let operator = match prefix.as_str() {
            "OR" | "AND" | "NOT" => prefix.as_str(),
            _ => "OR",
        };
        let force_regex = prefix == "REGEX";
        let patterns = keywords
            .into_iter()
            .map(|keyword| {
                if self.regex_enabled || force_regex {
                    format!("regex:{keyword}")
                } else {
                    keyword.to_lowercase()
                }
            })
            .collect();
        Ok(clause(operator, patterns))
    }

    fn skip_space(&self, mut index: usize) -> usize {
        while index < self.chars.len() && self.chars[index].is_whitespace() {
            index += 1;
        }
        index
    }

    fn read_and_connector(&self, index: usize, allow_word_not: bool) -> Option<(bool, usize)> {
        let char_value = *self.chars.get(index)?;
        if char_value == '+' {
            return allow_word_not.then_some((false, index + 1));
        }
        if char_value == '×' {
            return allow_word_not.then_some((true, index + 1));
        }
        if allow_word_not {
            if let Some(next_index) = self.read_not_connector(index) {
                return Some((true, next_index));
            }
        }
        None
    }

    fn read_not_connector(&self, index: usize) -> Option<usize> {
        if index + 3 > self.chars.len() {
            return None;
        }
        let token: String = self.chars[index..index + 3].iter().collect();
        if token.to_uppercase() != "NOT" {
            return None;
        }

        let before = if index > 0 {
            Some(self.chars[index - 1])
        } else {
            None
        };
        let after = self.chars.get(index + 3).copied();
        if before.is_some_and(|value| value.is_alphanumeric() || value == '_') {
            return None;
        }
        if after == Some('=') || after.is_some_and(|value| value.is_alphanumeric() || value == '_')
        {
            return None;
        }

        let mut next_index = index + 3;
        while next_index < self.chars.len() && self.chars[next_index].is_whitespace() {
            next_index += 1;
        }
        Some(next_index)
    }

    fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }
}

fn collapse_children(
    kind: &str,
    children: Vec<RuleExpressionNodeDto>,
) -> Option<RuleExpressionNodeDto> {
    if children.is_empty() {
        None
    } else if children.len() == 1 {
        children.into_iter().next()
    } else {
        Some(RuleExpressionNodeDto {
            kind: kind.to_string(),
            operator: String::new(),
            patterns: Vec::new(),
            children,
        })
    }
}

fn apply_not_connector(child: RuleExpressionNodeDto) -> RuleExpressionNodeDto {
    if child.kind == "clause" && child.operator == "NOT" {
        return child;
    }
    RuleExpressionNodeDto {
        kind: "not".to_string(),
        operator: String::new(),
        patterns: Vec::new(),
        children: vec![child],
    }
}

fn clause(operator: &str, patterns: Vec<String>) -> RuleExpressionNodeDto {
    RuleExpressionNodeDto {
        kind: "clause".to_string(),
        operator: operator.to_string(),
        patterns,
        children: Vec::new(),
    }
}

fn collect_compiled_fields(node: &RuleExpressionNodeDto, compiled: &mut CompiledRuleDto) {
    match node.kind.as_str() {
        "all" | "any" => {
            for child in &node.children {
                collect_compiled_fields(child, compiled);
            }
        }
        "not" => {}
        "clause" if node.operator == "OR" && !node.patterns.is_empty() => {
            compiled.or_blocks.push(node.patterns.clone());
        }
        "clause" if node.operator == "AND" => {
            compiled.and_patterns.extend(node.patterns.clone());
        }
        "clause" if node.operator == "NOT" => {
            compiled.not_patterns.extend(node.patterns.clone());
        }
        _ => {}
    }
}
