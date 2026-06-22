// 中文导读：Weaviate 派生索引合同层，负责稳定集合名、对象 ID、schema 和请求 payload。
// 维护重点：Weaviate 只能保存可重建副本；PostgreSQL 生命周期、反馈和审计仍是权威。
// 不变式：所有对象都必须带 userId / featureSchemaVersion / postgresSourceId，避免跨用户召回和派生状态漂移。

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use std::collections::BTreeMap;

use crate::import_learning::{
    normalize_import_learning_text, DEFAULT_FEATURE_DIMENSION, FEATURE_SCHEMA_VERSION,
};

pub const WEAVIATE_DEFAULT_COLLECTION_PREFIX: &str = "BillAnalyser";
pub const WEAVIATE_CLASS_IMPORT_LEARNING_SAMPLE: &str = "ImportLearningSample";
pub const WEAVIATE_CLASS_IMPORT_LEARNING_SUGGESTION_VECTOR: &str = "ImportLearningSuggestionVector";
pub const WEAVIATE_CLASS_COUNTERPARTY_FEATURE: &str = "CounterpartyFeature";
pub const WEAVIATE_CLASS_DESCRIPTION_FEATURE: &str = "DescriptionFeature";
pub const WEAVIATE_DEFAULT_VECTOR_DIMENSIONS: usize = DEFAULT_FEATURE_DIMENSION;
pub const WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE: &str = "postgres_authoritative";
pub const WEAVIATE_RECALL_DEFAULT_LIMIT: usize = 5;
pub const WEAVIATE_RECALL_MAX_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaviateDerivedClass {
    ImportLearningSample,
    ImportLearningSuggestionVector,
    CounterpartyFeature,
    DescriptionFeature,
}

impl WeaviateDerivedClass {
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::ImportLearningSample => WEAVIATE_CLASS_IMPORT_LEARNING_SAMPLE,
            Self::ImportLearningSuggestionVector => {
                WEAVIATE_CLASS_IMPORT_LEARNING_SUGGESTION_VECTOR
            }
            Self::CounterpartyFeature => WEAVIATE_CLASS_COUNTERPARTY_FEATURE,
            Self::DescriptionFeature => WEAVIATE_CLASS_DESCRIPTION_FEATURE,
        }
    }

    pub const fn all() -> [Self; 4] {
        [
            Self::ImportLearningSample,
            Self::ImportLearningSuggestionVector,
            Self::CounterpartyFeature,
            Self::DescriptionFeature,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateCollectionNames {
    pub import_learning_sample: String,
    pub import_learning_suggestion_vector: String,
    pub counterparty_feature: String,
    pub description_feature: String,
}

impl WeaviateCollectionNames {
    /// 按派生类返回实际 Weaviate collection 名称，避免调用方重复拼接 prefix。
    pub fn name_for(&self, class: WeaviateDerivedClass) -> &str {
        match class {
            WeaviateDerivedClass::ImportLearningSample => &self.import_learning_sample,
            WeaviateDerivedClass::ImportLearningSuggestionVector => {
                &self.import_learning_suggestion_vector
            }
            WeaviateDerivedClass::CounterpartyFeature => &self.counterparty_feature,
            WeaviateDerivedClass::DescriptionFeature => &self.description_feature,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaviateDerivedObject {
    pub class: String,
    pub id: String,
    pub properties: Value,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaviateFilterValue {
    Text(String),
    Int(i64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaviateMetadataFilter {
    pub path: String,
    pub value: WeaviateFilterValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaviateImportLearningRecallQuery {
    pub class_name: String,
    pub feature_key: String,
    pub feature_payload: Value,
    pub filters: Vec<WeaviateMetadataFilter>,
    pub limit: usize,
}

/// 校验 Weaviate collection prefix，要求短 ASCII 标识并以字母开头。
#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_weaviate_collection_prefix(prefix: &str) -> bool {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return false;
    }

    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// 基于合法 prefix 构建 D8 派生索引的所有 collection 名称。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_collection_names(prefix: &str) -> Option<WeaviateCollectionNames> {
    let prefix = prefix.trim();
    if !validate_weaviate_collection_prefix(prefix) {
        return None;
    }

    Some(WeaviateCollectionNames {
        import_learning_sample: format!("{prefix}{WEAVIATE_CLASS_IMPORT_LEARNING_SAMPLE}"),
        import_learning_suggestion_vector: format!(
            "{prefix}{WEAVIATE_CLASS_IMPORT_LEARNING_SUGGESTION_VECTOR}"
        ),
        counterparty_feature: format!("{prefix}{WEAVIATE_CLASS_COUNTERPARTY_FEATURE}"),
        description_feature: format!("{prefix}{WEAVIATE_CLASS_DESCRIPTION_FEATURE}"),
    })
}

/// 构建 Weaviate schema class payload，固定 vectorizer=none 以使用应用自带 vector。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_schema_classes(prefix: &str) -> Option<Vec<Value>> {
    let names = build_weaviate_collection_names(prefix)?;
    Some(
        WeaviateDerivedClass::all()
            .into_iter()
            .map(|class| build_schema_class(names.name_for(class)))
            .collect(),
    )
}

fn build_schema_class(class_name: &str) -> Value {
    json!({
        "class": class_name,
        "vectorizer": "none",
        "properties": [
            { "name": "userId", "dataType": ["int"] },
            { "name": "featureSchemaVersion", "dataType": ["text"] },
            { "name": "parserId", "dataType": ["text"] },
            { "name": "transactionType", "dataType": ["text"] },
            { "name": "categoryId", "dataType": ["int"] },
            { "name": "sourceAccountId", "dataType": ["int"] },
            { "name": "destinationAccountId", "dataType": ["int"] },
            { "name": "recommendationKey", "dataType": ["text"] },
            { "name": "sampleKey", "dataType": ["text"] },
            { "name": "featureKey", "dataType": ["text"] },
            { "name": "featureHash", "dataType": ["text"] },
            { "name": "ruleState", "dataType": ["text"] },
            { "name": "postgresSourceId", "dataType": ["text"] },
            { "name": "payloadJson", "dataType": ["text"] },
            { "name": "createdAt", "dataType": ["date"] },
            { "name": "updatedAt", "dataType": ["date"] }
        ]
    })
}

/// 构建所有派生对象必须携带的用户、特征版本和 PostgreSQL 来源元数据。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_required_metadata(
    user_id: i64,
    postgres_source_id: impl Into<String>,
) -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert("userId".to_string(), json!(user_id));
    properties.insert(
        "featureSchemaVersion".to_string(),
        json!(FEATURE_SCHEMA_VERSION),
    );
    properties.insert(
        "postgresSourceId".to_string(),
        json!(postgres_source_id.into()),
    );
    properties
}

/// 归一化交易类型 scope，保证 recall filter 使用稳定英文枚举。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_weaviate_transaction_type_scope(value: &str) -> String {
    match normalize_import_learning_text(Some(&json!(value))).as_str() {
        "收入" | "income" | "2" => "income",
        "支出" | "expense" | "3" => "expense",
        "转账" | "transfer" | "4" => "transfer",
        "投资" | "investment" | "5" => "investment",
        other => other,
    }
    .to_string()
}

/// 构建导入学习向量召回过滤条件，强制 user、schema 和 PostgreSQL 权威状态。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_learning_vector_recall_filters(
    user_id: i64,
    transaction_type_scope: &str,
) -> Vec<WeaviateMetadataFilter> {
    let mut filters = vec![
        WeaviateMetadataFilter {
            path: "userId".to_string(),
            value: WeaviateFilterValue::Int(user_id.max(0)),
        },
        WeaviateMetadataFilter {
            path: "featureSchemaVersion".to_string(),
            value: WeaviateFilterValue::Text(FEATURE_SCHEMA_VERSION.to_string()),
        },
        WeaviateMetadataFilter {
            path: "ruleState".to_string(),
            value: WeaviateFilterValue::Text(
                WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE.to_string(),
            ),
        },
    ];
    let transaction_type = normalize_weaviate_transaction_type_scope(transaction_type_scope);
    if !transaction_type.is_empty() {
        filters.push(WeaviateMetadataFilter {
            path: "transactionType".to_string(),
            value: WeaviateFilterValue::Text(transaction_type),
        });
    }
    filters
}

/// 根据导入学习特征构建 counterparty、description 和 composite 三类 recall 查询。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_learning_vector_recall_queries(
    collection_prefix: &str,
    user_id: i64,
    features: &BTreeMap<String, String>,
    transaction_type_scope: &str,
    limit: usize,
) -> Vec<WeaviateImportLearningRecallQuery> {
    let Some(names) = build_weaviate_collection_names(collection_prefix) else {
        return Vec::new();
    };
    let limit = limit.clamp(1, WEAVIATE_RECALL_MAX_LIMIT);
    let filters = build_import_learning_vector_recall_filters(user_id, transaction_type_scope);
    let transaction_type = normalize_weaviate_transaction_type_scope(transaction_type_scope);
    let feature_json = json!(features);
    let mut queries = Vec::new();
    for (feature_key, class_name) in [
        ("counterparty", names.counterparty_feature.as_str()),
        ("description", names.description_feature.as_str()),
    ] {
        let Some(feature_value) = features.get(feature_key) else {
            continue;
        };
        if feature_value.trim().is_empty() {
            continue;
        }
        queries.push(WeaviateImportLearningRecallQuery {
            class_name: class_name.to_string(),
            feature_key: feature_key.to_string(),
            feature_payload: json!({
                "feature_schema_version": FEATURE_SCHEMA_VERSION,
                "feature_key": feature_key,
                "feature_value": feature_value,
                "transaction_type": transaction_type,
                "features": feature_json,
            }),
            filters: filters.clone(),
            limit,
        });
    }
    if features.len() >= 2 {
        queries.push(WeaviateImportLearningRecallQuery {
            class_name: names.import_learning_sample,
            feature_key: "composite".to_string(),
            feature_payload: json!({
                "feature_schema_version": FEATURE_SCHEMA_VERSION,
                "feature_key": "composite",
                "transaction_type": transaction_type,
                "features": feature_json,
            }),
            filters,
            limit,
        });
    }
    queries
}

/// 构建 Weaviate 派生对象，使用 deterministic id 让同一来源可幂等 upsert。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_derived_object(
    class_name: impl Into<String>,
    source_key: impl AsRef<str>,
    mut properties: Map<String, Value>,
    vector: Vec<f32>,
) -> WeaviateDerivedObject {
    let class_name = class_name.into();
    let id = deterministic_weaviate_object_id(&class_name, source_key.as_ref());
    properties
        .entry("payloadJson")
        .or_insert_with(|| json!("{}"));
    WeaviateDerivedObject {
        class: class_name,
        id,
        properties: Value::Object(properties),
        vector,
    }
}

/// 为 class/source_key 生成稳定 UUID，避免派生索引重复对象。
#[tracing::instrument(level = "debug", skip_all)]
pub fn deterministic_weaviate_object_id(class_name: &str, source_key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b"bill-analyser-weaviate-derived-object-v1");
    hasher.update(class_name.as_bytes());
    hasher.update([0]);
    hasher.update(source_key.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format_uuid(bytes)
}

fn format_uuid(bytes: [u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

/// 从 payload 派生固定维度伪向量，保证无外部 embedding provider 时仍可构建测试合同。
#[tracing::instrument(level = "debug", skip_all)]
pub fn derive_weaviate_feature_vector(payload: &Value, dimensions: usize) -> Vec<f32> {
    let dimensions = dimensions.max(1);
    let canonical = serde_json::to_string(payload).unwrap_or_else(|_| "null".to_string());
    (0..dimensions)
        .map(|index| {
            let mut hasher = Sha256::new();
            hasher.update(b"bill-analyser-weaviate-feature-vector-v1");
            hasher.update(index.to_string().as_bytes());
            hasher.update([0]);
            hasher.update(canonical.as_bytes());
            let digest = hasher.finalize();
            let raw = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
            ((raw as f64 / u32::MAX as f64) * 2.0 - 1.0) as f32
        })
        .collect()
}

/// 构建 Weaviate batch upsert 请求体，保持 class/id/properties/vector 四元组不变。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_batch_upsert_payload(objects: &[WeaviateDerivedObject]) -> Value {
    json!({
        "objects": objects
            .iter()
            .map(|object| {
                json!({
                    "class": object.class,
                    "id": object.id,
                    "properties": object.properties,
                    "vector": object.vector,
                })
            })
            .collect::<Vec<_>>()
    })
}

/// 构建单个 Weaviate 对象删除路径。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_delete_path(class_name: &str, object_id: &str) -> String {
    format!("/v1/objects/{class_name}/{object_id}")
}

/// 构建 Weaviate GraphQL nearVector 查询，包含 recall filter 和固定返回字段。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_weaviate_graphql_query(
    class_name: &str,
    vector: &[f32],
    limit: usize,
    filters: &[WeaviateMetadataFilter],
) -> Value {
    let limit = limit.clamp(1, 100);
    let vector_literal = vector
        .iter()
        .map(|value| format!("{value:.8}"))
        .collect::<Vec<_>>()
        .join(", ");
    let where_clause = build_graphql_where_clause(filters);
    let query = format!(
        "{{ Get {{ {class_name}(nearVector: {{ vector: [{vector_literal}] }}{where_clause}, limit: {limit}) {{ postgresSourceId recommendationKey featureKey ruleState transactionType categoryId sourceAccountId destinationAccountId payloadJson _additional {{ id distance }} }} }} }}"
    );
    json!({ "query": query })
}

fn build_graphql_where_clause(filters: &[WeaviateMetadataFilter]) -> String {
    if filters.is_empty() {
        return String::new();
    }

    let operands = filters
        .iter()
        .map(|filter| {
            let path = graphql_string(&filter.path);
            match &filter.value {
                WeaviateFilterValue::Text(value) => {
                    format!(
                        "{{ path: [{path}], operator: Equal, valueText: {} }}",
                        graphql_string(value)
                    )
                }
                WeaviateFilterValue::Int(value) => {
                    format!("{{ path: [{path}], operator: Equal, valueInt: {value} }}")
                }
                WeaviateFilterValue::Bool(value) => {
                    format!("{{ path: [{path}], operator: Equal, valueBoolean: {value} }}")
                }
            }
        })
        .collect::<Vec<_>>();

    if operands.len() == 1 {
        format!(", where: {}", operands[0])
    } else {
        format!(
            ", where: {{ operator: And, operands: [{}] }}",
            operands.join(", ")
        )
    }
}

fn graphql_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("\"{escaped}\"")
}
