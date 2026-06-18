// 中文导读：核心业务合同层，负责账户识别规则的表达式匹配和匹配解释。
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
pub fn normalize_account_role_scope(value: Option<&str>) -> Result<String, String> {
    normalize_scope(
        value,
        ACCOUNT_ROLE_ANY,
        ACCOUNT_ROLE_SCOPES,
        "account_role_scope",
    )
}

#[tracing::instrument(level = "debug", skip_all)]
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
fn match_account_rule_expression(text: &str, expr: &str, regex_enabled: bool) -> bool {
    let compiled = compile_rule_expression(expr, regex_enabled);
    let field = AccountRulePreparedField::new(String::new(), text.to_string());
    match_compiled_account_rule_expression_prepared(&field, &compiled, regex_enabled)
}

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
fn match_account_rule_expression_node(text: &str, node: &RuleExpressionNodeDto) -> bool {
    let field = AccountRulePreparedField::new(String::new(), text.to_string());
    match_account_rule_expression_node_prepared(&field, node)
}

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
fn account_rule_cross_field_match(
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
fn account_rule_plain_pattern_matches(text: &str, pattern: &str) -> bool {
    let field = AccountRulePreparedField::new(String::new(), text.to_string());
    let pattern = normalize_account_rule_match_piece(pattern);
    account_rule_plain_pattern_matches_prepared(&field, &pattern)
}

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
    fn contextual_prepared_field_values(&self) -> Vec<AccountRulePreparedField> {
        self.contextual_field_values()
            .into_iter()
            .map(|(field, value)| AccountRulePreparedField::new(field, value))
            .collect()
    }

    fn contextual_field_values(&self) -> Vec<(String, String)> {
        let fields = FIELD_SCOPES
            .iter()
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        self.field_values(&fields)
    }

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::category_rules::RuleExpressionNodeDto;

    use super::{
        account_rule_cross_field_match, account_rule_plain_pattern_matches,
        compile_account_rule_candidates, match_account_rule_expression,
        match_account_rule_expression_node, match_account_rules, match_compiled_account_rules,
        normalize_account_role_scope, normalize_account_rule_field_scope,
        normalize_transaction_type_scope, AccountRuleCandidate, AccountRuleMatchContext,
        ACCOUNT_ROLE_SOURCE, FIELD_COUNTERPARTY, FIELD_DESCRIPTION, FIELD_EXPENSE_COUNTERPARTY,
        FIELD_EXPENSE_DESCRIPTION, FIELD_EXPENSE_PAYMENT_METHOD, FIELD_INCOME_COUNTERPARTY,
        FIELD_INCOME_DESCRIPTION, FIELD_INCOME_PAYMENT_METHOD, FIELD_INVESTMENT_COUNTERPARTY,
        FIELD_INVESTMENT_DESCRIPTION, FIELD_PARSER, FIELD_PAYMENT_METHOD, TRANSACTION_SCOPE_ALL,
        TRANSACTION_SCOPE_EXPENSE, TRANSACTION_SCOPE_TRANSFER,
    };

    #[test]
    fn account_rule_scopes_normalize_and_reject_unknown_values() {
        assert_eq!(
            normalize_account_role_scope(Some("payment-method-source")).unwrap(),
            "payment_method_source"
        );
        assert_eq!(
            normalize_transaction_type_scope(Some("Transfer")).unwrap(),
            TRANSACTION_SCOPE_TRANSFER
        );
        assert_eq!(
            normalize_transaction_type_scope(Some("any")).unwrap(),
            TRANSACTION_SCOPE_ALL
        );
        assert_eq!(normalize_account_role_scope(Some(" ")).unwrap(), "any");
        assert!(normalize_account_role_scope(Some("wallet")).is_err());
        assert!(normalize_transaction_type_scope(Some("refund")).is_err());
        assert_eq!(
            normalize_account_rule_field_scope(None).unwrap(),
            vec![
                FIELD_COUNTERPARTY.to_string(),
                FIELD_PAYMENT_METHOD.to_string(),
                FIELD_DESCRIPTION.to_string()
            ]
        );
        assert_eq!(
            normalize_account_rule_field_scope(Some(&json!(null))).unwrap(),
            vec![
                FIELD_COUNTERPARTY.to_string(),
                FIELD_PAYMENT_METHOD.to_string(),
                FIELD_DESCRIPTION.to_string()
            ]
        );
        assert_eq!(
            normalize_account_rule_field_scope(Some(&json!([
                "counterparty",
                "counterparty",
                "parser"
            ])))
            .unwrap(),
            vec![FIELD_COUNTERPARTY.to_string(), FIELD_PARSER.to_string()]
        );
        assert!(normalize_account_rule_field_scope(Some(&json!(["counterparty", "bad"]))).is_err());
        assert!(normalize_account_rule_field_scope(Some(&json!(true))).is_err());
        assert!(normalize_account_rule_field_scope(Some(&json!([]))).is_err());
        assert!(normalize_account_rule_field_scope(Some(&json!(" "))).is_err());
        assert!(
            normalize_account_rule_field_scope(Some(&json!([1, true, null, {"x": 1}]))).is_err()
        );
    }

    #[test]
    fn account_rule_matching_uses_priority_and_requested_context() {
        let context = AccountRuleMatchContext {
            parser_id: "wechat_pay".to_string(),
            counterparty: "招商银行".to_string(),
            payment_method: "工资卡".to_string(),
            description: "午餐 支付".to_string(),
            ..AccountRuleMatchContext::default()
        };
        let rules = vec![
            AccountRuleCandidate {
                rule_id: 2,
                account_id: 20,
                rule_expression: "OR={招商银行}".to_string(),
                regex_enabled: false,
                enabled: true,
                priority: 1,
            },
            AccountRuleCandidate {
                rule_id: 1,
                account_id: 10,
                rule_expression: "OR={工资卡}".to_string(),
                regex_enabled: false,
                enabled: true,
                priority: 5,
            },
        ];
        let matched = match_account_rules(
            &rules,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_EXPENSE,
        )
        .expect("source account match");
        assert_eq!(matched.account_id, 20);
        assert_eq!(matched.matched_fields, vec![FIELD_COUNTERPARTY]);
        assert_eq!(matched.account_role_scope, ACCOUNT_ROLE_SOURCE);
        assert_eq!(matched.transaction_type_scope, TRANSACTION_SCOPE_EXPENSE);

        let transfer_matched = match_account_rules(
            &rules,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_TRANSFER,
        )
        .expect("requested transaction scope is reflected");
        assert_eq!(transfer_matched.account_id, 20);
        assert_eq!(
            transfer_matched.transaction_type_scope,
            TRANSACTION_SCOPE_TRANSFER
        );
    }

    #[test]
    fn account_rule_matching_reports_combined_field_fallback() {
        let context = AccountRuleMatchContext {
            counterparty: "支付宝".to_string(),
            description: "余额宝 转入".to_string(),
            ..AccountRuleMatchContext::default()
        };
        let rules = vec![AccountRuleCandidate {
            rule_id: 7,
            account_id: 70,
            rule_expression: "AND={支付宝,余额宝}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 1,
        }];
        let matched = match_account_rules(
            &rules,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_EXPENSE,
        )
        .expect("combined match");
        assert_eq!(matched.account_id, 70);
        assert!(matched.fallback_used);
        assert!(matched.matched_fields.is_empty());
    }

    #[test]
    fn compiled_account_rules_preserve_priority_and_direct_match_semantics() {
        let context = AccountRuleMatchContext {
            counterparty: "支付宝".to_string(),
            payment_method: "招商工资卡".to_string(),
            description: "余额宝 转入".to_string(),
            ..AccountRuleMatchContext::default()
        };
        let rules = vec![
            AccountRuleCandidate {
                rule_id: 2,
                account_id: 20,
                rule_expression: "AND={支付宝,余额宝}".to_string(),
                regex_enabled: false,
                enabled: true,
                priority: 5,
            },
            AccountRuleCandidate {
                rule_id: 1,
                account_id: 10,
                rule_expression: "OR={招商工资卡}".to_string(),
                regex_enabled: false,
                enabled: true,
                priority: 1,
            },
        ];

        let compiled = compile_account_rule_candidates(&rules);

        assert_eq!(compiled[0].rule.rule_id, 1);
        let matched = match_compiled_account_rules(
            &compiled,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_EXPENSE,
        )
        .expect("compiled account rule match");
        assert_eq!(matched.account_id, 10);
        assert_eq!(matched.matched_fields, vec![FIELD_PAYMENT_METHOD]);
    }

    #[test]
    fn account_rule_matching_does_not_match_embedded_pos_channel_text() {
        let context = AccountRuleMatchContext {
            payment_method: "本行POS".to_string(),
            description: "消费".to_string(),
            ..AccountRuleMatchContext::default()
        };
        let rules = vec![AccountRuleCandidate {
            rule_id: 11,
            account_id: 110,
            rule_expression: "OR={本行}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 1,
        }];

        let matched = match_account_rules(
            &rules,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_EXPENSE,
        );
        assert!(matched.is_none());
    }

    #[test]
    fn account_rule_expression_matching_covers_plain_regex_and_not_edges() {
        assert!(match_account_rule_expression(
            "招商银行",
            "OR={招商.*}",
            true
        ));
        assert!(!match_account_rule_expression("", "OR={招商银行}", false));
        assert!(match_account_rule_expression(
            "支付宝 余额宝",
            "AND={支付宝,余额宝}",
            false,
        ));
        assert!(!match_account_rule_expression(
            "支付宝 退款",
            "AND={支付宝}+NOT={退款}",
            false,
        ));
        assert!(!match_account_rule_expression(
            "支付宝",
            "AND={支付宝,余额宝}",
            false,
        ));
        assert!(match_account_rule_expression(
            "招商银行",
            "OR={招商银行,工资卡}",
            false,
        ));
    }

    #[test]
    fn account_rule_expression_node_covers_ast_branches() {
        let merchant_clause = RuleExpressionNodeDto {
            kind: "clause".to_string(),
            operator: "OR".to_string(),
            patterns: vec!["招商银行".to_string()],
            children: Vec::new(),
        };
        let card_clause = RuleExpressionNodeDto {
            kind: "clause".to_string(),
            operator: "AND".to_string(),
            patterns: vec!["工资卡".to_string()],
            children: Vec::new(),
        };
        let not_clause = RuleExpressionNodeDto {
            kind: "clause".to_string(),
            operator: "NOT".to_string(),
            patterns: vec!["退款".to_string()],
            children: Vec::new(),
        };
        let all_node = RuleExpressionNodeDto {
            kind: "all".to_string(),
            operator: String::new(),
            patterns: Vec::new(),
            children: vec![merchant_clause.clone(), card_clause.clone()],
        };
        let any_node = RuleExpressionNodeDto {
            kind: "any".to_string(),
            operator: String::new(),
            patterns: Vec::new(),
            children: vec![merchant_clause.clone(), card_clause.clone()],
        };
        let not_node = RuleExpressionNodeDto {
            kind: "not".to_string(),
            operator: String::new(),
            patterns: Vec::new(),
            children: vec![merchant_clause.clone()],
        };

        assert!(match_account_rule_expression_node(
            "招商银行 工资卡",
            &all_node
        ));
        assert!(match_account_rule_expression_node("招商银行", &any_node));
        assert!(match_account_rule_expression_node("支付宝", &not_node));
        assert!(match_account_rule_expression_node("支付宝", &not_clause));
        assert!(!match_account_rule_expression_node(
            "招商银行",
            &RuleExpressionNodeDto {
                kind: "unknown".to_string(),
                operator: String::new(),
                patterns: Vec::new(),
                children: Vec::new(),
            },
        ));
        assert!(!match_account_rule_expression_node(
            "招商银行",
            &RuleExpressionNodeDto {
                kind: "all".to_string(),
                operator: String::new(),
                patterns: Vec::new(),
                children: Vec::new(),
            },
        ));
    }

    #[test]
    fn account_rule_cross_field_match_rejects_empty_or_ambiguous_rules() {
        let fields = vec![
            (FIELD_COUNTERPARTY.to_string(), "支付宝".to_string()),
            (FIELD_DESCRIPTION.to_string(), "余额宝".to_string()),
        ];

        assert!(account_rule_cross_field_match(
            &fields,
            "AND={支付宝,余额宝}",
            false,
        ));
        assert!(!account_rule_cross_field_match(
            &fields,
            "AND={支付宝}+NOT={余额宝}",
            false,
        ));
        assert!(!account_rule_cross_field_match(
            &fields,
            "OR={支付宝}",
            false
        ));
        assert!(!account_rule_cross_field_match(
            &[(FIELD_COUNTERPARTY.to_string(), String::new())],
            "AND={支付宝}",
            false,
        ));
        assert!(account_rule_cross_field_match(
            &fields,
            "AND={支付宝,余额宝}",
            true,
        ));
        assert!(!account_rule_plain_pattern_matches("支付宝", ""));
    }

    #[test]
    fn account_rule_matching_uses_context_derived_fields() {
        let context = AccountRuleMatchContext {
            payment_method: "默认付款账户".to_string(),
            expense_counterparty: "支出方".to_string(),
            expense_payment_method: "支出卡".to_string(),
            expense_description: "支出备注".to_string(),
            income_counterparty: "收入方".to_string(),
            income_payment_method: "收入卡".to_string(),
            income_description: "收入备注".to_string(),
            investment_counterparty: "券商账户".to_string(),
            investment_description: "投资备注".to_string(),
            ..AccountRuleMatchContext::default()
        };

        let default_scope_rule = AccountRuleCandidate {
            rule_id: 8,
            account_id: 80,
            rule_expression: "OR={默认付款账户}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 1,
        };
        let matched = match_account_rules(
            &[default_scope_rule],
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_EXPENSE,
        )
        .expect("default scope");
        assert_eq!(matched.matched_fields, vec![FIELD_PAYMENT_METHOD]);

        for (index, (field, term)) in [
            (FIELD_EXPENSE_COUNTERPARTY, "支出方"),
            (FIELD_EXPENSE_PAYMENT_METHOD, "支出卡"),
            (FIELD_EXPENSE_DESCRIPTION, "支出备注"),
            (FIELD_INCOME_COUNTERPARTY, "收入方"),
            (FIELD_INCOME_PAYMENT_METHOD, "收入卡"),
            (FIELD_INCOME_DESCRIPTION, "收入备注"),
            (FIELD_INVESTMENT_COUNTERPARTY, "券商账户"),
            (FIELD_INVESTMENT_DESCRIPTION, "投资备注"),
        ]
        .into_iter()
        .enumerate()
        {
            let rule = AccountRuleCandidate {
                rule_id: 100 + index as i64,
                account_id: 900 + index as i64,
                rule_expression: format!("OR={{{term}}}"),
                regex_enabled: false,
                enabled: true,
                priority: 1,
            };
            let matched = match_account_rules(
                &[rule],
                &context,
                ACCOUNT_ROLE_SOURCE,
                TRANSACTION_SCOPE_TRANSFER,
            )
            .expect("hidden field match");
            assert_eq!(matched.matched_fields, vec![field.to_string()]);
        }
    }
}
