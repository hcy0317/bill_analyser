use std::{env, error::Error, str::FromStr};

use bill_analyser_db::{
    run_postgres_migrations,
    taxonomy::settings_bundle::{
        export_taxonomy_sections, import_postgres_settings_bundle,
        normalize_settings_bundle_sections,
    },
};
use serde_json::{json, Value};
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};

#[test]
fn settings_bundle_taxonomy_export_preserves_importable_refs_and_minor_units(
) -> Result<(), Box<dyn Error>> {
    let sections = export_taxonomy_sections(&json!({
        "accounts": [
            {"id": 1, "name": "现金", "type": 1, "currency": "CNY", "balanceCents": 1234},
            {"id": 2, "name": "工资卡", "type": 1, "currency": "CNY", "balanceCents": 0}
        ],
        "categories": [{
            "id": 3,
            "type": 3,
            "main_category": "餐饮",
            "sub_category": "午餐"
        }],
        "tags": [{"id": 7, "name": "项目"}],
        "templates": [{
            "id": "8",
            "templateType": 1,
            "name": "午餐模板",
            "type": 3,
            "categoryId": "3",
            "sourceAccountId": "1",
            "sourceAmountCents": 1999,
            "destinationAmountCents": 0,
            "tagIds": ["7"]
        }],
        "scheduled": [{
            "id": "9",
            "templateType": 2,
            "name": "房租计划",
            "type": 4,
            "categoryId": "3",
            "sourceAccountId": "1",
            "destinationAccountId": "2",
            "sourceAmountCents": 250000,
            "destinationAmountCents": 250000,
            "tagIds": ["7"],
            "scheduledStartDate": "2026-06-01"
        }]
    }))?;

    let account = &sections["accounts"][0];
    assert_eq!(account["externalRef"], "account:1");
    assert_eq!(account["balanceCents"], 1234);

    let template = &sections["transactionTemplates"][0];
    assert_eq!(template["sourceAccountRef"], "account:1");
    assert_eq!(template["sourceAccountName"], "现金");
    assert_eq!(template["categoryRef"], "category:3");
    assert_eq!(template["tagRefs"], json!(["tag:7"]));
    assert_eq!(template["sourceAmountCents"], 1999);
    assert!(template.get("id").is_none());
    assert!(template.get("tagIds").is_none());

    let scheduled = &sections["scheduledTransactions"][0];
    assert_eq!(scheduled["sourceAccountRef"], "account:1");
    assert_eq!(scheduled["destinationAccountRef"], "account:2");
    assert_eq!(scheduled["categoryRef"], "category:3");
    assert_eq!(scheduled["tagRefs"], json!(["tag:7"]));
    assert_eq!(scheduled["sourceAmountCents"], 250000);
    assert_eq!(scheduled["destinationAmountCents"], 250000);
    assert!(scheduled.get("id").is_none());
    assert!(scheduled.get("tagIds").is_none());

    let normalized = normalize_settings_bundle_sections(&json!({
        "schemaVersion": 1,
        "sections": sections
    }))?;
    assert_eq!(
        normalized["transactionTemplates"][0]["sourceAmountCents"],
        1999
    );
    assert_eq!(
        normalized["scheduledTransactions"][0]["destinationAmountCents"],
        250000
    );
    assert!(normalized["accountRecognitionRules"]
        .as_array()
        .unwrap()
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn settings_bundle_import_upserts_templates_when_postgres_available(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("settings_bundle_test_{unique}");
    let base_options = PgConnectOptions::from_str(&postgres_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.clone().database("postgres"))
        .await?;
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{}""#, test_db).as_str())
        .await?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(base_options.database(&test_db))
        .await?;
    run_postgres_migrations(&pool).await?;

    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("settings-bundle-{unique}"))
            .bind(format!("settings-bundle-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let cash_name = format!("现金-{unique}");
    let bank_name = format!("工资卡-{unique}");

    let bundle = settings_bundle_payload(unique, 1234, 250000, false);
    let dry_run = import_postgres_settings_bundle(&pool, &bundle, user_id, true).await?;
    assert_eq!(dry_run["dryRun"], true);
    assert_eq!(dry_run["sections"]["transactionTemplates"]["created"], 1);
    assert_eq!(dry_run["sections"]["scheduledTransactions"]["created"], 1);
    assert_eq!(dry_run["sections"]["accountRecognitionRules"]["created"], 1);
    assert_no_unsupported_template_warning(&dry_run);
    assert_deprecated_scope_warning(&dry_run);

    let template_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM transaction_templates WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(template_count, 0);
    assert_account_rule_scope_columns_absent(&pool).await?;

    let imported = import_postgres_settings_bundle(&pool, &bundle, user_id, false).await?;
    assert_eq!(imported["sections"]["transactionTemplates"]["created"], 1);
    assert_eq!(imported["sections"]["scheduledTransactions"]["created"], 1);
    assert_eq!(
        imported["sections"]["accountRecognitionRules"]["created"],
        1
    );
    assert_no_unsupported_template_warning(&imported);
    assert_deprecated_scope_warning(&imported);

    assert_template_row(&pool, user_id, 1, "午餐模板", 1234, 0, false).await?;
    assert_template_row(&pool, user_id, 2, "房租计划", 250000, 250000, false).await?;
    assert_account_balance_cents(&pool, user_id, &cash_name, 1234).await?;
    assert_account_balance_cents(&pool, user_id, &bank_name, 0).await?;
    assert_account_rule_row(
        &pool,
        user_id,
        "工资卡识别",
        &bank_name,
        false,
        "OR={工资卡-",
        3,
    )
    .await?;

    let ambiguous_amount_bundle = json!({
        "schemaVersion": 1,
        "sections": {
            "accounts": [{
                "externalRef": format!("account:ambiguous-cash:{unique}"),
                "name": format!("泛型现金-{unique}"),
                "type": 1,
                "currency": "CNY",
                "balanceCents": 0
            }],
            "transactionCategories": [{
                "externalRef": format!("category:ambiguous-lunch:{unique}"),
                "type": 3,
                "mainCategory": format!("泛型餐饮-{unique}"),
                "subCategory": "午餐"
            }],
            "transactionTags": [{
                "externalRef": format!("tag:ambiguous:{unique}"),
                "name": format!("泛型标签-{unique}")
            }],
            "transactionTemplates": [{
                "name": "泛型金额模板",
                "type": 3,
                "categoryRef": format!("category:ambiguous-lunch:{unique}"),
                "sourceAccountRef": format!("account:ambiguous-cash:{unique}"),
                "amount_cents": 9999,
                "tagRefs": [format!("tag:ambiguous:{unique}")]
            }],
            "scheduledTransactions": [{
                "name": "非法金额计划",
                "type": 4,
                "categoryRef": format!("category:ambiguous-lunch:{unique}"),
                "sourceAccountRef": format!("account:ambiguous-cash:{unique}"),
                "sourceAmountCents": true
            }]
        }
    });
    let ambiguous_import =
        import_postgres_settings_bundle(&pool, &ambiguous_amount_bundle, user_id, false).await?;
    assert_eq!(
        ambiguous_import["sections"]["transactionTemplates"]["created"],
        1
    );
    assert_eq!(
        ambiguous_import["sections"]["scheduledTransactions"]["skipped"],
        1
    );
    assert_template_row(&pool, user_id, 1, "泛型金额模板", 0, 0, false).await?;
    let warnings = ambiguous_import["warnings"]
        .as_array()
        .expect("settings warnings");
    assert!(warnings.iter().any(|warning| warning
        .as_str()
        .unwrap_or_default()
        .contains("invalid sourceAmountCents")));

    let invalid_account_name = format!("坏金额账户-{unique}");
    let invalid_account_bundle = json!({
        "schemaVersion": 1,
        "sections": {
            "accounts": [{
                "externalRef": format!("account:invalid-cents:{unique}"),
                "name": invalid_account_name,
                "type": 1,
                "currency": "CNY",
                "balanceCents": "12.34",
                "initialBalanceCents": true
            }]
        }
    });
    let invalid_account_import =
        import_postgres_settings_bundle(&pool, &invalid_account_bundle, user_id, false).await?;
    assert_eq!(invalid_account_import["sections"]["accounts"]["skipped"], 1);
    let invalid_account_warnings = invalid_account_import["warnings"]
        .as_array()
        .expect("invalid account warnings");
    assert!(invalid_account_warnings.iter().any(|warning| warning
        .as_str()
        .unwrap_or_default()
        .contains("invalid balanceCents")));
    assert!(invalid_account_warnings.iter().any(|warning| warning
        .as_str()
        .unwrap_or_default()
        .contains("invalid initialBalanceCents")));
    let invalid_account_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM accounts WHERE user_id = $1 AND name = $2",
    )
    .bind(user_id)
    .bind(&invalid_account_name)
    .fetch_one(&pool)
    .await?;
    assert_eq!(invalid_account_count, 0);

    let name_only_rule = json!({
        "schemaVersion": 1,
        "sections": {
            "accountRecognitionRules": [{
                "name": "工资卡名称回退识别",
                "accountName": bank_name.clone(),
                "ruleExpression": format!("OR={{名称回退-{unique}}}"),
                "priority": 4,
                "regexEnabled": true,
                "enabled": true
            }]
        }
    });
    let name_only_import =
        import_postgres_settings_bundle(&pool, &name_only_rule, user_id, false).await?;
    assert_eq!(
        name_only_import["sections"]["accountRecognitionRules"]["created"],
        1
    );
    assert_account_rule_row(
        &pool,
        user_id,
        "工资卡名称回退识别",
        &bank_name,
        true,
        "OR={名称回退-",
        4,
    )
    .await?;

    let updated_bundle = settings_bundle_payload(unique, 1999, 300000, true);
    let updated = import_postgres_settings_bundle(&pool, &updated_bundle, user_id, false).await?;
    assert_eq!(updated["sections"]["transactionTemplates"]["updated"], 1);
    assert_eq!(updated["sections"]["scheduledTransactions"]["updated"], 1);
    assert_eq!(updated["sections"]["accountRecognitionRules"]["updated"], 1);
    assert_no_unsupported_template_warning(&updated);
    assert_deprecated_scope_warning(&updated);

    assert_template_row(&pool, user_id, 1, "午餐模板", 1999, 0, true).await?;
    assert_template_row(&pool, user_id, 2, "房租计划", 300000, 300000, true).await?;
    assert_account_balance_cents(&pool, user_id, &cash_name, 1234).await?;
    assert_account_rule_row(
        &pool,
        user_id,
        "工资卡识别",
        &bank_name,
        false,
        "OR={工资卡-",
        3,
    )
    .await?;

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;

    Ok(())
}

async fn assert_account_balance_cents(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    name: &str,
    expected_balance_cents: i64,
) -> Result<(), Box<dyn Error>> {
    let balance_cents: i64 = sqlx::query_scalar(
        r#"
        SELECT balance_cents::BIGINT
        FROM accounts
        WHERE user_id = $1 AND name = $2
        "#,
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?;
    assert_eq!(balance_cents, expected_balance_cents);
    Ok(())
}

fn settings_bundle_payload(
    unique: i64,
    template_amount: i64,
    scheduled_amount: i64,
    hidden: bool,
) -> Value {
    json!({
        "schemaVersion": 1,
        "sections": {
            "accounts": [
                {
                    "externalRef": format!("account:cash:{unique}"),
                    "name": format!("现金-{unique}"),
                    "type": 1,
                    "currency": "CNY",
                    "balanceCents": 1234
                },
                {
                    "externalRef": format!("account:bank:{unique}"),
                    "name": format!("工资卡-{unique}"),
                    "type": 1,
                    "currency": "CNY",
                    "balanceCents": 0
                }
            ],
            "transactionCategories": [{
                "externalRef": format!("category:lunch:{unique}"),
                "type": 3,
                "mainCategory": format!("餐饮-{unique}"),
                "subCategory": "午餐"
            }],
            "transactionTags": [{
                "externalRef": format!("tag:project:{unique}"),
                "name": format!("项目-{unique}")
            }],
            "transactionTemplates": [{
                "name": "午餐模板",
                "type": 3,
                "categoryRef": format!("category:lunch:{unique}"),
                "sourceAccountRef": format!("account:cash:{unique}"),
                "sourceAmountCents": template_amount,
                "destinationAmountCents": 0,
                "hideAmount": false,
                "tagRefs": [format!("tag:project:{unique}")],
                "comment": "工作日午餐",
                "displayOrder": 4,
                "hidden": hidden
            }],
            "scheduledTransactions": [{
                "name": "房租计划",
                "type": 4,
                "categoryRef": format!("category:lunch:{unique}"),
                "sourceAccountRef": format!("account:cash:{unique}"),
                "destinationAccountRef": format!("account:bank:{unique}"),
                "sourceAmountCents": scheduled_amount,
                "destinationAmountCents": scheduled_amount,
                "hideAmount": false,
                "tagRefs": [format!("tag:project:{unique}")],
                "scheduledFrequencyType": 3,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "enabled": true,
                "autoCreate": true,
                "displayOrder": 5,
                "hidden": hidden
            }],
            "accountRecognitionRules": [{
                "name": "工资卡识别",
                "accountRef": format!("account:bank:{unique}"),
                "ruleExpression": format!("OR={{工资卡-{unique}}}"),
                "priority": 3,
                "regexEnabled": false,
                "enabled": true,
                "accountRoleScope": "source",
                "transactionTypeScope": "expense",
                "fieldScope": ["payment_method"]
            }]
        }
    })
}

fn assert_no_unsupported_template_warning(result: &Value) {
    let warnings = result
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(warnings.iter().all(|warning| {
        !warning
            .as_str()
            .unwrap_or_default()
            .contains("not supported")
    }));
}

fn assert_deprecated_scope_warning(result: &Value) {
    let warnings = result
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(warnings.iter().any(|warning| {
        warning
            .as_str()
            .unwrap_or_default()
            .contains("Ignored deprecated account rule scope fields")
    }));
}

async fn assert_account_rule_scope_columns_absent(
    pool: &bill_analyser_db::PostgresPool,
) -> Result<(), Box<dyn Error>> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM information_schema.columns
        WHERE table_name = 'account_rules'
          AND column_name IN ('account_role_scope', 'transaction_type_scope', 'field_scope')
        "#,
    )
    .fetch_one(pool)
    .await?;
    assert_eq!(count, 0);
    Ok(())
}

async fn assert_account_rule_row(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    name: &str,
    expected_account_name: &str,
    regex_enabled: bool,
    expected_expression_prefix: &str,
    expected_priority: i32,
) -> Result<(), Box<dyn Error>> {
    let row = sqlx::query(
        r#"
        SELECT ar.name, ar.rule_expression, ar.regex_enabled, ar.enabled, ar.priority,
            accounts.name AS account_name
        FROM account_rules ar
        JOIN accounts ON accounts.id = ar.account_id AND accounts.user_id = ar.user_id
        WHERE ar.user_id = $1 AND ar.name = $2
        "#,
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.try_get::<String, _>("name")?, name);
    assert_eq!(
        row.try_get::<String, _>("account_name")?,
        expected_account_name
    );
    assert!(row
        .try_get::<Value, _>("rule_expression")?
        .get("expression")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .starts_with(expected_expression_prefix));
    assert_eq!(row.try_get::<bool, _>("regex_enabled")?, regex_enabled);
    assert!(row.try_get::<bool, _>("enabled")?);
    assert_eq!(row.try_get::<i32, _>("priority")?, expected_priority);
    Ok(())
}

async fn assert_template_row(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    template_type: i32,
    name: &str,
    source_amount_minor_units: i64,
    destination_amount_minor_units: i64,
    hidden: bool,
) -> Result<(), Box<dyn Error>> {
    let row = sqlx::query(
        r#"
        SELECT template_type, name, category_id, source_account_id, destination_account_id,
            source_amount_minor_units::BIGINT AS source_amount_minor_units,
            destination_amount_minor_units::BIGINT AS destination_amount_minor_units,
            tag_ids, scheduled_frequency_type, scheduled_frequency, scheduled_start_date,
            scheduled_next_date, enabled, auto_create, display_order, hidden
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = $2 AND name = $3
        "#,
    )
    .bind(user_id)
    .bind(template_type)
    .bind(name)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.try_get::<i32, _>("template_type")?, template_type);
    assert_eq!(
        row.try_get::<i64, _>("source_amount_minor_units")?,
        source_amount_minor_units
    );
    assert_eq!(
        row.try_get::<i64, _>("destination_amount_minor_units")?,
        destination_amount_minor_units
    );
    assert!(!row
        .try_get::<Option<String>, _>("category_id")?
        .unwrap_or_default()
        .is_empty());
    assert_ne!(
        row.try_get::<Option<String>, _>("source_account_id")?
            .unwrap_or_default(),
        "0"
    );
    assert!(row
        .try_get::<Value, _>("tag_ids")?
        .as_array()
        .is_some_and(|items| items.len() == 1));
    assert_eq!(row.try_get::<bool, _>("hidden")?, hidden);

    if template_type == 2 {
        assert_eq!(
            row.try_get::<Option<i32>, _>("scheduled_frequency_type")?,
            Some(3)
        );
        assert_eq!(
            row.try_get::<Option<String>, _>("scheduled_start_date")?
                .as_deref(),
            Some("2026-06-01")
        );
        assert_eq!(
            row.try_get::<Option<String>, _>("scheduled_next_date")?
                .as_deref(),
            Some("2026-06-01")
        );
        assert!(row.try_get::<bool, _>("enabled")?);
        assert!(row.try_get::<bool, _>("auto_create")?);
        assert_ne!(
            row.try_get::<Option<String>, _>("destination_account_id")?
                .unwrap_or_default(),
            "0"
        );
    } else {
        assert!(row
            .try_get::<Option<i32>, _>("scheduled_frequency_type")?
            .is_none());
    }

    Ok(())
}
