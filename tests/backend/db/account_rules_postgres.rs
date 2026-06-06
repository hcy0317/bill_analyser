use std::{env, error::Error, fs, str::FromStr};

use bill_analyser_db::{
    create_postgres_account, postgres_migrations_dir, run_postgres_migrations,
    taxonomy::postgres_reads::{
        create_postgres_account_rule, list_postgres_account_rules, update_postgres_account_rule,
    },
};
use serde_json::json;
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};

#[tokio::test]
async fn account_rule_repository_ignores_deprecated_scope_payload_without_columns(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("account_rule_repo_test_{unique}");
    let base_options = PgConnectOptions::from_str(&postgres_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.clone().database("postgres"))
        .await?;
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{}""#, test_db).as_str())
        .await?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.database(&test_db))
        .await?;
    run_postgres_migrations(&pool).await?;

    assert_account_rule_scope_columns_absent(&pool).await?;

    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("account-rule-repo-{unique}"))
            .bind(format!("account-rule-repo-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let account_id = create_postgres_account(
        &pool,
        &json!({
            "name": format!("工资卡-{unique}"),
            "type": "1",
            "currency": "CNY",
            "balance": 0
        }),
        user_id,
    )
    .await?;

    let rule_id = create_postgres_account_rule(
        &pool,
        &json!({
            "account_id": account_id,
            "name": "工资卡识别",
            "rule_expression": format!("OR={{工资卡-{unique}}}"),
            "regex_enabled": false,
            "enabled": true,
            "priority": 5,
            "source": "manual",
            "account_role_scope": "destination",
            "transaction_type_scope": "income",
            "field_scope": ["parser"]
        }),
        user_id,
    )
    .await?
    .expect("created account rule");

    let rules = list_postgres_account_rules(&pool, user_id, None, false).await?;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["id"], rule_id);
    assert_eq!(
        rules[0]["rule_expression"],
        format!("OR={{工资卡-{unique}}}")
    );
    assert!(rules[0].get("account_role_scope").is_none());
    assert!(rules[0].get("transaction_type_scope").is_none());
    assert!(rules[0].get("field_scope").is_none());

    let updated = update_postgres_account_rule(
        &pool,
        rule_id,
        &json!({
            "priority": 2,
            "id": rule_id,
            "user_id": user_id,
            "userId": user_id,
            "created_at": "ignored",
            "createdAt": "ignored",
            "updated_at": "ignored",
            "updatedAt": "ignored",
            "applied_count": 1,
            "appliedCount": 1,
            "last_applied_at": "ignored",
            "lastAppliedAt": "ignored",
            "match_count": 9,
            "matchCount": 9,
            "last_matched_at": "ignored",
            "lastMatchedAt": "ignored",
            "source_key": "ignored",
            "sourceKey": "ignored",
            "account_name": "ignored",
            "accountName": "ignored",
            "account_type": "ignored",
            "accountType": "ignored",
            "account_hidden": true,
            "accountHidden": true,
            "account_role_scope": "source",
            "accountRoleScope": "destination",
            "transaction_type_scope": "expense",
            "transactionTypeScope": "income",
            "field_scope": ["counterparty"],
            "fieldScope": ["parser"]
        }),
        user_id,
    )
    .await?;
    assert!(updated);

    let rules = list_postgres_account_rules(&pool, user_id, Some(account_id), false).await?;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["priority"], 2);
    assert!(rules[0].get("account_role_scope").is_none());
    assert!(rules[0].get("transaction_type_scope").is_none());
    assert!(rules[0].get("field_scope").is_none());

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;

    Ok(())
}

#[tokio::test]
async fn account_rule_scope_cleanup_migrates_existing_database_with_original_initial_schema(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("account_rule_scope_migration_test_{unique}");
    let base_options = PgConnectOptions::from_str(&postgres_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.clone().database("postgres"))
        .await?;
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{}""#, test_db).as_str())
        .await?;

    let old_migrations_dir = env::temp_dir().join(format!("bill-analyser-old-migrations-{unique}"));
    fs::create_dir_all(&old_migrations_dir)?;
    for entry in fs::read_dir(postgres_migrations_dir())? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.ends_with(".sql") && !file_name.starts_with("0012_") {
            fs::copy(entry.path(), old_migrations_dir.join(file_name.as_ref()))?;
        }
    }

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.database(&test_db))
        .await?;

    let old_migrator = sqlx::migrate::Migrator::new(old_migrations_dir.clone()).await?;
    old_migrator.run(&pool).await?;
    assert_account_rule_scope_columns_present(&pool).await?;
    assert_index_present(&pool, "idx_account_rules_user_enabled_type_priority").await?;

    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("account-rule-migration-{unique}"))
            .bind(format!("account-rule-migration-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let account_id = create_postgres_account(
        &pool,
        &json!({
            "name": format!("迁移账户-{unique}"),
            "type": "1",
            "currency": "CNY",
            "balance": 0
        }),
        user_id,
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO account_rules (
            user_id,
            account_id,
            name,
            account_role_scope,
            transaction_type_scope,
            field_scope,
            rule_expression,
            regex_enabled,
            enabled,
            priority
        ) VALUES ($1, $2, $3, 'source', 'expense', '["summary"]'::jsonb, '{}'::jsonb, false, true, 7)
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(format!("旧 scope 规则-{unique}"))
    .execute(&pool)
    .await?;
    let before_count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM account_rules")
        .fetch_one(&pool)
        .await?;

    run_postgres_migrations(&pool).await?;

    assert_account_rule_scope_columns_absent(&pool).await?;
    assert_index_present(&pool, "idx_account_rules_user_enabled_priority").await?;
    let after_count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM account_rules")
        .fetch_one(&pool)
        .await?;
    assert_eq!(after_count, before_count);

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;
    fs::remove_dir_all(&old_migrations_dir)?;

    Ok(())
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

async fn assert_account_rule_scope_columns_present(
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
    assert_eq!(count, 3);
    Ok(())
}

async fn assert_index_present(
    pool: &bill_analyser_db::PostgresPool,
    index_name: &str,
) -> Result<(), Box<dyn Error>> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM pg_indexes
            WHERE schemaname = 'public'
              AND indexname = $1
        )
        "#,
    )
    .bind(index_name)
    .fetch_one(pool)
    .await?;
    assert!(exists, "missing index {index_name}");
    Ok(())
}
