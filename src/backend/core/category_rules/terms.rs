// 中文导读：分类规则表达式 term 转义、拆分和连接符识别工具。
// 维护重点：转义字符集合是表达式语法合同，改动必须同步前端表达式编辑器测试。

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

pub(super) fn split_rule_expression_terms(content: &str) -> Vec<String> {
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

pub(super) fn is_escapable_char(value: char) -> bool {
    matches!(
        value,
        '\\' | ',' | '+' | '{' | '}' | '|' | '(' | ')' | '/' | '×'
    )
}

pub(super) fn is_or_connector(value: char) -> bool {
    matches!(value, '|' | '/')
}

pub(super) fn is_factor_terminator(value: char) -> bool {
    matches!(value, '+' | '×' | '|' | '/' | ')')
}
