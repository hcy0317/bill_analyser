use bill_analyser_core::{
    account_rules::{
        match_account_rules, AccountRuleCandidate, AccountRuleMatchContext,
        ACCOUNT_ROLE_INVESTMENT, ACCOUNT_ROLE_SOURCE, FIELD_DESCRIPTION, TRANSACTION_SCOPE_EXPENSE,
        TRANSACTION_SCOPE_INVESTMENT,
    },
    category_rules::{compile_rule_expression, escape_rule_expression_term, match_rule_expression},
};

#[test]
fn category_rule_expression_contract_locks_nested_regex_not_and_escape_semantics() {
    let expression = "(OR={星巴克,瑞幸}+NOT={退款})/REGEX={^coffee\\s+shop$}";

    assert!(match_rule_expression("瑞幸 活动", expression, false));
    assert!(!match_rule_expression("瑞幸 退款", expression, false));
    assert!(match_rule_expression("coffee shop", expression, false));
    assert!(!match_rule_expression("coffee refund", expression, false));

    let escaped = escape_rule_expression_term("早餐,咖啡");
    assert_eq!(escaped, "早餐\\,咖啡");
    assert!(match_rule_expression(
        "本地早餐,咖啡门店",
        &format!("OR={{{escaped}}}"),
        false,
    ));
    assert!(!match_rule_expression(
        "本地早餐 咖啡门店",
        &format!("OR={{{escaped}}}"),
        false,
    ));

    assert!(compile_rule_expression("", false).is_empty);
    assert!(!match_rule_expression("星巴克", "OR={星巴克", false));
}

#[test]
fn account_rule_matching_contract_locks_context_fields_priority_and_token_boundaries() {
    let context = AccountRuleMatchContext {
        parser_id: "broker_parser".to_string(),
        parser_label: "券商账单".to_string(),
        parser_tags: vec!["investment".to_string(), "portfolio".to_string()],
        counterparty: "支付宝".to_string(),
        payment_method: "招商工资卡".to_string(),
        description: "余额宝 转入".to_string(),
        investment_counterparty: "银河证券".to_string(),
        investment_description: "基金申购".to_string(),
        ..AccountRuleMatchContext::default()
    };

    let rules = vec![
        AccountRuleCandidate {
            rule_id: 1,
            account_id: 10,
            rule_expression: "OR={招商}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 1,
        },
        AccountRuleCandidate {
            rule_id: 2,
            account_id: 20,
            rule_expression: "OR={银河证券}".to_string(),
            regex_enabled: false,
            enabled: false,
            priority: 2,
        },
        AccountRuleCandidate {
            rule_id: 3,
            account_id: 30,
            rule_expression: "OR={银河证券}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 3,
        },
        AccountRuleCandidate {
            rule_id: 4,
            account_id: 40,
            rule_expression: "AND={支付宝,余额宝}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 4,
        },
    ];

    let matched = match_account_rules(
        &rules,
        &context,
        ACCOUNT_ROLE_INVESTMENT,
        TRANSACTION_SCOPE_INVESTMENT,
    )
    .expect("investment context should match enabled rule");

    assert_eq!(matched.account_id, 30);
    assert_eq!(matched.rule_id, 3);
    assert_eq!(matched.matched_fields, vec!["investment_counterparty"]);
    assert!(!matched.fallback_used);
    assert_eq!(matched.account_role_scope, ACCOUNT_ROLE_INVESTMENT);
    assert_eq!(matched.transaction_type_scope, TRANSACTION_SCOPE_INVESTMENT);

    let fallback = match_account_rules(
        &[AccountRuleCandidate {
            rule_id: 5,
            account_id: 50,
            rule_expression: "AND={支付宝,余额宝}".to_string(),
            regex_enabled: false,
            enabled: true,
            priority: 1,
        }],
        &context,
        ACCOUNT_ROLE_SOURCE,
        TRANSACTION_SCOPE_EXPENSE,
    )
    .expect("cross-field account rule should match combined context");

    assert_eq!(fallback.account_id, 50);
    assert!(fallback.fallback_used);
    assert!(fallback.matched_fields.is_empty());
    assert_eq!(fallback.account_role_scope, ACCOUNT_ROLE_SOURCE);
    assert_eq!(fallback.transaction_type_scope, TRANSACTION_SCOPE_EXPENSE);

    let direct_description = match_account_rules(
        &[AccountRuleCandidate {
            rule_id: 6,
            account_id: 60,
            rule_expression: "OR={^余额宝\\s+转入$}".to_string(),
            regex_enabled: true,
            enabled: true,
            priority: 1,
        }],
        &context,
        ACCOUNT_ROLE_SOURCE,
        TRANSACTION_SCOPE_EXPENSE,
    )
    .expect("regex-enabled account rule should match description with regex");

    assert_eq!(direct_description.matched_fields, vec![FIELD_DESCRIPTION]);
}
