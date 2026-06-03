#![cfg(not(coverage))]

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use bill_analyser_core::{
    build_weaviate_required_metadata, WeaviateDerivedClass, WeaviateFilterValue,
    WeaviateMetadataFilter, WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
};
use bill_analyser_db::{
    enqueue_vector_outbox_event, load_import_learning_feature_vector_sources,
    run_postgres_migrations, ImportLearningFeatureVectorSource, VectorOutboxEventDraft,
};
use bill_analyser_http::{
    build_object_from_feature_source, probe_weaviate_health, process_weaviate_outbox_once,
    rebuild_weaviate_from_postgres, recall_import_learning_candidates, HttpShellConfig,
    WeaviateHttpClient, WeaviateImportLearningRecallRequest, WeaviateRuntimeConfig,
    WeaviateRuntimeError,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Row};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::sleep,
};

#[tokio::test]
async fn weaviate_health_probe_is_disabled_by_default() {
    let status = probe_weaviate_health(&HttpShellConfig::default()).await;

    assert_eq!(status.status, "disabled");
    assert_eq!(status.health_detail_value(), "disabled");
}

#[test]
fn weaviate_cli_health_is_safe_when_disabled() {
    let bin = env!("CARGO_BIN_EXE_bill_weaviate_derived_index");
    let output = Command::new(bin)
        .args(["--mode", "health"])
        .env("BILL_ANALYSER_WEAVIATE_ENABLED", "false")
        .env_remove("BILL_ANALYSER_WEAVIATE_ENDPOINT")
        .env_remove("BILL_ANALYSER_WEAVIATE_API_KEY")
        .output()
        .expect("run weaviate cli health");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"status\": \"disabled\""));
}

#[test]
fn weaviate_cli_covers_help_disabled_bootstrap_and_config_errors() {
    let bin = env!("CARGO_BIN_EXE_bill_weaviate_derived_index");

    let help = Command::new(bin)
        .arg("--help")
        .output()
        .expect("run weaviate cli help");
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)
        .expect("utf8 help")
        .contains("process-outbox"));

    let bootstrap = Command::new(bin)
        .arg("--mode=bootstrap")
        .env("BILL_ANALYSER_WEAVIATE_ENABLED", "false")
        .env_remove("BILL_ANALYSER_WEAVIATE_ENDPOINT")
        .output()
        .expect("run weaviate cli bootstrap");
    assert!(bootstrap.status.success());
    assert!(String::from_utf8(bootstrap.stdout)
        .expect("utf8 bootstrap")
        .contains("\"enabled\": false"));

    let invalid_postgres = Command::new(bin)
        .args(["--mode", "process-outbox"])
        .env("BILL_ANALYSER_POSTGRES_URL", "not-postgres://data/app.db")
        .output()
        .expect("run weaviate cli process-outbox with invalid postgres url");
    assert!(!invalid_postgres.status.success());
    assert!(String::from_utf8_lossy(&invalid_postgres.stderr).contains("invalid PostgreSQL URL"));

    let unknown = Command::new(bin)
        .arg("unknown-mode")
        .output()
        .expect("run weaviate cli unknown mode");
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown mode"));
}

#[tokio::test]
async fn enabled_weaviate_without_endpoint_degrades_not_panics() {
    let config = HttpShellConfig::default().with_weaviate_config(WeaviateRuntimeConfig {
        enabled: true,
        endpoint: None,
        api_key: None,
        collection_prefix: "BillDev".to_string(),
        timeout: Duration::from_millis(1),
        retry_attempts: 0,
        batch_size: 1,
        vector_dimensions: 8,
    });

    let status = probe_weaviate_health(&config).await;
    assert_eq!(status.status, "degraded");
    assert_eq!(status.health_detail_value(), "degraded:missing_endpoint");

    let error = WeaviateHttpClient::new(&config.weaviate).unwrap_err();
    assert!(matches!(error, WeaviateRuntimeError::MissingEndpoint));
}

#[tokio::test]
async fn enabled_weaviate_ready_failure_reports_degraded_without_secret_leakage(
) -> Result<(), Box<dyn Error>> {
    let (endpoint, _requests) =
        spawn_weaviate_status_mock(1, "503 Service Unavailable", "{}").await?;
    let config = HttpShellConfig::default().with_weaviate_config(WeaviateRuntimeConfig {
        enabled: true,
        endpoint: Some(endpoint),
        api_key: Some("super-secret-weaviate-key".to_string()),
        collection_prefix: "BillDev".to_string(),
        timeout: Duration::from_secs(5),
        retry_attempts: 1,
        batch_size: 10,
        vector_dimensions: WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
    });

    let status = probe_weaviate_health(&config).await;

    assert_eq!(status.status, "degraded");
    assert!(status.health_detail_value().starts_with("degraded:"));
    let rendered = format!("{status:?} {}", status.health_detail_value());
    assert!(!rendered.contains("super-secret-weaviate-key"));
    Ok(())
}

#[tokio::test]
async fn weaviate_recall_search_filters_and_parses_derived_hits() -> Result<(), Box<dyn Error>> {
    let body = r#"{"data":{"Get":{"BillDevCounterpartyFeature":[{"postgresSourceId":"feature:42","recommendationKey":"rk-42","featureKey":"counterparty","ruleState":"postgres_authoritative","transactionType":"transfer","categoryId":4,"sourceAccountId":10,"destinationAccountId":20,"payloadJson":"{\"target\":\"transfer\"}","_additional":{"id":"uuid-42","distance":0.125}},{"postgresSourceId":"feature:dup","recommendationKey":"rk-dup-old","featureKey":"counterparty","ruleState":"postgres_authoritative","transactionType":"transfer","_additional":{"distance":0.30}},{"postgresSourceId":"   ","_additional":{"distance":0.05}}],"BillDevDescriptionFeature":[{"postgresSourceId":"feature:dup","recommendationKey":"rk-dup-new","featureKey":"description","ruleState":"postgres_authoritative","transactionType":"转账","_additional":{"distance":0.20}}],"BillDevImportLearningSample":[]}}}"#;
    let (endpoint, requests) = spawn_weaviate_graphql_mock(3, body).await?;
    let config = weaviate_config(&endpoint);
    let request = WeaviateImportLearningRecallRequest {
        user_id: 42,
        features: BTreeMap::from([
            ("counterparty".to_string(), "wallet".to_string()),
            ("description".to_string(), "internal transfer".to_string()),
        ]),
        transaction_type_scope: "转账".to_string(),
        limit: 5,
    };

    let hits = recall_import_learning_candidates(&config, &request).await?;

    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].postgres_source_id, "feature:42");
    assert_eq!(hits[0].transaction_type.as_deref(), Some("transfer"));
    assert_eq!(hits[0].category_id, Some(4));
    assert_eq!(hits[0].source_account_id, Some(10));
    assert_eq!(hits[0].destination_account_id, Some(20));
    assert!(hits[0].score > 0.87);
    assert_eq!(hits[1].postgres_source_id, "feature:dup");
    assert_eq!(hits[1].recommendation_key.as_deref(), Some("rk-dup-new"));
    wait_for_requests(&requests, 3).await;
    let joined = requests.lock().expect("request log").join("\n");
    assert!(joined.contains("path: [\\\"userId\\\"]"));
    assert!(joined.contains("path: [\\\"featureSchemaVersion\\\"]"));
    assert!(joined.contains("path: [\\\"transactionType\\\"]"));
    assert!(joined.contains("path: [\\\"ruleState\\\"]"));
    assert!(joined.contains("valueText: \\\"transfer\\\""));
    Ok(())
}

#[test]
fn weaviate_feature_object_metadata_is_postgres_authoritative_without_postgres() {
    let config = weaviate_config("http://127.0.0.1:1");
    let object = build_object_from_feature_source(
        &config,
        &ImportLearningFeatureVectorSource {
            feature_id: 7,
            user_id: 42,
            sample_id: 5,
            sample_key: "sample-7".to_string(),
            feature_key: "counterparty".to_string(),
            feature_hash: "hash-7".to_string(),
            feature_payload: json!({"counterparty": "基金公司"}),
            normalized_features: json!({"parser_id": "manual-parser"}),
            target_payload: json!({
                "transaction_type": "投资",
                "annotated_category_id": 8,
                "annotated_source_account_id": 3,
                "annotated_destination_account_id": 4
            }),
            source_payload: json!({}),
        },
    )
    .expect("feature object");

    assert_eq!(object.properties["parserId"], "manual-parser");
    assert_eq!(object.properties["transactionType"], "investment");
    assert_eq!(object.properties["categoryId"], 8);
    assert_eq!(object.properties["sourceAccountId"], 3);
    assert_eq!(object.properties["destinationAccountId"], 4);
    assert_eq!(object.properties["ruleState"], "postgres_authoritative");
}

#[tokio::test]
async fn readiness_probe_reports_healthy_for_ready_endpoint() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer).await;
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await;
    });

    let config = HttpShellConfig::default().with_weaviate_config(WeaviateRuntimeConfig {
        enabled: true,
        endpoint: Some(format!("http://{addr}")),
        api_key: Some("secret-key".to_string()),
        collection_prefix: "BillDev".to_string(),
        timeout: Duration::from_secs(2),
        retry_attempts: 0,
        batch_size: 1,
        vector_dimensions: 8,
    });

    let status = probe_weaviate_health(&config).await;

    assert_eq!(status.status, "healthy");
    assert_eq!(status.health_detail_value(), "healthy");
    assert!(!format!("{status:?}").contains("secret-key"));

    Ok(())
}

#[tokio::test]
async fn weaviate_client_covers_disabled_empty_and_existing_schema_edges(
) -> Result<(), Box<dyn Error>> {
    let disabled = WeaviateRuntimeConfig::disabled();
    assert!(matches!(
        WeaviateHttpClient::new(&disabled),
        Err(WeaviateRuntimeError::Disabled)
    ));

    let (endpoint, requests) = spawn_existing_schema_mock(4).await?;
    let config = weaviate_config(&endpoint);
    let client = WeaviateHttpClient::new(&config)?;
    let empty_batch = client.upsert_objects(&[]).await?;
    assert_eq!(empty_batch.status, "skipped_empty");

    let bootstrap = client.bootstrap_schema().await?;
    assert_eq!(bootstrap.collections.len(), 4);
    wait_for_requests(&requests, 4).await;
    let requests = requests.lock().expect("request log").join("\n---\n");
    assert_eq!(requests.matches("GET /v1/schema/").count(), 4);
    assert!(!requests.contains("POST /v1/schema"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn weaviate_http_client_processes_outbox_and_rebuilds_from_postgres_when_available(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;
    sqlx::query("DELETE FROM vector_outbox_events")
        .execute(&pool)
        .await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("weaviate-runtime-{unique}"))
            .bind(format!("weaviate-runtime-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
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
    .bind(json!({"parser_id": "wechat"}))
    .bind(json!({
        "type": "expense",
        "category_id": 8,
        "source_account_id": 3
    }))
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
    .bind(format!("feature-hash-{unique}"))
    .bind(json!({"counterparty": "coffee shop", "amount": 1280}))
    .execute(&pool)
    .await?;

    let (endpoint, requests) = spawn_weaviate_mock(18).await?;
    let mut config = weaviate_config(&endpoint);
    config.retry_attempts = 0;
    let names = bill_analyser_core::build_weaviate_collection_names(&config.collection_prefix)
        .expect("valid collection prefix");

    let mut upsert_properties = build_weaviate_required_metadata(user_id, "manual-upsert");
    upsert_properties.insert("parserId".to_string(), json!("wechat"));
    enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "manual-upsert".to_string(),
            event_type: "upsert".to_string(),
            payload: json!({
                "class": names.name_for(WeaviateDerivedClass::CounterpartyFeature),
                "postgresSourceId": "manual-upsert",
                "properties": upsert_properties,
                "vector": [0.25, 0.5, 0.75, 1.0]
            }),
            available_at: None,
        },
    )
    .await?;
    enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "manual-delete".to_string(),
            event_type: "delete".to_string(),
            payload: json!({
                "class": names.name_for(WeaviateDerivedClass::CounterpartyFeature),
                "id": "manual-delete"
            }),
            available_at: None,
        },
    )
    .await?;
    let invalid_event_id = enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "invalid".to_string(),
            event_type: "upsert".to_string(),
            payload: json!({"postgresSourceId": "invalid"}),
            available_at: None,
        },
    )
    .await?;
    enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "fallback-upsert".to_string(),
            event_type: "upsert".to_string(),
            payload: json!({
                "class": names.name_for(WeaviateDerivedClass::ImportLearningSample),
                "sourceKey": "fallback-upsert"
            }),
            available_at: None,
        },
    )
    .await?;
    enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "missing-source-key".to_string(),
            event_type: "upsert".to_string(),
            payload: json!({
                "class": names.name_for(WeaviateDerivedClass::ImportLearningSample)
            }),
            available_at: None,
        },
    )
    .await?;
    enqueue_vector_outbox_event(
        &pool,
        &VectorOutboxEventDraft {
            user_id: Some(user_id),
            aggregate_type: "import_learning_feature".to_string(),
            aggregate_id: "missing-delete-id".to_string(),
            event_type: "delete".to_string(),
            payload: json!({
                "class": names.name_for(WeaviateDerivedClass::CounterpartyFeature)
            }),
            available_at: None,
        },
    )
    .await?;

    let process_report = process_weaviate_outbox_once(&pool, &config).await?;
    assert_eq!(process_report.claimed, 6);
    assert_eq!(process_report.succeeded, 3);
    assert_eq!(process_report.failed, 3);
    let failed_event =
        sqlx::query("SELECT status, attempts, last_error FROM vector_outbox_events WHERE id = $1")
            .bind(invalid_event_id)
            .fetch_one(&pool)
            .await?;
    let failed_status: String = failed_event.try_get("status")?;
    let failed_attempts: i32 = failed_event.try_get("attempts")?;
    let failed_error: String = failed_event.try_get("last_error")?;
    assert_eq!(failed_status, "failed");
    assert_eq!(failed_attempts, 1);
    assert!(failed_error.contains("missing class"));

    let disabled_report = process_weaviate_outbox_once(
        &pool,
        &WeaviateRuntimeConfig {
            enabled: false,
            ..config.clone()
        },
    )
    .await?;
    assert!(!disabled_report.enabled);

    let sources = load_import_learning_feature_vector_sources(&pool, Some(user_id), 10).await?;
    let object = build_object_from_feature_source(&config, &sources[0])?;
    assert_eq!(
        object.class,
        names
            .name_for(WeaviateDerivedClass::CounterpartyFeature)
            .to_string()
    );
    assert_eq!(object.properties["userId"], user_id);
    assert_eq!(object.properties["parserId"], "wechat");
    assert_eq!(
        object.properties["featureSchemaVersion"],
        bill_analyser_core::FEATURE_SCHEMA_VERSION
    );
    assert_eq!(object.properties["transactionType"], "expense");
    assert_eq!(object.properties["categoryId"], 8);
    assert_eq!(object.properties["sourceAccountId"], 3);
    assert_eq!(object.properties["ruleState"], "postgres_authoritative");

    let rebuild_report = rebuild_weaviate_from_postgres(&pool, &config, Some(user_id)).await?;
    assert_eq!(rebuild_report.source_count, 1);
    assert_eq!(rebuild_report.deleted_classes, 4);
    assert_eq!(rebuild_report.upserted, 1);
    let disabled_rebuild = rebuild_weaviate_from_postgres(
        &pool,
        &WeaviateRuntimeConfig {
            enabled: false,
            ..config.clone()
        },
        Some(user_id),
    )
    .await?;
    assert!(!disabled_rebuild.enabled);

    let client = WeaviateHttpClient::new(&config)?;
    let search = client
        .search(
            names.name_for(WeaviateDerivedClass::ImportLearningSample),
            &[0.1, 0.2, 0.3, 0.4],
            5,
            &[
                WeaviateMetadataFilter {
                    path: "userId".to_string(),
                    value: WeaviateFilterValue::Int(user_id),
                },
                WeaviateMetadataFilter {
                    path: "featureSchemaVersion".to_string(),
                    value: WeaviateFilterValue::Text("1".to_string()),
                },
            ],
        )
        .await?;
    assert_eq!(
        search["data"]["Get"].as_object().expect("Get object").len(),
        0
    );
    client
        .delete_object(
            names.name_for(WeaviateDerivedClass::CounterpartyFeature),
            "success-delete",
        )
        .await?;

    let bin = env!("CARGO_BIN_EXE_bill_weaviate_derived_index");
    let cli_process = Command::new(bin)
        .args(["--mode", "process-outbox"])
        .env("BILL_ANALYSER_POSTGRES_URL", &postgres_url)
        .env("BILL_ANALYSER_WEAVIATE_ENABLED", "false")
        .output()
        .expect("run process-outbox cli disabled");
    assert!(cli_process.status.success());
    assert!(String::from_utf8(cli_process.stdout)
        .expect("utf8 process-outbox")
        .contains("\"enabled\": false"));

    let cli_rebuild = Command::new(bin)
        .args(["--mode", "rebuild", "--user-id", &user_id.to_string()])
        .env("BILL_ANALYSER_POSTGRES_URL", &postgres_url)
        .env("BILL_ANALYSER_WEAVIATE_ENABLED", "false")
        .output()
        .expect("run rebuild cli disabled");
    assert!(cli_rebuild.status.success());
    assert!(String::from_utf8(cli_rebuild.stdout)
        .expect("utf8 rebuild")
        .contains("\"enabled\": false"));

    let (cli_endpoint, cli_requests) = spawn_weaviate_mock(8).await?;
    let cli_bootstrap = Command::new(bin)
        .args(["--mode", "bootstrap"])
        .env("BILL_ANALYSER_WEAVIATE_ENABLED", "1")
        .env("BILL_ANALYSER_WEAVIATE_ENDPOINT", cli_endpoint)
        .env("BILL_ANALYSER_WEAVIATE_API_KEY", "secret-key")
        .env("BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX", "BillDev")
        .output()
        .expect("run bootstrap cli enabled");
    assert!(cli_bootstrap.status.success());
    assert!(String::from_utf8(cli_bootstrap.stdout)
        .expect("utf8 bootstrap enabled")
        .contains("\"status\": \"bootstrapped\""));
    wait_for_requests(&cli_requests, 8).await;

    wait_for_requests(&requests, 18).await;
    let requests = requests.lock().expect("request log").join("\n---\n");
    assert!(requests
        .to_ascii_lowercase()
        .contains("authorization: bearer secret-key"));
    assert!(requests.contains("POST /v1/batch/objects"));
    assert!(requests.contains("DELETE /v1/objects/"));
    assert!(requests.contains("DELETE /v1/batch/objects"));
    assert!(requests.contains("POST /v1/graphql"));
    assert!(requests.contains("\"valueInt\":"));

    Ok(())
}

fn weaviate_config(endpoint: &str) -> WeaviateRuntimeConfig {
    WeaviateRuntimeConfig {
        enabled: true,
        endpoint: Some(endpoint.to_string()),
        api_key: Some("secret-key".to_string()),
        collection_prefix: "BillDev".to_string(),
        timeout: Duration::from_secs(5),
        retry_attempts: 2,
        batch_size: 10,
        vector_dimensions: WEAVIATE_DEFAULT_VECTOR_DIMENSIONS,
    }
}

async fn spawn_weaviate_mock(
    expected_requests: usize,
) -> Result<(String, Arc<Mutex<Vec<String>>>), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let request_log = Arc::clone(&requests);
    tokio::spawn(async move {
        for _ in 0..expected_requests {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let Ok(read) = stream.read(&mut buffer).await else {
                    return;
                };
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if request_is_complete(&bytes) {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&bytes).to_string();
            let first_line = request.lines().next().unwrap_or_default().to_string();
            request_log.lock().expect("request log").push(request);

            let (status, body) = if first_line.starts_with("GET /v1/schema/")
                || (first_line.starts_with("DELETE /v1/objects/")
                    && !first_line.contains("success-delete"))
            {
                ("404 Not Found", "")
            } else if first_line.starts_with("POST /v1/graphql") {
                ("200 OK", r#"{"data":{"Get":{}}}"#)
            } else {
                ("200 OK", "{}")
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    Ok((format!("http://{addr}"), requests))
}

async fn spawn_weaviate_graphql_mock(
    expected_requests: usize,
    graphql_body: &'static str,
) -> Result<(String, Arc<Mutex<Vec<String>>>), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let request_log = Arc::clone(&requests);
    tokio::spawn(async move {
        for _ in 0..expected_requests {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let Ok(read) = stream.read(&mut buffer).await else {
                    return;
                };
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if request_is_complete(&bytes) {
                    break;
                }
            }
            request_log
                .lock()
                .expect("request log")
                .push(String::from_utf8_lossy(&bytes).to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{graphql_body}",
                graphql_body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    Ok((format!("http://{addr}"), requests))
}

async fn spawn_weaviate_status_mock(
    expected_requests: usize,
    status: &'static str,
    body: &'static str,
) -> Result<(String, Arc<Mutex<Vec<String>>>), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let request_log = Arc::clone(&requests);
    tokio::spawn(async move {
        for _ in 0..expected_requests {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let Ok(read) = stream.read(&mut buffer).await else {
                    return;
                };
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if request_is_complete(&bytes) {
                    break;
                }
            }
            request_log
                .lock()
                .expect("request log")
                .push(String::from_utf8_lossy(&bytes).to_string());
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    Ok((format!("http://{addr}"), requests))
}

async fn spawn_existing_schema_mock(
    expected_requests: usize,
) -> Result<(String, Arc<Mutex<Vec<String>>>), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let request_log = Arc::clone(&requests);
    tokio::spawn(async move {
        for _ in 0..expected_requests {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let Ok(read) = stream.read(&mut buffer).await else {
                    return;
                };
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if request_is_complete(&bytes) {
                    break;
                }
            }
            request_log
                .lock()
                .expect("request log")
                .push(String::from_utf8_lossy(&bytes).to_string());
            let body = "{}";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    Ok((format!("http://{addr}"), requests))
}

fn request_is_complete(bytes: &[u8]) -> bool {
    let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    bytes.len() >= header_end + 4 + content_length
}

async fn wait_for_requests(requests: &Arc<Mutex<Vec<String>>>, expected: usize) {
    for _ in 0..50 {
        if requests.lock().expect("request log").len() >= expected {
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
}
