// 中文导读：账户识别规则匹配的模块内合同测试。
// 维护重点：这些测试锁定拆分前语义，后续字段准备或匹配函数移动必须保持断言不变。

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
    assert!(normalize_account_rule_field_scope(Some(&json!([1, true, null, {"x": 1}]))).is_err());
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
