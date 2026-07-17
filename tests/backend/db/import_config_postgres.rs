mod postgres_test_support;

use std::error::Error;

use bill_analyser_core::{suggest_import_config, ImportConfigDraft, UserId};
use bill_analyser_db::{
    delete_postgres_import_config, list_postgres_import_configs, match_postgres_import_config,
    postgres_migration_manifest, postgres_migrations_dir, save_postgres_import_config,
    ImportConfigRepositoryError,
};
use postgres_test_support::isolated_postgres_database;
use serde_json::json;
use sqlx::Row;

#[tokio::test]
async fn closed_pool_maps_to_repository_database_error() -> Result<(), Box<dyn Error>> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://bill:secret@127.0.0.1:1/bill_analyser")?;
    pool.close().await;
    let result = list_postgres_import_configs(&pool, UserId::new(1)?, "csv").await;
    assert!(matches!(
        result,
        Err(ImportConfigRepositoryError::Database(_))
    ));
    Ok(())
}

#[test]
fn import_config_migration_freezes_jsonb_and_uniqueness_contracts() {
    let descriptor = postgres_migration_manifest()
        .iter()
        .find(|descriptor| descriptor.version == 22)
        .expect("import config migration descriptor");
    assert_eq!(descriptor.file_name, "0022_import_configs.sql");
    let sql = std::fs::read_to_string(postgres_migrations_dir().join(descriptor.file_name))
        .expect("import config migration");
    for column in [
        "field_mappings JSONB",
        "sample_headers JSONB",
        "custom_rules JSONB",
    ] {
        assert!(sql.contains(column), "missing JSONB contract: {column}");
    }
    assert!(sql.contains("uq_import_configs_user_name_format"));
    assert!(sql.contains("uq_import_configs_user_format_default"));
    assert!(sql.contains("WHERE is_default"));
}

#[tokio::test]
async fn postgres_import_configs_are_user_scoped_ordered_and_conflict_safe(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) = isolated_postgres_database("import_config_contract").await? else {
        return Ok(());
    };
    let user_a = seed_user(&test_db.pool, "import-config-a").await?;
    let user_b = seed_user(&test_db.pool, "import-config-b").await?;

    let first_id = save_postgres_import_config(
        &test_db.pool,
        user_a,
        &draft(
            None,
            " Monthly   Card ",
            "CSV",
            false,
            &["交易时间", "金额"],
        ),
    )
    .await?;
    let other_user_id = save_postgres_import_config(
        &test_db.pool,
        user_b,
        &draft(None, "monthly card", "csv", true, &["other", "headers"]),
    )
    .await?;
    assert_ne!(first_id, other_user_id);

    let duplicate = save_postgres_import_config(
        &test_db.pool,
        user_a,
        &draft(None, "monthly card", " csv ", false, &["different"]),
    )
    .await
    .expect_err("normalized duplicate");
    assert!(matches!(duplicate, ImportConfigRepositoryError::Conflict));

    let second_id = save_postgres_import_config(
        &test_db.pool,
        user_a,
        &draft(None, "Fallback", "csv", true, &["fallback"]),
    )
    .await?;
    let updated_first_id = save_postgres_import_config(
        &test_db.pool,
        user_a,
        &draft(
            Some(first_id),
            "Monthly Card",
            "csv",
            true,
            &["交易时间", "金额"],
        ),
    )
    .await?;
    assert_eq!(updated_first_id, first_id);

    let listed = list_postgres_import_configs(&test_db.pool, user_a, "CSV").await?;
    assert_eq!(
        listed.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![first_id, second_id]
    );
    assert_eq!(listed.iter().filter(|item| item.is_default).count(), 1);
    assert!(listed[0].is_default);
    assert_eq!(
        listed[0].field_mappings,
        json!({"columnMapping":{"1":0,"8":1}})
    );
    assert_eq!(listed[0].sample_headers, vec!["交易时间", "金额"]);
    assert_eq!(listed[0].custom_rules, json!({"trim":true}));
    assert!(chrono::DateTime::parse_from_rfc3339(&listed[0].created_at).is_ok());
    assert!(listed[0].created_at.ends_with('Z'));
    assert!(listed[0].updated_at.ends_with('Z'));

    let exact = match_postgres_import_config(
        &test_db.pool,
        user_a,
        "csv",
        &[" 交易时间 ".to_string(), " 金额 ".to_string()],
    )
    .await?
    .expect("exact match");
    assert_eq!(exact.id, first_id);
    assert!(!exact.default_recommendation);
    assert_eq!(exact.match_reason, "exact_headers");
    assert_eq!(exact.match_score, 1.0);

    let fallback =
        match_postgres_import_config(&test_db.pool, user_a, "csv", &["unknown".to_string()])
            .await?
            .expect("default fallback");
    assert_eq!(fallback.id, first_id);
    assert!(fallback.default_recommendation);
    assert_eq!(fallback.match_reason, "default_template_fallback");

    let reordered = match_postgres_import_config(
        &test_db.pool,
        user_a,
        "csv",
        &["金额".to_string(), "交易时间".to_string()],
    )
    .await?
    .expect("order mismatch falls back");
    assert_eq!(reordered.match_reason, "default_template_fallback");

    let empty_user = seed_user(&test_db.pool, "import-config-empty").await?;
    assert!(match_postgres_import_config(
        &test_db.pool,
        empty_user,
        "csv",
        &["unknown".to_string()],
    )
    .await?
    .is_none());

    let configs_before: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM import_configs")
        .fetch_one(&test_db.pool)
        .await?;
    let learning_before: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM import_learning_samples) + (SELECT COUNT(*) FROM import_learning_suggestions)",
    )
    .fetch_one(&test_db.pool)
    .await?;
    let suggestion_headers = vec!["交易时间".to_string(), "金额".to_string()];
    let left = suggest_import_config(&suggestion_headers, None)?;
    let right = suggest_import_config(&suggestion_headers, Some(&[]))?;
    assert_eq!(left, right);
    let configs_after: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM import_configs")
        .fetch_one(&test_db.pool)
        .await?;
    let learning_after: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM import_learning_samples) + (SELECT COUNT(*) FROM import_learning_suggestions)",
    )
    .fetch_one(&test_db.pool)
    .await?;
    assert_eq!(
        (configs_after, learning_after),
        (configs_before, learning_before)
    );

    assert!(matches!(
        save_postgres_import_config(
            &test_db.pool,
            user_a,
            &draft(Some(other_user_id), "hidden", "csv", false, &["hidden"]),
        )
        .await,
        Err(ImportConfigRepositoryError::NotFound)
    ));
    assert!(matches!(
        delete_postgres_import_config(&test_db.pool, user_a, other_user_id).await,
        Err(ImportConfigRepositoryError::NotFound)
    ));
    assert_eq!(
        delete_postgres_import_config(&test_db.pool, user_a, second_id).await?,
        second_id
    );
    assert!(matches!(
        delete_postgres_import_config(&test_db.pool, user_a, second_id).await,
        Err(ImportConfigRepositoryError::NotFound)
    ));

    let type_rows = sqlx::query(
        "SELECT column_name, data_type FROM information_schema.columns WHERE table_name = 'import_configs' AND column_name = ANY($1) ORDER BY column_name",
    )
    .bind(vec!["custom_rules", "field_mappings", "sample_headers"])
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(type_rows.len(), 3);
    assert!(type_rows
        .iter()
        .all(|row| row.get::<String, _>("data_type") == "jsonb"));

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_default_writes_settle_with_one_default() -> Result<(), Box<dyn Error>> {
    let Some(test_db) = isolated_postgres_database("import_config_default").await? else {
        return Ok(());
    };
    let user = seed_user(&test_db.pool, "import-config-concurrent").await?;
    let left_pool = test_db.pool.clone();
    let right_pool = test_db.pool.clone();
    let left = tokio::spawn(async move {
        save_postgres_import_config(
            &left_pool,
            user,
            &draft(None, "left", "csv", true, &["left"]),
        )
        .await
    });
    let right = tokio::spawn(async move {
        save_postgres_import_config(
            &right_pool,
            user,
            &draft(None, "right", "csv", true, &["right"]),
        )
        .await
    });
    left.await??;
    right.await??;

    let configs = list_postgres_import_configs(&test_db.pool, user, "csv").await?;
    assert_eq!(configs.len(), 2);
    assert_eq!(configs.iter().filter(|item| item.is_default).count(), 1);
    test_db.cleanup().await?;
    Ok(())
}

fn draft(
    id: Option<i64>,
    name: &str,
    file_format: &str,
    is_default: bool,
    headers: &[&str],
) -> ImportConfigDraft {
    ImportConfigDraft {
        id,
        name: name.to_string(),
        file_format: file_format.to_string(),
        description: "A mapping template".to_string(),
        field_mappings: json!({"columnMapping":{"1":0,"8":1}}),
        sample_headers: headers.iter().map(|item| (*item).to_string()).collect(),
        date_format: "%Y-%m-%d %H:%M:%S".to_string(),
        delimiter: Some(",".to_string()),
        encoding: "utf-8".to_string(),
        skip_rows: 0,
        has_header: true,
        custom_rules: json!({"trim":true}),
        is_default,
    }
}

async fn seed_user(
    pool: &bill_analyser_db::PostgresPool,
    prefix: &str,
) -> Result<UserId, Box<dyn Error>> {
    let unique = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .abs();
    let id: i64 =
        sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("{prefix}-{unique}"))
            .bind(format!("{prefix}-{unique}@example.test"))
            .fetch_one(pool)
            .await?;
    Ok(UserId::new(u64::try_from(id)?)?)
}
