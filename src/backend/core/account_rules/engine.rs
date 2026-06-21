// 中文导读：账户识别规则匹配引擎，负责规则候选编译、scope 归一和上下文字段匹配。
// 维护重点：本模块只处理与请求无关的规则语义，仓储和 HTTP 层不得重复实现匹配规则。
// 不变式：账户规则复用分类规则表达式语法；导入账户识别由稳定后的预览上下文决定角色、类型和字段包。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::category_rules::{
    compile_rule_expression, match_compiled_rule, CompiledRuleDto, RuleExpressionNodeDto,
};

pub const ACCOUNT_ROLE_ANY: &str = "any";
pub const ACCOUNT_ROLE_SOURCE: &str = "source";
pub const ACCOUNT_ROLE_DESTINATION: &str = "destination";
pub const ACCOUNT_ROLE_INVESTMENT: &str = "investment";
pub const ACCOUNT_ROLE_PAYMENT_METHOD_SOURCE: &str = "payment_method_source";

pub const TRANSACTION_SCOPE_ALL: &str = "all";
pub const TRANSACTION_SCOPE_INCOME: &str = "income";
pub const TRANSACTION_SCOPE_EXPENSE: &str = "expense";
pub const TRANSACTION_SCOPE_TRANSFER: &str = "transfer";
pub const TRANSACTION_SCOPE_INVESTMENT: &str = "investment";

pub const FIELD_PARSER: &str = "parser";
pub const FIELD_COUNTERPARTY: &str = "counterparty";
pub const FIELD_PAYMENT_METHOD: &str = "payment_method";
pub const FIELD_DESCRIPTION: &str = "description";
pub const FIELD_EXPENSE_COUNTERPARTY: &str = "expense_counterparty";
pub const FIELD_EXPENSE_PAYMENT_METHOD: &str = "expense_payment_method";
pub const FIELD_EXPENSE_DESCRIPTION: &str = "expense_description";
pub const FIELD_INCOME_COUNTERPARTY: &str = "income_counterparty";
pub const FIELD_INCOME_PAYMENT_METHOD: &str = "income_payment_method";
pub const FIELD_INCOME_DESCRIPTION: &str = "income_description";
pub const FIELD_INVESTMENT_COUNTERPARTY: &str = "investment_counterparty";
pub const FIELD_INVESTMENT_DESCRIPTION: &str = "investment_description";

pub const DEFAULT_FIELD_SCOPES: &[&str] =
    &[FIELD_COUNTERPARTY, FIELD_PAYMENT_METHOD, FIELD_DESCRIPTION];

const ACCOUNT_ROLE_SCOPES: &[&str] = &[
    ACCOUNT_ROLE_ANY,
    ACCOUNT_ROLE_SOURCE,
    ACCOUNT_ROLE_DESTINATION,
    ACCOUNT_ROLE_INVESTMENT,
    ACCOUNT_ROLE_PAYMENT_METHOD_SOURCE,
];
const TRANSACTION_TYPE_SCOPES: &[&str] = &[
    TRANSACTION_SCOPE_ALL,
    TRANSACTION_SCOPE_INCOME,
    TRANSACTION_SCOPE_EXPENSE,
    TRANSACTION_SCOPE_TRANSFER,
    TRANSACTION_SCOPE_INVESTMENT,
];
const FIELD_SCOPES: &[&str] = &[
    FIELD_PARSER,
    FIELD_COUNTERPARTY,
    FIELD_PAYMENT_METHOD,
    FIELD_DESCRIPTION,
    FIELD_EXPENSE_COUNTERPARTY,
    FIELD_EXPENSE_PAYMENT_METHOD,
    FIELD_EXPENSE_DESCRIPTION,
    FIELD_INCOME_COUNTERPARTY,
    FIELD_INCOME_PAYMENT_METHOD,
    FIELD_INCOME_DESCRIPTION,
    FIELD_INVESTMENT_COUNTERPARTY,
    FIELD_INVESTMENT_DESCRIPTION,
];

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountRuleMatchContext {
    pub parser_id: String,
    pub parser_label: String,
    pub parser_tags: Vec<String>,
    pub counterparty: String,
    pub payment_method: String,
    pub description: String,
    pub expense_counterparty: String,
    pub expense_payment_method: String,
    pub expense_description: String,
    pub income_counterparty: String,
    pub income_payment_method: String,
    pub income_description: String,
    pub investment_counterparty: String,
    pub investment_description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRuleCandidate {
    pub rule_id: i64,
    pub account_id: i64,
    pub rule_expression: String,
    pub regex_enabled: bool,
    pub enabled: bool,
    pub priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAccountRuleCandidate {
    pub rule: AccountRuleCandidate,
    compiled_expression: CompiledRuleDto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountRulePreparedField {
    field: String,
    value: String,
    normalized_value: String,
    tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRuleMatch {
    pub account_id: i64,
    pub rule_id: i64,
    pub matched_fields: Vec<String>,
    pub priority: i64,
    pub fallback_used: bool,
    pub account_role_scope: String,
    pub transaction_type_scope: String,
}

#[tracing::instrument(level = "debug", skip_all)]
/// 编译账户规则候选并按优先级排序，避免导入链路每条账单重复解析表达式。
pub fn compile_account_rule_candidates(
    rules: &[AccountRuleCandidate],
) -> Vec<CompiledAccountRuleCandidate> {
    let mut compiled = rules
        .iter()
        .cloned()
        .map(|rule| {
            let compiled_expression =
                compile_rule_expression(&rule.rule_expression, rule.regex_enabled);
            CompiledAccountRuleCandidate {
                rule,
                compiled_expression,
            }
        })
        .collect::<Vec<_>>();
    compiled.sort_by_key(|candidate| (candidate.rule.priority, candidate.rule.rule_id));
    compiled
}

#[tracing::instrument(level = "debug", skip_all)]
/// 归一账户角色 scope，兼容历史 payload 中的大小写和连字符写法。
pub fn normalize_account_role_scope(value: Option<&str>) -> Result<String, String> {
    normalize_scope(
        value,
        ACCOUNT_ROLE_ANY,
        ACCOUNT_ROLE_SCOPES,
        "account_role_scope",
    )
}

#[tracing::instrument(level = "debug", skip_all)]
/// 归一交易类型 scope，并把旧版 `any` 映射到当前的 `all` 合同。
pub fn normalize_transaction_type_scope(value: Option<&str>) -> Result<String, String> {
    let normalized = value
        .map(|value| value.trim().to_ascii_lowercase().replace('-', "_"))
        .filter(|value| value == "any")
        .map(|_| TRANSACTION_SCOPE_ALL.to_string());
    if let Some(value) = normalized {
        return Ok(value);
    }
    normalize_scope(
        value,
        TRANSACTION_SCOPE_ALL,
        TRANSACTION_TYPE_SCOPES,
        "transaction_type_scope",
    )
}

#[tracing::instrument(level = "debug", skip_all)]
/// 解析账户规则字段 scope，支持数组、逗号字符串和默认三字段集合。
pub fn normalize_account_rule_field_scope(value: Option<&Value>) -> Result<Vec<String>, String> {
    let raw_values = match value {
        None | Some(Value::Null) => DEFAULT_FIELD_SCOPES
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        Some(Value::Array(items)) => items.iter().map(json_value_text).collect(),
        Some(Value::String(text)) => parse_field_scope_text(text),
        Some(Value::Bool(_)) | Some(Value::Number(_)) | Some(Value::Object(_)) => {
            return Err("field_scope must be a list or comma-separated string".to_string())
        }
    };

    let mut scopes = Vec::new();
    for raw in raw_values {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if !FIELD_SCOPES.contains(&normalized.as_str()) {
            return Err(format!(
                "unsupported account rule field scope: {normalized}"
            ));
        }
        if !scopes.contains(&normalized) {
            scopes.push(normalized);
        }
    }
    if scopes.is_empty() {
        return Err("field_scope must contain at least one supported field".to_string());
    }
    Ok(scopes)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 对未预编译的账户规则执行一次匹配，适用于测试端点和低频调用。
pub fn match_account_rules(
    rules: &[AccountRuleCandidate],
    context: &AccountRuleMatchContext,
    requested_role_scope: &str,
    transaction_type: &str,
) -> Option<AccountRuleMatch> {
    let compiled = compile_account_rule_candidates(rules);
    match_compiled_account_rules(&compiled, context, requested_role_scope, transaction_type)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 在预编译账户规则中按优先级寻找首个命中规则，并返回命中字段解释。
pub fn match_compiled_account_rules(
    rules: &[CompiledAccountRuleCandidate],
    context: &AccountRuleMatchContext,
    requested_role_scope: &str,
    transaction_type: &str,
) -> Option<AccountRuleMatch> {
    let requested_role_scope = normalize_account_role_scope(Some(requested_role_scope)).ok()?;
    let transaction_type = normalize_transaction_type_scope(Some(transaction_type)).ok()?;
    let field_values = context.contextual_prepared_field_values();

    for candidate in rules {
        let rule = &candidate.rule;
        if !rule.enabled {
            continue;
        }
        let matched_fields = field_values
            .iter()
            .filter(|field| {
                !field.normalized_value.is_empty()
                    && match_compiled_account_rule_expression_prepared(
                        field,
                        &candidate.compiled_expression,
                        rule.regex_enabled,
                    )
            })
            .map(|field| field.field.clone())
            .collect::<Vec<_>>();
        let fallback_used = matched_fields.is_empty()
            && account_rule_compiled_cross_field_match(
                &field_values,
                &candidate.compiled_expression,
                rule.regex_enabled,
            );
        if matched_fields.is_empty() && !fallback_used {
            continue;
        }
        return Some(AccountRuleMatch {
            account_id: rule.account_id,
            rule_id: rule.rule_id,
            fallback_used,
            matched_fields,
            priority: rule.priority,
            account_role_scope: requested_role_scope.clone(),
            transaction_type_scope: transaction_type.clone(),
        });
    }
    None
}

#[cfg(test)]
/// 测试入口：用未编译表达式验证账户规则单字段匹配合同。
pub(super) fn match_account_rule_expression(text: &str, expr: &str, regex_enabled: bool) -> bool {
    let compiled = compile_rule_expression(expr, regex_enabled);
    let field = AccountRulePreparedField::new(String::new(), text.to_string());
    match_compiled_account_rule_expression_prepared(&field, &compiled, regex_enabled)
}

/// 在单个预处理字段上匹配已编译表达式，正则和普通表达式走不同合同。
fn match_compiled_account_rule_expression_prepared(
    field: &AccountRulePreparedField,
    compiled: &CompiledRuleDto,
    regex_enabled: bool,
) -> bool {
    if regex_enabled {
        return match_compiled_rule(&field.value, compiled);
    }
    if compiled.is_empty || field.normalized_value.is_empty() {
        return false;
    }
    compiled
        .expression_ast
        .as_ref()
        .is_some_and(|expression_ast| {
            match_account_rule_expression_node_prepared(field, expression_ast)
        })
}

#[cfg(test)]
/// 测试入口：直接验证账户规则 AST 节点在单字段上的匹配语义。
pub(super) fn match_account_rule_expression_node(text: &str, node: &RuleExpressionNodeDto) -> bool {
    let field = AccountRulePreparedField::new(String::new(), text.to_string());
    match_account_rule_expression_node_prepared(&field, node)
}

/// 在账户字段的预处理文本上执行分类规则 AST，保持非正则账户规则的 token 边界语义。
fn match_account_rule_expression_node_prepared(
    field: &AccountRulePreparedField,
    node: &RuleExpressionNodeDto,
) -> bool {
    match node.kind.as_str() {
        "all" => {
            !node.children.is_empty()
                && node
                    .children
                    .iter()
                    .all(|child| match_account_rule_expression_node_prepared(field, child))
        }
        "any" => node
            .children
            .iter()
            .any(|child| match_account_rule_expression_node_prepared(field, child)),
        "not" => {
            node.children.len() == 1
                && !match_account_rule_expression_node_prepared(field, &node.children[0])
        }
        "clause" if node.operator == "OR" => node
            .patterns
            .iter()
            .any(|pattern| account_rule_plain_pattern_matches_prepared(field, pattern)),
        "clause" if node.operator == "AND" => {
            !node.patterns.is_empty()
                && node
                    .patterns
                    .iter()
                    .all(|pattern| account_rule_plain_pattern_matches_prepared(field, pattern))
        }
        "clause" if node.operator == "NOT" => !node
            .patterns
            .iter()
            .any(|pattern| account_rule_plain_pattern_matches_prepared(field, pattern)),
        _ => false,
    }
}

#[cfg(test)]
/// 测试入口：验证账户规则跨字段 fallback 是否保持 AND-only 合同。
pub(super) fn account_rule_cross_field_match(
    field_values: &[(String, String)],
    expr: &str,
    regex_enabled: bool,
) -> bool {
    let compiled = compile_rule_expression(expr, regex_enabled);
    let prepared = field_values
        .iter()
        .map(|(field, value)| AccountRulePreparedField::new(field.clone(), value.clone()))
        .collect::<Vec<_>>();
    account_rule_compiled_cross_field_match(&prepared, &compiled, regex_enabled)
}

/// 在单字段都未命中时执行跨字段 AND fallback，避免把 OR 表达式错误扩展到多字段拼接。
fn account_rule_compiled_cross_field_match(
    field_values: &[AccountRulePreparedField],
    compiled: &CompiledRuleDto,
    regex_enabled: bool,
) -> bool {
    let values = field_values
        .iter()
        .map(|field| field.value.as_str())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return false;
    }
    let combined_text = values.join(" ");
    if regex_enabled {
        return match_compiled_rule(&combined_text, compiled);
    }
    if compiled.is_empty || compiled.and_patterns.is_empty() || !compiled.or_blocks.is_empty() {
        return false;
    }
    if compiled.not_patterns.iter().any(|pattern| {
        field_values
            .iter()
            .any(|field| account_rule_plain_pattern_matches_prepared(field, pattern))
    }) {
        return false;
    }
    compiled.and_patterns.iter().all(|pattern| {
        field_values
            .iter()
            .any(|field| account_rule_plain_pattern_matches_prepared(field, pattern))
    })
}

#[cfg(test)]
/// 测试入口：验证普通 pattern 的完整值和 token 边界匹配。
pub(super) fn account_rule_plain_pattern_matches(text: &str, pattern: &str) -> bool {
    let field = AccountRulePreparedField::new(String::new(), text.to_string());
    let pattern = normalize_account_rule_match_piece(pattern);
    account_rule_plain_pattern_matches_prepared(&field, &pattern)
}

/// 对普通账户规则 pattern 执行完整值或分词匹配，防止 POS/渠道等短词命中嵌入文本。
fn account_rule_plain_pattern_matches_prepared(
    field: &AccountRulePreparedField,
    pattern: &str,
) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() || field.normalized_value.is_empty() {
        return false;
    }
    if field.normalized_value == pattern {
        return true;
    }
    field.tokens.iter().any(|token| token == pattern)
}

fn normalize_account_rule_match_piece(value: &str) -> String {
    value.trim().to_lowercase()
}

/// 将账户规则候选文本按中英文常见分隔符拆成可精确比较的 token。
fn account_rule_match_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    ',' | '，'
                        | ';'
                        | '；'
                        | ':'
                        | '：'
                        | '/'
                        | '\\'
                        | '|'
                        | '('
                        | ')'
                        | '（'
                        | '）'
                        | '['
                        | ']'
                        | '【'
                        | '】'
                        | '{'
                        | '}'
                        | '<'
                        | '>'
                        | '《'
                        | '》'
                        | '-'
                        | '_'
                        | '+'
                )
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

impl AccountRuleMatchContext {
    /// 构造所有可参与账户识别的上下文字段，并预先生成归一文本与 token。
    fn contextual_prepared_field_values(&self) -> Vec<AccountRulePreparedField> {
        self.contextual_field_values()
            .into_iter()
            .map(|(field, value)| AccountRulePreparedField::new(field, value))
            .collect()
    }

    /// 按稳定字段顺序展开账户识别上下文，保证匹配解释的字段顺序可预测。
    fn contextual_field_values(&self) -> Vec<(String, String)> {
        let fields = FIELD_SCOPES
            .iter()
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        self.field_values(&fields)
    }

    /// 将请求中的字段 scope 映射到具体文本，parser 字段会合并 id、label 和 tags。
    fn field_values(&self, fields: &[String]) -> Vec<(String, String)> {
        fields
            .iter()
            .map(|field| {
                let value = match field.as_str() {
                    FIELD_PARSER => {
                        let mut values = vec![self.parser_id.clone(), self.parser_label.clone()];
                        values.extend(self.parser_tags.clone());
                        values.join(" ")
                    }
                    FIELD_COUNTERPARTY => self.counterparty.clone(),
                    FIELD_PAYMENT_METHOD => self.payment_method.clone(),
                    FIELD_DESCRIPTION => self.description.clone(),
                    FIELD_EXPENSE_COUNTERPARTY => self.expense_counterparty.clone(),
                    FIELD_EXPENSE_PAYMENT_METHOD => self.expense_payment_method.clone(),
                    FIELD_EXPENSE_DESCRIPTION => self.expense_description.clone(),
                    FIELD_INCOME_COUNTERPARTY => self.income_counterparty.clone(),
                    FIELD_INCOME_PAYMENT_METHOD => self.income_payment_method.clone(),
                    FIELD_INCOME_DESCRIPTION => self.income_description.clone(),
                    FIELD_INVESTMENT_COUNTERPARTY => self.investment_counterparty.clone(),
                    FIELD_INVESTMENT_DESCRIPTION => self.investment_description.clone(),
                    _ => String::new(),
                };
                (field.clone(), value)
            })
            .collect()
    }
}

impl AccountRulePreparedField {
    /// 保存账户规则匹配所需的原文、归一文本和 token，避免匹配阶段重复拆词。
    fn new(field: String, value: String) -> Self {
        let normalized_value = normalize_account_rule_match_piece(&value);
        let tokens = account_rule_match_tokens(&normalized_value);
        Self {
            field,
            value,
            normalized_value,
            tokens,
        }
    }
}

/// 归一单值 scope，并在遇到未知值时返回可直接暴露给 API 的错误文案。
fn normalize_scope(
    value: Option<&str>,
    default_value: &str,
    allowed: &[&str],
    field: &str,
) -> Result<String, String> {
    let normalized = value
        .unwrap_or(default_value)
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    if normalized.is_empty() {
        return Ok(default_value.to_string());
    }
    if allowed.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(format!("unsupported {field}: {normalized}"))
    }
}

/// 解析旧版字段 scope 字符串，兼容 JSON 数组字符串和多种中英文分隔符。
fn parse_field_scope_text(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('[') {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) {
            return items.iter().map(json_value_text).collect();
        }
    }
    text.split([',', ';', '|', '，', '；'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn json_value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
