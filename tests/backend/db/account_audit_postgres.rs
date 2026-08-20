mod postgres_test_support;

use std::{env, error::Error};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_postgres_account_audit_event, get_postgres_operation_password, AccountAuditEventDraft,
    PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

async fn required_isolated_postgres(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err("BILL_ANALYSER_TEST_POSTGRES_URL is required for account audit tests".into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| "account audit PostgreSQL test database was not created".into())
}

#[tokio::test]
async fn account_audit_event_preserves_the_typed_repository_contract() -> Result<(), Box<dyn Error>>
{
    let test_db = required_isolated_postgres("account_audit_writer_contract").await?;
    let pool: &PostgresPool = &test_db.pool;
    let user_id: i64 =
        sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind("account-audit-owner")
            .bind("account-audit-owner@example.test")
            .fetch_one(pool)
            .await?;

    let event_id = create_postgres_account_audit_event(
        pool,
        user_id,
        AccountAuditEventDraft {
            operation_type: "move_transactions",
            target_id: 41,
            details: json!({"from_account_id": 41, "to_account_id": 42}),
            affected_count: 7,
            status: "failed",
            error_message: Some("contract error".to_string()),
            ip_address: "203.0.113.9".to_string(),
            user_agent: "account-audit-contract".to_string(),
        },
    )
    .await?;

    let row = sqlx::query(
        r#"
        SELECT user_id, entity_type, entity_id, action, actor,
               before_payload, after_payload, metadata
        FROM business_audit_events
        WHERE id = $1
        "#,
    )
    .bind(event_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.try_get::<i64, _>("user_id")?, user_id);
    assert_eq!(row.try_get::<String, _>("entity_type")?, "account");
    assert_eq!(row.try_get::<String, _>("entity_id")?, "41");
    assert_eq!(row.try_get::<String, _>("action")?, "move_transactions");
    assert_eq!(row.try_get::<String, _>("actor")?, "runtime");
    assert_eq!(row.try_get::<Value, _>("before_payload")?, json!({}));
    assert_eq!(row.try_get::<Value, _>("after_payload")?, json!({}));
    assert_eq!(
        row.try_get::<Value, _>("metadata")?,
        json!({
            "details": {"from_account_id": 41, "to_account_id": 42},
            "affected_count": 7,
            "status": "failed",
            "error_message": "contract error",
            "ip_address": "203.0.113.9",
            "user_agent": "account-audit-contract"
        })
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn operation_password_read_supports_only_documented_legacy_shapes(
) -> Result<(), Box<dyn Error>> {
    let test_db = required_isolated_postgres("operation_password_read_contract").await?;
    let pool: &PostgresPool = &test_db.pool;
    let raw_user_id: i64 =
        sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind("operation-password-owner")
            .bind("operation-password-owner@example.test")
            .fetch_one(pool)
            .await?;
    let user_id = UserId::new(u64::try_from(raw_user_id)?)?;

    sqlx::query("INSERT INTO settings (user_id, key, value) VALUES ($1, $2, $3)")
        .bind(raw_user_id)
        .bind("operation_password")
        .bind(json!({"value": "legacy-secret"}))
        .execute(pool)
        .await?;
    assert_eq!(
        get_postgres_operation_password(pool, user_id).await?,
        Some("legacy-secret".to_string())
    );

    for (stored, expected) in [
        (json!("plain-secret"), Some("plain-secret".to_string())),
        (json!(42), None),
        (json!({"unexpected": "shape"}), None),
        (Value::Null, None),
    ] {
        sqlx::query("UPDATE settings SET value = $1 WHERE user_id = $2 AND key = $3")
            .bind(stored)
            .bind(raw_user_id)
            .bind("operation_password")
            .execute(pool)
            .await?;
        assert_eq!(
            get_postgres_operation_password(pool, user_id).await?,
            expected
        );
    }

    test_db.cleanup().await?;
    Ok(())
}
