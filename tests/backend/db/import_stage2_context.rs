use std::{env, error::Error};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    load_import_account_catalog_records, load_import_category_catalog_records,
    load_import_stage2_context, load_import_stage2_learning_lifecycle_views,
};
use serde_json::json;
use sqlx::Row;

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for Stage 2 context tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for Stage 2 context tests",
            )
            .into()
        })
}

#[tokio::test]
async fn real_postgres_stage2_context_is_user_scoped_ordered_and_fresh_per_batch(
) -> Result<(), Box<dyn Error>> {
    let database = strict_isolated_postgres_database("import_stage2_context").await?;
    let first_user = insert_user(&database.pool, "stage2-first").await?;
    let second_user = insert_user(&database.pool, "stage2-second").await?;

    let expense_category: i64 = sqlx::query(
        "INSERT INTO categories (user_id,name,category_type,path,display_order) VALUES ($1,'餐饮','3','生活/餐饮',20) RETURNING id",
    )
    .bind(first_user)
    .fetch_one(&database.pool)
    .await?
    .try_get("id")?;
    let cash_transfer_category: i64 = sqlx::query(
        "INSERT INTO categories (user_id,name,category_type,path,display_order) VALUES ($1,'现金转账','4','转账/现金',10) RETURNING id",
    )
    .bind(first_user)
    .fetch_one(&database.pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        "INSERT INTO categories (user_id,name,category_type,path,display_order,is_active) VALUES ($1,'停用分类','3','停用/分类',1,false)",
    )
    .bind(first_user)
    .execute(&database.pool)
    .await?;
    sqlx::query(
        "INSERT INTO categories (user_id,name,category_type,path,display_order) VALUES ($1,'他人分类','3','他人/分类',1)",
    )
    .bind(second_user)
    .execute(&database.pool)
    .await?;

    let account_id: i64 = sqlx::query(
        "INSERT INTO accounts (user_id,name,account_type,display_order) VALUES ($1,'工资卡','asset',2) RETURNING id",
    )
    .bind(first_user)
    .fetch_one(&database.pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        "INSERT INTO accounts (user_id,name,account_type,display_order,is_active) VALUES ($1,'停用账户','asset',1,false)",
    )
    .bind(first_user)
    .execute(&database.pool)
    .await?;
    sqlx::query(
        "INSERT INTO accounts (user_id,name,account_type,display_order) VALUES ($1,'他人账户','asset',1)",
    )
    .bind(second_user)
    .execute(&database.pool)
    .await?;

    sqlx::query(
        "INSERT INTO category_rules (user_id,category_id,name,rule_expression,priority,enabled) VALUES ($1,$2,'餐饮规则',$3,4,true)",
    )
    .bind(first_user)
    .bind(expense_category)
    .bind(json!({"expression":"OR={餐厅}","regex_enabled":false}))
    .execute(&database.pool)
    .await?;
    sqlx::query(
        "INSERT INTO account_rules (user_id,account_id,name,rule_expression,regex_enabled,priority,enabled) VALUES ($1,$2,'工资卡规则',$3,false,5,true)",
    )
    .bind(first_user)
    .bind(account_id)
    .bind(json!({"expression":"OR={工资}"}))
    .execute(&database.pool)
    .await?;
    sqlx::query(
        "INSERT INTO import_learning_lifecycle (user_id,recommendation_key,recommendation_type,status,metadata) VALUES ($1,'stage2-learning','expense','accepted',$2)",
    )
    .bind(first_user)
    .bind(json!({"parser_id":"stage2-fixture"}))
    .execute(&database.pool)
    .await?;
    sqlx::query(
        "INSERT INTO transaction_templates (user_id,template_type,name,transaction_type,metadata,display_order) VALUES ($1,2,'月度订阅','3',$2,7)",
    )
    .bind(first_user)
    .bind(json!({"counterparty":"订阅商户"}))
    .execute(&database.pool)
    .await?;

    let catalog_categories =
        load_import_category_catalog_records(&database.pool, first_user).await?;
    assert_eq!(
        catalog_categories
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["现金转账", "餐饮"]
    );
    assert!(catalog_categories
        .iter()
        .all(|row| row.name != "停用分类" && row.name != "他人分类"));

    let catalog_accounts = load_import_account_catalog_records(&database.pool, first_user).await?;
    assert_eq!(catalog_accounts.len(), 1);
    assert_eq!(catalog_accounts[0].id, account_id);
    assert_eq!(catalog_accounts[0].name, "工资卡");

    let canonical_user = UserId::new(first_user as u64).expect("positive fixture user");
    let first = load_import_stage2_context(&database.pool, canonical_user).await?;
    assert_eq!(first.user_id, first_user);
    assert_eq!(
        first
            .categories
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["现金转账", "餐饮"]
    );
    assert_eq!(
        first.cash_transfer_category_id,
        Some(cash_transfer_category)
    );
    assert_eq!(first.accounts.len(), 1);
    assert_eq!(first.accounts[0].name, "工资卡");
    assert_eq!(first.category_rules.len(), 1);
    assert_eq!(first.account_rules.len(), 1);
    assert_eq!(first.learning_rules.len(), 1);
    assert_eq!(first.recurring_templates.len(), 1);

    sqlx::query("UPDATE categories SET name='餐饮更新', path='生活/餐饮更新' WHERE id=$1")
        .bind(expense_category)
        .execute(&database.pool)
        .await?;
    let second = load_import_stage2_context(&database.pool, canonical_user).await?;
    assert_eq!(first.categories[1].name, "餐饮");
    assert_eq!(second.categories[1].name, "餐饮更新");
    assert!(second
        .categories
        .iter()
        .all(|row| !row.name.contains("他人") && !row.name.contains("停用")));

    database.cleanup().await
}

#[tokio::test]
async fn real_postgres_stage2_learning_lifecycles_are_loaded_in_one_user_scoped_batch(
) -> Result<(), Box<dyn Error>> {
    let database = strict_isolated_postgres_database("import_stage2_learning_batch").await?;
    let first_user = insert_user(&database.pool, "stage2-learning-first").await?;
    let second_user = insert_user(&database.pool, "stage2-learning-second").await?;

    for (key, status, accepted, rejected, auto_applied, auto_apply, suppressed) in [
        ("accepted-key", "accepted", 3, 0, 0, false, false),
        ("pending-key", "pending", 0, 0, 0, false, false),
        ("suppressed-key", "auto_applied", 4, 1, 2, true, true),
    ] {
        sqlx::query(
            r#"
            INSERT INTO import_learning_lifecycle (
                user_id, recommendation_key, recommendation_type, status,
                accepted_count, rejected_count, auto_applied_count,
                auto_apply_enabled, suppressed_until
            ) VALUES ($1,$2,'expense',$3,$4,$5,$6,$7,
                      CASE WHEN $8 THEN now() + interval '1 day' ELSE NULL END)
            "#,
        )
        .bind(first_user)
        .bind(key)
        .bind(status)
        .bind(accepted)
        .bind(rejected)
        .bind(auto_applied)
        .bind(auto_apply)
        .bind(suppressed)
        .execute(&database.pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO import_learning_lifecycle (user_id,recommendation_key,recommendation_type,status) VALUES ($1,'accepted-key','income','accepted')",
    )
    .bind(second_user)
    .execute(&database.pool)
    .await?;

    let user_id = UserId::new(first_user as u64).expect("positive fixture user");
    let keys = vec![
        "suppressed-key".to_string(),
        "missing-key".to_string(),
        "accepted-key".to_string(),
        "pending-key".to_string(),
        "accepted-key".to_string(),
    ];
    let views = load_import_stage2_learning_lifecycle_views(&database.pool, user_id, &keys).await?;
    assert_eq!(
        views
            .iter()
            .map(|view| view.recommendation_key.as_str())
            .collect::<Vec<_>>(),
        vec!["accepted-key", "pending-key", "suppressed-key"]
    );
    assert_eq!(views[0].status, "accepted");
    assert_eq!(views[0].signal_state, "yellow");
    assert_eq!(views[0].accepted_count, 3);
    assert!(!views[0].auto_apply_enabled);
    assert_eq!(views[1].status, "pending");
    assert_eq!(views[1].signal_state, "yellow");
    assert_eq!(views[2].status, "auto_applied");
    assert_eq!(views[2].signal_state, "green");
    assert_eq!(views[2].accepted_count, 4);
    assert_eq!(views[2].rejected_count, 1);
    assert_eq!(views[2].auto_applied_count, 2);
    assert!(views[2].auto_apply_enabled);
    assert!(views[2].suppressed);

    assert!(
        load_import_stage2_learning_lifecycle_views(&database.pool, user_id, &[])
            .await?
            .is_empty()
    );
    database.cleanup().await
}

async fn insert_user(
    pool: &bill_analyser_db::PostgresPool,
    prefix: &str,
) -> Result<i64, sqlx::Error> {
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    sqlx::query("INSERT INTO users (username,email) VALUES ($1,$2) RETURNING id")
        .bind(format!("{prefix}-{unique}"))
        .bind(format!("{prefix}-{unique}@example.test"))
        .fetch_one(pool)
        .await?
        .try_get("id")
}
