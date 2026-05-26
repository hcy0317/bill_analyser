// 中文导读：Weaviate 派生索引运行适配层，负责可选健康检查、schema bootstrap 和 HTTP 请求。
// 维护重点：默认禁用；启用后失败只能降级向量链路，不得阻断确定性导入和 PostgreSQL 权威状态。
// 不变式：所有外部请求使用配置 endpoint/API key，响应和日志不能泄露密钥。

use bill_analyser_core::{
    build_weaviate_batch_upsert_payload, build_weaviate_delete_path, build_weaviate_derived_object,
    build_weaviate_graphql_query, build_weaviate_required_metadata, build_weaviate_schema_classes,
    derive_weaviate_feature_vector, normalize_weaviate_transaction_type_scope,
    WeaviateDerivedClass, WeaviateDerivedObject, WeaviateMetadataFilter,
    WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE,
};
use bill_analyser_db::{
    claim_pending_vector_outbox_events, load_import_learning_feature_vector_sources,
    mark_vector_outbox_event_failed, mark_vector_outbox_event_succeeded,
    ImportLearningFeatureVectorSource, PostgresPool, VectorOutboxEvent,
};
use reqwest::{header::CONTENT_TYPE, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::config::HttpShellConfig;
use crate::config_weaviate::WeaviateRuntimeConfig;
pub use crate::weaviate_recall::{
    recall_import_learning_candidates, WeaviateImportLearningRecallHit,
    WeaviateImportLearningRecallRequest,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateHealthStatus {
    pub status: String,
    pub detail: String,
}

impl WeaviateHealthStatus {
    pub fn disabled() -> Self {
        Self {
            status: "disabled".to_string(),
            detail: "disabled".to_string(),
        }
    }

    pub fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            detail: "ready".to_string(),
        }
    }

    pub fn degraded(detail: impl Into<String>) -> Self {
        Self {
            status: "degraded".to_string(),
            detail: detail.into(),
        }
    }

    pub fn health_detail_value(&self) -> String {
        if self.status == "disabled" || self.status == "healthy" {
            self.status.clone()
        } else {
            format!("{}:{}", self.status, self.detail)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateBootstrapReport {
    pub enabled: bool,
    pub status: String,
    pub collections: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateBatchReport {
    pub enabled: bool,
    pub attempted: usize,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateOutboxProcessReport {
    pub enabled: bool,
    pub claimed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateRebuildReport {
    pub enabled: bool,
    pub user_id: Option<i64>,
    pub source_count: usize,
    pub deleted_classes: usize,
    pub upserted: usize,
}

#[derive(Debug, Error)]
pub enum WeaviateRuntimeError {
    #[error("Weaviate is disabled")]
    Disabled,
    #[error("Weaviate endpoint is not configured")]
    MissingEndpoint,
    #[error("invalid Weaviate collection prefix")]
    InvalidCollectionPrefix,
    #[error("serialize Weaviate request: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("Weaviate HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Weaviate request failed: {status} {body}")]
    Unsuccessful { status: StatusCode, body: String },
}

#[derive(Debug, Clone)]
pub struct WeaviateHttpClient {
    config: WeaviateRuntimeConfig,
    client: reqwest::Client,
}

impl WeaviateHttpClient {
    pub fn new(config: &WeaviateRuntimeConfig) -> Result<Self, WeaviateRuntimeError> {
        if !config.enabled {
            return Err(WeaviateRuntimeError::Disabled);
        }
        if config.endpoint.is_none() {
            return Err(WeaviateRuntimeError::MissingEndpoint);
        }
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            config: config.clone(),
            client,
        })
    }

    pub async fn readiness(&self) -> Result<(), WeaviateRuntimeError> {
        let response = self
            .authenticated(
                self.client
                    .get(self.endpoint_path("/v1/.well-known/ready")?),
            )
            .send()
            .await?;
        ensure_success(response).await
    }

    pub async fn bootstrap_schema(&self) -> Result<WeaviateBootstrapReport, WeaviateRuntimeError> {
        let classes = build_weaviate_schema_classes(&self.config.collection_prefix)
            .ok_or(WeaviateRuntimeError::InvalidCollectionPrefix)?;
        let mut collections = Vec::with_capacity(classes.len());
        for class in classes {
            let Some(class_name) = class["class"].as_str() else {
                continue;
            };
            collections.push(class_name.to_string());
            if self.collection_exists(class_name).await? {
                continue;
            }
            self.post_json("/v1/schema", &class).await?;
        }

        Ok(WeaviateBootstrapReport {
            enabled: true,
            status: "bootstrapped".to_string(),
            collections,
        })
    }

    pub async fn upsert_objects(
        &self,
        objects: &[WeaviateDerivedObject],
    ) -> Result<WeaviateBatchReport, WeaviateRuntimeError> {
        if objects.is_empty() {
            return Ok(WeaviateBatchReport {
                enabled: true,
                attempted: 0,
                status: "skipped_empty".to_string(),
            });
        }
        let payload = build_weaviate_batch_upsert_payload(objects);
        self.post_json("/v1/batch/objects", &payload).await?;
        Ok(WeaviateBatchReport {
            enabled: true,
            attempted: objects.len(),
            status: "upserted".to_string(),
        })
    }

    pub async fn delete_object(
        &self,
        class_name: &str,
        object_id: &str,
    ) -> Result<(), WeaviateRuntimeError> {
        let path = build_weaviate_delete_path(class_name, object_id);
        let response = self
            .authenticated(self.client.delete(self.endpoint_path(&path)?))
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }
        ensure_success(response).await
    }

    pub async fn delete_user_objects(
        &self,
        class_name: &str,
        user_id: i64,
    ) -> Result<Value, WeaviateRuntimeError> {
        let payload = json!({
            "match": {
                "class": class_name,
                "where": {
                    "path": ["userId"],
                    "operator": "Equal",
                    "valueInt": user_id
                }
            },
            "output": "minimal"
        });
        let body = serde_json::to_vec(&payload)?;
        let response = self
            .authenticated(self.client.delete(self.endpoint_path("/v1/batch/objects")?))
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(WeaviateRuntimeError::Unsuccessful { status, body: text });
        }
        if text.trim().is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).map_err(WeaviateRuntimeError::Serialize)
        }
    }

    pub async fn search(
        &self,
        class_name: &str,
        vector: &[f32],
        limit: usize,
        filters: &[WeaviateMetadataFilter],
    ) -> Result<Value, WeaviateRuntimeError> {
        let payload = build_weaviate_graphql_query(class_name, vector, limit, filters);
        self.post_json("/v1/graphql", &payload).await
    }

    async fn collection_exists(&self, class_name: &str) -> Result<bool, WeaviateRuntimeError> {
        let response = self
            .authenticated(
                self.client
                    .get(self.endpoint_path(&format!("/v1/schema/{class_name}"))?),
            )
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        ensure_success(response).await?;
        Ok(true)
    }

    async fn post_json(&self, path: &str, payload: &Value) -> Result<Value, WeaviateRuntimeError> {
        let body = serde_json::to_vec(payload)?;
        let response = self
            .authenticated(self.client.post(self.endpoint_path(path)?))
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(WeaviateRuntimeError::Unsuccessful { status, body: text });
        }
        if text.trim().is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).map_err(WeaviateRuntimeError::Serialize)
        }
    }

    fn endpoint_path(&self, path: &str) -> Result<String, WeaviateRuntimeError> {
        let endpoint = self
            .config
            .endpoint
            .as_deref()
            .ok_or(WeaviateRuntimeError::MissingEndpoint)?;
        Ok(format!("{}{}", endpoint.trim_end_matches('/'), path))
    }

    fn authenticated(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.config.api_key.as_deref() {
            Some(api_key) => request.bearer_auth(api_key),
            None => request,
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn probe_weaviate_health(config: &HttpShellConfig) -> WeaviateHealthStatus {
    if !config.weaviate.enabled {
        return WeaviateHealthStatus::disabled();
    }
    let client = match WeaviateHttpClient::new(&config.weaviate) {
        Ok(client) => client,
        Err(WeaviateRuntimeError::MissingEndpoint) => {
            return WeaviateHealthStatus::degraded("missing_endpoint");
        }
        Err(error) => return WeaviateHealthStatus::degraded(error.to_string()),
    };

    match client.readiness().await {
        Ok(()) => WeaviateHealthStatus::healthy(),
        Err(error) => WeaviateHealthStatus::degraded(error.to_string()),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn process_weaviate_outbox_once(
    pool: &PostgresPool,
    config: &WeaviateRuntimeConfig,
) -> Result<WeaviateOutboxProcessReport, WeaviateRuntimeError> {
    if !config.enabled {
        return Ok(WeaviateOutboxProcessReport {
            enabled: false,
            claimed: 0,
            succeeded: 0,
            failed: 0,
        });
    }
    let client = WeaviateHttpClient::new(config)?;
    let events = claim_pending_vector_outbox_events(pool, config.batch_size)
        .await
        .map_err(|error| WeaviateRuntimeError::Unsuccessful {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: error.to_string(),
        })?;
    let mut succeeded = 0;
    let mut failed = 0;
    for event in &events {
        match apply_outbox_event(&client, event).await {
            Ok(()) => {
                succeeded += 1;
                mark_vector_outbox_event_succeeded(pool, event.id)
                    .await
                    .map_err(|error| WeaviateRuntimeError::Unsuccessful {
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        body: error.to_string(),
                    })?;
            }
            Err(error) => {
                failed += 1;
                let delay = retry_delay_seconds(event.attempts, config.retry_attempts);
                mark_vector_outbox_event_failed(
                    pool,
                    event.id,
                    &error.to_string(),
                    delay,
                    (config.retry_attempts + 1) as i32,
                )
                .await
                .map_err(|error| WeaviateRuntimeError::Unsuccessful {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    body: error.to_string(),
                })?;
            }
        }
    }

    Ok(WeaviateOutboxProcessReport {
        enabled: true,
        claimed: events.len(),
        succeeded,
        failed,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn rebuild_weaviate_from_postgres(
    pool: &PostgresPool,
    config: &WeaviateRuntimeConfig,
    user_id: Option<i64>,
) -> Result<WeaviateRebuildReport, WeaviateRuntimeError> {
    if !config.enabled {
        return Ok(WeaviateRebuildReport {
            enabled: false,
            user_id,
            source_count: 0,
            deleted_classes: 0,
            upserted: 0,
        });
    }
    let client = WeaviateHttpClient::new(config)?;
    client.bootstrap_schema().await?;
    let sources = load_import_learning_feature_vector_sources(pool, user_id, config.batch_size)
        .await
        .map_err(|error| WeaviateRuntimeError::Unsuccessful {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: error.to_string(),
        })?;
    let names = bill_analyser_core::build_weaviate_collection_names(&config.collection_prefix)
        .ok_or(WeaviateRuntimeError::InvalidCollectionPrefix)?;

    let mut deleted_classes = 0;
    if let Some(user_id) = user_id {
        for class in WeaviateDerivedClass::all() {
            client
                .delete_user_objects(names.name_for(class), user_id)
                .await?;
            deleted_classes += 1;
        }
    }

    let objects = sources
        .iter()
        .map(|source| build_object_from_feature_source(config, source))
        .collect::<Result<Vec<_>, _>>()?;
    let upserted = objects.len();
    for chunk in objects.chunks(config.batch_size.max(1)) {
        client.upsert_objects(chunk).await?;
    }

    Ok(WeaviateRebuildReport {
        enabled: true,
        user_id,
        source_count: sources.len(),
        deleted_classes,
        upserted,
    })
}

pub fn build_object_from_feature_source(
    config: &WeaviateRuntimeConfig,
    source: &ImportLearningFeatureVectorSource,
) -> Result<WeaviateDerivedObject, WeaviateRuntimeError> {
    let names = bill_analyser_core::build_weaviate_collection_names(&config.collection_prefix)
        .ok_or(WeaviateRuntimeError::InvalidCollectionPrefix)?;
    let class = match source.feature_key.as_str() {
        "counterparty" => WeaviateDerivedClass::CounterpartyFeature,
        "description" => WeaviateDerivedClass::DescriptionFeature,
        _ => WeaviateDerivedClass::ImportLearningSample,
    };
    let class_name = names.name_for(class).to_string();
    let mut properties =
        build_weaviate_required_metadata(source.user_id, format!("feature:{}", source.feature_id));
    properties.insert("sampleKey".to_string(), json!(source.sample_key));
    properties.insert("featureKey".to_string(), json!(source.feature_key));
    properties.insert("featureHash".to_string(), json!(source.feature_hash));
    properties.insert(
        "payloadJson".to_string(),
        json!(source.feature_payload.to_string()),
    );
    if let Some(parser_id) = source
        .source_payload
        .get("parser_id")
        .or_else(|| source.normalized_features.get("parser_id"))
        .and_then(Value::as_str)
    {
        properties.insert("parserId".to_string(), json!(parser_id));
    }
    if let Some(transaction_type) = source
        .target_payload
        .get("type")
        .or_else(|| source.target_payload.get("transaction_type"))
        .and_then(Value::as_str)
    {
        properties.insert(
            "transactionType".to_string(),
            json!(normalize_weaviate_transaction_type_scope(transaction_type)),
        );
    }
    for (property, keys) in [
        ("categoryId", ["category_id", "annotated_category_id"]),
        (
            "sourceAccountId",
            ["source_account_id", "annotated_source_account_id"],
        ),
        (
            "destinationAccountId",
            ["destination_account_id", "annotated_destination_account_id"],
        ),
    ] {
        if let Some(value) = keys
            .iter()
            .find_map(|key| source.target_payload.get(*key).and_then(Value::as_i64))
        {
            properties.insert(property.to_string(), json!(value));
        }
    }
    properties.insert(
        "ruleState".to_string(),
        json!(WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE),
    );

    let vector = derive_weaviate_feature_vector(&source.feature_payload, config.vector_dimensions);
    Ok(build_weaviate_derived_object(
        class_name,
        format!("feature:{}", source.feature_id),
        properties,
        vector,
    ))
}

async fn apply_outbox_event(
    client: &WeaviateHttpClient,
    event: &VectorOutboxEvent,
) -> Result<(), WeaviateRuntimeError> {
    match event.event_type.as_str() {
        "delete" => {
            let class_name = event
                .payload
                .get("class")
                .and_then(Value::as_str)
                .ok_or_else(|| WeaviateRuntimeError::Unsuccessful {
                    status: StatusCode::BAD_REQUEST,
                    body: "vector outbox delete payload missing class".to_string(),
                })?;
            let object_id = event
                .payload
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| WeaviateRuntimeError::Unsuccessful {
                    status: StatusCode::BAD_REQUEST,
                    body: "vector outbox delete payload missing id".to_string(),
                })?;
            client.delete_object(class_name, object_id).await
        }
        _ => {
            let class_name = event
                .payload
                .get("class")
                .and_then(Value::as_str)
                .ok_or_else(|| WeaviateRuntimeError::Unsuccessful {
                    status: StatusCode::BAD_REQUEST,
                    body: "vector outbox upsert payload missing class".to_string(),
                })?;
            let source_key = event
                .payload
                .get("sourceKey")
                .or_else(|| event.payload.get("postgresSourceId"))
                .and_then(Value::as_str)
                .ok_or_else(|| WeaviateRuntimeError::Unsuccessful {
                    status: StatusCode::BAD_REQUEST,
                    body: "vector outbox upsert payload missing sourceKey".to_string(),
                })?;
            let properties = event
                .payload
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_else(|| {
                    let mut properties = bill_analyser_core::build_weaviate_required_metadata(
                        event.user_id.unwrap_or_default(),
                        source_key,
                    );
                    properties.insert("payloadJson".to_string(), json!(event.payload.to_string()));
                    properties
                });
            let vector = event
                .payload
                .get("vector")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_f64)
                        .map(|value| value as f32)
                        .collect::<Vec<_>>()
                })
                .filter(|vector| !vector.is_empty())
                .unwrap_or_else(|| {
                    derive_weaviate_feature_vector(&event.payload, client.config.vector_dimensions)
                });
            let object = build_weaviate_derived_object(class_name, source_key, properties, vector);
            client.upsert_objects(&[object]).await.map(|_| ())
        }
    }
}

fn retry_delay_seconds(attempts: i32, retry_attempts: usize) -> i64 {
    if attempts as usize > retry_attempts {
        0
    } else {
        2_i64.pow(attempts.max(0) as u32).min(300)
    }
}

async fn ensure_success(response: reqwest::Response) -> Result<(), WeaviateRuntimeError> {
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(WeaviateRuntimeError::Unsuccessful { status, body })
    }
}
