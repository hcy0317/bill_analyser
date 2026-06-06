use std::{env, error::Error, str::FromStr};

use bill_analyser_db::{
    create_postgres_category, list_postgres_category_rules, run_postgres_migrations,
};
use serde_json::json;
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};

#[tokio::test]
async fn category_rule_list_skips_legacy_only_empty_projection_when_postgres_available(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("category_rule_list_test_{unique}");
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

    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("category-rule-list-{unique}"))
            .bind(format!("category-rule-list-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let category_id = create_postgres_category(
        &pool,
        &json!({
            "main_category": format!("餐饮-{unique}"),
            "type": 3
        }),
        user_id,
    )
    .await?
    .expect("category id");

    sqlx::query(
        r#"
        INSERT INTO category_rules (user_id, category_id, name, rule_expression, priority, enabled)
        VALUES
            ($1, $2, 'current expression', '{"expression":"OR={午餐}","regex_enabled":false}'::jsonb, 1, true),
            ($1, $2, 'legacy only', '{"legacy_expression":"OR={旧规则}"}'::jsonb, 2, true)
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .execute(&pool)
    .await?;

    let rules = list_postgres_category_rules(&pool, user_id, None, false).await?;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["name"], "current expression");
    assert_eq!(rules[0]["rule_expression"], "OR={午餐}");

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;

    Ok(())
}
