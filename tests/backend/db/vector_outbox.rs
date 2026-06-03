use std::{env, error::Error};

use bill_analyser_db::{
    claim_pending_vector_outbox_events, enqueue_vector_outbox_event,
    load_import_learning_feature_vector_sources, mark_vector_outbox_event_failed,
    mark_vector_outbox_event_succeeded, run_postgres_migrations, VectorOutboxEventDraft,
    VECTOR_OUTBOX_STATUS_COMPLETED, VECTOR_OUTBOX_STATUS_PENDING, VECTOR_OUTBOX_STATUS_PROCESSING,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Row};

#[tokio::test]
async fn vector_outbox_round_trips_and_feature_sources_load_when_postgres_available(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("vector-outbox-{unique}"))
            .bind(format!("vector-outbox-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;

    let event_id = enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "feature-1".to_string(),
            event_type: "upsert".to_string(),
            payload: json!({
                "class": "BillDevCounterpartyFeature",
                "postgresSourceId": "feature-1"
            }),
            available_at: None,
        },
    )
    .await?;

    let claimed = claim_pending_vector_outbox_events(&pool, 10).await?;
    let event = claimed
        .iter()
        .find(|event| event.id == event_id)
        .expect("new event is claimed");
    assert_eq!(event.status, VECTOR_OUTBOX_STATUS_PROCESSING);
    assert_eq!(event.attempts, 1);
    assert_eq!(event.payload["postgresSourceId"], "feature-1");

    mark_vector_outbox_event_failed(&pool, event_id, "temporary unavailable", 0, 3).await?;
    let failed_status: String =
        sqlx::query("SELECT status FROM vector_outbox_events WHERE id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await?
            .try_get("status")?;
    assert_eq!(failed_status, VECTOR_OUTBOX_STATUS_PENDING);

    let reclaimed = claim_pending_vector_outbox_events(&pool, 10).await?;
    assert!(reclaimed.iter().any(|event| event.id == event_id));
    mark_vector_outbox_event_succeeded(&pool, event_id).await?;
    let completed_status: String =
        sqlx::query("SELECT status FROM vector_outbox_events WHERE id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await?
            .try_get("status")?;
    assert_eq!(completed_status, VECTOR_OUTBOX_STATUS_COMPLETED);

    let sample_id: i64 = sqlx::query(
        r#"
        INSERT INTO import_learning_samples (
            user_id, sample_key, normalized_features, target_payload, source_payload
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(format!("sample-{unique}"))
    .bind(json!({"counterparty": "coffee"}))
    .bind(json!({"type": "expense"}))
    .bind(json!({"parser_id": "wechat"}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO import_learning_features (
            user_id, sample_id, feature_key, feature_hash, feature_payload
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(sample_id)
    .bind("counterparty")
    .bind("hash-1")
    .bind(json!({"counterparty": "coffee"}))
    .execute(&pool)
    .await?;

    let sources = load_import_learning_feature_vector_sources(&pool, Some(user_id), 10).await?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].user_id, user_id);
    assert_eq!(sources[0].feature_key, "counterparty");
    assert_eq!(sources[0].feature_payload["counterparty"], "coffee");

    Ok(())
}
