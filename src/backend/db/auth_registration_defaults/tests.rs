#[cfg(test)]
mod tests {
    use bill_analyser_core::category_rules::{compile_rule_expression, match_rule_expression};

    use super::{STANDARD_DAILY_V1_ACCOUNT_RULES, STANDARD_DAILY_V1_CATEGORY_RULES};

    #[test]
    fn standard_daily_v1_category_rule_expressions_compile_non_empty() {
        for rule in STANDARD_DAILY_V1_CATEGORY_RULES {
            let compiled = compile_rule_expression(rule.rule_expression, false);

            assert!(
                !compiled.is_empty,
                "{} should compile: {}",
                rule.name, rule.rule_expression
            );
        }
    }

    #[test]
    fn standard_daily_v1_account_rule_expressions_compile_non_empty() {
        for rule in STANDARD_DAILY_V1_ACCOUNT_RULES {
            let compiled = compile_rule_expression(rule.rule_expression, false);

            assert!(
                !compiled.is_empty,
                "{} should compile: {}",
                rule.name, rule.rule_expression
            );
        }
    }

    #[test]
    fn standard_daily_v1_representative_category_rules_match_expected_text() {
        let cases = [
            (
                "美团外卖订单",
                "default:standard_daily_v1:餐饮食品/外卖",
                true,
            ),
            (
                "饿了么午餐",
                "default:standard_daily_v1:餐饮食品/外卖",
                true,
            ),
            ("滴滴出行", "default:standard_daily_v1:交通通信/打车", true),
            ("高德打车", "default:standard_daily_v1:交通通信/打车", true),
            ("本月工资", "default:standard_daily_v1:职业收入/工资", true),
            ("薪资入账", "default:standard_daily_v1:职业收入/工资", true),
            (
                "信用卡还款",
                "default:standard_daily_v1:账户互转/信用卡还款",
                true,
            ),
            (
                "黄金礼品",
                "default:standard_daily_v1:投资理财/黄金贵金属",
                false,
            ),
            (
                "借款还款",
                "default:standard_daily_v1:账户互转/信用卡还款",
                false,
            ),
            ("退款到账", "default:standard_daily_v1:职业收入/工资", false),
        ];

        for (text, rule_name, expected) in cases {
            let rule = STANDARD_DAILY_V1_CATEGORY_RULES
                .iter()
                .find(|item| item.name == rule_name)
                .expect("rule exists");

            assert_eq!(
                match_rule_expression(text, rule.rule_expression, false),
                expected,
                "{text} against {}",
                rule.rule_expression
            );
        }
    }

    #[test]
    fn standard_daily_v1_representative_account_rules_match_expected_text() {
        let cases = [
            (
                "微信支付-餐饮",
                "default:standard_daily_v1:account:wechat",
                true,
            ),
            (
                "财付通付款",
                "default:standard_daily_v1:account:wechat",
                true,
            ),
            (
                "支付宝消费",
                "default:standard_daily_v1:account:alipay",
                true,
            ),
            (
                "余额宝转入",
                "default:standard_daily_v1:account:alipay",
                true,
            ),
            (
                "花呗分期",
                "default:standard_daily_v1:account:consumer-credit",
                true,
            ),
            (
                "信用卡消费",
                "default:standard_daily_v1:account:credit-card",
                true,
            ),
            (
                "银行卡快捷支付",
                "default:standard_daily_v1:account:bank-card",
                true,
            ),
            (
                "花呗快捷支付",
                "default:standard_daily_v1:account:bank-card",
                false,
            ),
        ];

        for (text, rule_name, expected) in cases {
            let rule = STANDARD_DAILY_V1_ACCOUNT_RULES
                .iter()
                .find(|item| item.name == rule_name)
                .expect("rule exists");

            assert_eq!(
                match_rule_expression(text, rule.rule_expression, false),
                expected,
                "{text} against {}",
                rule.rule_expression
            );
        }
    }
}
