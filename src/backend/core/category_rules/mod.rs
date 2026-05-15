use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleExpressionNodeDto {
    pub kind: String,
    pub operator: String,
    pub patterns: Vec<String>,
    pub children: Vec<RuleExpressionNodeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledRuleDto {
    pub or_blocks: Vec<Vec<String>>,
    pub not_patterns: Vec<String>,
    pub and_patterns: Vec<String>,
    pub is_empty: bool,
    pub expression_ast: Option<RuleExpressionNodeDto>,
}

impl CompiledRuleDto {
    fn empty() -> Self {
        Self {
            or_blocks: Vec::new(),
            not_patterns: Vec::new(),
            and_patterns: Vec::new(),
            is_empty: true,
            expression_ast: None,
        }
    }
}

pub fn compile_rule_expression(expr: &str, regex_enabled: bool) -> CompiledRuleDto {
    if expr.is_empty() {
        return CompiledRuleDto::empty();
    }

    if !expr.contains("={") {
        return compile_legacy_rule(expr);
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
                collect_legacy_compiled_fields(&node, &mut compiled);
            }
            compiled
        }
        Ok(None) | Err(_) => CompiledRuleDto::empty(),
    }
}

pub fn match_rule_expression(text: &str, expr: &str, regex_enabled: bool) -> bool {
    let compiled = compile_rule_expression(expr, regex_enabled);
    match_compiled_rule(text, &compiled)
}

pub fn escape_rule_expression_term(term: &str) -> String {
    let mut output = String::new();
    for value in term.chars() {
        if is_escapable_char(value) {
            output.push('\\');
        }
        output.push(value);
    }
    output
}

pub fn match_compiled_rule(text: &str, compiled: &CompiledRuleDto) -> bool {
    if compiled.is_empty || text.is_empty() {
        return false;
    }

    let text_lower = text.to_lowercase();
    if let Some(expression_ast) = &compiled.expression_ast {
        return match_rule_expression_node(&text_lower, expression_ast);
    }

    if compiled
        .not_patterns
        .iter()
        .any(|pattern| match_rule_pattern(&text_lower, pattern))
    {
        return false;
    }

    if compiled
        .and_patterns
        .iter()
        .any(|pattern| !match_rule_pattern(&text_lower, pattern))
    {
        return false;
    }

    if !compiled.or_blocks.is_empty() {
        return compiled.or_blocks.iter().all(|block| {
            block
                .iter()
                .any(|pattern| match_rule_pattern(&text_lower, pattern))
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
        return regex::RegexBuilder::new(regex_pattern)
            .case_insensitive(true)
            .build()
            .is_ok_and(|regex| regex.is_match(text_lower));
    }
    text_lower.contains(pattern)
}

fn compile_legacy_rule(rule: &str) -> CompiledRuleDto {
    if rule.is_empty() {
        return CompiledRuleDto::empty();
    }

    let mut compiled = CompiledRuleDto {
        or_blocks: Vec::new(),
        not_patterns: Vec::new(),
        and_patterns: Vec::new(),
        is_empty: false,
        expression_ast: None,
    };
    let mut simple_patterns = Vec::new();

    for raw_part in rule.split('&') {
        let part = raw_part.trim();
        if part.is_empty() {
            continue;
        }
        let part_upper = part.to_uppercase();
        if part_upper.starts_with("OR:") {
            let block: Vec<String> = part[3..]
                .split('|')
                .map(|keyword| keyword.trim().to_lowercase())
                .filter(|keyword| !keyword.is_empty())
                .collect();
            if !block.is_empty() {
                compiled.or_blocks.push(block);
            }
        } else if part_upper.starts_with("NOT:") {
            compiled.not_patterns.extend(
                part[4..]
                    .split('|')
                    .map(|keyword| keyword.trim().to_lowercase())
                    .filter(|keyword| !keyword.is_empty()),
            );
        } else if part_upper.starts_with("AND:") {
            compiled.and_patterns.extend(
                part[4..]
                    .split('|')
                    .map(|keyword| keyword.trim().to_lowercase())
                    .filter(|keyword| !keyword.is_empty()),
            );
        } else if part_upper.starts_with("REGEX:") {
            let regex_pattern = part[6..].trim();
            if !regex_pattern.is_empty() {
                compiled
                    .or_blocks
                    .push(vec![format!("regex:{regex_pattern}")]);
            }
        } else {
            simple_patterns.push(part.to_lowercase());
        }
    }

    if !simple_patterns.is_empty() {
        compiled.or_blocks.push(simple_patterns);
    }
    compiled
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

fn collect_legacy_compiled_fields(node: &RuleExpressionNodeDto, compiled: &mut CompiledRuleDto) {
    match node.kind.as_str() {
        "all" | "any" => {
            for child in &node.children {
                collect_legacy_compiled_fields(child, compiled);
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

fn split_rule_expression_terms(content: &str) -> Vec<String> {
    let chars: Vec<char> = content.chars().collect();
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let char_value = chars[index];
        if char_value == '\\' && index + 1 < chars.len() && is_escapable_char(chars[index + 1]) {
            current.push(char_value);
            current.push(chars[index + 1]);
            index += 2;
            continue;
        }
        if char_value == ',' {
            let term = unescape_rule_expression_term(current.trim());
            if !term.is_empty() {
                terms.push(term);
            }
            current.clear();
            index += 1;
            continue;
        }
        current.push(char_value);
        index += 1;
    }

    let term = unescape_rule_expression_term(current.trim());
    if !term.is_empty() {
        terms.push(term);
    }
    terms
}

fn unescape_rule_expression_term(term: &str) -> String {
    let chars: Vec<char> = term.chars().collect();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let char_value = chars[index];
        if char_value == '\\' && index + 1 < chars.len() && is_escapable_char(chars[index + 1]) {
            output.push(chars[index + 1]);
            index += 2;
            continue;
        }
        output.push(char_value);
        index += 1;
    }
    output
}

fn is_escapable_char(value: char) -> bool {
    matches!(
        value,
        '\\' | ',' | '+' | '{' | '}' | '|' | '(' | ')' | '/' | '×'
    )
}

fn is_or_connector(value: char) -> bool {
    matches!(value, '|' | '/')
}

fn is_factor_terminator(value: char) -> bool {
    matches!(value, '+' | '×' | '|' | '/' | ')')
}

#[cfg(test)]
mod tests {
    use super::{compile_rule_expression, match_rule_expression};

    #[test]
    fn compiles_legacy_rule_expression() {
        let compiled = compile_rule_expression("OR:滴滴|快的&AND:打车&NOT:退款", false);

        assert_eq!(compiled.or_blocks, vec![vec!["滴滴", "快的"]]);
        assert_eq!(compiled.and_patterns, vec!["打车"]);
        assert_eq!(compiled.not_patterns, vec!["退款"]);
        assert!(compiled.expression_ast.is_none());
    }

    #[test]
    fn compiles_composite_expression_with_visible_not() {
        let compiled = compile_rule_expression(
            "(OR={早餐}/OR={早饭})+AND={咖啡}× NOT={退款}|OR={午餐}",
            false,
        );

        assert!(!compiled.is_empty);
        assert_eq!(
            compiled.or_blocks,
            vec![vec!["早餐"], vec!["早饭"], vec!["午餐"]]
        );
        assert_eq!(compiled.and_patterns, vec!["咖啡"]);
        assert_eq!(compiled.not_patterns, vec!["退款"]);
        let ast = compiled.expression_ast.expect("compiled expression ast");
        assert_eq!(ast.kind, "any");
        assert_eq!(ast.children.len(), 2);
    }

    #[test]
    fn preserves_escaped_expression_delimiters() {
        let compiled =
            compile_rule_expression(r"OR={商户A\,咖啡\+拿铁\/燕麦\×热\{杯\}\|杯}", false);

        assert_eq!(
            compiled.or_blocks,
            vec![vec!["商户a,咖啡+拿铁/燕麦×热{杯}|杯"]]
        );
    }

    #[test]
    fn regex_enabled_marks_clause_patterns_as_regex() {
        let compiled = compile_rule_expression("OR={^星巴克.*咖啡$}+NOT={退款}", true);

        assert_eq!(compiled.or_blocks, vec![vec!["regex:^星巴克.*咖啡$"]]);
        assert_eq!(compiled.not_patterns, vec!["regex:退款"]);
    }

    #[test]
    fn invalid_nested_expression_compiles_empty() {
        let compiled = compile_rule_expression("(OR={早餐}/AND={咖啡}", false);

        assert!(compiled.is_empty);
        assert!(compiled.expression_ast.is_none());
    }

    #[test]
    fn dangling_expression_connectors_compile_empty() {
        for expr in [
            "OR={早餐}+",
            "OR={早餐}|",
            "OR={早餐}/",
            "OR={早餐}×",
            "OR={早餐} NOT",
            "+OR={早餐}",
            "×OR={早餐}",
            "NOT OR={早餐}",
            "OR={早餐}++AND={咖啡}",
        ] {
            let compiled = compile_rule_expression(expr, false);

            assert!(compiled.is_empty, "{expr}");
            assert!(compiled.expression_ast.is_none(), "{expr}");
        }
    }

    #[test]
    fn matches_legacy_and_expression_rules() {
        assert!(match_rule_expression(
            "滴滴 打车",
            "OR:滴滴|快的&AND:打车",
            false
        ));
        assert!(!match_rule_expression(
            "滴滴 打车 退款",
            "OR:滴滴|快的&AND:打车&NOT:退款",
            false
        ));
        assert!(match_rule_expression(
            "星巴克燕麦拿铁",
            "OR={星巴克}+AND={拿铁}",
            false
        ));
        assert!(!match_rule_expression(
            "星巴克燕麦拿铁 退款",
            "OR={星巴克}+AND={拿铁}×OR={退款}",
            false
        ));
    }

    #[test]
    fn matches_regex_rules_case_insensitively() {
        assert!(match_rule_expression(
            "STARBUCKS latte",
            "REGEX={star.*latte}",
            false
        ));
        assert!(!match_rule_expression(
            "refund starbucks latte",
            "REGEX={star.*latte}+NOT={refund}",
            false
        ));
    }
}
