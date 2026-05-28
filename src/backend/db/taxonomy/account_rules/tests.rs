use rusqlite::Connection;
use serde_json::json;

use bill_analyser_core::account_rules::AccountRuleMatchContext;

use super::helpers::{
    account_rule_candidate_from_record, bool_int_value, field_scope_json, is_constraint_error,
    normalize_alias, normalize_rule_expression_alias, required_i64, required_rule_expression,
    sql_text_value, value_as_i64,
};
use super::{AccountRuleRecord, AccountRulesRepository};

#[test]
fn account_rules_repository_crud_reorder_and_match_are_user_scoped() {
    let mut connection = fixture_connection();
    let mut repository = AccountRulesRepository::new(&mut connection);

    let enabled = repository
        .list_rules(42, None, true, None, None)
        .expect("list enabled");
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0]["id"], 80);
    assert_eq!(enabled[0]["account_name"], "工资卡");
    assert_eq!(enabled[0]["field_scope"], json!(["payment_method"]));
    assert!(!enabled.iter().any(|rule| rule["user_id"] == 77));
    assert_eq!(
        repository
            .list_rules(42, None, true, Some("source"), Some("expense"))
            .expect("scoped list")
            .len(),
        1
    );
    assert!(repository
        .list_rules(42, None, true, Some("bad"), None)
        .expect_err("bad account role scope")
        .to_string()
        .contains("unsupported account_role_scope"));

    let created_id = repository
        .create_rule(
            &json!({
                "account_id": 11,
                "name": "招商规则",
                "priority": 5,
                "rule_expression": "OR={招商}",
                "regex_enabled": false,
                "enabled": true,
                "account_role_scope": "destination",
                "transaction_type_scope": "transfer",
                "field_scope": ["counterparty", "description"]
            }),
            42,
        )
        .expect("create")
        .expect("created id");
    let created = repository
        .get_rule(created_id, 42)
        .expect("get")
        .expect("created");
    assert_eq!(created["account_role_scope"], "destination");
    assert_eq!(created["transaction_type_scope"], "transfer");

    let camel_id = repository
        .create_rule(
            &json!({
                "accountId": 11,
                "name": true,
                "priority": "6",
                "ruleExpression": "OR={招商}",
                "regexEnabled": "yes",
                "enabled": 0,
                "accountRoleScope": "payment_method_source",
                "transactionTypeScope": "income",
                "fieldScope": "parser,description",
                "source": 5
            }),
            42,
        )
        .expect("camel create")
        .expect("camel id");
    let camel = repository
        .get_rule(camel_id, 42)
        .expect("get camel")
        .expect("camel");
    assert_eq!(camel["name"], "true");
    assert_eq!(camel["regex_enabled"], 1);
    assert_eq!(camel["enabled"], 0);
    assert_eq!(camel["source"], "5");
    assert_eq!(camel["field_scope"], json!(["parser", "description"]));

    assert!(repository
        .update_rule(
            created_id,
            &json!({
                "priority": 1,
                "enabled": "off",
                "fieldScope": "parser,payment_method"
            }),
            42,
        )
        .expect("update"));
    let updated = repository
        .get_rule(created_id, 42)
        .expect("updated")
        .expect("updated");
    assert_eq!(updated["priority"], 1);
    assert_eq!(updated["enabled"], 0);
    assert_eq!(updated["field_scope"], json!(["parser", "payment_method"]));

    assert!(repository
        .update_rule(created_id, &json!(false), 42)
        .expect_err("non object update")
        .to_string()
        .contains("account rule update payload must be an object"));
    assert!(!repository
        .update_rule(created_id, &json!({}), 42)
        .expect("empty update"));
    assert!(!repository
        .update_rule(created_id, &json!({"id": 1, "accountName": "ignored"}), 42)
        .expect("ignored update"));
    assert!(repository
        .update_rule(
            camel_id,
            &json!({
                "accountId": 11,
                "name": 123,
                "priority": "4",
                "ruleExpression": "OR={子卡}",
                "regexEnabled": true,
                "enabled": "on",
                "accountRoleScope": "source",
                "transactionTypeScope": "expense",
                "fieldScope": ["counterparty"],
                "source": false
            }),
            42,
        )
        .expect("wide update"));
    let candidates = repository.list_match_candidates(42).expect("candidates");
    assert!(candidates
        .iter()
        .any(|candidate| candidate.rule_id == camel_id));
    let matched = repository
        .test_rule_match(
            camel_id,
            42,
            &AccountRuleMatchContext {
                counterparty: "子卡".to_string(),
                ..AccountRuleMatchContext::default()
            },
            "source",
            "expense",
        )
        .expect("test match")
        .expect("matched");
    assert_eq!(matched.rule_id, camel_id);
    assert!(repository
        .test_rule_match(
            9000,
            42,
            &AccountRuleMatchContext::default(),
            "source",
            "expense"
        )
        .expect("missing test")
        .is_none());

    assert!(!repository
        .reorder_rules(&[created_id, 80, 90], 42)
        .expect("cross-user reorder rejected"));
    assert!(!repository
        .reorder_rules(&[created_id, created_id], 42)
        .expect("duplicate reorder rejected"));
    assert!(repository
        .reorder_rules(&[created_id, 80, 81], 42)
        .expect("reorder"));
    assert_eq!(
        repository
            .get_rule(created_id, 42)
            .expect("get reordered")
            .expect("reordered")["priority"],
        1
    );
    assert!(!repository
        .update_rule(created_id, &json!({"account_id": 99}), 42)
        .expect("cross-account update"));
    assert!(repository.get_rule(90, 42).expect("cross user").is_none());
    assert!(!repository.delete_rule(9000, 42).expect("missing delete"));
    assert!(repository.delete_rule(created_id, 42).expect("delete"));
    assert!(repository
        .get_rule(created_id, 42)
        .expect("deleted")
        .is_none());
}

#[test]
fn account_alias_migration_is_idempotent_and_skips_hidden_accounts() {
    let mut connection = fixture_connection();
    let mut repository = AccountRulesRepository::new(&mut connection);

    let migrated = repository
        .migrate_aliases_to_rules(42)
        .expect("migrate aliases");
    assert_eq!(migrated.migrated, 1);
    assert_eq!(migrated.skipped, 2);

    let rules = repository
        .list_rules(42, Some(11), false, None, None)
        .expect("visible account rules");
    assert!(rules.iter().any(|rule| {
        rule["source"] == "alias_migration" && rule["rule_expression"] == "OR={子卡}"
    }));
    assert!(!repository
        .list_rules(42, Some(10), false, None, None)
        .expect("hidden account rules")
        .iter()
        .any(|rule| rule["source"] == "alias_migration"));

    let repeated = repository
        .migrate_aliases_to_rules(42)
        .expect("repeat migration");
    assert_eq!(repeated.migrated, 0);
    assert_eq!(repeated.skipped, 3);
}

#[test]
fn account_alias_migration_detects_legacy_expression_without_source_key() {
    let mut connection = fixture_connection();
    connection
        .execute(
            "INSERT INTO account_rules(
                id, user_id, account_id, name, priority, rule_expression,
                regex_enabled, enabled, account_role_scope, transaction_type_scope,
                field_scope, source, source_key, created_at, updated_at
            ) VALUES (120, 42, 11, 'legacy expression', 1, 'OR={子卡}', 0, 1,
                      'any', 'all', '[\"counterparty\"]', 'alias_migration', NULL, 'now', 'now')",
            [],
        )
        .expect("legacy expression");
    let mut repository = AccountRulesRepository::new(&mut connection);

    let migrated = repository
        .migrate_aliases_to_rules(42)
        .expect("migrate aliases");
    assert_eq!(migrated.migrated, 0);
    assert_eq!(migrated.skipped, 3);
}

#[test]
fn account_rules_validate_payload_edges() {
    let mut connection = fixture_connection();
    let mut repository = AccountRulesRepository::new(&mut connection);

    assert!(repository
        .create_rule(&json!({"rule_expression": "OR={x}"}), 42)
        .expect_err("missing account")
        .to_string()
        .contains("account_id is required"));
    assert!(repository
        .create_rule(&json!({"account_id": 10}), 42)
        .expect_err("missing expression")
        .to_string()
        .contains("rule_expression is required"));
    assert!(repository
        .create_rule(
            &json!({
                "account_id": 10,
                "rule_expression": "OR={x}",
                "account_role_scope": "bad"
            }),
            42,
        )
        .expect_err("bad role")
        .to_string()
        .contains("unsupported account_role_scope"));
    assert!(repository
        .create_rule(
            &json!({
                "account_id": 10,
                "rule_expression": "OR={x}",
                "field_scope": ["counterparty", "bad"]
            }),
            42,
        )
        .expect_err("bad field")
        .to_string()
        .contains("unsupported account rule field scope"));
}

#[test]
fn account_rule_scalar_helpers_cover_error_edges() {
    assert_eq!(value_as_i64(Some(&json!("42"))), Some(42));
    assert_eq!(value_as_i64(Some(&json!(true))), Some(1));
    assert_eq!(value_as_i64(Some(&json!([]))), None);
    assert_eq!(
        required_rule_expression(Some(&json!("OR={现金}"))).expect("expression"),
        "OR={现金}"
    );
    assert!(required_rule_expression(None)
        .expect_err("missing expression")
        .to_string()
        .contains("rule_expression is required"));
    assert!(required_rule_expression(Some(&json!(null)))
        .expect_err("null expression")
        .to_string()
        .contains("rule_expression is required"));
    assert!(required_rule_expression(Some(&json!(" ")))
        .expect_err("blank expression")
        .to_string()
        .contains("rule_expression is required"));
    assert!(required_i64(&json!("x"), "priority")
        .expect_err("invalid integer")
        .to_string()
        .contains("priority must be an integer-compatible value"));
    assert_eq!(bool_int_value(&json!("off")).expect("bool"), 0);
    assert!(bool_int_value(&json!(1.25))
        .expect_err("float bool")
        .to_string()
        .contains("boolean field must be an integer"));
    assert!(bool_int_value(&json!("maybe"))
        .expect_err("invalid bool")
        .to_string()
        .contains("boolean field must be truthy or falsy"));
    assert!(bool_int_value(&json!([]))
        .expect_err("array bool")
        .to_string()
        .contains("boolean field must be truthy or falsy"));
    assert_eq!(sql_text_value(&json!(false)).expect("text"), "false");
    assert_eq!(sql_text_value(&json!(null)).expect("null text"), "");
    assert!(sql_text_value(&json!({}))
        .expect_err("object text")
        .to_string()
        .contains("text field must be scalar"));
    assert_eq!(
        field_scope_json(&["counterparty".to_string(), "description".to_string()])
            .expect("field scope json"),
        "[\"counterparty\",\"description\"]"
    );
    assert_eq!(normalize_alias("  VISA Card  "), "visa card");
    assert_eq!(normalize_rule_expression_alias(" OR={\\子卡} "), "子卡");
}

#[test]
fn account_rule_row_helpers_cover_sqlite_dynamic_values_and_constraints() {
    let mut connection = fixture_connection();
    connection
        .execute(
            "INSERT INTO accounts(id, user_id, name, type, aliases, hidden, display_order)
             VALUES (12, 42, X'E78EB0E98791', 1, '[]', 0, 3)",
            [],
        )
        .expect("blob account");
    connection
        .execute(
            "INSERT INTO account_rules(
                id, user_id, account_id, name, priority, rule_expression,
                regex_enabled, enabled, account_role_scope, transaction_type_scope,
                field_scope, source, source_key, created_at, updated_at
            ) VALUES (121, 42, 12, 'dynamic', 12.5, 'OR={现金}', 0, 1,
                      'source', 'expense', 'not-a-valid-scope', 'manual', NULL, 'now', 'now')",
            [],
        )
        .expect("dynamic rule");

    let duplicate_error = connection
        .execute(
            "INSERT INTO accounts(id, user_id, name) VALUES (12, 42, 'duplicate')",
            [],
        )
        .expect_err("duplicate primary key");
    assert!(is_constraint_error(&duplicate_error));

    let mut repository = AccountRulesRepository::new(&mut connection);
    let rules = repository
        .list_rules(42, Some(12), false, None, None)
        .expect("list dynamic rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["account_name"], "现金");
    assert_eq!(rules[0]["priority"].as_f64(), Some(12.5));
    assert_eq!(
        rules[0]["field_scope"],
        json!(["counterparty", "payment_method", "description"])
    );
}

#[test]
fn account_rule_candidate_conversion_covers_truthy_scalar_edges() {
    let candidate =
        account_rule_candidate_from_record(account_rule_record(json!(true), json!("yes")))
            .expect("truthy candidate");
    assert!(candidate.regex_enabled);
    assert!(candidate.enabled);
    assert_eq!(
        candidate.field_scope,
        vec!["counterparty", "payment_method", "description"]
    );

    let disabled = account_rule_candidate_from_record(account_rule_record(json!(null), json!({})))
        .expect("falsy candidate");
    assert!(!disabled.regex_enabled);
    assert!(!disabled.enabled);
}

fn account_rule_record(
    regex_enabled: serde_json::Value,
    enabled: serde_json::Value,
) -> AccountRuleRecord {
    let mut record = AccountRuleRecord::new();
    record.insert("id".to_string(), json!(501));
    record.insert("account_id".to_string(), json!(10));
    record.insert("account_role_scope".to_string(), json!("source"));
    record.insert("transaction_type_scope".to_string(), json!("expense"));
    record.insert("field_scope".to_string(), json!(null));
    record.insert("rule_expression".to_string(), json!("OR={现金}"));
    record.insert("regex_enabled".to_string(), regex_enabled);
    record.insert("enabled".to_string(), enabled);
    record.insert("priority".to_string(), json!("7"));
    record
}

fn fixture_connection() -> Connection {
    let connection = Connection::open_in_memory().expect("open");
    connection
            .execute_batch(
                "
                CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    type INTEGER DEFAULT 1,
                    aliases TEXT,
                    hidden INTEGER DEFAULT 0,
                    display_order INTEGER DEFAULT 0
                );
                CREATE TABLE account_rules (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    account_id INTEGER NOT NULL,
                    name TEXT NOT NULL DEFAULT '',
                    priority INTEGER NOT NULL DEFAULT 100,
                    rule_expression TEXT NOT NULL,
                    regex_enabled INTEGER DEFAULT 0,
                    enabled INTEGER DEFAULT 1,
                    applied_count INTEGER DEFAULT 0,
                    last_applied_at TEXT,
                    match_count INTEGER DEFAULT 0,
                    last_matched_at TEXT,
                    account_role_scope TEXT NOT NULL DEFAULT 'any',
                    transaction_type_scope TEXT NOT NULL DEFAULT 'all',
                    field_scope TEXT NOT NULL DEFAULT '[\"counterparty\",\"payment_method\",\"description\"]',
                    source TEXT NOT NULL DEFAULT 'manual',
                    source_key TEXT,
                    created_at TEXT,
                    updated_at TEXT
                );
                INSERT INTO accounts(id, user_id, name, type, aliases, hidden, display_order)
                VALUES
                    (10, 42, '工资卡', 1, '[\"主卡\", \"工资\"]', 1, 1),
                    (11, 42, '工资子账户', 1, '[\"子卡\"]', 0, 2),
                    (99, 77, '其他用户', 1, '[\"其他\"]', 0, 1);
                INSERT INTO account_rules(
                    id, user_id, account_id, name, priority, rule_expression,
                    regex_enabled, enabled, applied_count, last_applied_at,
                    match_count, last_matched_at,
                    account_role_scope, transaction_type_scope, field_scope, source, source_key,
                    created_at, updated_at
                )
                VALUES
                    (80, 42, 10, '工资卡规则', 10, 'OR={工资}', 0, 1, 3, '2026-01-02T00:00:00',
                     3, '2026-01-02T00:00:00', 'source', 'expense', '[\"payment_method\"]',
                     'manual', NULL, 'now', 'now'),
                    (81, 42, 10, '禁用规则', 20, 'OR={禁用}', 0, 0, 0, NULL,
                     0, NULL, 'any', 'all', '[\"counterparty\"]', 'manual', NULL, 'now', 'now'),
                    (90, 77, 99, '其他用户规则', 1, 'OR={其他}', 0, 1, 1, NULL,
                     1, NULL, 'any', 'all', '[\"counterparty\"]', 'manual', NULL, 'now', 'now');
                ",
            )
            .expect("schema");
    connection
}
