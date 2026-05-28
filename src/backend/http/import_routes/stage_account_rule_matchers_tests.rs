// 中文导读：导入账户规则切换单测，覆盖别名退役和转账/投资/收入/支出规则顺序。
// 维护重点：测试 account_rules 在 stage2 的权威匹配行为，不恢复旧别名导入链路。
// 不变式：这些测试只验证 include 后的私有导入 helper 合同。

#[cfg(test)]
mod stage_account_rule_matchers_tests {
    use super::*;

    fn init_rule_order_schema(connection: &Connection) -> rusqlite::Result<()> {
        connection.execute_batch(
            r#"
            CREATE TABLE categories (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                type INTEGER,
                main_category TEXT,
                sub_category TEXT,
                priority INTEGER DEFAULT 0
            );
            CREATE TABLE category_rules (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                category_id INTEGER NOT NULL,
                rule_expression TEXT NOT NULL,
                regex_enabled INTEGER DEFAULT 0,
                enabled INTEGER DEFAULT 1,
                priority INTEGER DEFAULT 100
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                hidden INTEGER DEFAULT 0
            );
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
            "#,
        )
    }

    #[test]
    fn transfer_account_rule_match_uses_source_chain_in_import_chain() -> rusqlite::Result<()> {
        let mut connection = Connection::open_in_memory()?;
        init_rule_order_schema(&connection)?;
        connection.execute_batch(
            r#"
            INSERT INTO accounts(id, user_id, name, hidden)
            VALUES
                (1, 42, '零钱', 0),
                (100, 42, '工资卡', 0);
            INSERT INTO account_rules(
                id, user_id, account_id, rule_expression, enabled, priority,
                account_role_scope, transaction_type_scope, field_scope
            )
            VALUES
                (11, 42, 100, 'OR={农业银行}', 1, 1, 'source', 'transfer', '["expense_payment_method"]'),
                (12, 42, 1, 'OR={微信钱包}', 1, 1, 'destination', 'transfer', '["income_payment_method"]');
            "#,
        )?;
        let mut drafts = vec![ImportPreviewDraft {
            preview_parser_id: "wechat".to_string(),
            preview_payment_method: "微信钱包".to_string(),
            preview_parser_tags: Some(json!(["parser:abc", "parser:wechat", "channel:wallet"])),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer",
                    "pair_order": "outgoing_first",
                    "source_chain": [
                        {"role": "outgoing", "parser_id": "abc", "payment_method": "农业银行"},
                        {"role": "incoming", "payment_method": "微信钱包"}
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
    fn import_account_rules_are_authoritative_after_cutover() -> rusqlite::Result<()> {
        let mut connection = Connection::open_in_memory()?;
        init_rule_order_schema(&connection)?;
        connection.execute_batch(
            r#"
            INSERT INTO accounts(id, user_id, name, hidden)
            VALUES (100, 42, '现金', 0);
            "#,
        )?;
        let mut drafts = vec![ImportPreviewDraft {
            preview_type: "支出".to_string(),
            preview_counterparty: "便利店".to_string(),
            preview_payment_method: "现金".to_string(),
            preview_description: "早餐".to_string(),
            preview_parser_id: "manual".to_string(),
            ..ImportPreviewDraft::default()
        }];

        apply_import_intelligence_chain(&mut connection, UserId::new(42).unwrap(), &mut drafts)?;

        assert_eq!(drafts[0].preview_source_account_id, None);
        assert!(
            drafts[0]
                .preview_matching_feedback
                .get("account_rule")
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn transfer_import_pass_uses_transfer_rules_and_hidden_side_account_fields(
    ) -> rusqlite::Result<()> {
        let mut connection = Connection::open_in_memory()?;
        init_rule_order_schema(&connection)?;
        connection.execute_batch(
            r#"
            INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
            VALUES
                (40, 42, 4, '一般转账', '账户互转', 1),
                (30, 42, 3, '错误支出', '不应命中', 2);
            INSERT INTO category_rules(id, user_id, category_id, rule_expression, enabled, priority)
            VALUES
                (400, 42, 40, 'OR={农业银行}', 1, 1),
                (300, 42, 30, 'OR={wallet transfer}', 1, 0);
            INSERT INTO accounts(id, user_id, name, hidden)
            VALUES
                (100, 42, '转出账户', 0),
                (200, 42, '转入账户', 0);
            INSERT INTO account_rules(
                id, user_id, account_id, rule_expression, enabled, priority,
                account_role_scope, transaction_type_scope, field_scope
            )
            VALUES
                (1000, 42, 100, 'OR={农业银行}', 1, 1, 'source', 'transfer', '["expense_payment_method"]'),
                (1001, 42, 200, 'OR={微信钱包}', 1, 1, 'destination', 'transfer', '["income_payment_method"]');
            "#,
        )?;
        let mut drafts = vec![ImportPreviewDraft {
            preview_type: "转账".to_string(),
            preview_counterparty: "wallet transfer".to_string(),
            preview_payment_method: "合并支付方式".to_string(),
            preview_description: "wallet transfer".to_string(),
            dedup_type: Some("transfer".to_string()),
            preview_matching_feedback: json!({
                "transfer": {
                    "candidate_type": "transfer",
                    "source_chain": [
                        {"role": "outgoing", "parser_id": "abc", "payment_method": "农业银行", "counterparty": "转出方", "description": "支出侧"},
                        {"role": "incoming", "parser_id": "wechat", "payment_method": "微信钱包", "counterparty": "转入方", "description": "收入侧"}
                    ]
                }
            }),
            ..ImportPreviewDraft::default()
        }];

        apply_import_intelligence_chain(&mut connection, UserId::new(42).unwrap(), &mut drafts)?;

        assert_eq!(drafts[0].preview_type, "转账");
        assert_eq!(drafts[0].preview_main_category, "一般转账");
        assert_eq!(drafts[0].preview_sub_category, "账户互转");
        assert_eq!(drafts[0].preview_source_account_id, Some(100));
        assert_eq!(drafts[0].preview_destination_account_id, Some(200));
        assert_eq!(
            drafts[0]
                .preview_matching_feedback
                .pointer("/account_rule/source/rule_id")
                .and_then(Value::as_i64),
            Some(1000)
        );
        assert_eq!(
            drafts[0]
                .preview_matching_feedback
                .pointer("/account_rule/destination/rule_id")
                .and_then(Value::as_i64),
            Some(1001)
        );
        Ok(())
    }

    #[test]
    fn investment_import_pass_matches_source_then_counterparty_or_description_account(
    ) -> rusqlite::Result<()> {
        let mut connection = Connection::open_in_memory()?;
        init_rule_order_schema(&connection)?;
        connection.execute_batch(
            r#"
            INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
            VALUES (50, 42, 5, '投资理财', '基金申购', 1);
            INSERT INTO category_rules(id, user_id, category_id, rule_expression, enabled, priority)
            VALUES (500, 42, 50, 'OR={华泰证券,基金申购}', 1, 1);
            INSERT INTO accounts(id, user_id, name, hidden)
            VALUES
                (10, 42, '支付宝资金账户', 0),
                (20, 42, '华泰证券账户', 0),
                (21, 42, '基金描述账户', 0);
            INSERT INTO account_rules(
                id, user_id, account_id, rule_expression, enabled, priority,
                account_role_scope, transaction_type_scope, field_scope
            )
            VALUES
                (100, 42, 10, 'OR={alipay,支付宝}', 1, 1, 'source', 'investment', '["parser","payment_method"]'),
                (200, 42, 20, 'OR={华泰证券}', 1, 1, 'investment', 'investment', '["investment_counterparty"]'),
                (201, 42, 21, 'OR={基金申购}', 1, 1, 'investment', 'investment', '["investment_description"]');
            "#,
        )?;
        let mut drafts = vec![
            ImportPreviewDraft {
                preview_type: "支出".to_string(),
                preview_counterparty: "华泰证券".to_string(),
                preview_payment_method: "支付宝".to_string(),
                preview_description: "普通买入".to_string(),
                preview_parser_id: "alipay".to_string(),
                ..ImportPreviewDraft::default()
            },
            ImportPreviewDraft {
                preview_type: "支出".to_string(),
                preview_counterparty: "未知平台".to_string(),
                preview_payment_method: "支付宝".to_string(),
                preview_description: "基金申购".to_string(),
                preview_parser_id: "alipay".to_string(),
                ..ImportPreviewDraft::default()
            },
        ];

        apply_import_intelligence_chain(&mut connection, UserId::new(42).unwrap(), &mut drafts)?;

        assert_eq!(drafts[0].preview_type, "投资");
        assert_eq!(drafts[0].preview_source_account_id, Some(10));
        assert_eq!(drafts[0].preview_destination_account_id, Some(20));
        assert_eq!(
            drafts[0]
                .preview_matching_feedback
                .pointer("/account_rule/investment/rule_id")
                .and_then(Value::as_i64),
            Some(200)
        );
        assert_eq!(drafts[1].preview_type, "投资");
        assert_eq!(drafts[1].preview_source_account_id, Some(10));
        assert_eq!(drafts[1].preview_destination_account_id, Some(21));
        assert_eq!(
            drafts[1]
                .preview_matching_feedback
                .pointer("/account_rule/investment/fallback")
                .and_then(Value::as_str),
            Some("description")
        );
        Ok(())
    }

    #[test]
    fn income_expense_import_pass_scopes_category_and_account_rules_by_preview_type(
    ) -> rusqlite::Result<()> {
        let mut connection = Connection::open_in_memory()?;
        init_rule_order_schema(&connection)?;
        connection.execute_batch(
            r#"
            INSERT INTO categories(id, user_id, type, main_category, sub_category, priority)
            VALUES
                (20, 42, 2, '工资收入', '月薪', 1),
                (30, 42, 3, '日常支出', '餐饮', 1);
            INSERT INTO category_rules(id, user_id, category_id, rule_expression, enabled, priority)
            VALUES
                (200, 42, 20, 'OR={共同关键字}', 1, 1),
                (300, 42, 30, 'OR={共同关键字}', 1, 1);
            INSERT INTO accounts(id, user_id, name, hidden)
            VALUES
                (20, 42, '工资卡', 0),
                (30, 42, '支付宝', 0);
            INSERT INTO account_rules(
                id, user_id, account_id, rule_expression, enabled, priority,
                account_role_scope, transaction_type_scope, field_scope
            )
            VALUES
                (1200, 42, 20, 'OR={公司}', 1, 1, 'source', 'income', '["counterparty"]'),
                (1300, 42, 30, 'OR={支付宝}', 1, 1, 'source', 'expense', '["payment_method"]');
            "#,
        )?;
        let mut drafts = vec![
            ImportPreviewDraft {
                preview_type: "收入".to_string(),
                preview_counterparty: "公司".to_string(),
                preview_payment_method: "工资卡".to_string(),
                preview_description: "共同关键字".to_string(),
                preview_parser_id: "generic".to_string(),
                ..ImportPreviewDraft::default()
            },
            ImportPreviewDraft {
                preview_type: "支出".to_string(),
                preview_counterparty: "餐厅".to_string(),
                preview_payment_method: "支付宝".to_string(),
                preview_description: "共同关键字".to_string(),
                preview_parser_id: "alipay".to_string(),
                ..ImportPreviewDraft::default()
            },
        ];

        apply_import_intelligence_chain(&mut connection, UserId::new(42).unwrap(), &mut drafts)?;

        assert_eq!(drafts[0].preview_main_category, "工资收入");
        assert_eq!(drafts[0].preview_sub_category, "月薪");
        assert_eq!(drafts[0].preview_source_account_id, Some(20));
        assert_eq!(
            drafts[0]
                .preview_matching_feedback
                .pointer("/category_rule/rule_id")
                .and_then(Value::as_i64),
            Some(200)
        );
        assert_eq!(
            drafts[0]
                .preview_matching_feedback
                .pointer("/account_rule/source/rule_id")
                .and_then(Value::as_i64),
            Some(1200)
        );
        assert_eq!(drafts[1].preview_main_category, "日常支出");
        assert_eq!(drafts[1].preview_sub_category, "餐饮");
        assert_eq!(drafts[1].preview_source_account_id, Some(30));
        assert_eq!(
            drafts[1]
                .preview_matching_feedback
                .pointer("/category_rule/rule_id")
                .and_then(Value::as_i64),
            Some(300)
        );
        assert_eq!(
            drafts[1]
                .preview_matching_feedback
                .pointer("/account_rule/source/rule_id")
                .and_then(Value::as_i64),
            Some(1300)
        );
        Ok(())
    }
}
