use std::{env, error::Error, str::FromStr};

use bill_analyser_db::{
    accept_postgres_llm_candidate, count_postgres_llm_candidates, create_postgres_llm_candidate,
    create_postgres_llm_config, effective_postgres_llm_config_from_saved, get_postgres_llm_config,
    has_postgres_llm_account_rule_candidate_duplicate, has_postgres_llm_rule_candidate_duplicate,
    list_postgres_llm_candidates, list_postgres_llm_configs, run_postgres_migrations,
    update_postgres_llm_config, LlmCandidateDraft, LlmConfigDraft, LlmConfigUpdate, PostgresPool,
};
use serde_json::json;
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};

#[tokio::test]
async fn llm_runtime_tables_round_trip_after_postgres_migrations() -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        eprintln!(
            "skipping PostgreSQL LLM runtime smoke: BILL_ANALYSER_TEST_POSTGRES_URL is not set"
        );
        return Ok(());
    };
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("llm_runtime_test_{unique}");
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
    assert_llm_runtime_table_exists(&pool, "llm_configs").await?;
    assert_llm_runtime_table_exists(&pool, "llm_candidates").await?;
    assert_llm_runtime_table_exists(&pool, "llm_memory_events").await?;
    assert_llm_runtime_table_exists(&pool, "import_annotation_samples").await?;

    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("llm-runtime-{unique}"))
            .bind(format!("llm-runtime-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;

    let inactive_default = effective_postgres_llm_config_from_saved(&pool, user_id).await?;
    assert_eq!(inactive_default["enabled"], false);

    let config = create_postgres_llm_config(
        &pool,
        user_id,
        &LlmConfigDraft {
            name: format!("primary-{unique}"),
            provider: "openai".to_string(),
            model: "gpt-test".to_string(),
            api_key: "secret-token".to_string(),
            base_url: "https://example.test/v1".to_string(),
            credential_config: json!({}),
            advanced_settings: json!({"temperature": 0.1}),
            is_active: true,
        },
    )
    .await?;
    assert_eq!(config["is_active"], true);
    assert_eq!(list_postgres_llm_configs(&pool, user_id).await?.len(), 1);
    let config_id = config["id"].as_i64().expect("saved config id");
    assert!(get_postgres_llm_config(&pool, config_id, user_id)
        .await?
        .is_some());
    assert!(get_postgres_llm_config(&pool, config_id, user_id + 1)
        .await?
        .is_none());

    let effective = effective_postgres_llm_config_from_saved(&pool, user_id).await?;
    assert_eq!(effective["enabled"], true);
    assert_eq!(effective["provider_config"]["model"], "gpt-test");
    assert_eq!(effective["provider_config"]["api_key"], "secret-token");

    let edited = update_postgres_llm_config(
        &pool,
        config_id,
        user_id,
        &LlmConfigUpdate {
            model: Some("gpt-edited".to_string()),
            advanced_settings: Some(json!({"api_protocol": "responses"})),
            ..LlmConfigUpdate::default()
        },
    )
    .await?
    .expect("edited config");
    assert_eq!(edited["model"], "gpt-edited");
    assert_eq!(edited["advanced_settings"]["api_protocol"], "responses");
    let edited_effective = effective_postgres_llm_config_from_saved(&pool, user_id).await?;
    assert_eq!(
        edited_effective["provider_config"]["api_key"],
        "secret-token"
    );
    assert_eq!(
        edited_effective["advanced_settings"]["api_protocol"],
        "responses"
    );

    let candidate = create_postgres_llm_candidate(
        &pool,
        &LlmCandidateDraft {
            user_id,
            candidate_type: "classification".to_string(),
            source_bill_ids: vec![1, 2],
            suggested_main_category: "餐饮".to_string(),
            suggested_sub_category: "咖啡".to_string(),
            suggested_account_id: None,
            suggested_account_name: String::new(),
            suggested_rule_expression: String::new(),
            confidence: 0.82,
            llm_provider: "openai".to_string(),
            llm_model: "gpt-test".to_string(),
            llm_response_raw: "{\"ok\":true}".to_string(),
        },
    )
    .await?;
    assert_eq!(candidate["status"], "pending");

    let candidates = list_postgres_llm_candidates(
        &pool,
        user_id,
        Some("pending"),
        Some("classification"),
        20,
        0,
    )
    .await?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        count_postgres_llm_candidates(&pool, user_id, Some("pending"), Some("classification"))
            .await?,
        1
    );

    let category_id: i64 = sqlx::query(
        "INSERT INTO categories (user_id, name, category_type, path) VALUES ($1, $2, '3', $3) RETURNING id",
    )
    .bind(user_id)
    .bind("咖啡")
    .bind("餐饮/咖啡")
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let category_expression = "OR={咖啡,拿铁}";
    assert!(
        !has_postgres_llm_rule_candidate_duplicate(
            &pool,
            user_id,
            " 餐饮 ",
            " 咖啡 ",
            &format!(" {category_expression} "),
        )
        .await?
    );
    sqlx::query(
        "INSERT INTO category_rules (user_id, category_id, name, rule_expression, priority, enabled) VALUES ($1, $2, '咖啡规则', $3, 1, true)",
    )
    .bind(user_id)
    .bind(category_id)
    .bind(json!({"rule_expression": category_expression, "regex_enabled": false}))
    .execute(&pool)
    .await?;
    assert!(
        has_postgres_llm_rule_candidate_duplicate(
            &pool,
            user_id,
            " 餐饮 ",
            " 咖啡 ",
            &format!(" {category_expression} "),
        )
        .await?
    );
    assert!(
        !has_postgres_llm_rule_candidate_duplicate(
            &pool,
            user_id + 1,
            "餐饮",
            "咖啡",
            category_expression,
        )
        .await?
    );

    let pending_category_expression = "OR={下午茶}";
    create_postgres_llm_candidate(
        &pool,
        &LlmCandidateDraft {
            user_id,
            candidate_type: "rule_induction".to_string(),
            source_bill_ids: vec![3],
            suggested_main_category: "餐饮".to_string(),
            suggested_sub_category: "咖啡".to_string(),
            suggested_account_id: None,
            suggested_account_name: String::new(),
            suggested_rule_expression: pending_category_expression.to_string(),
            confidence: 0.8,
            llm_provider: "openai".to_string(),
            llm_model: "gpt-test".to_string(),
            llm_response_raw: "{\"pending_rule\":true}".to_string(),
        },
    )
    .await?;
    assert!(
        has_postgres_llm_rule_candidate_duplicate(
            &pool,
            user_id,
            "餐饮",
            "咖啡",
            pending_category_expression,
        )
        .await?
    );

    let account_id: i64 = sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type) VALUES ($1, $2, 'bank') RETURNING id",
    )
    .bind(user_id)
    .bind("工资卡")
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    assert!(
        !has_postgres_llm_account_rule_candidate_duplicate(
            &pool,
            user_id,
            account_id,
            "OR={工资,薪资}",
        )
        .await?
    );
    let account_candidate = create_postgres_llm_candidate(
        &pool,
        &LlmCandidateDraft {
            user_id,
            candidate_type: "account_rule_induction".to_string(),
            source_bill_ids: vec![9, 10],
            suggested_main_category: String::new(),
            suggested_sub_category: String::new(),
            suggested_account_id: Some(account_id),
            suggested_account_name: "工资卡".to_string(),
            suggested_rule_expression: "OR={工资,薪资}".to_string(),
            confidence: 0.9,
            llm_provider: "openai".to_string(),
            llm_model: "gpt-test".to_string(),
            llm_response_raw: "{\"target_account_id\":1}".to_string(),
        },
    )
    .await?;
    assert_eq!(account_candidate["suggested_account_id"], account_id);
    assert_eq!(account_candidate["suggested_account_name"], "工资卡");
    let accepted = accept_postgres_llm_candidate(
        &pool,
        account_candidate["id"]
            .as_i64()
            .expect("account candidate id"),
        user_id,
    )
    .await?
    .expect("accepted account candidate");
    assert!(accepted["created_account_rule_id"].as_i64().is_some());
    let account_rule_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM account_rules WHERE user_id = $1 AND account_id = $2",
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(account_rule_count, 1);
    assert!(
        has_postgres_llm_account_rule_candidate_duplicate(
            &pool,
            user_id,
            account_id,
            " OR={工资,薪资} ",
        )
        .await?
    );
    assert!(
        !has_postgres_llm_account_rule_candidate_duplicate(
            &pool,
            user_id + 1,
            account_id,
            "OR={工资,薪资}",
        )
        .await?
    );

    let pending_account_expression = "OR={银行卡入账}";
    create_postgres_llm_candidate(
        &pool,
        &LlmCandidateDraft {
            user_id,
            candidate_type: "account_rule_induction".to_string(),
            source_bill_ids: vec![11],
            suggested_main_category: String::new(),
            suggested_sub_category: String::new(),
            suggested_account_id: Some(account_id),
            suggested_account_name: "工资卡".to_string(),
            suggested_rule_expression: pending_account_expression.to_string(),
            confidence: 0.75,
            llm_provider: "openai".to_string(),
            llm_model: "gpt-test".to_string(),
            llm_response_raw: "{\"pending_account_rule\":true}".to_string(),
        },
    )
    .await?;
    assert!(
        has_postgres_llm_account_rule_candidate_duplicate(
            &pool,
            user_id,
            account_id,
            pending_account_expression,
        )
        .await?
    );

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;

    Ok(())
}

async fn assert_llm_runtime_table_exists(
    pool: &PostgresPool,
    table_name: &str,
) -> Result<(), Box<dyn Error>> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public'
              AND table_name = $1
        )
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;
    assert!(exists, "missing LLM runtime table {table_name}");
    Ok(())
}
