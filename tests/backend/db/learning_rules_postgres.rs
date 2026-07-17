#[path = "postgres_test_support.rs"]
mod postgres_test_support;

use std::{env, error::Error};

use bill_analyser_db::{
    taxonomy::postgres_reads::{update_postgres_learning_rule, PostgresLearningRuleUpdate},
    DbError, PostgresPool,
};
use postgres_test_support::{isolated_postgres_database, IsolatedPostgres};
use serde_json::{json, Value};
use sqlx::Row;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for PostgreSQL learning-rule tests",
        )
        .into());
    }
    isolated_postgres_database(prefix).await?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for PostgreSQL learning-rule tests",
        )
        .into()
    })
}

async fn insert_user(pool: &PostgresPool, username: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(username)
        .fetch_one(pool)
        .await
}

async fn insert_category(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO categories (user_id, name, category_type) VALUES ($1, $2, 'expense') RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_learning_rule(
    pool: &PostgresPool,
    user_id: i64,
    key: &str,
    recommendation_type: &str,
    status: &str,
    accepted_count: i32,
    auto_applied_count: i32,
    auto_apply_enabled: bool,
    metadata: Value,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO import_learning_lifecycle (
            user_id, recommendation_key, recommendation_type, status,
            accepted_count, auto_applied_count, auto_apply_enabled, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(key)
    .bind(recommendation_type)
    .bind(status)
    .bind(accepted_count)
    .bind(auto_applied_count)
    .bind(auto_apply_enabled)
    .bind(metadata)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn learning_rule_owner_can_update_fields_disable_restore_and_clear_category(
) -> Result<(), Box<dyn Error>> {
    let database = strict_isolated_postgres_database("learning_rule_owner_update").await?;
    let pool = &database.pool;
    let user_id = insert_user(pool, "learning-rule-owner").await?;
    let category_id = insert_category(pool, user_id, "Learning category").await?;
    let rule_id = insert_learning_rule(
        pool,
        user_id,
        "merchant:old",
        "expense",
        "green",
        4,
        3,
        true,
        json!({
            "match_type": "composite",
            "match_value": "old-value",
            "match_features": {"merchant": "old"},
            "learned_type": "expense"
        }),
    )
    .await?;

    let disabled = update_postgres_learning_rule(
        pool,
        user_id,
        rule_id,
        &PostgresLearningRuleUpdate {
            match_value: Some("merchant:new".to_string()),
            match_features: Some(json!({"merchant": "new", "source": "wechat"})),
            learned_type: Some("transfer".to_string()),
            learned_category_id: Some(Some(category_id)),
            enabled: Some(false),
        },
    )
    .await?
    .expect("owner update should return the current projection");

    assert_eq!(disabled["id"], rule_id);
    assert_eq!(disabled["match_type"], "composite");
    assert_eq!(disabled["match_value"], "merchant:new");
    assert_eq!(disabled["learned_type"], "transfer");
    assert_eq!(disabled["learned_category_id"], category_id);
    assert_eq!(disabled["enabled"], false);
    assert_eq!(disabled["applied_count"], 7);
    assert_eq!(disabled["version"], 2);
    assert!(disabled["created_at"].as_str().is_some());
    assert!(disabled["updated_at"].as_str().is_some());

    let persisted = sqlx::query(
        r#"
        SELECT recommendation_type, status, auto_apply_enabled, metadata, version
        FROM import_learning_lifecycle
        WHERE id = $1
        "#,
    )
    .bind(rule_id)
    .fetch_one(pool)
    .await?;
    let persisted_metadata: Value = persisted.try_get("metadata")?;
    assert_eq!(
        persisted.try_get::<String, _>("recommendation_type")?,
        "transfer"
    );
    assert_eq!(persisted.try_get::<String, _>("status")?, "disabled");
    assert!(!persisted.try_get::<bool, _>("auto_apply_enabled")?);
    assert_eq!(persisted.try_get::<i64, _>("version")?, 2);
    assert_eq!(persisted_metadata["match_value"], "merchant:new");
    assert_eq!(persisted_metadata["match_features"]["source"], "wechat");
    assert_eq!(persisted_metadata["learned_category_id"], category_id);
    assert_eq!(persisted_metadata["disabled_from_status"], "green");
    assert_eq!(persisted_metadata["disabled_from_auto_apply_enabled"], true);

    let restored = update_postgres_learning_rule(
        pool,
        user_id,
        rule_id,
        &PostgresLearningRuleUpdate {
            learned_category_id: Some(None),
            enabled: Some(true),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await?
    .expect("restoring a disabled rule should return its projection");
    assert_eq!(restored["learned_category_id"], Value::Null);
    assert_eq!(restored["enabled"], true);
    assert_eq!(restored["version"], 3);

    let restored_row = sqlx::query(
        "SELECT status, auto_apply_enabled, metadata FROM import_learning_lifecycle WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_one(pool)
    .await?;
    let restored_metadata: Value = restored_row.try_get("metadata")?;
    assert_eq!(restored_row.try_get::<String, _>("status")?, "green");
    assert!(restored_row.try_get::<bool, _>("auto_apply_enabled")?);
    assert_eq!(restored_metadata["learned_category_id"], Value::Null);
    assert!(restored_metadata.get("disabled_from_status").is_none());
    assert!(restored_metadata
        .get("disabled_from_auto_apply_enabled")
        .is_none());

    database.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn learning_rule_update_is_user_scoped_and_rejects_invalid_references(
) -> Result<(), Box<dyn Error>> {
    let database = strict_isolated_postgres_database("learning_rule_scope_errors").await?;
    let pool = &database.pool;
    let owner_id = insert_user(pool, "learning-rule-scope-owner").await?;
    let other_user_id = insert_user(pool, "learning-rule-scope-other").await?;
    let other_category_id = insert_category(pool, other_user_id, "Other category").await?;
    let rule_id = insert_learning_rule(
        pool,
        owner_id,
        "merchant:scoped",
        "expense",
        "yellow",
        0,
        0,
        false,
        json!({"match_type": "composite", "match_value": "original"}),
    )
    .await?;

    let cross_user = update_postgres_learning_rule(
        pool,
        other_user_id,
        rule_id,
        &PostgresLearningRuleUpdate {
            learned_type: Some("income".to_string()),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await?;
    assert!(cross_user.is_none());

    let missing = update_postgres_learning_rule(
        pool,
        owner_id,
        i64::MAX,
        &PostgresLearningRuleUpdate::default(),
    )
    .await?;
    assert!(missing.is_none());

    let invalid_category = update_postgres_learning_rule(
        pool,
        owner_id,
        rule_id,
        &PostgresLearningRuleUpdate {
            learned_category_id: Some(Some(other_category_id)),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await
    .expect_err("another user's category must be rejected");
    assert!(matches!(
        invalid_category,
        DbError::InvalidOperation(message) if message == "invalid_learned_category_id"
    ));

    let invalid_composite = update_postgres_learning_rule(
        pool,
        owner_id,
        rule_id,
        &PostgresLearningRuleUpdate {
            match_value: Some("changed-without-features".to_string()),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await
    .expect_err("composite match updates require feature data");
    assert!(matches!(
        invalid_composite,
        DbError::InvalidOperation(message) if message == "invalid_match_value"
    ));

    let unchanged = sqlx::query(
        "SELECT recommendation_type, metadata, version FROM import_learning_lifecycle WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_one(pool)
    .await?;
    let unchanged_metadata: Value = unchanged.try_get("metadata")?;
    assert_eq!(
        unchanged.try_get::<String, _>("recommendation_type")?,
        "expense"
    );
    assert_eq!(unchanged.try_get::<i64, _>("version")?, 1);
    assert_eq!(unchanged_metadata["match_value"], "original");

    database.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn learning_rule_update_handles_status_and_metadata_boundary_states(
) -> Result<(), Box<dyn Error>> {
    let database = strict_isolated_postgres_database("learning_rule_boundaries").await?;
    let pool = &database.pool;
    let user_id = insert_user(pool, "learning-rule-boundaries-owner").await?;

    let suppressed_id = insert_learning_rule(
        pool,
        user_id,
        "merchant:suppressed",
        "expense",
        "suppressed",
        1,
        2,
        true,
        json!({}),
    )
    .await?;
    let unsuppressed = update_postgres_learning_rule(
        pool,
        user_id,
        suppressed_id,
        &PostgresLearningRuleUpdate {
            enabled: Some(true),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await?
    .expect("suppressed rule should be restorable");
    assert_eq!(unsuppressed["enabled"], true);
    assert_eq!(unsuppressed["match_type"], "expense");
    assert_eq!(unsuppressed["match_value"], "merchant:suppressed");
    assert_eq!(unsuppressed["learned_type"], "expense");
    assert_eq!(unsuppressed["learned_category_id"], Value::Null);
    assert_eq!(unsuppressed["applied_count"], 3);
    let unsuppressed_row = sqlx::query(
        "SELECT status, auto_apply_enabled FROM import_learning_lifecycle WHERE id = $1",
    )
    .bind(suppressed_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(unsuppressed_row.try_get::<String, _>("status")?, "yellow");
    assert!(!unsuppressed_row.try_get::<bool, _>("auto_apply_enabled")?);

    let invalid_restore_id = insert_learning_rule(
        pool,
        user_id,
        "merchant:invalid-restore",
        "income",
        "disabled",
        0,
        0,
        true,
        json!({
            "disabled_from_status": "suppressed",
            "disabled_from_auto_apply_enabled": "not-a-bool"
        }),
    )
    .await?;
    let default_restored = update_postgres_learning_rule(
        pool,
        user_id,
        invalid_restore_id,
        &PostgresLearningRuleUpdate {
            enabled: Some(true),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await?
    .expect("invalid disabled metadata should restore to safe defaults");
    assert_eq!(default_restored["enabled"], true);
    let default_restored_row = sqlx::query(
        "SELECT status, auto_apply_enabled, metadata FROM import_learning_lifecycle WHERE id = $1",
    )
    .bind(invalid_restore_id)
    .fetch_one(pool)
    .await?;
    let default_metadata: Value = default_restored_row.try_get("metadata")?;
    assert_eq!(
        default_restored_row.try_get::<String, _>("status")?,
        "yellow"
    );
    assert!(!default_restored_row.try_get::<bool, _>("auto_apply_enabled")?);
    assert!(default_metadata.get("disabled_from_status").is_none());
    assert!(default_metadata
        .get("disabled_from_auto_apply_enabled")
        .is_none());

    let already_disabled_id = insert_learning_rule(
        pool,
        user_id,
        "merchant:already-disabled",
        "expense",
        "disabled",
        0,
        0,
        false,
        json!({
            "disabled_from_status": "green",
            "disabled_from_auto_apply_enabled": true
        }),
    )
    .await?;
    update_postgres_learning_rule(
        pool,
        user_id,
        already_disabled_id,
        &PostgresLearningRuleUpdate {
            enabled: Some(false),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await?
    .expect("re-disabling a rule should be idempotent");
    let already_disabled_metadata: Value =
        sqlx::query_scalar("SELECT metadata FROM import_learning_lifecycle WHERE id = $1")
            .bind(already_disabled_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(already_disabled_metadata["disabled_from_status"], "green");
    assert_eq!(
        already_disabled_metadata["disabled_from_auto_apply_enabled"],
        true
    );

    let scalar_metadata_id = insert_learning_rule(
        pool,
        user_id,
        "merchant:scalar-metadata",
        "expense",
        "yellow",
        0,
        0,
        false,
        json!(["legacy", "metadata"]),
    )
    .await?;
    let scalar_projection = update_postgres_learning_rule(
        pool,
        user_id,
        scalar_metadata_id,
        &PostgresLearningRuleUpdate::default(),
    )
    .await?
    .expect("non-object metadata should project through safe fallbacks");
    assert_eq!(scalar_projection["match_type"], "expense");
    assert_eq!(scalar_projection["match_value"], "merchant:scalar-metadata");
    assert_eq!(scalar_projection["learned_type"], "expense");
    assert_eq!(scalar_projection["learned_category_id"], Value::Null);

    let feature_cleanup_id = insert_learning_rule(
        pool,
        user_id,
        "merchant:feature-cleanup",
        "expense",
        "green",
        0,
        0,
        false,
        json!({
            "match_type": "merchant",
            "match_value": "old",
            "match_features": {"legacy": true}
        }),
    )
    .await?;
    let feature_cleanup = update_postgres_learning_rule(
        pool,
        user_id,
        feature_cleanup_id,
        &PostgresLearningRuleUpdate {
            match_value: Some(String::new()),
            enabled: Some(true),
            ..PostgresLearningRuleUpdate::default()
        },
    )
    .await?
    .expect("non-composite match values may be cleared without features");
    assert_eq!(feature_cleanup["match_value"], "");
    let feature_cleanup_metadata: Value =
        sqlx::query_scalar("SELECT metadata FROM import_learning_lifecycle WHERE id = $1")
            .bind(feature_cleanup_id)
            .fetch_one(pool)
            .await?;
    assert!(feature_cleanup_metadata.get("match_features").is_none());

    database.cleanup().await?;
    Ok(())
}
