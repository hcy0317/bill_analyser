use std::{cmp::Ordering, collections::BTreeMap};

use bill_analyser_core::{
    build_import_learning_vector_recall_queries, derive_weaviate_feature_vector,
    normalize_weaviate_transaction_type_scope, WEAVIATE_RECALL_MAX_LIMIT,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config_weaviate::WeaviateRuntimeConfig,
    weaviate::{WeaviateHttpClient, WeaviateRuntimeError},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaviateImportLearningRecallRequest {
    pub user_id: i64,
    pub features: BTreeMap<String, String>,
    pub transaction_type_scope: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaviateImportLearningRecallHit {
    pub postgres_source_id: String,
    pub recommendation_key: Option<String>,
    pub feature_key: Option<String>,
    pub rule_state: Option<String>,
    pub transaction_type: Option<String>,
    pub category_id: Option<i64>,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub payload_json: Option<Value>,
    pub distance: Option<f64>,
    pub score: f64,
}

#[tracing::instrument(level = "debug", skip_all)]
/// 使用 Weaviate 派生索引召回导入学习候选；结果只作为 evidence，不越过 PostgreSQL 权威状态。
pub async fn recall_import_learning_candidates(
    config: &WeaviateRuntimeConfig,
    request: &WeaviateImportLearningRecallRequest,
) -> Result<Vec<WeaviateImportLearningRecallHit>, WeaviateRuntimeError> {
    if !config.enabled {
        return Ok(Vec::new());
    }
    let client = WeaviateHttpClient::new(config)?;
    let queries = build_import_learning_vector_recall_queries(
        &config.collection_prefix,
        request.user_id,
        &request.features,
        &request.transaction_type_scope,
        request.limit.max(1),
    );
    let mut hits = Vec::new();
    for query in queries {
        let vector =
            derive_weaviate_feature_vector(&query.feature_payload, config.vector_dimensions);
        let response = client
            .search(&query.class_name, &vector, query.limit, &query.filters)
            .await?;
        hits.extend(parse_weaviate_recall_hits(&response, &query.class_name));
    }
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });
    hits.dedup_by(|left, right| {
        !left.postgres_source_id.is_empty() && left.postgres_source_id == right.postgres_source_id
    });
    hits.truncate(request.limit.clamp(1, WEAVIATE_RECALL_MAX_LIMIT));
    Ok(hits)
}

fn parse_weaviate_recall_hits(
    response: &Value,
    class_name: &str,
) -> Vec<WeaviateImportLearningRecallHit> {
    let Some(items) = response
        .pointer(&format!("/data/Get/{class_name}"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let postgres_source_id = item
                .get("postgresSourceId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if postgres_source_id.is_empty() {
                return None;
            }
            let distance = item
                .pointer("/_additional/distance")
                .and_then(Value::as_f64);
            let score = distance
                .map(|distance| (1.0 - distance).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            Some(WeaviateImportLearningRecallHit {
                postgres_source_id,
                recommendation_key: string_field(item, "recommendationKey"),
                feature_key: string_field(item, "featureKey"),
                rule_state: string_field(item, "ruleState"),
                transaction_type: string_field(item, "transactionType")
                    .map(|value| normalize_weaviate_transaction_type_scope(&value)),
                category_id: item.get("categoryId").and_then(Value::as_i64),
                source_account_id: item.get("sourceAccountId").and_then(Value::as_i64),
                destination_account_id: item.get("destinationAccountId").and_then(Value::as_i64),
                payload_json: item
                    .get("payloadJson")
                    .and_then(Value::as_str)
                    .and_then(|value| serde_json::from_str::<Value>(value).ok()),
                distance,
                score,
            })
        })
        .collect()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
