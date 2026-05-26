// 中文导读：导入 stage handler 内部账户匹配与 transfer 保护规则单测。
// 维护重点：测试 stage2 智能链路顺序与隐藏 source_chain 账户解析，不承载生产逻辑。
// 不变式：单测只验证 include 到 import_routes 模块后的私有 helper 合同。

#[cfg(test)]
mod stage_handler_transfer_account_tests {
    use super::*;

    fn transfer_accounts() -> Vec<ImportIntelligenceAccount> {
        vec![
            ImportIntelligenceAccount {
                id: 100,
                name: "工资卡".to_string(),
                aliases: vec!["工资卡".to_string(), "农业银行".to_string(), "abc".to_string()],
            },
            ImportIntelligenceAccount {
                id: 200,
                name: "零钱".to_string(),
                aliases: vec!["零钱".to_string(), "微信钱包".to_string(), "wallet".to_string()],
            },
        ]
    }

    #[test]
    fn parse_account_aliases_keeps_json_and_delimited_alias_contract() {
        assert_eq!(
            parse_account_aliases(Some(r#"[" 微信钱包 ", "", 42, true, "余额宝"]"#)),
            vec!["微信钱包", "42", "true", "余额宝"]
        );
        assert_eq!(
            parse_account_aliases(Some(" 微信钱包,农业银行； 余额宝|零钱通 ")),
            vec!["微信钱包", "农业银行", "余额宝", "零钱通"]
        );
        assert!(parse_account_aliases(Some("   ")).is_empty());
        assert!(parse_account_aliases(None).is_empty());
    }

    #[test]
    fn account_alias_match_uses_case_insensitive_overlap_tokens() {
        let account = ImportIntelligenceAccount {
            id: 10,
            name: "微信钱包".to_string(),
            aliases: vec![
                "WeChat Wallet".to_string(),
                "abc".to_string(),
                "零钱".to_string(),
            ],
        };

        assert!(account_matches_tokens(&account, &["wechat wallet".to_string()]));
        assert!(account_matches_tokens(
            &account,
            &["parser:abc-bank".to_string()]
        ));
        assert!(account_matches_tokens(&account, &["零钱".to_string()]));
        assert!(!account_matches_tokens(&account, &["支付宝".to_string()]));
    }

    #[test]
    fn account_rule_shadow_loader_reads_enabled_rules_without_cutting_over_alias_match() {
        let connection = Connection::open_in_memory().expect("open");
        connection
            .execute_batch(
                r#"
                CREATE TABLE account_rules (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    account_id INTEGER NOT NULL,
                    rule_expression TEXT NOT NULL,
                    regex_enabled INTEGER DEFAULT 0,
                    enabled INTEGER DEFAULT 1,
                    priority INTEGER DEFAULT 100,
                    account_role_scope TEXT DEFAULT 'any',
                    transaction_type_scope TEXT DEFAULT 'all',
                    field_scope TEXT DEFAULT '["counterparty","payment_method","description"]'
                );
                INSERT INTO account_rules(
                    id, user_id, account_id, rule_expression, regex_enabled, enabled,
                    priority, account_role_scope, transaction_type_scope, field_scope
                )
                VALUES
                    (1, 42, 10, 'OR={工资卡}', 0, 1, 3, 'source', 'expense', '["payment_method"]'),
                    (2, 42, 11, 'OR={禁用}', 0, 0, 1, 'source', 'expense', '["counterparty"]'),
                    (3, 77, 90, 'OR={其他}', 0, 1, 1, 'source', 'expense', '["counterparty"]');
                "#,
            )
            .expect("schema");

        let rules = load_import_intelligence_account_rules(&connection, 42).expect("rules");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_id, 1);
        assert_eq!(rules[0].account_id, 10);
        assert_eq!(rules[0].field_scope, vec!["payment_method"]);
    }

    #[test]
    fn category_rule_is_applied_before_learning_can_override_current_preview() {
        let mut draft = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_counterparty: "咖啡店".to_string(),
            preview_description: "拿铁".to_string(),
            preview_payment_method: "支付宝".to_string(),
            preview_parser_id: "alipay".to_string(),
            ..ImportPreviewDraft::default()
        };
        let food_category = ImportIntelligenceCategory {
            id: 11,
            type_code: 3,
            main_category: "餐饮".to_string(),
            sub_category: "咖啡".to_string(),
        };
        let shopping_category = ImportIntelligenceCategory {
            id: 12,
            type_code: 3,
            main_category: "购物".to_string(),
            sub_category: "日用".to_string(),
        };
        let categories_by_id = [food_category.clone(), shopping_category.clone()]
            .into_iter()
            .map(|category| (category.id, category))
            .collect::<BTreeMap<_, _>>();
        let category_values = categories_by_id
            .values()
            .map(|category| {
                json!({
                    "id": category.id,
                    "main_category": category.main_category,
                    "sub_category": category.sub_category,
                    "type": category.type_code,
                })
            })
            .collect::<Vec<_>>();
        let rules = vec![ImportIntelligenceRule {
            id: 501,
            category_id: 11,
            category_type: 3,
            main_category: "餐饮".to_string(),
            sub_category: "咖啡".to_string(),
            priority: 1,
            rule_expression: "咖啡店".to_string(),
            regex_enabled: false,
        }];
        let learning_rules = vec![ImportIntelligenceLearningRule {
            id: 601,
            parser_id: "alipay".to_string(),
            composite_hash: String::new(),
            match_features: build_composite_match_features(
                "alipay",
                "咖啡店",
                "拿铁",
                "支付宝",
            )
            .expect("composite match features"),
            learned_type: Some("支出".to_string()),
            learned_category_id: Some(12),
            learned_source_account_id: None,
            learned_destination_account_id: None,
        }];

        assert!(apply_category_rule_match(&mut draft, &rules));
        assert_eq!(draft.preview_main_category, "餐饮");
        assert_eq!(draft.preview_sub_category, "咖啡");
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/category_rule/rule_id")
                .and_then(Value::as_i64),
            Some(501)
        );

        assert_eq!(
            apply_learning_rule_match(
                &mut draft,
                &learning_rules,
                &categories_by_id,
                &category_values,
                &[],
            ),
            Some(601)
        );
        assert_eq!(draft.preview_main_category, "购物");
        assert_eq!(draft.preview_sub_category, "日用");
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/learning/rule_id")
                .and_then(Value::as_i64),
            Some(601)
        );
    }

    #[test]
    fn transfer_pair_account_match_fills_source_and_destination_from_source_chain() {
        let mut draft = ImportPreviewDraft {
            preview_matching_feedback: json!({
                "transfer": {
                    "pair_order": "outgoing_first",
                    "source_chain": [
                        {
                            "role": "outgoing",
                            "parser_id": "abc",
                            "payment_method": "农业银行",
                            "account_name": "工资卡",
                            "source_account_id": "abc",
                            "tags": ["parser:abc"]
                        },
                        {
                            "role": "incoming",
                            "payment_method": "微信钱包",
                            "account_name": "零钱",
                            "source_account_id": "wallet",
                            "tags": ["channel:wallet"]
                        }
                    ]
                }
            }),
            ..ImportPreviewDraft::default()
        };

        assert!(apply_transfer_pair_account_match(&mut draft, &transfer_accounts()));
        assert_eq!(draft.preview_source_account_id, Some(100));
        assert_eq!(draft.preview_destination_account_id, Some(200));
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/transfer/resolved_source_account_id")
                .and_then(Value::as_i64),
            Some(100)
        );
        assert_eq!(
            draft
                .preview_matching_feedback
                .pointer("/transfer/resolved_destination_account_id")
                .and_then(Value::as_i64),
            Some(200)
        );
    }

    #[test]
    fn transfer_pair_account_match_does_not_fill_same_destination_account() {
        let mut draft = ImportPreviewDraft {
            preview_matching_feedback: json!({
                "transfer": {
                    "source_chain": [
                        {"role": "outgoing", "source_account_id": 100},
                        {"role": "incoming", "source_account_id": 100}
                    ]
                }
            }),
            ..ImportPreviewDraft::default()
        };

        assert!(apply_transfer_pair_account_match(&mut draft, &transfer_accounts()));
        assert_eq!(draft.preview_source_account_id, Some(100));
        assert_eq!(draft.preview_destination_account_id, None);
    }

    fn fallback_categories() -> Vec<ImportIntelligenceCategory> {
        vec![
            ImportIntelligenceCategory {
                id: 10,
                type_code: 2,
                main_category: "投资收益".to_string(),
                sub_category: "理财收益".to_string(),
            },
            ImportIntelligenceCategory {
                id: 11,
                type_code: 3,
                main_category: "金融保险".to_string(),
                sub_category: "投资支出".to_string(),
            },
            ImportIntelligenceCategory {
                id: 12,
                type_code: 3,
                main_category: "交通出行".to_string(),
                sub_category: "公交地铁".to_string(),
            },
        ]
    }

    #[test]
    fn builtin_category_rule_fallback_fills_investment_income_expense_and_transport_paths() {
        let categories = fallback_categories();
        let mut income = ImportPreviewDraft {
            preview_type: "收入".to_string(),
            preview_description: "余额宝-2026.01.14-收益发放".to_string(),
            preview_counterparty: "支付宝（中国）网络技术有限公司".to_string(),
            preview_payment_method: "余额宝".to_string(),
            preview_parser_id: "alipay".to_string(),
            ..ImportPreviewDraft::default()
        };
        assert!(apply_builtin_category_rule_fallback(&mut income, &categories));
        assert_eq!(income.preview_main_category, "投资收益");
        assert_eq!(income.preview_sub_category, "理财收益");
        assert_eq!(
            income
                .preview_matching_feedback
                .pointer("/category_rule/category_id")
                .and_then(Value::as_i64),
            Some(10)
        );

        let mut expense = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_description: "理财通购买".to_string(),
            preview_parser_id: "wechat".to_string(),
            ..ImportPreviewDraft::default()
        };
        assert!(apply_builtin_category_rule_fallback(&mut expense, &categories));
        assert_eq!(expense.preview_main_category, "金融保险");
        assert_eq!(expense.preview_sub_category, "投资支出");

        let mut transport = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_sub_category: "公共交通".to_string(),
            ..ImportPreviewDraft::default()
        };
        assert!(apply_builtin_category_rule_fallback(&mut transport, &categories));
        assert_eq!(transport.preview_main_category, "交通出行");
        assert_eq!(transport.preview_sub_category, "公交地铁");
    }

    #[test]
    fn transfer_pair_account_match_runs_before_generic_alias_match_in_chain() -> rusqlite::Result<()>
    {
        let mut connection = Connection::open_in_memory()?;
        connection.execute_batch(
            "
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                aliases TEXT,
                hidden INTEGER DEFAULT 0
            );
            INSERT INTO accounts(id, user_id, name, aliases, hidden)
            VALUES
                (1, 42, '零钱', '[\"微信钱包\", \"wallet\"]', 0),
                (100, 42, '工资卡', '[\"农业银行\", \"abc\"]', 0);
            ",
        )?;
        let mut drafts = vec![ImportPreviewDraft {
            preview_parser_id: "wechat".to_string(),
            preview_payment_method: "微信钱包".to_string(),
            preview_parser_tags: Some(json!(["parser:abc", "parser:wechat", "channel:wallet"])),
            preview_matching_feedback: json!({
                "transfer": {
                    "pair_order": "outgoing_first",
                    "source_chain": [
                        {
                            "role": "outgoing",
                            "parser_id": "abc",
                            "payment_method": "农业银行",
                            "account_name": "工资卡",
                            "source_account_id": "abc"
                        },
                        {
                            "role": "incoming",
                            "payment_method": "微信钱包",
                            "account_name": "零钱",
                            "source_account_id": "wallet"
                        }
                    ]
                }
            }),
            ..ImportPreviewDraft::default()
        }];

        apply_import_intelligence_chain(&mut connection, UserId::new(42).unwrap(), &mut drafts)?;

        assert_eq!(drafts[0].preview_source_account_id, Some(100));
        assert_eq!(drafts[0].preview_destination_account_id, Some(1));
        Ok(())
    }

    #[test]
    fn user_cash_transfer_category_id_reads_optional_profile_setting() -> rusqlite::Result<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            "
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                cash_transfer_category_id INTEGER
            );
            INSERT INTO users(id, cash_transfer_category_id) VALUES (42, 904);
            ",
        )?;

        assert_eq!(load_user_cash_transfer_category_id(&connection, 42)?, Some(904));
        assert_eq!(load_user_cash_transfer_category_id(&connection, 77)?, None);
        Ok(())
    }

    #[test]
    fn transfer_default_category_prefers_user_cash_transfer_category() {
        let categories = vec![
            ImportIntelligenceCategory {
                id: 901,
                type_code: 4,
                main_category: "一般转账".to_string(),
                sub_category: "银行转账".to_string(),
            },
            ImportIntelligenceCategory {
                id: 904,
                type_code: 4,
                main_category: "账户互转".to_string(),
                sub_category: "电子支付".to_string(),
            },
        ];
        let categories_by_id = categories
            .iter()
            .cloned()
            .map(|category| (category.id, category))
            .collect::<BTreeMap<_, _>>();

        let selected = default_transfer_category(&categories, &categories_by_id, Some(904))
            .expect("user transfer category");
        assert_eq!(selected.id, 904);

        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            dedup_type: Some("transfer".to_string()),
            ..ImportPreviewDraft::default()
        };
        assert!(apply_transfer_default_category(
            &mut draft,
            &categories,
            Some(selected)
        ));
        assert_eq!(draft.preview_main_category, "账户互转");
        assert_eq!(draft.preview_sub_category, "电子支付");
    }

    #[test]
    fn transfer_protection_requires_match_signal_not_type_only() {
        let type_only = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            ..ImportPreviewDraft::default()
        };
        assert!(!is_transfer_protected_preview(&type_only));

        let dedup_matched = ImportPreviewDraft {
            preview_type: "支出".to_string(),
            dedup_type: Some("transfer".to_string()),
            ..ImportPreviewDraft::default()
        };
        assert!(is_transfer_protected_preview(&dedup_matched));

        let signal_matched = ImportPreviewDraft {
            preview_type: "收入".to_string(),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"}
            }),
            ..ImportPreviewDraft::default()
        };
        assert!(is_transfer_protected_preview(&signal_matched));
    }

    #[test]
    fn transfer_learning_domain_allows_account_only_rule() {
        let rule = ImportIntelligenceLearningRule {
            id: 1,
            parser_id: String::new(),
            composite_hash: String::new(),
            match_features: BTreeMap::new(),
            learned_type: None,
            learned_category_id: None,
            learned_source_account_id: Some(100),
            learned_destination_account_id: Some(200),
        };
        assert!(learning_rule_keeps_transfer_domain(
            &rule,
            &BTreeMap::new()
        ));
    }

    #[test]
    fn transfer_learning_does_not_override_source_chain_accounts() {
        let features = build_composite_match_features(
            "cmbc",
            "支付宝（中国）网络技术有限公司客户备付金",
            "支付宝快捷支付",
            "网络银行",
        )
        .expect("transfer learning features");
        let composite_hash = composite_hash_from_features(&features);
        let rule = ImportIntelligenceLearningRule {
            id: 7103,
            parser_id: "cmbc".to_string(),
            composite_hash,
            match_features: features,
            learned_type: None,
            learned_category_id: None,
            learned_source_account_id: Some(9001),
            learned_destination_account_id: Some(9002),
        };
        let mut draft = ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_parser_id: "cmbc".to_string(),
            preview_counterparty: "支付宝（中国）网络技术有限公司客户备付金".to_string(),
            preview_description: "支付宝快捷支付".to_string(),
            preview_payment_method: "网络银行".to_string(),
            preview_source_account_id: Some(1001),
            preview_destination_account_id: Some(1002),
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"}
            }),
            ..ImportPreviewDraft::default()
        };
        let account_values = vec![
            json!({"id": 1001, "name": "民生银行"}),
            json!({"id": 1002, "name": "支付宝"}),
            json!({"id": 9001, "name": "旧来源"}),
            json!({"id": 9002, "name": "旧目标"}),
        ];

        assert_eq!(
            apply_learning_rule_match(
                &mut draft,
                &[rule],
                &BTreeMap::new(),
                &[],
                &account_values,
            ),
            Some(7103)
        );
        assert_eq!(draft.preview_source_account_id, Some(1001));
        assert_eq!(draft.preview_destination_account_id, Some(1002));
    }

    #[test]
    fn transfer_invariants_clear_stale_account_review_annotation() {
        let mut drafts = vec![ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_source_account_id: Some(100),
            preview_destination_account_id: Some(200),
            preview_matching_feedback: json!({
                "transfer": {"candidate_type": "transfer"},
                "annotation": {
                    "type": "transfer_account_direction",
                    "reason": "transfer preview is missing source or destination account"
                }
            }),
            ..ImportPreviewDraft::default()
        }];

        enforce_import_preview_invariants(drafts.as_mut_slice());

        assert!(drafts[0].preview_matching_feedback.get("annotation").is_none());
    }
}
