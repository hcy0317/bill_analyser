use std::error::Error;

use bill_analyser_db::{
    auth_postgres::seed_postgres_standard_daily_defaults_for_user,
    create_postgres_registered_user_with_defaults,
    taxonomy::{
        postgres_reads::{
            list_postgres_account_rules, list_postgres_accounts, list_postgres_categories,
            list_postgres_category_rules, list_postgres_templates,
        },
        settings_bundle::{export_taxonomy_sections, import_postgres_settings_bundle},
    },
    AuthLogDraft, PostgresPool, RegisterDefaultSeedPackage, RegisterUserDraft,
};
use serde_json::{json, Map, Value};

#[path = "postgres_test_support.rs"]
mod postgres_test_support;

#[tokio::test]
async fn registration_standard_daily_defaults_are_opt_in_idempotent_and_importable(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("registration_defaults").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let unique = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .abs();

    let opt_in_draft = register_draft(unique, "opt-in");
    let opt_in = create_postgres_registered_user_with_defaults(
        pool,
        &opt_in_draft,
        &[],
        RegisterDefaultSeedPackage::StandardDailyV1,
        &auth_log_draft(&opt_in_draft),
    )
    .await?;

    assert_eq!(
        opt_in.default_seed.package.as_deref(),
        Some(RegisterDefaultSeedPackage::STANDARD_DAILY_V1)
    );
    assert!(opt_in.default_seed.categories_created > 30);
    assert!(opt_in.default_seed.rules_created > 30);
    assert!(opt_in.default_seed.accounts_created > 0);
    assert_eq!(opt_in.default_seed.account_rules_created, 9);
    assert_eq!(opt_in.default_seed.rules_missing_targets, 0);

    assert_seeded_accounts_and_rules(pool, opt_in.user_id).await?;
    assert_eq!(
        legacy_minimal_category_rule_count(pool, opt_in.user_id).await?,
        0,
        "standard_daily_v1 opt-in should not run post-commit legacy defaults"
    );

    let repeated =
        seed_postgres_standard_daily_defaults_for_user(pool, opt_in.user_id, &opt_in_draft).await?;
    assert_eq!(repeated.categories_created, 0);
    assert_eq!(repeated.rules_created, 0);
    assert_eq!(repeated.accounts_created, 0);
    assert_eq!(repeated.account_rules_created, 0);
    assert!(repeated.categories_skipped > 30);
    assert!(repeated.rules_skipped > 30);
    assert!(repeated.accounts_skipped >= 9);
    assert_eq!(repeated.account_rules_skipped, 9);

    let opt_out_draft = register_draft(unique, "opt-out");
    let opt_out = create_postgres_registered_user_with_defaults(
        pool,
        &opt_out_draft,
        &[],
        RegisterDefaultSeedPackage::None,
        &auth_log_draft(&opt_out_draft),
    )
    .await?;
    assert!(opt_out.default_seed.package.is_none());
    assert_eq!(
        standard_daily_account_rule_count(pool, opt_out.user_id).await?,
        0
    );
    assert_eq!(
        standard_daily_category_rule_count(pool, opt_out.user_id).await?,
        0
    );
    assert!(
        legacy_minimal_category_rule_count(pool, opt_out.user_id).await? > 0,
        "opt-out registration keeps the legacy minimal defaults"
    );

    let bundle = export_seeded_settings_bundle(pool, opt_in.user_id).await?;
    let imported = import_postgres_settings_bundle(pool, &bundle, opt_out.user_id, false).await?;
    assert_eq!(imported["warnings"], json!([]));
    assert!(
        imported["sections"]["accounts"]["created"]
            .as_i64()
            .unwrap_or_default()
            + imported["sections"]["accounts"]["updated"]
                .as_i64()
                .unwrap_or_default()
            >= 9
    );
    assert_eq!(
        imported["sections"]["accountRecognitionRules"]["created"],
        json!(9)
    );
    assert_seeded_accounts_and_rules(pool, opt_out.user_id).await?;

    test_db.cleanup().await?;
    Ok(())
}

fn register_draft(unique: i64, suffix: &str) -> RegisterUserDraft {
    RegisterUserDraft {
        username: format!("registration-defaults-{suffix}-{unique}"),
        email: format!("registration-defaults-{suffix}-{unique}@example.test"),
        password_hash: "bcrypt-placeholder".to_string(),
        nickname: format!("注册默认包 {suffix}"),
        language: "zh-Hans".to_string(),
        default_currency: "CNY".to_string(),
        first_day_of_week: 1,
        email_verified: true,
        created_at: "2026-06-13T12:00:00".to_string(),
    }
}

fn auth_log_draft(draft: &RegisterUserDraft) -> AuthLogDraft {
    AuthLogDraft {
        user_id: None,
        username: draft.username.clone(),
        event_type: "register".to_string(),
        ip_address: "127.0.0.1".to_string(),
        user_agent: "registration-defaults-postgres-test".to_string(),
        success: true,
        error_message: None,
        metadata: None,
        created_at: draft.created_at.clone(),
    }
}

async fn assert_seeded_accounts_and_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(standard_daily_account_rule_count(pool, user_id).await?, 9);
    assert!(standard_daily_category_rule_count(pool, user_id).await? > 30);

    let account_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM accounts
        WHERE user_id = $1
          AND name IN (
              '现金', '银行卡', '信用卡', '微信', '支付宝', '花呗/消费信贷',
              '理财/投资账户', '应收款', '应付款/借款'
          )
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(account_count, 9);

    let category_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM categories
        WHERE user_id = $1
          AND path IN ('餐饮食品/外卖', '居住水电/水电燃气', '医疗健康/药品就医', '投资理财/金融理财')
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(category_count, 4);

    let wechat_rule_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM account_rules ar
        JOIN accounts a ON a.id = ar.account_id AND a.user_id = ar.user_id
        WHERE ar.user_id = $1
          AND ar.source = 'standard_daily_v1'
          AND ar.source_key = 'standard_daily_v1:account:wechat'
          AND a.name = '微信'
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(wechat_rule_count, 1);

    Ok(())
}

async fn legacy_minimal_category_rule_count(
    pool: &PostgresPool,
    user_id: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM category_rules
        WHERE user_id = $1
          AND name LIKE 'default:%'
          AND name NOT LIKE 'default:standard_daily_v1:%'
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

async fn standard_daily_account_rule_count(
    pool: &PostgresPool,
    user_id: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM account_rules
        WHERE user_id = $1 AND source = 'standard_daily_v1'
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

async fn standard_daily_category_rule_count(
    pool: &PostgresPool,
    user_id: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM category_rules
        WHERE user_id = $1 AND name LIKE 'default:standard_daily_v1:%'
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

async fn export_seeded_settings_bundle(
    pool: &PostgresPool,
    user_id: i64,
) -> Result<Value, Box<dyn Error>> {
    let accounts = list_postgres_accounts(pool, user_id).await?;
    let categories = list_postgres_categories(pool, user_id).await?;
    let templates = list_postgres_templates(pool, user_id, Some(1)).await?;
    let scheduled = list_postgres_templates(pool, user_id, Some(2)).await?;
    let taxonomy_sections = export_taxonomy_sections(&json!({
        "accounts": map_records_to_array(accounts),
        "categories": map_records_to_array(categories),
        "tags": [],
        "templates": map_records_to_array(templates),
        "scheduled": map_records_to_array(scheduled),
    }))?;
    let mut sections = taxonomy_sections
        .as_object()
        .cloned()
        .expect("taxonomy sections object");

    let category_rules = list_postgres_category_rules(pool, user_id, None, false).await?;
    sections.insert(
        "categoryRecognitionRules".to_string(),
        Value::Array(
            category_rules
                .iter()
                .filter(|rule| {
                    value_string(rule.get("name")).starts_with("default:standard_daily_v1:")
                })
                .map(export_category_rule)
                .collect(),
        ),
    );

    let account_rules = list_postgres_account_rules(pool, user_id, None, false).await?;
    sections.insert(
        "accountRecognitionRules".to_string(),
        Value::Array(
            account_rules
                .iter()
                .filter(|rule| value_string(rule.get("source")) == "standard_daily_v1")
                .map(export_account_rule)
                .collect(),
        ),
    );

    Ok(json!({
        "schemaVersion": 1,
        "sections": Value::Object(sections),
    }))
}

fn map_records_to_array(records: Vec<Map<String, Value>>) -> Value {
    Value::Array(records.into_iter().map(Value::Object).collect())
}

fn export_category_rule(rule: &Map<String, Value>) -> Value {
    let category_id = value_i64(rule.get("category_id"));
    json!({
        "externalRef": format!("categoryRule:{}", value_string(rule.get("id"))),
        "categoryRef": format!("category:{category_id}"),
        "mainCategory": value_string(rule.get("main_category")),
        "subCategory": value_string(rule.get("sub_category")),
        "name": value_string(rule.get("name")),
        "priority": value_i64(rule.get("priority")),
        "ruleExpression": value_string(rule.get("rule_expression")),
        "regexEnabled": value_i64(rule.get("regex_enabled")) != 0,
        "enabled": value_i64(rule.get("enabled")) != 0,
    })
}

fn export_account_rule(rule: &Map<String, Value>) -> Value {
    let account_id = value_i64(rule.get("account_id"));
    json!({
        "externalRef": format!("accountRule:{}", value_string(rule.get("id"))),
        "accountRef": format!("account:{account_id}"),
        "accountName": value_string(rule.get("account_name")),
        "name": value_string(rule.get("name")),
        "priority": value_i64(rule.get("priority")),
        "ruleExpression": value_string(rule.get("rule_expression")),
        "regexEnabled": value_i64(rule.get("regex_enabled")) != 0,
        "enabled": value_i64(rule.get("enabled")) != 0,
        "source": value_string(rule.get("source")),
        "sourceKey": rule.get("source_key").cloned().unwrap_or(Value::Null),
    })
}

fn value_i64(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or_default(),
        Some(Value::String(text)) => text.parse::<i64>().unwrap_or_default(),
        Some(Value::Bool(value)) => i64::from(*value),
        _ => 0,
    }
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}
