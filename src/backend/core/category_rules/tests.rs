// 中文导读：分类规则表达式的模块内合同测试。
// 维护重点：这些测试锁定拆分前语义，后续 parser/matcher 移动必须保持断言不变。

use super::{
    compile_rule_expression, match_compiled_rule, match_compiled_rule_lowercase_text,
    match_rule_expression, CompiledRuleDto, RuleExpressionNodeDto,
};

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
    let compiled = compile_rule_expression(r"OR={商户A\,咖啡\+拿铁\/燕麦\×热\{杯\}\|杯}", false);

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
fn matches_current_expression_rules() {
    let compiled = compile_rule_expression("OR={星巴克}+AND={拿铁}", false);
    assert_eq!(
        match_compiled_rule("星巴克燕麦拿铁", &compiled),
        match_compiled_rule_lowercase_text("星巴克燕麦拿铁", &compiled)
    );
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

#[test]
fn legacy_compiled_rule_fallback_preserves_pre_ast_matching_contract() {
    let legacy = CompiledRuleDto {
        or_blocks: vec![
            vec!["星巴克".to_string(), "瑞幸".to_string()],
            vec!["拿铁".to_string()],
        ],
        not_patterns: vec!["退款".to_string()],
        and_patterns: Vec::new(),
        is_empty: false,
        expression_ast: None,
    };

    assert!(match_compiled_rule_lowercase_text(
        "星巴克燕麦拿铁",
        &legacy
    ));
    assert!(!match_compiled_rule_lowercase_text("星巴克燕麦", &legacy));
    assert!(!match_compiled_rule_lowercase_text(
        "星巴克燕麦拿铁退款",
        &legacy
    ));
    assert!(!match_compiled_rule_lowercase_text("", &legacy));

    let and_only = CompiledRuleDto {
        or_blocks: Vec::new(),
        not_patterns: Vec::new(),
        and_patterns: vec!["咖啡".to_string(), "早餐".to_string()],
        is_empty: false,
        expression_ast: None,
    };
    assert!(match_compiled_rule_lowercase_text("早餐咖啡", &and_only));
    assert!(!match_compiled_rule_lowercase_text("早餐", &and_only));
}

#[test]
fn matching_handles_plain_regex_literals_cache_hits_and_unknown_ast() {
    assert!(match_rule_expression(
        "STARBUCKS",
        "REGEX={starbucks}",
        false
    ));

    assert!(match_rule_expression("abc123", "REGEX={abc\\d+}", false));
    assert!(match_rule_expression("ABC456", "REGEX={abc\\d+}", false));

    let unknown_ast = CompiledRuleDto {
        or_blocks: Vec::new(),
        not_patterns: Vec::new(),
        and_patterns: Vec::new(),
        is_empty: false,
        expression_ast: Some(RuleExpressionNodeDto {
            kind: "unknown".to_string(),
            operator: String::new(),
            patterns: Vec::new(),
            children: Vec::new(),
        }),
    };

    assert!(!match_compiled_rule("任意文本", &unknown_ast));
}
